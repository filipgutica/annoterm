use annoterm::annotations::{
    AnchorState, Annotation, ReanchorOutcome, capture_anchor, reanchor_annotation,
    reanchor_annotation_with_snapshots,
};
use std::collections::BTreeMap;

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
fn preserves_ambiguous_annotations_as_outdated_with_a_jump_hint() {
    let original = "Review this paragraph.\n";
    let mut annotation = Annotation::new(
        capture_anchor(original, 0..22, "sha256:before", "paragraph").unwrap(),
        "Clarify this.",
    );
    let revised = "New introduction.\nReview this paragraph.\n\nReview this paragraph.\n";

    assert_eq!(
        reanchor_annotation(&mut annotation, revised, "sha256:after"),
        ReanchorOutcome::Outdated
    );
    assert_eq!(annotation.anchor_state, AnchorState::Outdated);
    assert_eq!(annotation.anchor.quote, "Review this paragraph.");
    assert_eq!(
        annotation
            .navigation_hint
            .as_ref()
            .map(|hint| hint.source_range.start.line),
        Some(1)
    );
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
fn maps_a_rewritten_annotation_through_its_saved_document_snapshot() {
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
    let revised = "New introduction.\nBefore.\nReplacement sentence.\nAfter.\n";
    let snapshots = BTreeMap::from([("sha256:before".to_owned(), original.to_owned())]);

    assert_eq!(
        reanchor_annotation_with_snapshots(&mut annotation, revised, "sha256:after", &snapshots,),
        ReanchorOutcome::Outdated
    );
    assert_eq!(annotation.anchor_state, AnchorState::Outdated);
    let hint = annotation.navigation_hint.as_ref().unwrap();
    assert_eq!(hint.document_fingerprint, "sha256:after");
    assert_eq!(hint.source_range.start.line, 3);
    assert_eq!(
        &revised[hint.source_range.start.byte..hint.source_range.end.byte],
        "Replacement sentence.\n"
    );
}

#[test]
fn empty_rewritten_document_leaves_the_annotation_orphaned() {
    let original = "Original sentence.\n";
    let mut annotation = Annotation::new(
        capture_anchor(
            original,
            0.."Original sentence.".len(),
            "sha256:before",
            "paragraph",
        )
        .unwrap(),
        "Clarify this.",
    );
    let snapshots = BTreeMap::from([("sha256:before".to_owned(), original.to_owned())]);

    assert_eq!(
        reanchor_annotation_with_snapshots(&mut annotation, "", "sha256:after", &snapshots,),
        ReanchorOutcome::Orphaned
    );
    assert_eq!(annotation.anchor_state, AnchorState::Orphaned);
    assert!(annotation.navigation_hint.is_none());
}

#[test]
fn total_replacement_with_no_unchanged_boundary_stays_orphaned() {
    let original = "Original sentence.\n";
    let mut annotation = Annotation::new(
        capture_anchor(
            original,
            0.."Original sentence.".len(),
            "sha256:before",
            "paragraph",
        )
        .unwrap(),
        "Clarify this.",
    );
    let snapshots = BTreeMap::from([("sha256:before".to_owned(), original.to_owned())]);

    assert_eq!(
        reanchor_annotation_with_snapshots(
            &mut annotation,
            "Completely unrelated replacement.\n",
            "sha256:after",
            &snapshots,
        ),
        ReanchorOutcome::Orphaned
    );
    assert_eq!(annotation.anchor_state, AnchorState::Orphaned);
    assert!(annotation.navigation_hint.is_none());
}

#[test]
fn legacy_annotation_without_a_snapshot_uses_its_last_known_line() {
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

    assert_eq!(
        reanchor_annotation_with_snapshots(
            &mut annotation,
            "Different first line.\nDifferent second line.\n",
            "sha256:after",
            &BTreeMap::new(),
        ),
        ReanchorOutcome::Outdated
    );
    assert_eq!(
        annotation.navigation_hint.unwrap().source_range.start.line,
        2
    );
}

#[test]
fn newline_terminated_selection_does_not_include_the_following_line() {
    let original = "Before.\nTarget.\nAfter.\n";
    let start = "Before.\n".len();
    let mut annotation = Annotation::new(
        capture_anchor(
            original,
            start..start + "Target.\n".len(),
            "sha256:before",
            "paragraph",
        )
        .unwrap(),
        "Clarify this.",
    );
    let revised = "Before.\nReplacement.\nAfter.\n";
    let snapshots = BTreeMap::from([("sha256:before".to_owned(), original.to_owned())]);

    assert_eq!(
        reanchor_annotation_with_snapshots(&mut annotation, revised, "sha256:after", &snapshots),
        ReanchorOutcome::Outdated
    );
    let hint = annotation.navigation_hint.unwrap();
    assert_eq!(
        &revised[hint.source_range.start.byte..hint.source_range.end.byte],
        "Replacement.\n"
    );
}
