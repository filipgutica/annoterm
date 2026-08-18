use annoterm::annotations::{
    AnchorState, Annotation, ReanchorOutcome, capture_anchor, reanchor_annotation,
};

#[test]
fn reanchors_a_unique_quote_after_inserted_lines() {
    let original = "# Title\n\nReview this paragraph.\n";
    let mut annotation = Annotation::new(
        capture_anchor(original, 9..31, "sha256:before", "paragraph").unwrap(),
        "Clarify this.",
    );
    let revised = "# Title\n\nAdded material.\n\nReview this paragraph.\n";

    assert_eq!(
        reanchor_annotation(&mut annotation, revised, "sha256:after"),
        ReanchorOutcome::Reanchored
    );
    assert_eq!(annotation.anchor_state, AnchorState::Anchored);
    assert_eq!(annotation.anchor.source_range.start.line, 5);
    assert_eq!(annotation.anchor.document_fingerprint, "sha256:after");
}

#[test]
fn preserves_ambiguous_annotations_as_orphans() {
    let original = "Review this paragraph.\n";
    let mut annotation = Annotation::new(
        capture_anchor(original, 0..22, "sha256:before", "paragraph").unwrap(),
        "Clarify this.",
    );
    let revised = "New introduction.\nReview this paragraph.\n\nReview this paragraph.\n";

    assert_eq!(
        reanchor_annotation(&mut annotation, revised, "sha256:after"),
        ReanchorOutcome::Orphaned
    );
    assert_eq!(annotation.anchor_state, AnchorState::Orphaned);
    assert_eq!(annotation.anchor.quote, "Review this paragraph.");
}

#[test]
fn reanchors_a_small_quote_edit_bounded_by_unique_context() {
    let original = "Before line.\nOriginal sentence.\nAfter line.\n";
    let start = "Before line.\n".len();
    let mut annotation = Annotation::new(
        capture_anchor(
            original,
            start..start + "Original sentence.".len(),
            "sha256:before",
            "paragraph",
        )
        .unwrap(),
        "Clarify this.",
    );
    let revised = "Before line.\nOriginal sentence!\nAfter line.\n";

    assert_eq!(
        reanchor_annotation(&mut annotation, revised, "sha256:after"),
        ReanchorOutcome::Reanchored
    );
    assert_eq!(annotation.anchor.quote, "Original sentence!");
    assert_eq!(annotation.anchor_state, AnchorState::Anchored);
}

#[test]
fn orphans_a_changed_quote_when_context_is_not_unique() {
    let original = "Before.\nOriginal sentence.\nAfter.\n";
    let start = "Before.\n".len();
    let mut annotation = Annotation::new(
        capture_anchor(
            original,
            start..start + "Original sentence.".len(),
            "sha256:before",
            "paragraph",
        )
        .unwrap(),
        "Clarify this.",
    );
    let revised = "Before.\nOriginal sentence!\nAfter.\nBefore.\nOriginal sentence!\nAfter.\n";

    assert_eq!(
        reanchor_annotation(&mut annotation, revised, "sha256:after"),
        ReanchorOutcome::Orphaned
    );
    assert_eq!(annotation.anchor_state, AnchorState::Orphaned);
}
