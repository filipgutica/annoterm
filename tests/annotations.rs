use annoterm::annotations::{
    Annotation, Sidecar, SidecarStore, SidecarStoreError, capture_anchor, default_sidecar_path,
};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

#[test]
fn default_sidecars_are_stored_in_user_home_by_canonical_document_path() {
    let directory = tempdir().unwrap();
    let document_path = directory.path().join("guide.md");
    let source = "# Guide\n\nReview this paragraph.\n";
    std::fs::write(&document_path, source).unwrap();

    let path = default_sidecar_path(&document_path).unwrap();
    let canonical_path = std::fs::canonicalize(&document_path).unwrap();
    let expected_filename = format!(
        "{:x}.annotations.json",
        Sha256::digest(canonical_path.as_os_str().as_encoded_bytes())
    );

    assert_eq!(
        path,
        std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
            .join(".annoterm")
            .join(expected_filename)
    );
}

#[test]
fn sidecars_round_trip_with_stable_annotations() {
    let directory = tempdir().unwrap();
    let document_path = directory.path().join("guide.md");
    let source = "# Guide\n\nReview this paragraph.\n";
    std::fs::write(&document_path, source).unwrap();
    let mut sidecar = Sidecar::new(&document_path, "sha256:document");
    sidecar.annotations.push(Annotation::new(
        capture_anchor(source, 9..31, "sha256:document", "paragraph").unwrap(),
        "Add an example.",
    ));

    let path = directory.path().join("annotations.json");
    SidecarStore::save(&path, &sidecar).unwrap();
    let loaded = SidecarStore::load(&path).unwrap();

    assert_eq!(loaded.schema_version, 2);
    assert_eq!(loaded.annotations.len(), 1);
    assert_eq!(loaded.annotations[0].comment, "Add an example.");
    assert_eq!(loaded.annotations[0].anchor.quote, "Review this paragraph.");
}

#[test]
fn schema_version_one_sidecars_migrate_with_empty_snapshots() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("annotations.json");
    std::fs::write(
        &path,
        r#"{
  "schema_version": 1,
  "document": {
    "id": "019c0000-0000-7000-8000-000000000000",
    "path": "../guide.md",
    "fingerprint": "sha256:document"
  },
  "annotations": []
}
"#,
    )
    .unwrap();

    let (sidecar, _) = SidecarStore::load_with_revision(&path).unwrap();

    assert_eq!(sidecar.schema_version, 2);
    assert!(sidecar.snapshots.is_empty());
}

#[test]
fn checked_sidecar_save_refuses_external_changes() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("annotations.json");
    let sidecar = Sidecar::new(std::path::Path::new("../guide.md"), "sha256:document");
    SidecarStore::save(&path, &sidecar).unwrap();
    let (_, revision) = SidecarStore::load_with_revision(&path).unwrap();
    std::fs::write(&path, b"external change").unwrap();

    assert!(matches!(
        SidecarStore::save_checked(&path, &sidecar, Some(&revision)),
        Err(SidecarStoreError::ExternalChange)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"external change");
}

#[test]
fn concurrent_checked_sidecar_writers_cannot_overwrite_each_other() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("annotations.json");
    let sidecar = Sidecar::new(std::path::Path::new("../guide.md"), "sha256:document");
    SidecarStore::save(&path, &sidecar).unwrap();
    let (_, revision) = SidecarStore::load_with_revision(&path).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

    let writers = ["sha256:writer-one", "sha256:writer-two"].map(|fingerprint| {
        let path = path.clone();
        let revision = revision.clone();
        let barrier = barrier.clone();
        let mut sidecar = sidecar.clone();
        sidecar.document.fingerprint = fingerprint.into();
        std::thread::spawn(move || {
            barrier.wait();
            SidecarStore::save_checked(&path, &sidecar, Some(&revision))
        })
    });
    barrier.wait();
    let results = writers.map(|writer| writer.join().unwrap());

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(SidecarStoreError::ExternalChange)))
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn private_sidecar_save_restricts_default_store_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempdir().unwrap();
    let store_directory = directory.path().join(".annoterm");
    let path = store_directory.join("annotations.json");
    let sidecar = Sidecar::new(std::path::Path::new("../guide.md"), "sha256:document");

    SidecarStore::save_checked_private(&path, &sidecar, None).unwrap();

    assert_eq!(
        std::fs::metadata(&store_directory)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let lock_path = store_directory.join("annotations.json.lock");
    assert_eq!(
        std::fs::metadata(lock_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn captured_context_is_limited_to_two_lines_and_512_utf8_bytes() {
    let before = "é".repeat(300);
    let after = "界".repeat(300);
    let source = format!("{before}\nselected\n{after}");
    let start = before.len() + 1;
    let anchor = capture_anchor(
        &source,
        start..start + "selected".len(),
        "sha256:document",
        "paragraph",
    )
    .unwrap();

    assert!(anchor.context_before.len() <= 512);
    assert!(anchor.context_after.len() <= 512);
    assert!(
        anchor
            .context_before
            .is_char_boundary(anchor.context_before.len())
    );
    assert!(
        anchor
            .context_after
            .is_char_boundary(anchor.context_after.len())
    );
}
