use std::{
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use atomic_write_file::AtomicWriteFile;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineEnding {
    #[default]
    Lf,
    CrLf,
}

impl LineEnding {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentFormat {
    pub has_utf8_bom: bool,
    pub has_final_newline: bool,
    pub line_endings: Vec<LineEnding>,
    pub default_line_ending: LineEnding,
}

#[derive(Debug)]
pub struct Document {
    path: PathBuf,
    text: String,
    format: DocumentFormat,
    fingerprint: String,
}

#[derive(Debug)]
pub enum SaveError {
    ExternalChange { expected: String, actual: String },
    Io(io::Error),
}

impl fmt::Display for SaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExternalChange { .. } => formatter.write_str("the document changed on disk"),
            Self::Io(error) => write!(formatter, "could not save document: {error}"),
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ExternalChange { .. } => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl Document {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("could not resolve {}", path.display()))?;
        let bytes =
            fs::read(&path).with_context(|| format!("could not read {}", path.display()))?;
        let has_utf8_bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
        let text_bytes = if has_utf8_bom { &bytes[3..] } else { &bytes };
        let source = std::str::from_utf8(text_bytes)
            .with_context(|| format!("{} is not valid UTF-8", path.display()))?;
        let (text, line_endings) = normalize_line_endings(source);
        let default_line_ending = line_endings.first().copied().unwrap_or_default();
        let has_final_newline = !line_endings.is_empty() && text.ends_with('\n');

        Ok(Self {
            path,
            text,
            format: DocumentFormat {
                has_utf8_bom,
                has_final_newline,
                line_endings,
                default_line_ending,
            },
            fingerprint: fingerprint_bytes(&bytes),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: String) {
        self.text = text.replace("\r\n", "\n");
    }

    pub fn format(&self) -> &DocumentFormat {
        &self.format
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn save(&mut self) -> Result<(), SaveError> {
        let current = fs::read(&self.path).map_err(SaveError::Io)?;
        let actual = fingerprint_bytes(&current);
        if actual != self.fingerprint {
            return Err(SaveError::ExternalChange {
                expected: self.fingerprint.clone(),
                actual,
            });
        }

        let bytes = self.serialized_bytes();
        atomic_write(&self.path, &bytes).map_err(SaveError::Io)?;
        self.fingerprint = fingerprint_bytes(&bytes);
        self.format.has_final_newline = self.text.ends_with('\n');
        Ok(())
    }

    fn serialized_bytes(&self) -> Vec<u8> {
        let mut output =
            Vec::with_capacity(self.text.len() + usize::from(self.format.has_utf8_bom) * 3);
        if self.format.has_utf8_bom {
            output.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }

        for (index, line) in self.text.split_inclusive('\n').enumerate() {
            if let Some(line_without_newline) = line.strip_suffix('\n') {
                output.extend_from_slice(line_without_newline.as_bytes());
                output.extend_from_slice(
                    self.format
                        .line_endings
                        .get(index)
                        .copied()
                        .unwrap_or(self.format.default_line_ending)
                        .as_str()
                        .as_bytes(),
                );
            } else {
                output.extend_from_slice(line.as_bytes());
            }
        }
        output
    }
}

pub fn fingerprint_text(source: &str) -> String {
    fingerprint_bytes(source.as_bytes())
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(bytes)?;
    file.commit()
}

fn fingerprint_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn normalize_line_endings(source: &str) -> (String, Vec<LineEnding>) {
    let mut text = String::with_capacity(source.len());
    let mut endings = Vec::new();
    let mut characters = source.chars().peekable();

    while let Some(character) = characters.next() {
        if character == '\r' && characters.peek() == Some(&'\n') {
            characters.next();
            text.push('\n');
            endings.push(LineEnding::CrLf);
        } else if character == '\n' {
            text.push('\n');
            endings.push(LineEnding::Lf);
        } else {
            text.push(character);
        }
    }
    (text, endings)
}
