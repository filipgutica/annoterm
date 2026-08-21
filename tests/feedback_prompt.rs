use annoterm::annotations::{AnchorState, Annotation, NavigationHint, Sidecar, capture_anchor};
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

#[test]
fn prompt_reports_outdated_annotations_at_their_approximate_current_location() {
    let original = "Original paragraph.\n";
    let current = "Intro\nReplacement paragraph.\n";
    let mut sidecar = Sidecar::new(Path::new("docs/guide.md"), "sha256:current");
    let mut annotation = Annotation::new(
        capture_anchor(original, 0..19, "sha256:original", "paragraph").unwrap(),
        "Clarify this.",
    );
    annotation.anchor_state = AnchorState::Outdated;
    annotation.navigation_hint = Some(NavigationHint {
        source_range: capture_anchor(current, 6..28, "sha256:current", "paragraph")
            .unwrap()
            .source_range,
        document_fingerprint: "sha256:current".into(),
    });
    sidecar.annotations.push(annotation);

    let prompt = generate_prompt(&sidecar);

    assert!(prompt.contains("Location: approximately lines 2–2; original lines 1–1"));
    assert!(prompt.contains("State: outdated"));
}
