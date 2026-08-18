use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::{
    fmt, fs,
    fs::OpenOptions,
    io,
    path::{Path, PathBuf},
};

use super::Sidecar;
use crate::document::atomic_write;

pub const SIDECAR_SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub enum SidecarStoreError {
    Io(io::Error),
    Json(serde_json::Error),
    UnsupportedSchema(u32),
    ExternalChange,
}

impl fmt::Display for SidecarStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "could not access annotation sidecar: {error}"),
            Self::Json(error) => write!(formatter, "could not parse annotation sidecar: {error}"),
            Self::UnsupportedSchema(version) => write!(
                formatter,
                "annotation sidecar schema {version} is unsupported"
            ),
            Self::ExternalChange => formatter.write_str("the annotation sidecar changed on disk"),
        }
    }
}

impl std::error::Error for SidecarStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::UnsupportedSchema(_) | Self::ExternalChange => None,
        }
    }
}

pub struct SidecarStore;

impl SidecarStore {
    pub fn load(path: &Path) -> Result<Sidecar, SidecarStoreError> {
        Self::load_with_revision(path).map(|(sidecar, _)| sidecar)
    }

    pub fn load_with_revision(path: &Path) -> Result<(Sidecar, String), SidecarStoreError> {
        let bytes = fs::read(path).map_err(SidecarStoreError::Io)?;
        let sidecar = serde_json::from_slice::<Sidecar>(&bytes).map_err(SidecarStoreError::Json)?;
        if sidecar.schema_version != SIDECAR_SCHEMA_VERSION {
            return Err(SidecarStoreError::UnsupportedSchema(sidecar.schema_version));
        }
        Ok((sidecar, revision(&bytes)))
    }

    pub fn save(path: &Path, sidecar: &Sidecar) -> Result<(), SidecarStoreError> {
        let bytes = serialized(sidecar)?;
        write_sidecar(path, &bytes)
    }

    pub fn save_checked(
        path: &Path,
        sidecar: &Sidecar,
        expected_revision: Option<&str>,
    ) -> Result<String, SidecarStoreError> {
        let parent = sidecar_parent(path)?;
        fs::create_dir_all(parent).map_err(SidecarStoreError::Io)?;
        let lock_path = lock_path(path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(SidecarStoreError::Io)?;
        FileExt::lock_exclusive(&lock).map_err(SidecarStoreError::Io)?;

        let actual_revision = match fs::read(path) {
            Ok(bytes) => Some(revision(&bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(SidecarStoreError::Io(error)),
        };
        if actual_revision.as_deref() != expected_revision {
            return Err(SidecarStoreError::ExternalChange);
        }
        let bytes = serialized(sidecar)?;
        write_sidecar(path, &bytes)?;
        Ok(revision(&bytes))
    }
}

fn serialized(sidecar: &Sidecar) -> Result<Vec<u8>, SidecarStoreError> {
    if sidecar.schema_version != SIDECAR_SCHEMA_VERSION {
        return Err(SidecarStoreError::UnsupportedSchema(sidecar.schema_version));
    }
    let mut bytes = serde_json::to_vec_pretty(sidecar).map_err(SidecarStoreError::Json)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_sidecar(path: &Path, bytes: &[u8]) -> Result<(), SidecarStoreError> {
    let parent = sidecar_parent(path)?;
    fs::create_dir_all(parent).map_err(SidecarStoreError::Io)?;
    atomic_write(path, bytes).map_err(SidecarStoreError::Io)
}

fn sidecar_parent(path: &Path) -> Result<&Path, SidecarStoreError> {
    path.parent().ok_or_else(|| {
        SidecarStoreError::Io(io::Error::other("sidecar path has no parent directory"))
    })
}

fn lock_path(path: &Path) -> Result<PathBuf, SidecarStoreError> {
    let filename = path
        .file_name()
        .ok_or_else(|| SidecarStoreError::Io(io::Error::other("sidecar path has no filename")))?;
    Ok(path.with_file_name(format!("{}.lock", filename.to_string_lossy())))
}

fn revision(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub fn default_sidecar_path(document_path: &Path) -> io::Result<PathBuf> {
    let parent = document_path.parent().unwrap_or_else(|| Path::new("."));
    let filename = document_path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "document path has no filename")
    })?;
    Ok(parent
        .join(".annoterm")
        .join(format!("{}.annotations.json", filename.to_string_lossy())))
}
