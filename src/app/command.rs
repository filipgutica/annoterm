use std::ops::Range;

use uuid::Uuid;

/// User-visible state changes. Effects such as saving remain at the UI boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    ToggleMode,
    ToggleCommentFocus,
    FocusDocument,
    Insert(char),
    DeleteBackward,
    Undo,
    Redo,
    MoveRawLeft { select: bool },
    MoveRawRight { select: bool },
    MoveRawUp { select: bool },
    MoveRawDown { select: bool },
    MoveNextBlock,
    MovePreviousBlock,
    SetRawSelection(Range<usize>),
    AddComment(String),
    BeginComment,
    BeginEditSelectedComment,
    AppendCommentCharacter(char),
    DeleteCommentCharacter,
    SubmitComment,
    CancelComment,
    SelectNextComment,
    SelectPreviousComment,
    ToggleSelectedCommentResolution,
    DeleteSelectedComment,
    RepairSelectedOrphan,
    ResolveComment(Uuid),
    DeleteComment(Uuid),
    RepairComment(Uuid),
    SaveCompleted,
}
