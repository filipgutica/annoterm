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
fn comment_editor_moves_and_edits_at_grapheme_boundaries() {
    let source = "# Title\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::BeginComment).unwrap();
    for character in ['a', '🦀', 'b'] {
        app.apply(Command::AppendCommentCharacter(character))
            .unwrap();
    }

    app.apply(Command::MoveCommentCursorLeft).unwrap();
    app.apply(Command::MoveCommentCursorLeft).unwrap();
    app.apply(Command::MoveCommentCursorRight).unwrap();
    app.apply(Command::AppendCommentCharacter('X')).unwrap();
    assert_eq!(app.comment_draft.as_deref(), Some("a🦀Xb"));

    app.apply(Command::MoveCommentCursorLeft).unwrap();
    app.apply(Command::DeleteCommentCharacter).unwrap();
    assert_eq!(app.comment_draft.as_deref(), Some("aXb"));

    app.apply(Command::MoveCommentCursorStart).unwrap();
    app.apply(Command::AppendCommentCharacter('>')).unwrap();
    app.apply(Command::MoveCommentCursorEnd).unwrap();
    app.apply(Command::AppendCommentCharacter('<')).unwrap();
    assert_eq!(app.comment_draft.as_deref(), Some(">aXb<"));
}

#[test]
fn comment_editor_moves_between_unicode_word_starts() {
    let source = "# Title\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::BeginComment).unwrap();
    for character in "one  two 🦀 three".chars() {
        app.apply(Command::AppendCommentCharacter(character))
            .unwrap();
    }

    app.apply(Command::MoveCommentCursorStart).unwrap();
    app.apply(Command::MoveCommentCursorWordRight).unwrap();
    assert_eq!(app.comment_cursor(), 5);
    app.apply(Command::MoveCommentCursorWordRight).unwrap();
    assert_eq!(app.comment_cursor(), 14);
    app.apply(Command::MoveCommentCursorWordLeft).unwrap();
    assert_eq!(app.comment_cursor(), 5);
    app.apply(Command::MoveCommentCursorWordLeft).unwrap();
    assert_eq!(app.comment_cursor(), 0);
}

#[test]
fn jump_to_comment_selects_its_rendered_block() {
    let source = "# One\n\nTwo\n\nThree\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::MoveNextBlock).unwrap();
    app.apply(Command::MoveNextBlock).unwrap();
    app.apply(Command::AddComment("Review three".into()))
        .unwrap();
    app.apply(Command::MovePreviousBlock).unwrap();
    app.apply(Command::MovePreviousBlock).unwrap();

    app.apply(Command::JumpToSelectedComment).unwrap();

    assert_eq!(app.selected_block, 2);
}

#[test]
fn jump_to_comment_selects_its_raw_source_range() {
    let source = "# One\n\nTwo\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::SetRawSelection(7..10)).unwrap();
    app.apply(Command::AddComment("Review two".into())).unwrap();
    app.apply(Command::SetRawSelection(0..5)).unwrap();
    app.apply(Command::ToggleCommentFocus).unwrap();

    app.apply(Command::JumpToSelectedComment).unwrap();

    assert_eq!(app.raw_selection, Some(7..10));
    assert_eq!(app.cursor, 10);
    assert!(!app.comments_focused());
}

#[test]
fn failed_comment_jump_preserves_comment_focus() {
    let source = "\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::SetRawSelection(0..1)).unwrap();
    app.apply(Command::AddComment("Review whitespace".into()))
        .unwrap();
    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::ToggleCommentFocus).unwrap();

    assert!(app.apply(Command::JumpToSelectedComment).is_err());
    assert!(app.comments_focused());
}

#[test]
fn jump_to_outdated_comment_uses_its_current_navigation_hint() {
    let source = "# One\n\nReplacement\n";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::MoveNextBlock).unwrap();
    app.apply(Command::AddComment("Review this".into()))
        .unwrap();
    app.comments[0].anchor_state = annoterm::annotations::AnchorState::Outdated;
    app.comments[0].navigation_hint = Some(annoterm::annotations::NavigationHint {
        source_range: app.comments[0].anchor.source_range.clone(),
        document_fingerprint: app.document_fingerprint().to_owned(),
    });
    app.apply(Command::MovePreviousBlock).unwrap();

    app.apply(Command::JumpToSelectedComment).unwrap();

    assert_eq!(app.selected_block, 1);
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
fn search_selects_literal_matches_in_both_modes_and_wraps() {
    let source = "# Needle\n\nneedle\n\nNeedle again\n";
    let mut app = App::new(source.into(), parse(source).unwrap());

    app.apply(Command::BeginSearch).unwrap();
    for character in "Needle".chars() {
        app.apply(Command::AppendSearchCharacter(character))
            .unwrap();
    }
    assert_eq!(app.selected_block, 0);
    assert!(app.status.contains("1/3"));

    app.apply(Command::NextSearchMatch).unwrap();
    assert_eq!(app.selected_block, 1);
    assert!(app.status.contains("2/3"));
    app.apply(Command::NextSearchMatch).unwrap();
    assert_eq!(app.selected_block, 2);
    assert!(app.status.contains("3/3"));
    app.apply(Command::NextSearchMatch).unwrap();
    assert_eq!(app.selected_block, 0);
    app.apply(Command::PreviousSearchMatch).unwrap();
    assert_eq!(app.selected_block, 2);

    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::BeginSearch).unwrap();
    for _ in 0..6 {
        app.apply(Command::DeleteSearchCharacter).unwrap();
    }
    for character in "nEeDlE".chars() {
        app.apply(Command::AppendSearchCharacter(character))
            .unwrap();
    }
    assert_eq!(app.raw_selection, Some(2..8));
    assert_eq!(app.cursor, 8);
    assert!(app.status.contains("1/3"));

    app.apply(Command::NextSearchMatch).unwrap();
    assert_eq!(app.raw_selection, Some(10..16));
}

#[test]
fn search_is_case_insensitive_for_unicode_without_losing_source_ranges() {
    let source = "Ärger ärger ÄRGER";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::ToggleMode).unwrap();
    app.apply(Command::BeginSearch).unwrap();
    for character in "äRgEr".chars() {
        app.apply(Command::AppendSearchCharacter(character))
            .unwrap();
    }

    assert_eq!(app.raw_selection, Some(0..6));
    assert!(app.status.contains("1/3"));
    app.apply(Command::NextSearchMatch).unwrap();
    assert_eq!(app.raw_selection, Some(7..13));
    app.apply(Command::NextSearchMatch).unwrap();
    assert_eq!(app.raw_selection, Some(14..20));

    let source = "Straße STRASSE";
    let mut expansion_app = App::new(source.into(), parse(source).unwrap());
    expansion_app.apply(Command::ToggleMode).unwrap();
    expansion_app.apply(Command::BeginSearch).unwrap();
    for character in "strasse".chars() {
        expansion_app
            .apply(Command::AppendSearchCharacter(character))
            .unwrap();
    }
    assert_eq!(expansion_app.raw_selection, Some(0..7));
    expansion_app.apply(Command::NextSearchMatch).unwrap();
    assert_eq!(expansion_app.raw_selection, Some(8..15));

    let source = "Sß";
    let mut adjacent_app = App::new(source.into(), parse(source).unwrap());
    adjacent_app.apply(Command::ToggleMode).unwrap();
    adjacent_app.apply(Command::BeginSearch).unwrap();
    for character in "ss".chars() {
        adjacent_app
            .apply(Command::AppendSearchCharacter(character))
            .unwrap();
    }
    assert_eq!(adjacent_app.raw_selection, Some(1..3));
    assert!(adjacent_app.status.contains("1/1"));
}

#[test]
fn search_backspace_is_grapheme_safe_and_reopening_retains_the_query() {
    let source = "a🦀b";
    let mut app = App::new(source.into(), parse(source).unwrap());
    app.apply(Command::ToggleMode).unwrap();

    app.apply(Command::BeginSearch).unwrap();
    app.apply(Command::AppendSearchCharacter('🦀')).unwrap();
    assert_eq!(app.raw_selection, Some(1..5));
    app.apply(Command::DeleteSearchCharacter).unwrap();
    assert_eq!(app.search_query(), "");
    assert!(app.status.contains("type to search"));

    app.apply(Command::AppendSearchCharacter('b')).unwrap();
    app.apply(Command::CloseSearch).unwrap();
    app.apply(Command::BeginSearch).unwrap();
    assert_eq!(app.search_query(), "b");
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
