use std::{collections::BTreeMap, ops::Range};

use anyhow::{Result, anyhow};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::{
    annotations::{
        AnchorState, Annotation, AnnotationStatus, capture_anchor,
        reanchor_annotation_with_snapshots,
    },
    document::fingerprint_text,
    markdown::{RenderedDocument, parse},
};

use super::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Rendered,
    Raw,
}

/// A comment shown by the UI is the durable annotation contract.
pub type Comment = Annotation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommentDraftMode {
    New,
    Edit(Uuid),
}

const HISTORY_LIMIT: usize = 100;
const TABLE_SCROLL_STEP: usize = 4;

/// UI state shared by raw and rendered modes.
#[derive(Clone, Debug)]
pub struct App {
    pub source: String,
    pub rendered: RenderedDocument,
    pub mode: Mode,
    pub cursor: usize,
    pub selected_block: usize,
    table_horizontal_scroll: usize,
    table_max_horizontal_scroll: usize,
    rendered_scroll_override: Option<(u16, u16, u16)>,
    preserve_rendered_scroll_once: bool,
    pub raw_selection: Option<Range<usize>>,
    raw_selection_anchor: Option<usize>,
    pub comments: Vec<Comment>,
    pub comment_draft: Option<String>,
    comment_cursor: usize,
    pub selected_comment: Option<usize>,
    comments_focused: bool,
    help_visible: bool,
    pub dirty: bool,
    pub status: String,
    document_fingerprint: String,
    annotation_snapshots: BTreeMap<String, String>,
    comment_draft_mode: Option<CommentDraftMode>,
    undo_history: Vec<String>,
    redo_history: Vec<String>,
}

impl App {
    pub fn new(source: String, rendered: RenderedDocument) -> Self {
        let document_fingerprint = fingerprint_text(&source);
        let annotation_snapshots = BTreeMap::from([(document_fingerprint.clone(), source.clone())]);
        Self {
            source,
            rendered,
            mode: Mode::Rendered,
            cursor: 0,
            selected_block: 0,
            table_horizontal_scroll: 0,
            table_max_horizontal_scroll: 0,
            rendered_scroll_override: None,
            preserve_rendered_scroll_once: false,
            raw_selection: None,
            raw_selection_anchor: None,
            comments: Vec::new(),
            comment_draft: None,
            comment_cursor: 0,
            selected_comment: None,
            comments_focused: false,
            help_visible: false,
            dirty: false,
            status: "Rendered mode · Ctrl+R switches to raw mode".into(),
            document_fingerprint,
            annotation_snapshots,
            comment_draft_mode: None,
            undo_history: Vec::new(),
            redo_history: Vec::new(),
        }
    }

    pub fn set_document_fingerprint(&mut self, fingerprint: impl Into<String>) {
        self.document_fingerprint = fingerprint.into();
        self.annotation_snapshots
            .insert(self.document_fingerprint.clone(), self.source.clone());
    }

    pub fn set_annotation_snapshots(&mut self, snapshots: BTreeMap<String, String>) {
        self.annotation_snapshots = snapshots;
        self.annotation_snapshots
            .insert(self.document_fingerprint.clone(), self.source.clone());
    }

    pub fn document_fingerprint(&self) -> &str {
        &self.document_fingerprint
    }

    pub fn comments_focused(&self) -> bool {
        self.comments_focused
    }

    pub fn help_visible(&self) -> bool {
        self.help_visible
    }

    pub fn comment_cursor(&self) -> usize {
        self.comment_cursor
    }

    pub(crate) fn rendered_scroll_override(&self, width: u16, height: u16) -> Option<u16> {
        self.rendered_scroll_override
            .filter(|(_, saved_width, saved_height)| {
                *saved_width == width && *saved_height == height
            })
            .map(|(scroll, _, _)| scroll)
    }

    pub(crate) fn preserved_rendered_scroll(&self, width: u16, height: u16) -> Option<u16> {
        self.preserve_rendered_scroll_once
            .then(|| self.rendered_scroll_override(width, height))
            .flatten()
    }

    pub(crate) fn select_rendered_block_from_click(
        &mut self,
        index: usize,
        scroll: u16,
        width: u16,
        height: u16,
    ) {
        self.select_rendered_block(index);
        self.rendered_scroll_override = Some((scroll, width, height));
        self.preserve_rendered_scroll_once = true;
    }

    pub(crate) fn remember_rendered_scroll(&mut self, scroll: u16, width: u16, height: u16) {
        self.rendered_scroll_override = Some((scroll, width, height));
        self.preserve_rendered_scroll_once = false;
    }

    pub(crate) fn table_horizontal_scroll(&self) -> usize {
        self.table_horizontal_scroll
    }

    pub(crate) fn remember_table_horizontal_extent(&mut self, max_scroll: usize) {
        self.table_max_horizontal_scroll = max_scroll;
        self.table_horizontal_scroll = self.table_horizontal_scroll.min(max_scroll);
    }

    pub(crate) fn scroll_table_horizontally(&mut self, direction: isize) {
        if direction < 0 {
            self.table_horizontal_scroll = self
                .table_horizontal_scroll
                .saturating_sub(TABLE_SCROLL_STEP);
        } else {
            self.table_horizontal_scroll = self
                .table_horizontal_scroll
                .saturating_add(TABLE_SCROLL_STEP)
                .min(self.table_max_horizontal_scroll);
        }
    }

    pub fn reanchor_comments(&mut self) {
        for comment in &mut self.comments {
            reanchor_annotation_with_snapshots(
                comment,
                &self.source,
                &self.document_fingerprint,
                &self.annotation_snapshots,
            );
        }
    }

    pub fn apply(&mut self, command: Command) -> Result<()> {
        match command {
            Command::ToggleHelp => self.help_visible = !self.help_visible,
            Command::CloseHelp => self.help_visible = false,
            Command::ToggleMode => self.toggle_mode()?,
            Command::ToggleCommentFocus => {
                if self.comments_focused {
                    self.comments_focused = false;
                    self.status = match self.mode {
                        Mode::Rendered => "Rendered document focused".into(),
                        Mode::Raw => "Raw document focused".into(),
                    };
                } else if self.comments.is_empty() {
                    self.status = "There are no comments to focus".into();
                } else {
                    self.comments_focused = true;
                    self.status =
                        "Comments focused · ↑/↓ or [/] select · j jump · e edit · x resolve/reopen · d delete · Esc returns"
                            .into();
                }
            }
            Command::FocusDocument => {
                self.comments_focused = false;
                self.status = match self.mode {
                    Mode::Rendered => "Rendered document focused".into(),
                    Mode::Raw => "Raw document focused".into(),
                };
            }
            Command::Insert(character) => self.insert(character),
            Command::DeleteBackward => self.delete_backward(),
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::MoveRawLeft { select } => self.move_raw_left(select),
            Command::MoveRawRight { select } => self.move_raw_right(select),
            Command::MoveRawUp { select } => self.move_raw_vertically(-1, select),
            Command::MoveRawDown { select } => self.move_raw_vertically(1, select),
            Command::MoveNextBlock => {
                if !self.rendered.blocks.is_empty() {
                    let selected_block = self
                        .selected_block
                        .saturating_add(1)
                        .min(self.rendered.blocks.len() - 1);
                    self.select_rendered_block(selected_block);
                }
            }
            Command::MovePreviousBlock => {
                let selected_block = self.selected_block.saturating_sub(1);
                self.select_rendered_block(selected_block);
            }
            Command::SetRawSelection(range) => {
                let range = normalize_range(range, &self.source)?;
                self.raw_selection_anchor = Some(range.start);
                self.cursor = range.end;
                self.raw_selection = Some(range);
            }
            Command::AddComment(body) => self.add_comment(body)?,
            Command::BeginComment => {
                self.comment_draft = Some(String::new());
                self.comment_cursor = 0;
                self.comment_draft_mode = Some(CommentDraftMode::New);
            }
            Command::BeginEditSelectedComment => {
                let comment = self.selected_comment()?;
                let (id, draft) = (comment.id, comment.comment.clone());
                self.comment_cursor = draft.len();
                self.comment_draft = Some(draft);
                self.comment_draft_mode = Some(CommentDraftMode::Edit(id));
            }
            Command::AppendCommentCharacter(character) => self.insert_comment_character(character),
            Command::DeleteCommentCharacter => self.delete_comment_character(),
            Command::MoveCommentCursorLeft => self.move_comment_cursor_left(),
            Command::MoveCommentCursorRight => self.move_comment_cursor_right(),
            Command::MoveCommentCursorWordLeft => self.move_comment_cursor_word_left(),
            Command::MoveCommentCursorWordRight => self.move_comment_cursor_word_right(),
            Command::MoveCommentCursorStart => self.comment_cursor = 0,
            Command::MoveCommentCursorEnd => {
                self.comment_cursor = self.comment_draft.as_ref().map_or(0, String::len)
            }
            Command::SubmitComment => {
                let body = self.comment_draft.take().unwrap_or_default();
                self.comment_cursor = 0;
                match self.comment_draft_mode.take() {
                    Some(CommentDraftMode::New) if !body.trim().is_empty() => {
                        self.add_comment(body)?
                    }
                    Some(CommentDraftMode::Edit(id)) if !body.trim().is_empty() => {
                        let comment = self.comment_mut(id)?;
                        comment.comment = body;
                        comment.touch();
                    }
                    _ => {}
                }
            }
            Command::CancelComment => {
                self.comment_draft = None;
                self.comment_cursor = 0;
                self.comment_draft_mode = None;
            }
            Command::SelectNextComment => self.move_comment_selection(1),
            Command::SelectPreviousComment => self.move_comment_selection(-1),
            Command::JumpToSelectedComment => self.jump_to_selected_comment()?,
            Command::ToggleSelectedCommentResolution => {
                let comment = self.selected_comment_mut()?;
                comment.status = match comment.status {
                    AnnotationStatus::Open => AnnotationStatus::Resolved,
                    AnnotationStatus::Resolved => AnnotationStatus::Open,
                };
                comment.touch();
            }
            Command::DeleteSelectedComment => {
                let index = self
                    .selected_comment
                    .ok_or_else(|| anyhow!("There is no selected comment"))?;
                self.comments.remove(index);
                self.selected_comment =
                    (!self.comments.is_empty()).then(|| index.min(self.comments.len() - 1));
                if self.comments.is_empty() {
                    self.comments_focused = false;
                    self.status = "No comments remain · document focused".into();
                }
            }
            Command::RepairSelectedOrphan => {
                let id = self.selected_comment()?.id;
                if self.selected_comment()?.anchor_state == AnchorState::Anchored {
                    return Err(anyhow!("Only outdated or detached comments need repair"));
                }
                self.apply(Command::RepairComment(id))?;
            }
            Command::ResolveComment(id) => {
                let comment = self.comment_mut(id)?;
                comment.status = AnnotationStatus::Resolved;
                comment.touch();
            }
            Command::DeleteComment(id) => {
                let index = self.comment_index(id)?;
                self.comments.remove(index);
            }
            Command::RepairComment(id) => {
                let range = self.selected_range()?;
                let anchor =
                    capture_anchor(&self.source, range, &self.document_fingerprint, "repaired")?;
                let comment = self.comment_mut(id)?;
                comment.anchor = anchor;
                comment.anchor_state = AnchorState::Anchored;
                comment.navigation_hint = None;
                comment.touch();
            }
            Command::SaveCompleted => {
                self.dirty = false;
                self.status = "Saved".into();
            }
        }
        Ok(())
    }

    fn insert_comment_character(&mut self, character: char) {
        let Some(draft) = &mut self.comment_draft else {
            return;
        };
        if !draft.is_char_boundary(self.comment_cursor) {
            return;
        }
        draft.insert(self.comment_cursor, character);
        self.comment_cursor += character.len_utf8();
    }

    fn delete_comment_character(&mut self) {
        let Some(draft) = &mut self.comment_draft else {
            return;
        };
        if self.comment_cursor == 0 || !draft.is_char_boundary(self.comment_cursor) {
            return;
        }
        let start = draft[..self.comment_cursor]
            .grapheme_indices(true)
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
        draft.replace_range(start..self.comment_cursor, "");
        self.comment_cursor = start;
    }

    fn move_comment_cursor_left(&mut self) {
        let Some(draft) = &self.comment_draft else {
            return;
        };
        self.comment_cursor = draft[..self.comment_cursor]
            .grapheme_indices(true)
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
    }

    fn move_comment_cursor_right(&mut self) {
        let Some(draft) = &self.comment_draft else {
            return;
        };
        self.comment_cursor = draft[self.comment_cursor..]
            .grapheme_indices(true)
            .nth(1)
            .map(|(index, _)| self.comment_cursor + index)
            .unwrap_or(draft.len());
    }

    fn move_comment_cursor_word_left(&mut self) {
        let Some(draft) = &self.comment_draft else {
            return;
        };
        self.comment_cursor = draft[..self.comment_cursor]
            .unicode_word_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
    }

    fn move_comment_cursor_word_right(&mut self) {
        let Some(draft) = &self.comment_draft else {
            return;
        };
        let mut words = draft[self.comment_cursor..].unicode_word_indices();
        self.comment_cursor = match words.next() {
            Some((index, _)) if index > 0 => self.comment_cursor + index,
            Some(_) => words
                .next()
                .map(|(index, _)| self.comment_cursor + index)
                .unwrap_or(draft.len()),
            None => draft.len(),
        };
    }

    fn jump_to_selected_comment(&mut self) -> Result<()> {
        let comment = self.selected_comment()?;
        let range = comment
            .current_range(&self.document_fingerprint)
            .map(|range| range.start.byte..range.end.byte)
            .ok_or_else(|| anyhow!("Cannot jump to a detached comment"))?;
        match self.mode {
            Mode::Rendered => {
                let selected_block = self
                    .rendered
                    .blocks
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, block)| {
                        if range.start < block.source_range.end
                            && range.end > block.source_range.start
                        {
                            0
                        } else if block.source_range.end <= range.start {
                            range.start - block.source_range.end
                        } else {
                            block.source_range.start.saturating_sub(range.end)
                        }
                    })
                    .map(|(index, _)| index)
                    .ok_or_else(|| anyhow!("The comment is outside the rendered document"))?;
                self.select_rendered_block(selected_block);
            }
            Mode::Raw => {
                let range = normalize_range(range, &self.source)?;
                self.raw_selection_anchor = Some(range.start);
                self.cursor = range.end;
                self.raw_selection = Some(range);
            }
        }
        self.comments_focused = false;
        self.status = "Jumped to selected comment".into();
        Ok(())
    }

    fn toggle_mode(&mut self) -> Result<()> {
        self.reset_table_horizontal_scroll();
        match self.mode {
            Mode::Rendered => {
                self.rendered_scroll_override = None;
                self.preserve_rendered_scroll_once = false;
                self.mode = Mode::Raw;
                self.status = "Raw mode · Ctrl+S saves · Ctrl+K comments · Ctrl+R renders".into();
            }
            Mode::Raw => {
                self.rendered_scroll_override = None;
                self.preserve_rendered_scroll_once = false;
                self.rendered = parse(&self.source)?;
                let current_fingerprint = fingerprint_text(&self.source);
                for comment in &mut self.comments {
                    reanchor_annotation_with_snapshots(
                        comment,
                        &self.source,
                        &current_fingerprint,
                        &self.annotation_snapshots,
                    );
                }
                self.selected_block = self
                    .selected_block
                    .min(self.rendered.blocks.len().saturating_sub(1));
                self.mode = Mode::Rendered;
                self.status = "Rendered mode · Ctrl+R switches to raw mode".into();
            }
        }
        Ok(())
    }

    fn reset_table_horizontal_scroll(&mut self) {
        self.table_horizontal_scroll = 0;
        self.table_max_horizontal_scroll = 0;
    }

    fn select_rendered_block(&mut self, index: usize) {
        let selected_block = index.min(self.rendered.blocks.len().saturating_sub(1));
        if self.selected_block != selected_block {
            self.reset_table_horizontal_scroll();
        }
        self.selected_block = selected_block;
    }

    fn insert(&mut self, character: char) {
        if self.mode != Mode::Raw || !self.source.is_char_boundary(self.cursor) {
            return;
        }
        self.record_edit();
        if let Some(selection) = self.raw_selection.take() {
            self.source.replace_range(selection.clone(), "");
            self.cursor = selection.start;
        }
        self.raw_selection_anchor = None;
        self.source.insert(self.cursor, character);
        self.cursor += character.len_utf8();
        self.dirty = true;
    }

    fn delete_backward(&mut self) {
        if self.mode != Mode::Raw || !self.source.is_char_boundary(self.cursor) {
            return;
        }
        if let Some(selection) = self.raw_selection.take() {
            self.record_edit();
            self.source.replace_range(selection.clone(), "");
            self.cursor = selection.start;
            self.raw_selection_anchor = None;
            self.dirty = true;
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let start = self.source[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
        self.record_edit();
        self.source.replace_range(start..self.cursor, "");
        self.cursor = start;
        self.dirty = true;
    }

    fn undo(&mut self) {
        if self.mode != Mode::Raw {
            return;
        }
        if let Some(previous) = self.undo_history.pop() {
            self.redo_history
                .push(std::mem::replace(&mut self.source, previous));
            self.cursor = valid_cursor(&self.source, self.cursor);
            self.raw_selection = None;
            self.raw_selection_anchor = None;
            self.dirty = true;
        }
    }

    fn redo(&mut self) {
        if self.mode != Mode::Raw {
            return;
        }
        if let Some(next) = self.redo_history.pop() {
            self.undo_history
                .push(std::mem::replace(&mut self.source, next));
            self.cursor = valid_cursor(&self.source, self.cursor);
            self.raw_selection = None;
            self.raw_selection_anchor = None;
            self.dirty = true;
        }
    }

    fn record_edit(&mut self) {
        if self.undo_history.len() == HISTORY_LIMIT {
            self.undo_history.remove(0);
        }
        self.undo_history.push(self.source.clone());
        self.redo_history.clear();
    }

    fn move_raw_left(&mut self, select: bool) {
        if self.mode != Mode::Raw || self.cursor == 0 {
            return;
        }
        let cursor = self.source[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
        self.set_raw_cursor(cursor, select);
    }

    fn move_raw_right(&mut self, select: bool) {
        if self.mode != Mode::Raw || self.cursor >= self.source.len() {
            return;
        }
        let cursor = self.source[self.cursor..]
            .grapheme_indices(true)
            .nth(1)
            .map(|(index, _)| self.cursor + index)
            .unwrap_or(self.source.len());
        self.set_raw_cursor(cursor, select);
    }

    fn move_raw_vertically(&mut self, direction: isize, select: bool) {
        if self.mode != Mode::Raw {
            return;
        }
        let line_start = self.source[..self.cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line_end = self.source[self.cursor..]
            .find('\n')
            .map_or(self.source.len(), |index| self.cursor + index);
        let column = self.source[line_start..self.cursor].graphemes(true).count();
        let target = if direction < 0 {
            line_start.checked_sub(1).and_then(|previous_end| {
                self.source[..previous_end]
                    .rfind('\n')
                    .map_or(Some(0), |index| Some(index + 1))
            })
        } else if line_end < self.source.len() {
            Some(line_end + 1)
        } else {
            None
        };
        let Some(target_start) = target else {
            return;
        };
        let target_end = self.source[target_start..]
            .find('\n')
            .map_or(self.source.len(), |index| target_start + index);
        let cursor = self.source[target_start..target_end]
            .grapheme_indices(true)
            .nth(column)
            .map(|(index, _)| target_start + index)
            .unwrap_or(target_end);
        self.set_raw_cursor(cursor, select);
    }

    fn set_raw_cursor(&mut self, cursor: usize, select: bool) {
        let anchor = self.raw_selection_anchor.unwrap_or(self.cursor);
        self.cursor = cursor;
        if select {
            self.raw_selection_anchor = Some(anchor);
            self.raw_selection = (anchor != cursor).then(|| anchor.min(cursor)..anchor.max(cursor));
        } else {
            self.raw_selection_anchor = None;
            self.raw_selection = None;
        }
    }

    fn add_comment(&mut self, body: String) -> Result<()> {
        let range = self.selected_range()?;
        let block_kind = self
            .rendered
            .blocks
            .get(self.selected_block)
            .map(|block| format!("{:?}", block.kind).to_lowercase())
            .unwrap_or_else(|| "source_selection".into());
        let anchor = capture_anchor(&self.source, range, &self.document_fingerprint, block_kind)?;
        self.comments.push(Annotation::new(anchor, body));
        self.selected_comment = Some(self.comments.len() - 1);
        self.status = "Comment added".into();
        Ok(())
    }

    fn selected_range(&self) -> Result<Range<usize>> {
        if self.mode == Mode::Raw {
            return self
                .raw_selection
                .clone()
                .ok_or_else(|| anyhow!("Select source text before adding a raw-mode comment"));
        }
        self.rendered
            .blocks
            .get(self.selected_block)
            .map(|block| block.source_range.clone())
            .ok_or_else(|| anyhow!("There is no rendered block to annotate"))
    }

    fn comment_index(&self, id: Uuid) -> Result<usize> {
        self.comments
            .iter()
            .position(|comment| comment.id == id)
            .ok_or_else(|| anyhow!("Unknown comment {id}"))
    }

    fn comment_mut(&mut self, id: Uuid) -> Result<&mut Comment> {
        let index = self.comment_index(id)?;
        Ok(&mut self.comments[index])
    }

    fn selected_comment(&self) -> Result<&Comment> {
        self.selected_comment
            .and_then(|index| self.comments.get(index))
            .ok_or_else(|| anyhow!("There is no selected comment"))
    }

    fn selected_comment_mut(&mut self) -> Result<&mut Comment> {
        let index = self
            .selected_comment
            .ok_or_else(|| anyhow!("There is no selected comment"))?;
        self.comments
            .get_mut(index)
            .ok_or_else(|| anyhow!("There is no selected comment"))
    }

    fn move_comment_selection(&mut self, direction: isize) {
        if self.comments.is_empty() {
            self.selected_comment = None;
            return;
        }
        let current = self.selected_comment.unwrap_or(0) as isize;
        self.selected_comment =
            Some((current + direction).rem_euclid(self.comments.len() as isize) as usize);
    }
}

fn normalize_range(range: Range<usize>, source: &str) -> Result<Range<usize>> {
    if range.start > range.end
        || range.end > source.len()
        || !source.is_char_boundary(range.start)
        || !source.is_char_boundary(range.end)
    {
        return Err(anyhow!("The selection is outside the document"));
    }
    Ok(range)
}

fn valid_cursor(source: &str, requested: usize) -> usize {
    if requested >= source.len() {
        return source.len();
    }
    source
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index <= requested)
        .last()
        .unwrap_or(0)
}
