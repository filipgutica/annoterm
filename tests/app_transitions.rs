use annoterm::app::{App, Command, Mode};
use annoterm::markdown::parse;

#[test]
fn toggles_modes_and_tracks_raw_edits() {
    let rendered = parse("# Title\n\nBody\n").unwrap();
    let mut app = App::new("# Title\n\nBody\n".into(), rendered);

    app.apply(Command::ToggleMode).unwrap();
    assert_eq!(app.mode, Mode::Raw);
    app.apply(Command::Insert('!')).unwrap();
    assert!(app.dirty);
    app.apply(Command::ToggleMode).unwrap();
    assert_eq!(app.mode, Mode::Rendered);
}

#[test]
fn comments_can_be_added_resolved_and_deleted() {
    let source = "# Title\n\nBody\n";
    let mut app = App::new(source.into(), parse(source).unwrap());

    app.apply(Command::AddComment("Clarify this".into()))
        .unwrap();
    let id = app.comments[0].id;
    app.apply(Command::ResolveComment(id)).unwrap();
    assert!(app.comments[0].status == annoterm::annotations::AnnotationStatus::Resolved);
    app.apply(Command::DeleteComment(id)).unwrap();
    assert!(app.comments.is_empty());
}

#[test]
fn raw_navigation_keeps_grapheme_safe_selection_and_comment_draft() {
    let source = "a🦀b\nsecond\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::MoveRawRight { select: false }).unwrap();
    app.apply(Command::MoveRawRight { select: true }).unwrap();
    assert_eq!(app.raw_selection, Some(1..5));
    app.apply(Command::BeginComment).unwrap();
    app.apply(Command::AppendCommentCharacter('C')).unwrap();
    app.apply(Command::SubmitComment).unwrap();
    assert_eq!(app.comments[0].comment, "C");
}

#[test]
fn raw_edits_support_undo_and_redo() {
    let source = "Body\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::Insert('!')).unwrap();
    assert_eq!(app.source, "!Body\n");
    app.apply(Command::Undo).unwrap();
    assert_eq!(app.source, source);
    app.apply(Command::Redo).unwrap();
    assert_eq!(app.source, "!Body\n");
}

#[test]
fn undo_keeps_the_cursor_on_a_grapheme_boundary() {
    for source in ["🦀", "e\u{301}"] {
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        app.apply(Command::Insert('x')).unwrap();
        app.apply(Command::Undo).unwrap();

        assert_eq!(app.cursor, 0);
    }
}

#[test]
fn raw_selection_rejects_non_utf8_boundaries() {
    let source = "🦀";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::ToggleMode).unwrap();

    assert!(app.apply(Command::SetRawSelection(1..2)).is_err());
}

#[test]
fn rendering_after_raw_edits_reanchors_comment_markers() {
    let source = "# Heading\n\nParagraph\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::MoveNextBlock).unwrap();
    app.apply(Command::AddComment("paragraph".into())).unwrap();
    let old_start = app.comments[0].anchor.source_range.start.byte;

    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::Insert('x')).unwrap();
    app.apply(Command::ToggleMode).unwrap();

    assert_eq!(
        app.comments[0].anchor.source_range.start.byte,
        old_start + 1
    );
}

#[test]
fn raw_edits_replace_selected_unicode_text_and_insert_newlines() {
    let source = "a🦀b";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::SetRawSelection(1..5)).unwrap();
    app.apply(Command::Insert('X')).unwrap();
    app.apply(Command::Insert('\n')).unwrap();

    assert_eq!(app.source, "aX\nb");
    assert!(app.raw_selection.is_none());
}

#[test]
fn raw_selection_keeps_its_anchor_when_direction_changes() {
    let source = "abc";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::MoveRawRight { select: false }).unwrap();
    app.apply(Command::MoveRawRight { select: true }).unwrap();
    app.apply(Command::MoveRawLeft { select: true }).unwrap();
    app.apply(Command::MoveRawLeft { select: true }).unwrap();

    assert_eq!(app.raw_selection, Some(0..1));
}

#[test]
fn selected_comment_can_be_edited_resolved_reopened_deleted_and_repaired() {
    let source = "# Heading\n\nParagraph\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::AddComment("heading".into())).unwrap();
    app.apply(Command::MoveNextBlock).unwrap();
    app.apply(Command::AddComment("paragraph".into())).unwrap();
    assert_eq!(app.selected_comment, Some(1));

    app.apply(Command::SelectPreviousComment).unwrap();
    app.apply(Command::BeginEditSelectedComment).unwrap();
    app.apply(Command::AppendCommentCharacter('!')).unwrap();
    app.apply(Command::SubmitComment).unwrap();
    assert_eq!(app.comments[0].comment, "heading!");

    app.apply(Command::ToggleSelectedCommentResolution).unwrap();
    assert_eq!(
        app.comments[0].status,
        annoterm::annotations::AnnotationStatus::Resolved
    );
    app.apply(Command::ToggleSelectedCommentResolution).unwrap();
    assert_eq!(
        app.comments[0].status,
        annoterm::annotations::AnnotationStatus::Open
    );

    app.comments[0].anchor_state = annoterm::annotations::AnchorState::Orphaned;
    app.apply(Command::RepairSelectedOrphan).unwrap();
    assert_eq!(
        app.comments[0].anchor_state,
        annoterm::annotations::AnchorState::Anchored
    );
    app.apply(Command::DeleteSelectedComment).unwrap();
    assert_eq!(app.comments.len(), 1);
}
