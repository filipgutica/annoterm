use annoterm::cli::{Cli, Command};
use clap::error::ErrorKind;

#[test]
fn parses_open_copy_and_export_commands() {
    let open = Cli::try_parse_from(["annoterm", "notes.md"]).unwrap();
    assert!(matches!(open.command, Command::Open { .. }));

    let copy = Cli::try_parse_from(["annoterm", "copy-feedback", "notes.md"]).unwrap();
    assert!(matches!(copy.command, Command::Copy { .. }));

    let legacy_copy = Cli::try_parse_from(["annoterm", "copy", "notes.md"]).unwrap();
    assert!(matches!(legacy_copy.command, Command::Copy { .. }));

    let export =
        Cli::try_parse_from(["annoterm", "export", "notes.md", "--output", "feedback.md"]).unwrap();
    assert!(matches!(export.command, Command::Export { .. }));
}

#[test]
fn parses_annotation_overrides_and_standard_help() {
    let open = Cli::try_parse_from(["annoterm", "notes.md", "--annotations", "review/notes.json"])
        .unwrap();
    assert!(matches!(
        open.command,
        Command::Open {
            annotations: Some(_),
            ..
        }
    ));

    let help = Cli::try_parse_from(["annoterm", "--help"]).unwrap_err();
    assert_eq!(help.kind(), ErrorKind::DisplayHelp);
    assert!(help.to_string().contains("copy-feedback"));
    assert!(!help.to_string().contains("\n  copy "));
}
