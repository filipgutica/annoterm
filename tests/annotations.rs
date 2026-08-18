use annoterm::annotations::{
    Annotation, Sidecar, SidecarStore, SidecarStoreError, capture_anchor, default_sidecar_path,
};
use tempfile::tempdir;

#[test]
fn sidecars_round_trip_with_stable_annotations() {
    let directory = tempdir().unwrap();
    let document_path = directory.path().join("guide.md");
    let source = "# Guide\n\nReview this paragraph.\n";
    let mut sidecar = Sidecar::new(&document_path, "sha256:document");
    sidecar.annotations.push(Annotation::new(
        capture_anchor(source, 9..31, "sha256:document", "paragraph").unwrap(),
        "Add an example.",
    ));

    let path = default_sidecar_path(&document_path).unwrap();
    SidecarStore::save(&path, &sidecar).unwrap();
    let loaded = SidecarStore::load(&path).unwrap();

    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.annotations.len(), 1);
    assert_eq!(loaded.annotations[0].comment, "Add an example.");
    assert_eq!(loaded.annotations[0].anchor.quote, "Review this paragraph.");
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
