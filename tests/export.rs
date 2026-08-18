use annoterm::feedback::{ExportError, export_prompt};
use std::fs;
use tempfile::tempdir;

#[test]
fn export_writes_atomically_and_refuses_an_existing_file_without_force() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("feedback.md");

    export_prompt(&path, "# Feedback\n", false).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Feedback\n");
    assert!(matches!(
        export_prompt(&path, "new", false),
        Err(ExportError::AlreadyExists(_))
    ));
}
