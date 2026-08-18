use std::{fmt, io, path::Path};

use crate::document::atomic_write;

#[derive(Debug)]
pub enum ExportError {
    AlreadyExists(std::path::PathBuf),
    Io(io::Error),
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists(path) => write!(
                formatter,
                "{} already exists; use --force to overwrite it",
                path.display()
            ),
            Self::Io(error) => write!(formatter, "could not export feedback: {error}"),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AlreadyExists(_) => None,
            Self::Io(error) => Some(error),
        }
    }
}

pub fn export_prompt(path: &Path, prompt: &str, force: bool) -> Result<(), ExportError> {
    if path.exists() && !force {
        return Err(ExportError::AlreadyExists(path.to_path_buf()));
    }
    atomic_write(path, prompt.as_bytes()).map_err(ExportError::Io)
}
