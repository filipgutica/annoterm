use std::{ffi::OsString, path::PathBuf};

use anyhow::{Result, anyhow};
use clap::{Args, Parser, Subcommand};

use crate::{
    annotations::{
        Sidecar, SidecarStore, SidecarStoreError, default_sidecar_path, reanchor_annotation,
    },
    app::App,
    document::Document,
    feedback::{ClipboardStatus, copy_prompt, export_prompt, generate_prompt},
    markdown::parse,
    ui,
};

/// Command-line entry point. A bare path opens the interactive editor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cli {
    pub command: Command,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Open {
        file: PathBuf,
        annotations: Option<PathBuf>,
    },
    Copy {
        file: PathBuf,
        annotations: Option<PathBuf>,
    },
    Export {
        file: PathBuf,
        annotations: Option<PathBuf>,
        output: PathBuf,
        force: bool,
    },
}

#[derive(Debug, Parser)]
#[command(
    name = "annoterm",
    version,
    about = "Read, edit, and review Markdown in the terminal",
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
struct RawCli {
    /// Markdown file to open in the interactive terminal UI.
    #[arg(value_name = "FILE", required = true)]
    file: Option<PathBuf>,

    /// Read and write annotations at this path instead of the default sidecar.
    #[arg(long, value_name = "SIDECAR")]
    annotations: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<RawCommand>,
}

#[derive(Debug, Subcommand)]
enum RawCommand {
    /// Copy the generated feedback prompt to the clipboard.
    #[command(name = "copy-feedback", alias = "copy")]
    CopyFeedback(FileArguments),
    /// Write the generated feedback prompt to a file.
    Export(ExportArguments),
}

#[derive(Debug, Args)]
struct FileArguments {
    /// Markdown file whose annotations should be used.
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Read annotations from this path instead of the default sidecar.
    #[arg(long, value_name = "SIDECAR")]
    annotations: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ExportArguments {
    /// Markdown file whose annotations should be used.
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Read annotations from this path instead of the default sidecar.
    #[arg(long, value_name = "SIDECAR")]
    annotations: Option<PathBuf>,

    /// Destination Markdown file for the generated prompt.
    #[arg(short, long, value_name = "FILE")]
    output: PathBuf,

    /// Replace an existing output file.
    #[arg(long)]
    force: bool,
}

impl Cli {
    pub fn try_parse_from<I, T>(arguments: I) -> std::result::Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        RawCli::try_parse_from(arguments).map(Into::into)
    }

    fn from_environment() -> Self {
        RawCli::parse().into()
    }
}

impl From<RawCli> for Cli {
    fn from(raw: RawCli) -> Self {
        let command = match raw.command {
            Some(RawCommand::CopyFeedback(arguments)) => Command::Copy {
                file: arguments.file,
                annotations: arguments.annotations,
            },
            Some(RawCommand::Export(arguments)) => Command::Export {
                file: arguments.file,
                annotations: arguments.annotations,
                output: arguments.output,
                force: arguments.force,
            },
            None => Command::Open {
                file: raw.file.expect("clap requires an open path"),
                annotations: raw.annotations,
            },
        };
        Self { command }
    }
}

pub fn run() -> Result<()> {
    match Cli::from_environment().command {
        Command::Open { file, annotations } => open(file, annotations),
        Command::Copy { file, annotations } => copy(file, annotations),
        Command::Export {
            file,
            annotations,
            output,
            force,
        } => export(file, annotations, output, force),
    }
}

fn open(file: PathBuf, annotation_path: Option<PathBuf>) -> Result<()> {
    let mut document = Document::load(&file)?;
    let source = document.text().to_owned();
    let rendered = parse(&source)?;
    let mut app = App::new(source, rendered);
    app.set_document_fingerprint(document.fingerprint());
    let sidecar_path = sidecar_path(&document, annotation_path)?;
    let (mut sidecar, mut sidecar_revision, sidecar_changed) =
        load_sidecar(&document, &sidecar_path)?;
    if sidecar_changed {
        match SidecarStore::save_checked(&sidecar_path, &sidecar, sidecar_revision.as_deref()) {
            Ok(revision) => sidecar_revision = Some(revision),
            Err(SidecarStoreError::Io(error))
                if error.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                app.status =
                    "Annotations re-anchored in memory, but the sidecar is read-only".into();
            }
            Err(error) => return Err(error.into()),
        }
    }
    app.comments = sidecar.annotations.clone();
    app.selected_comment = (!app.comments.is_empty()).then_some(0);
    ui::run(
        &mut app,
        |source| {
            document.set_text(source.to_owned());
            document
                .save()
                .map_err(anyhow::Error::from)
                .map(|()| document.fingerprint().to_owned())
        },
        |annotations, fingerprint| {
            sidecar.annotations = annotations.to_vec();
            sidecar.document.fingerprint = fingerprint.to_owned();
            sidecar_revision = Some(
                SidecarStore::save_checked(&sidecar_path, &sidecar, sidecar_revision.as_deref())
                    .map_err(anyhow::Error::from)?,
            );
            match copy_prompt(&generate_prompt(&sidecar)) {
                ClipboardStatus::Copied { .. } | ClipboardStatus::SentOsc52 { .. } => Ok(()),
                ClipboardStatus::Unavailable => Err(anyhow!(
                    "comment saved, but no clipboard backend is available"
                )),
                ClipboardStatus::Failed { backend, message } => Err(anyhow!(
                    "comment saved, but {backend:?} clipboard copy failed: {message}"
                )),
            }
        },
    )
}

fn copy(file: PathBuf, annotation_path: Option<PathBuf>) -> Result<()> {
    let document = Document::load(&file)?;
    let path = sidecar_path(&document, annotation_path)?;
    let (sidecar, _, _) = load_sidecar(&document, &path)?;
    let prompt = generate_prompt(&sidecar);
    match copy_prompt(&prompt) {
        ClipboardStatus::Copied { backend } => {
            println!("Copied feedback with {backend:?}");
            Ok(())
        }
        ClipboardStatus::SentOsc52 { payload_bytes } => {
            println!(
                "Sent {payload_bytes} bytes with OSC 52; terminal confirmation is unavailable"
            );
            Ok(())
        }
        ClipboardStatus::Unavailable => Err(anyhow!("no clipboard backend is available")),
        ClipboardStatus::Failed { backend, message } => {
            Err(anyhow!("{backend:?} clipboard copy failed: {message}"))
        }
    }
}

fn export(
    file: PathBuf,
    annotation_path: Option<PathBuf>,
    output: PathBuf,
    force: bool,
) -> Result<()> {
    let document = Document::load(&file)?;
    let path = sidecar_path(&document, annotation_path)?;
    let prompt = generate_prompt(&load_sidecar(&document, &path)?.0);
    export_prompt(&output, &prompt, force).map_err(anyhow::Error::from)
}

fn sidecar_path(document: &Document, override_path: Option<PathBuf>) -> Result<PathBuf> {
    override_path.map_or_else(
        || default_sidecar_path(document.path()).map_err(anyhow::Error::from),
        Ok,
    )
}

fn load_sidecar(
    document: &Document,
    path: &std::path::Path,
) -> Result<(Sidecar, Option<String>, bool)> {
    let (mut sidecar, revision) = if path.exists() {
        let (sidecar, revision) =
            SidecarStore::load_with_revision(path).map_err(anyhow::Error::from)?;
        (sidecar, Some(revision))
    } else {
        (
            Sidecar::new(
                &document_reference(path, document.path())?,
                document.fingerprint(),
            ),
            None,
        )
    };
    let original = sidecar.clone();
    sidecar.document.path = document_reference(path, document.path())?
        .to_string_lossy()
        .into_owned();
    for annotation in &mut sidecar.annotations {
        reanchor_annotation(annotation, document.text(), document.fingerprint());
    }
    sidecar.document.fingerprint = document.fingerprint().to_owned();
    let changed = revision.is_some() && sidecar != original;
    Ok((sidecar, revision, changed))
}

fn document_reference(
    sidecar_path: &std::path::Path,
    document_path: &std::path::Path,
) -> Result<PathBuf> {
    let sidecar_directory = sidecar_path
        .parent()
        .ok_or_else(|| anyhow!("sidecar path has no parent directory"))?;
    let base = if sidecar_directory.is_absolute() {
        sidecar_directory.to_path_buf()
    } else {
        std::env::current_dir()?.join(sidecar_directory)
    };
    Ok(relative_path(&base, document_path))
}

fn relative_path(base: &std::path::Path, target: &std::path::Path) -> PathBuf {
    let base = base.components().collect::<Vec<_>>();
    let target = target.components().collect::<Vec<_>>();
    let shared = base
        .iter()
        .zip(&target)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in shared..base.len() {
        relative.push("..");
    }
    for component in &target[shared..] {
        relative.push(component.as_os_str());
    }
    relative
}

#[cfg(test)]
mod tests {
    use super::{document_reference, load_sidecar};
    use crate::{
        annotations::{Annotation, Sidecar, SidecarStore, capture_anchor},
        document::Document,
    };

    #[test]
    fn document_paths_are_relative_to_the_sidecar() {
        let root = tempfile::tempdir().unwrap();
        let document = root.path().join("guide.md");
        let sidecar = root
            .path()
            .join(".annoterm")
            .join("guide.md.annotations.json");

        assert_eq!(
            document_reference(&sidecar, &document).unwrap(),
            std::path::PathBuf::from("../guide.md")
        );
    }

    #[test]
    fn startup_reanchors_can_be_persisted_with_the_loaded_revision() {
        let root = tempfile::tempdir().unwrap();
        let document_path = root.path().join("guide.md");
        std::fs::write(&document_path, "prefix\nselected\n").unwrap();
        let document = Document::load(&document_path).unwrap();
        let sidecar_path = root.path().join("annotations.json");
        let mut sidecar = Sidecar::new(std::path::Path::new("guide.md"), "sha256:old");
        sidecar.annotations.push(Annotation::new(
            capture_anchor("selected\n", 0..8, "sha256:old", "paragraph").unwrap(),
            "Review this.",
        ));
        SidecarStore::save(&sidecar_path, &sidecar).unwrap();

        let (sidecar, revision, changed) = load_sidecar(&document, &sidecar_path).unwrap();
        assert!(changed);
        SidecarStore::save_checked(&sidecar_path, &sidecar, revision.as_deref()).unwrap();
        let persisted = SidecarStore::load(&sidecar_path).unwrap();

        assert_eq!(persisted.annotations[0].anchor.source_range.start.byte, 7);
        assert_eq!(persisted.document.fingerprint, document.fingerprint());
    }
}
