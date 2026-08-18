use std::fs;

use annoterm::document::{Document, SaveError};
use tempfile::tempdir;

#[test]
fn load_and_save_preserve_bom_crlf_and_missing_final_newline() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("notes.md");
    fs::write(&path, b"\xef\xbb\xbfone\r\ntwo").unwrap();

    let mut document = Document::load(&path).unwrap();
    assert_eq!(document.text(), "one\ntwo");
    assert!(document.format().has_utf8_bom);
    assert!(!document.format().has_final_newline);

    document.set_text("one\ntwo\nthree".into());
    document.save().unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"\xef\xbb\xbfone\r\ntwo\r\nthree");
}

#[test]
fn save_refuses_to_replace_an_externally_changed_file() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("notes.md");
    fs::write(&path, "first\n").unwrap();

    let mut document = Document::load(&path).unwrap();
    document.set_text("edited\n".into());
    fs::write(&path, "external\n").unwrap();

    assert!(matches!(
        document.save(),
        Err(SaveError::ExternalChange { .. })
    ));
    assert_eq!(fs::read_to_string(path).unwrap(), "external\n");
}
