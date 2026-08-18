use annoterm::annotations::{AnchorState, Annotation, Sidecar, capture_anchor};
use annoterm::feedback::generate_prompt;
use std::path::Path;

#[test]
fn prompt_includes_open_annotations_in_source_order_and_orphans() {
    let source = "first\nsecond\n";
    let mut sidecar = Sidecar::new(Path::new("docs/guide.md"), "sha256:document");
    let later = Annotation::new(
        capture_anchor(source, 6..12, "sha256:document", "paragraph").unwrap(),
        "Second.",
    );
    let mut orphan = Annotation::new(
        capture_anchor(source, 0..5, "sha256:document", "paragraph").unwrap(),
        "First.",
    );
    orphan.anchor_state = AnchorState::Orphaned;
    sidecar.annotations.extend([later.clone(), orphan.clone()]);

    let prompt = generate_prompt(&sidecar);

    assert!(prompt.contains("Document: `docs/guide.md`"));
    assert!(
        prompt.find(&later.id.to_string()).unwrap() < prompt.find(&orphan.id.to_string()).unwrap()
    );
    assert!(prompt.contains("Location: orphaned; last known lines 1–1"));
    assert!(prompt.contains("Location: lines 2–2"));
}
