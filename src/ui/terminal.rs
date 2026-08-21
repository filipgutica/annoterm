use std::{sync::OnceLock, time::Duration};

use anyhow::{Result, anyhow};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    annotations::{AnchorState, Annotation, AnnotationStatus},
    app::{App, Command, Mode},
    markdown::{BlockKind, RenderBlock, RenderSpan, RenderStyle},
};

use super::{LayoutMode, layout};

struct MouseCapture {
    active: bool,
}

impl MouseCapture {
    fn enable() -> std::io::Result<Self> {
        crossterm::execute!(std::io::stdout(), EnableMouseCapture)?;
        Ok(Self { active: true })
    }

    fn disable(mut self) -> std::io::Result<()> {
        let result = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
        if result.is_ok() {
            self.active = false;
        }
        result
    }
}

impl Drop for MouseCapture {
    fn drop(&mut self) {
        if self.active {
            let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
        }
    }
}

/// Run the terminal interface. The callback is responsible for durable saves.
pub fn run<F, G>(app: &mut App, mut save: F, mut save_annotations: G) -> Result<()>
where
    F: FnMut(&str) -> Result<String>,
    G: FnMut(&[Annotation], &str, &str) -> Result<()>,
{
    let mouse_capture = MouseCapture::enable()?;
    let run_result = ratatui::run(|terminal| -> std::io::Result<()> {
        let mut quit_pending = false;
        loop {
            terminal.draw(|frame| draw(frame, app))?;
            if !event::poll(Duration::from_millis(250))? {
                continue;
            }
            let input = event::read()?;
            if let Event::Mouse(mouse) = input {
                let size = terminal.size()?;
                let area = Rect::new(0, 0, size.width, size.height);
                if !app.help_visible()
                    && let Err(error) = handle_mouse(app, mouse, area)
                {
                    app.status = format!("Action failed: {error}");
                }
                continue;
            }
            let Event::Key(key) = input else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let draft_was_open = app.comment_draft.is_some();
            if is_quit(key, app.mode)
                && !draft_was_open
                && !app.comments_focused()
                && !app.help_visible()
            {
                if !app.dirty || quit_pending {
                    break Ok(());
                }
                app.status = "Unsaved changes · press the quit key again to discard them".into();
                quit_pending = true;
                continue;
            }
            quit_pending = false;
            handle_key_and_persist(app, key, &mut save, &mut save_annotations);
        }
    });
    let disable_result = mouse_capture.disable();
    match (run_result, disable_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error.into()),
        (Ok(()), Err(error)) => Err(error.into()),
        (Err(run_error), Err(disable_error)) => Err(anyhow!(
            "{run_error}; additionally failed to disable mouse capture: {disable_error}"
        )),
    }
}

fn handle_key_and_persist<F, G>(
    app: &mut App,
    key: KeyEvent,
    save: &mut F,
    save_annotations: &mut G,
) where
    F: FnMut(&str) -> Result<String>,
    G: FnMut(&[Annotation], &str, &str) -> Result<()>,
{
    let comments_before = app.comments.clone();
    let transient_reanchor = app.dirty
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('r');
    let action_error = handle_key(app, key, save).err();
    let persistence_error = (!transient_reanchor && comments_before != app.comments)
        .then(|| save_annotations(&app.comments, app.document_fingerprint(), &app.source).err())
        .flatten();
    match (action_error, persistence_error) {
        (Some(action), Some(persistence)) => {
            app.status = format!(
                "Action failed: {action}; comment persistence or clipboard also failed: {persistence}"
            );
        }
        (Some(error), None) => app.status = format!("Action failed: {error}"),
        (None, Some(error)) => {
            app.status =
                format!("Comment changed in memory, but persistence or clipboard failed: {error}");
        }
        (None, None) => {}
    }
}

fn handle_mouse(app: &mut App, mouse: MouseEvent, terminal_area: Rect) -> Result<()> {
    if app.comment_draft.is_some() {
        return Ok(());
    }
    if app.comments_focused() {
        return match mouse.kind {
            MouseEventKind::ScrollDown => app.apply(Command::SelectNextComment),
            MouseEventKind::ScrollUp => app.apply(Command::SelectPreviousComment),
            _ => Ok(()),
        };
    }
    match (app.mode, mouse.kind) {
        (Mode::Rendered, MouseEventKind::Down(MouseButton::Left)) => {
            select_clicked_rendered_block(app, mouse, terminal_area);
            Ok(())
        }
        (Mode::Rendered, MouseEventKind::ScrollDown) => app.apply(Command::MoveNextBlock),
        (Mode::Rendered, MouseEventKind::ScrollUp) => app.apply(Command::MovePreviousBlock),
        (Mode::Raw, MouseEventKind::ScrollDown) => {
            app.apply(Command::MoveRawDown { select: false })
        }
        (Mode::Raw, MouseEventKind::ScrollUp) => app.apply(Command::MoveRawUp { select: false }),
        _ => Ok(()),
    }
}

fn select_clicked_rendered_block(app: &mut App, mouse: MouseEvent, terminal_area: Rect) {
    let Some(area) = rendered_document_area(app, terminal_area) else {
        return;
    };
    let right_border = area.x.saturating_add(area.width).saturating_sub(1);
    let bottom_border = area.y.saturating_add(area.height).saturating_sub(1);
    if mouse.column <= area.x
        || mouse.column >= right_border
        || mouse.row <= area.y
        || mouse.row >= bottom_border
    {
        return;
    }
    let scroll = document_scroll(app, area, None);
    let clicked_row = usize::from(scroll) + usize::from(mouse.row - area.y - 1);
    let width = area.width.saturating_sub(2).max(1);
    let mut block_start = 0usize;
    let clicked_block = app
        .rendered
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| {
            if index > 0 {
                block_start += 1;
            }
            let block_height = rendered_block_height(block, width);
            if (block_start..block_start.saturating_add(block_height)).contains(&clicked_row) {
                return Some(index);
            }
            block_start = block_start.saturating_add(block_height);
            None
        });
    if let Some(index) = clicked_block {
        app.select_rendered_block_from_click(index, scroll, area.width, area.height);
    }
}

fn rendered_document_area(app: &App, terminal_area: Rect) -> Option<Rect> {
    let (_, body, _) = terminal_rows(terminal_area, app.comment_draft.is_none());
    let document_column = match layout(terminal_area.width, terminal_area.height) {
        LayoutMode::TooSmall => return None,
        LayoutMode::DocumentOnly => body,
        LayoutMode::ThreePane => {
            let document_width = if app.comments.is_empty() { 82 } else { 72 };
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(document_width),
                    Constraint::Percentage(100 - document_width),
                ])
                .split(body)[0]
        }
    };
    Some(document_and_comment_areas(document_column, app.comment_draft.is_some()).0)
}

fn is_quit(key: KeyEvent, mode: Mode) -> bool {
    key.code == KeyCode::Esc
        || (key.code == KeyCode::Char('q')
            && (mode == Mode::Rendered || key.modifiers.contains(KeyModifiers::CONTROL)))
}

fn handle_key<F>(app: &mut App, key: KeyEvent, save: &mut F) -> Result<()>
where
    F: FnMut(&str) -> Result<String>,
{
    if app.help_visible() {
        return match key.code {
            KeyCode::Esc | KeyCode::F(1) => app.apply(Command::CloseHelp),
            KeyCode::Char('?') if app.comment_draft.is_none() && app.mode == Mode::Rendered => {
                app.apply(Command::CloseHelp)
            }
            _ => Ok(()),
        };
    }
    if app.comment_draft.is_some() {
        return match key.code {
            KeyCode::F(1) => app.apply(Command::ToggleHelp),
            KeyCode::Esc => app.apply(Command::CancelComment),
            KeyCode::Enter => {
                save_source_if_dirty(app, save)?;
                app.apply(Command::SubmitComment)
            }
            KeyCode::Backspace => app.apply(Command::DeleteCommentCharacter),
            KeyCode::Left if moves_by_word(key.modifiers) => {
                app.apply(Command::MoveCommentCursorWordLeft)
            }
            KeyCode::Right if moves_by_word(key.modifiers) => {
                app.apply(Command::MoveCommentCursorWordRight)
            }
            KeyCode::Left => app.apply(Command::MoveCommentCursorLeft),
            KeyCode::Right => app.apply(Command::MoveCommentCursorRight),
            KeyCode::Home => app.apply(Command::MoveCommentCursorStart),
            KeyCode::End => app.apply(Command::MoveCommentCursorEnd),
            KeyCode::Char(character) if accepts_text(key.modifiers) => {
                app.apply(Command::AppendCommentCharacter(character))
            }
            _ => Ok(()),
        };
    }
    if key.code == KeyCode::F(1)
        || (key.code == KeyCode::Char('?')
            && (app.mode == Mode::Rendered || app.comments_focused()))
    {
        return app.apply(Command::ToggleHelp);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('w') {
        return app.apply(Command::ToggleCommentFocus);
    }
    if app.comments_focused() {
        return handle_focused_comment_key(app, key, save);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
        return app.apply(Command::ToggleMode);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
        let fingerprint = save(&app.source)?;
        app.set_document_fingerprint(fingerprint);
        app.reanchor_comments();
        return app.apply(Command::SaveCompleted);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('z') {
        return app.apply(Command::Undo);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('y') {
        return app.apply(Command::Redo);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('k') {
        return begin_comment(app, save);
    }

    match key.code {
        KeyCode::Down | KeyCode::Tab if app.mode == Mode::Rendered => {
            app.apply(Command::MoveNextBlock)
        }
        KeyCode::Up | KeyCode::BackTab if app.mode == Mode::Rendered => {
            app.apply(Command::MovePreviousBlock)
        }
        KeyCode::Backspace => app.apply(Command::DeleteBackward),
        KeyCode::Enter if app.mode == Mode::Raw => app.apply(Command::Insert('\n')),
        KeyCode::Tab if app.mode == Mode::Raw => app.apply(Command::Insert('\t')),
        KeyCode::Left if app.mode == Mode::Raw => app.apply(Command::MoveRawLeft {
            select: key.modifiers.contains(KeyModifiers::SHIFT),
        }),
        KeyCode::Right if app.mode == Mode::Raw => app.apply(Command::MoveRawRight {
            select: key.modifiers.contains(KeyModifiers::SHIFT),
        }),
        KeyCode::Up if app.mode == Mode::Raw => app.apply(Command::MoveRawUp {
            select: key.modifiers.contains(KeyModifiers::SHIFT),
        }),
        KeyCode::Down if app.mode == Mode::Raw => app.apply(Command::MoveRawDown {
            select: key.modifiers.contains(KeyModifiers::SHIFT),
        }),
        KeyCode::Char('a') if app.mode == Mode::Rendered => begin_comment(app, save),
        KeyCode::Char('x') if app.mode == Mode::Rendered => {
            save_source_if_dirty(app, save)?;
            app.apply(Command::ToggleSelectedCommentResolution)
        }
        KeyCode::Char('d') if app.mode == Mode::Rendered => {
            save_source_if_dirty(app, save)?;
            app.apply(Command::DeleteSelectedComment)
        }
        KeyCode::Char('o') if app.mode == Mode::Rendered => {
            save_source_if_dirty(app, save)?;
            app.apply(Command::RepairSelectedOrphan)
        }
        KeyCode::Char('e') if app.mode == Mode::Rendered => {
            save_source_if_dirty(app, save)?;
            app.apply(Command::BeginEditSelectedComment)
        }
        KeyCode::Char('j') if app.mode == Mode::Rendered => {
            app.apply(Command::JumpToSelectedComment)
        }
        KeyCode::Char('[') if app.mode == Mode::Rendered => {
            app.apply(Command::SelectPreviousComment)
        }
        KeyCode::Char(']') if app.mode == Mode::Rendered => app.apply(Command::SelectNextComment),
        KeyCode::Char(character) if app.mode == Mode::Raw && accepts_text(key.modifiers) => {
            app.apply(Command::Insert(character))
        }
        _ => Ok(()),
    }
}

fn handle_focused_comment_key<F>(app: &mut App, key: KeyEvent, save: &mut F) -> Result<()>
where
    F: FnMut(&str) -> Result<String>,
{
    match key.code {
        KeyCode::Esc => app.apply(Command::FocusDocument),
        KeyCode::Down | KeyCode::Char(']') => app.apply(Command::SelectNextComment),
        KeyCode::Up | KeyCode::Char('k' | '[') => app.apply(Command::SelectPreviousComment),
        KeyCode::Char('j') => app.apply(Command::JumpToSelectedComment),
        KeyCode::Enter | KeyCode::Char('e') => {
            save_source_if_dirty(app, save)?;
            app.apply(Command::BeginEditSelectedComment)
        }
        KeyCode::Char('x') => {
            save_source_if_dirty(app, save)?;
            app.apply(Command::ToggleSelectedCommentResolution)
        }
        KeyCode::Char('d') => {
            save_source_if_dirty(app, save)?;
            app.apply(Command::DeleteSelectedComment)
        }
        KeyCode::Char('o') => {
            save_source_if_dirty(app, save)?;
            app.apply(Command::RepairSelectedOrphan)
        }
        _ => Ok(()),
    }
}

fn accepts_text(modifiers: KeyModifiers) -> bool {
    modifiers.is_empty() || modifiers == KeyModifiers::SHIFT
}

fn moves_by_word(modifiers: KeyModifiers) -> bool {
    modifiers.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL)
}

fn begin_comment<F>(app: &mut App, save: &mut F) -> Result<()>
where
    F: FnMut(&str) -> Result<String>,
{
    save_source_if_dirty(app, save)?;
    app.apply(Command::BeginComment)
}

fn save_source_if_dirty<F>(app: &mut App, save: &mut F) -> Result<()>
where
    F: FnMut(&str) -> Result<String>,
{
    if app.dirty {
        let fingerprint = save(&app.source)?;
        app.set_document_fingerprint(fingerprint);
        app.reanchor_comments();
        app.apply(Command::SaveCompleted)?;
    }
    Ok(())
}

fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    match layout(area.width, area.height) {
        LayoutMode::TooSmall => {
            frame.render_widget(
                Paragraph::new("Terminal too small. Resize to at least 30×6."),
                area,
            );
        }
        LayoutMode::DocumentOnly => draw_document_only(frame, app),
        LayoutMode::ThreePane => draw_three_pane(frame, app),
    }
    if app.help_visible() && layout(area.width, area.height) != LayoutMode::TooSmall {
        draw_help(frame, app);
    }
}

fn draw_document_only(frame: &mut Frame, app: &mut App) {
    let (status_area, body, shortcut_area) =
        terminal_rows(frame.area(), app.comment_draft.is_none());
    let (document_area, comment_area) =
        document_and_comment_areas(body, app.comment_draft.is_some());
    frame.render_widget(Paragraph::new(safe_display(&app.status)), status_area);
    draw_document_pane(frame, app, document_area);
    if let Some(area) = comment_area {
        draw_comment_draft(frame, app, area);
    }
    if let Some(area) = shortcut_area {
        draw_shortcut_bar(frame, app, area);
    }
}

fn draw_three_pane(frame: &mut Frame, app: &mut App) {
    let (status_area, body, shortcut_area) =
        terminal_rows(frame.area(), app.comment_draft.is_none());
    let document_width = if app.comments.is_empty() { 82 } else { 72 };
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(document_width),
            Constraint::Percentage(100 - document_width),
        ])
        .split(body);
    let (document_area, comment_area) =
        document_and_comment_areas(horizontal[0], app.comment_draft.is_some());
    frame.render_widget(Paragraph::new(safe_display(&app.status)), status_area);
    draw_document_pane(frame, app, document_area);
    frame.render_widget(comments_widget(app, horizontal[1]), horizontal[1]);
    if let Some(area) = comment_area {
        draw_comment_draft(frame, app, area);
    }
    if let Some(area) = shortcut_area {
        draw_shortcut_bar(frame, app, area);
    }
}

fn terminal_rows(area: Rect, show_shortcuts: bool) -> (Rect, Rect, Option<Rect>) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(u16::from(show_shortcuts)),
        ])
        .split(area);
    (rows[0], rows[1], show_shortcuts.then_some(rows[2]))
}

fn document_and_comment_areas(area: Rect, comment_open: bool) -> (Rect, Option<Rect>) {
    if !comment_open {
        return (area, None);
    }
    let comment_height = area.height.saturating_sub(3).clamp(2, 5);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(comment_height)])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

fn draw_document_pane(frame: &mut Frame, app: &mut App, area: Rect) {
    let raw_cursor = (app.mode == Mode::Raw)
        .then(|| raw_cursor_position(app, area.width.saturating_sub(2).max(1)));
    let scroll = document_scroll(app, area, raw_cursor);
    if app.mode == Mode::Rendered {
        app.remember_rendered_scroll(scroll, area.width, area.height);
    }
    frame.render_widget(document_widget(app, scroll, area.width), area);
    if !app.help_visible() {
        place_raw_cursor(frame, app, area, raw_cursor, scroll);
    }
}

fn draw_comment_draft(frame: &mut Frame, app: &App, area: Rect) {
    let Some(draft) = &app.comment_draft else {
        return;
    };
    frame.render_widget(Clear, area);
    if area.height < 3 {
        let prefix = "Comment: ";
        let display = format!("{prefix}{}", compact_comment_text(draft));
        let before = format!(
            "{prefix}{}",
            compact_comment_text(&draft[..app.comment_cursor()])
        );
        let cursor_column = unicode_width::UnicodeWidthStr::width(before.as_str());
        let scroll = cursor_column
            .saturating_add(1)
            .saturating_sub(usize::from(area.width));
        frame.render_widget(
            Paragraph::new(display).scroll((0, scroll.try_into().unwrap_or(u16::MAX))),
            Rect::new(area.x, area.y, area.width, 1),
        );
        frame.render_widget(
            Paragraph::new(comment_editor_controls(area.width)),
            Rect::new(area.x, area.y + 1, area.width, 1),
        );
        frame.set_cursor_position((
            area.x
                + cursor_column
                    .saturating_sub(scroll)
                    .try_into()
                    .unwrap_or(u16::MAX),
            area.y,
        ));
        return;
    }
    let inner_width = area.width.saturating_sub(2).max(1);
    let inner_height = area.height.saturating_sub(2).max(1);
    let (cursor_row, cursor_column) =
        text_cursor_position(draft, app.comment_cursor(), inner_width);
    let scroll = cursor_row.saturating_add(1).saturating_sub(inner_height);
    frame.render_widget(
        Paragraph::new(safe_display(draft))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Comment")
                    .title_bottom(comment_editor_controls(area.width)),
            )
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
    frame.set_cursor_position((
        area.x + 1 + cursor_column.min(inner_width.saturating_sub(1)),
        area.y + 1 + cursor_row.saturating_sub(scroll),
    ));
}

fn compact_comment_text(value: &str) -> String {
    safe_display(value).replace('\n', "↵")
}

fn comment_editor_controls(width: u16) -> &'static str {
    if width >= 76 {
        "←/→ move · Option/Ctrl+←/→ words · Enter save · Esc cancel · F1 help"
    } else if width >= 38 {
        "←/→ move · Enter save · Esc cancel"
    } else {
        "←/→ · Enter · Esc · F1"
    }
}

fn place_raw_cursor(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    raw_cursor: Option<(u16, u16)>,
    scroll: u16,
) {
    if app.mode != Mode::Raw
        || app.comments_focused()
        || app.comment_draft.is_some()
        || area.width < 3
        || area.height < 3
    {
        return;
    }
    let inner_width = area.width.saturating_sub(2).max(1);
    let Some((line, column)) = raw_cursor else {
        return;
    };
    let y = area.y
        + 1
        + line
            .saturating_sub(scroll)
            .min(area.height.saturating_sub(2));
    frame.set_cursor_position((area.x + 1 + column.min(inner_width.saturating_sub(1)), y));
}

fn raw_cursor_position(app: &App, width: u16) -> (u16, u16) {
    text_cursor_position(&app.source, app.cursor, width)
}

fn text_cursor_position(text: &str, cursor: usize, width: u16) -> (u16, u16) {
    let line_start = text[..cursor].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[cursor..]
        .find('\n')
        .map_or(text.len(), |index| cursor + index);
    let rows_before = if line_start == 0 {
        0
    } else {
        Paragraph::new(safe_display(&text[..line_start]))
            .wrap(Wrap { trim: false })
            .line_count(width)
    };
    let before = safe_display(&text[line_start..cursor]);
    let after = &text[cursor..line_end];
    let (target, remainder) = if let Some(grapheme) = after.graphemes(true).next() {
        (
            safe_display(grapheme),
            safe_display(&after[grapheme.len()..]),
        )
    } else {
        (" ".into(), String::new())
    };
    let probe_style = Style::default().add_modifier(Modifier::RAPID_BLINK);
    let estimated_row = Paragraph::new(Line::from(vec![
        Span::raw(before.as_str()),
        Span::styled(target.as_str(), probe_style),
    ]))
    .wrap(Wrap { trim: false })
    .line_count(width)
    .saturating_sub(1);
    let window_start = estimated_row.saturating_sub(1).min(usize::from(u16::MAX));
    let area = Rect::new(0, 0, width, 4);
    let mut buffer = Buffer::empty(area);
    Paragraph::new(Line::from(vec![
        Span::raw(before.as_str()),
        Span::styled(target.as_str(), probe_style),
        Span::raw(remainder.as_str()),
    ]))
    .wrap(Wrap { trim: false })
    .scroll((window_start as u16, 0))
    .render(area, &mut buffer);
    for y in 0..area.height {
        for x in 0..width {
            if buffer[(x, y)].modifier.contains(Modifier::RAPID_BLINK) {
                return (
                    rows_before
                        .saturating_add(window_start)
                        .saturating_add(usize::from(y))
                        .try_into()
                        .unwrap_or(u16::MAX),
                    x,
                );
            }
        }
    }
    (
        rows_before
            .saturating_add(estimated_row)
            .try_into()
            .unwrap_or(u16::MAX),
        (unicode_width::UnicodeWidthStr::width(before.as_str()) % usize::from(width)) as u16,
    )
}

fn document_widget(app: &App, scroll: u16, width: u16) -> Paragraph<'static> {
    let title = match app.mode {
        Mode::Rendered => app
            .rendered
            .blocks
            .get(app.selected_block)
            .map(|block| {
                format!(
                    "Rendered · selected block: lines {}–{}",
                    block.line_range.start(),
                    block.line_range.end()
                )
            })
            .unwrap_or_else(|| "Rendered · no selectable blocks".into()),
        Mode::Raw => "Raw".into(),
    };
    let text = match app.mode {
        Mode::Rendered => rendered_text_at_width(app, width.saturating_sub(2).max(1)),
        Mode::Raw => raw_text(app),
    };
    Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
}

fn raw_text(app: &App) -> Text<'static> {
    let Some(selection) = &app.raw_selection else {
        return Text::raw(safe_display(&app.source));
    };
    let mut lines = Vec::new();
    let mut line_start = 0;
    for source_line in app.source.split_inclusive('\n') {
        let content = source_line.strip_suffix('\n').unwrap_or(source_line);
        let line_end = line_start + content.len();
        let selected_start = selection.start.clamp(line_start, line_end);
        let selected_end = selection.end.clamp(line_start, line_end);
        let mut spans = Vec::new();
        if line_start < selected_start {
            spans.push(Span::raw(safe_display(
                &app.source[line_start..selected_start],
            )));
        }
        if selected_start < selected_end {
            spans.push(Span::styled(
                safe_display(&app.source[selected_start..selected_end]),
                selection_style(),
            ));
        }
        if selected_end < line_end {
            spans.push(Span::raw(safe_display(&app.source[selected_end..line_end])));
        }
        lines.push(Line::from(spans));
        line_start = line_end + usize::from(source_line.ends_with('\n'));
    }
    if app.source.is_empty() || app.source.ends_with('\n') {
        lines.push(Line::default());
    }
    Text::from(lines)
}

fn document_scroll(app: &App, area: Rect, raw_cursor: Option<(u16, u16)>) -> u16 {
    let viewport_height = usize::from(area.height.saturating_sub(2)).max(1);
    let scroll = match app.mode {
        Mode::Rendered => {
            if let Some(scroll) = app.preserved_rendered_scroll(area.width, area.height) {
                return scroll;
            }
            let width = area.width.saturating_sub(2).max(1);
            let selected_start = app
                .rendered
                .blocks
                .iter()
                .take(app.selected_block)
                .map(|block| rendered_block_height(block, width).saturating_add(1))
                .sum::<usize>();
            let selected_height = app
                .rendered
                .blocks
                .get(app.selected_block)
                .map(|block| rendered_block_height(block, width))
                .unwrap_or(0);
            if let Some(scroll) = app.rendered_scroll_override(area.width, area.height) {
                let scroll = usize::from(scroll);
                if selected_start < scroll {
                    selected_start
                } else if selected_height >= viewport_height
                    && selected_start >= scroll.saturating_add(viewport_height)
                {
                    selected_start
                        .saturating_add(1)
                        .saturating_sub(viewport_height)
                } else if selected_start.saturating_add(selected_height)
                    > scroll.saturating_add(viewport_height)
                    && selected_height < viewport_height
                {
                    selected_start
                        .saturating_add(selected_height)
                        .saturating_sub(viewport_height)
                } else {
                    scroll
                }
            } else if selected_height >= viewport_height {
                selected_start
            } else {
                selected_start
                    .saturating_add(selected_height)
                    .saturating_sub(viewport_height)
            }
        }
        Mode::Raw => usize::from(raw_cursor.map(|position| position.0).unwrap_or(0))
            .saturating_add(1)
            .saturating_sub(viewport_height),
    };
    scroll.try_into().unwrap_or(u16::MAX)
}

fn rendered_block_height(block: &RenderBlock, width: u16) -> usize {
    Paragraph::new(Text::from(render_block_lines(block, "  ", false, width)))
        .wrap(Wrap { trim: false })
        .line_count(width)
        .max(1)
}

fn comments_widget(app: &App, area: Rect) -> Paragraph<'static> {
    let inner_width = area.width.saturating_sub(2).max(1);
    let (text, scroll) = if app.comments.is_empty() {
        (Text::raw("No comments yet"), 0)
    } else {
        let mut lines = Vec::new();
        let mut entry_heights = Vec::with_capacity(app.comments.len());
        for (index, comment) in app.comments.iter().enumerate() {
            let state = match (comment.status, comment.anchor_state) {
                (AnnotationStatus::Resolved, _) => "Resolved (not in feedback)",
                (AnnotationStatus::Open, AnchorState::Outdated) => {
                    "Open (outdated, approximate, in feedback)"
                }
                (AnnotationStatus::Open, AnchorState::Orphaned) => "Open (detached, in feedback)",
                (AnnotationStatus::Open, AnchorState::Anchored) => "Open (in feedback)",
            };
            let selected = app.selected_comment == Some(index);
            let marker = if selected { "›" } else { " " };
            let preview = safe_display(&comment.comment)
                .split('\n')
                .collect::<Vec<_>>()
                .join(" ↵ ");
            let style = if selected {
                selection_style()
            } else {
                Style::default()
            };
            let entry = vec![
                Line::from(Span::styled(
                    format!("{marker} {}/{} · {state}", index + 1, app.comments.len()),
                    style,
                )),
                Line::from(Span::styled(format!("  {preview}"), style)),
                Line::default(),
            ];
            entry_heights.push(
                Paragraph::new(Text::from(entry.clone()))
                    .wrap(Wrap { trim: false })
                    .line_count(inner_width)
                    .max(1),
            );
            lines.extend(entry);
        }
        let viewport_height = usize::from(area.height.saturating_sub(2)).max(1);
        let scroll = app
            .selected_comment
            .and_then(|index| {
                let selected_start = entry_heights.get(..index)?.iter().sum::<usize>();
                let selected_height = *entry_heights.get(index)?;
                Some(if selected_height >= viewport_height {
                    selected_start
                } else {
                    selected_start
                        .saturating_add(selected_height)
                        .saturating_sub(viewport_height)
                })
            })
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u16::MAX);
        (Text::from(lines), scroll)
    };
    let title = if app.comments_focused() {
        format!("Comments ({}) · Focused", app.comments.len())
    } else {
        format!("Comments ({})", app.comments.len())
    };
    let mut block = Block::default().borders(Borders::ALL).title(title);
    if app.comments_focused() {
        block = block.border_style(focus_style(colors_enabled()));
    }
    Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
}

fn focus_style(colors: bool) -> Style {
    if colors {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

fn draw_shortcut_bar(frame: &mut Frame, app: &App, area: Rect) {
    frame.render_widget(
        Paragraph::new(shortcut_bar_text(app, area.width))
            .style(Style::default().add_modifier(Modifier::REVERSED)),
        area,
    );
}

fn shortcut_bar_text(app: &App, width: u16) -> String {
    let candidates = if app.comments_focused() {
        let resolution = app
            .selected_comment
            .and_then(|index| app.comments.get(index))
            .map_or("Resolve", |comment| match comment.status {
                AnnotationStatus::Open => "Resolve",
                AnnotationStatus::Resolved => "Reopen",
            });
        vec![
            format!(
                "↑/↓ or [/] Select · j Jump · x {resolution} · e Edit · d Delete · Esc Document · ? Help"
            ),
            format!("↑/↓ Select · j Jump · x {resolution} · e Edit · Esc Document · ? Help"),
            format!("↑/↓ · j Jump · x {resolution} · Esc · ? Help"),
            format!("↑/↓ · j · x · Esc · ? Help"),
        ]
    } else if app.mode == Mode::Raw {
        vec![
            "Arrows/wheel Move · Shift+arrows Select · Ctrl+S Save · Ctrl+K Comment · Ctrl+W Comments · Ctrl+R Render · F1 Help".into(),
            "Arrows Move · Shift+arrows Select · Ctrl+S Save · Ctrl+K Comment · F1 Help".into(),
            "Arrows · Shift+↑/↓ · Ctrl+S · Ctrl+K · F1 Help".into(),
            "Arrows · Shift+↑↓ · ^S · F1".into(),
        ]
    } else if app.comments.is_empty() {
        vec![
            "↑/↓/wheel Move · a Comment · Ctrl+R Raw · ? Help".into(),
            "↑/↓ Move · a Comment · ? Help".into(),
            "↑/↓ · a Comment · ? Help".into(),
        ]
    } else {
        vec![
            "↑/↓/wheel Move · a Comment · [/] Comments · j Jump · Ctrl+R Raw · ? Help".into(),
            "↑/↓ Move · a Comment · [/] Comments · j Jump · ? Help".into(),
            "↑/↓ · a Comment · [/] · j Jump · ? Help".into(),
            "↑/↓ · a · [/] · j · ? Help".into(),
        ]
    };
    candidates
        .into_iter()
        .find(|candidate| {
            unicode_width::UnicodeWidthStr::width(candidate.as_str()) <= usize::from(width)
        })
        .unwrap_or_else(|| "? Help".into())
}

fn draw_help(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < 76 || area.height < 18 {
        let close_key = if app.comment_draft.is_some() || app.mode == Mode::Raw {
            "Esc/F1"
        } else {
            "Esc/?"
        };
        let context = if app.comments_focused() {
            "↑/↓ · j Jump · x Resolve"
        } else if app.comment_draft.is_some() {
            "←/→ · Enter · Esc Cancel"
        } else if app.mode == Mode::Raw {
            "Arrows · Shift+↑/↓ Select"
        } else {
            "↑/↓ · a Comment · j Jump"
        };
        let text = Text::from(vec![
            Line::from("Compact help"),
            Line::from(context),
            Line::from("Resize to 76×18 for all."),
            Line::from(format!("{close_key} closes help.")),
        ]);
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("Help"))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let width = area.width.saturating_sub(4).clamp(1, 76);
    let height = area.height.saturating_sub(2).clamp(1, 18);
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let close_key = if app.comment_draft.is_some() || app.mode == Mode::Raw {
        "F1"
    } else {
        "?"
    };
    let text = Text::from(vec![
        Line::from(vec![
            Span::styled("General", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("                           "),
            Span::styled("Comments", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from("? Help (F1 in raw)               ↑/↓ or [/] Select"),
        Line::from("Esc Close / quit                  j Jump"),
        Line::from("Ctrl+R Rendered / raw              e or Enter Edit"),
        Line::from("Ctrl+W Focus comments              x Resolve / reopen"),
        Line::from("                                   d Delete · o Repair"),
        Line::from(vec![
            Span::styled(
                "Rendered document",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("                 "),
            Span::styled("Raw editor", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from("↑/↓, Tab, wheel Move               Arrows Move cursor"),
        Line::from("a Comment block                    Shift+arrows Select"),
        Line::from("[/] Select comment                 Ctrl+S Save"),
        Line::from("j Jump to comment                  Ctrl+K Comment selection"),
        Line::from("                                   Ctrl+Z/Y Undo / redo"),
        Line::from(Span::styled(
            "Comment editor",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("←/→ Move · Option/Ctrl+←/→ Words · Enter Save · Esc Cancel"),
    ]);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Keyboard shortcuts · Esc/{close_key} closes")),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

#[cfg(test)]
fn rendered_text(app: &App) -> Text<'static> {
    rendered_text_at_width(app, u16::MAX)
}

fn rendered_text_at_width(app: &App, width: u16) -> Text<'static> {
    let mut output = Vec::new();
    for (index, block) in app.rendered.blocks.iter().enumerate() {
        if index > 0 {
            output.push(Line::default());
        }
        let anchored = app.comments.iter().any(|comment| {
            comment.anchor_state == AnchorState::Anchored
                && comment
                    .current_range(app.document_fingerprint())
                    .is_some_and(|range| {
                        range.start.byte < block.source_range.end
                            && range.end.byte > block.source_range.start
                    })
        });
        let outdated = app.comments.iter().any(|comment| {
            comment.anchor_state == AnchorState::Outdated
                && comment
                    .current_range(app.document_fingerprint())
                    .is_some_and(|range| {
                        range.start.byte < block.source_range.end
                            && range.end.byte > block.source_range.start
                    })
        });
        let marker = if anchored {
            "● "
        } else if outdated {
            "◌ "
        } else if index == app.selected_block {
            "› "
        } else {
            "  "
        };
        output.extend(render_block_lines(
            block,
            marker,
            index == app.selected_block,
            width,
        ));
    }
    Text::from(output)
}

fn render_block_lines(
    block: &RenderBlock,
    marker: &'static str,
    selected: bool,
    width: u16,
) -> Vec<Line<'static>> {
    if block.kind == BlockKind::Table {
        let table = table_cells(block);
        if !table.is_empty() {
            return render_table_block_lines(&table, marker, selected, width);
        }
    }
    let colors = colors_enabled();
    let highlighted = block.language.as_deref().map(|language| {
        highlight_code(
            language,
            block
                .lines
                .iter()
                .skip(1)
                .take(block.lines.len().saturating_sub(1)),
        )
    });

    let mut lines = block
        .lines
        .iter()
        .enumerate()
        .map(|(line_index, line)| {
            let mut spans = vec![Span::styled(
                if line_index == 0 { marker } else { "  " },
                marker_style(colors),
            )];
            if block.kind == BlockKind::Code
                && line_index > 0
                && let Some(highlighted) = &highlighted
            {
                spans.extend(highlighted[line_index - 1].clone());
            } else {
                let block_style = if block.kind == BlockKind::Code && line_index > 0 {
                    Style::default()
                } else {
                    block_style(block.kind, block.heading_level, colors)
                };
                if let Some(styled_line) = block.styled_lines.get(line_index) {
                    spans.extend(styled_line.iter().map(|span| {
                        Span::styled(
                            span.text.clone(),
                            block_style.patch(inline_style(span.style, colors)),
                        )
                    }));
                } else {
                    spans.push(Span::styled(line.clone(), block_style));
                }
            }
            if selected {
                let style = selection_style();
                for span in &mut spans {
                    span.style = span.style.patch(style);
                }
            }
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    if let Some(rule) = heading_rule(block) {
        let style = block_style(block.kind, block.heading_level, colors);
        let mut spans = vec![Span::raw("  "), Span::styled(rule, style)];
        if selected {
            let selection = selection_style();
            for span in &mut spans {
                span.style = span.style.patch(selection);
            }
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn render_table_block_lines(
    table: &TableCells,
    marker: &'static str,
    selected: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let colors = colors_enabled();
    responsive_table_lines(table, width.saturating_sub(2))
        .into_iter()
        .enumerate()
        .map(|(line_index, line)| {
            let mut spans = vec![Span::styled(
                if line_index == 0 { marker } else { "  " },
                marker_style(colors),
            )];
            spans.extend(
                line.into_iter()
                    .map(|span| Span::styled(span.text, inline_style(span.style, colors))),
            );
            if selected {
                let selection = selection_style();
                for span in &mut spans {
                    span.style = span.style.patch(selection);
                }
            }
            Line::from(spans)
        })
        .collect()
}

fn responsive_table_lines(table: &TableCells, available_width: u16) -> Vec<Vec<RenderSpan>> {
    let column_count = table.iter().map(Vec::len).max().unwrap_or(0);
    if column_count == 0 {
        return Vec::new();
    }
    let widths = (0..column_count)
        .map(|column| {
            table
                .iter()
                .filter_map(|row| row.get(column))
                .map(|cell| unicode_width::UnicodeWidthStr::width(table_cell_text(cell).as_str()))
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let natural_width = widths.iter().sum::<usize>() + column_count.saturating_sub(1) * 3;
    if natural_width <= usize::from(available_width) {
        return aligned_table_lines(table, &widths, column_count);
    }
    if table.len() == 1 {
        return table[0].to_vec();
    }
    stacked_table_lines(table, column_count)
}

fn aligned_table_lines(
    table: &TableCells,
    widths: &[usize],
    column_count: usize,
) -> Vec<Vec<RenderSpan>> {
    table
        .iter()
        .map(|row| {
            let mut output = Vec::new();
            for (column, column_width) in widths.iter().enumerate().take(column_count) {
                let cell = row.get(column).map(Vec::as_slice).unwrap_or_default();
                let cell_width =
                    unicode_width::UnicodeWidthStr::width(table_cell_text(cell).as_str());
                output.extend(cell.iter().cloned());
                output.push(RenderSpan {
                    text: " ".repeat(column_width.saturating_sub(cell_width)),
                    style: RenderStyle::default(),
                });
                if column + 1 < column_count {
                    output.push(RenderSpan {
                        text: " │ ".into(),
                        style: RenderStyle {
                            dim: true,
                            ..RenderStyle::default()
                        },
                    });
                }
            }
            output
        })
        .collect()
}

fn stacked_table_lines(table: &TableCells, column_count: usize) -> Vec<Vec<RenderSpan>> {
    let Some(headers) = table.first() else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for (row_index, row) in table.iter().enumerate().skip(1) {
        if row_index > 1 {
            output.push(Vec::new());
        }
        for column in 0..column_count {
            let mut line = headers.get(column).cloned().unwrap_or_else(|| {
                vec![RenderSpan {
                    text: format!("Column {}", column + 1),
                    style: RenderStyle::default(),
                }]
            });
            for span in &mut line {
                span.style.bold = true;
            }
            line.push(RenderSpan {
                text: ": ".into(),
                style: RenderStyle {
                    bold: true,
                    ..RenderStyle::default()
                },
            });
            output.push(line);
            if let Some(cell) = row.get(column) {
                output
                    .last_mut()
                    .expect("stacked table line was just added")
                    .extend(cell.iter().cloned());
            }
        }
    }
    output
}

type TableCells = Vec<Vec<Vec<RenderSpan>>>;

fn table_cells(block: &RenderBlock) -> TableCells {
    block
        .styled_lines
        .iter()
        .map(|line| {
            let mut cells = vec![Vec::new()];
            for span in line {
                if span.is_table_cell_boundary() {
                    trim_table_padding(cells.last_mut().expect("table row has a cell"));
                    cells.push(Vec::new());
                } else {
                    cells
                        .last_mut()
                        .expect("table row has a cell")
                        .push(span.clone());
                }
            }
            cells
        })
        .collect()
}

fn trim_table_padding(cell: &mut Vec<RenderSpan>) {
    while cell.last().is_some_and(|span| {
        span.style == RenderStyle::default() && span.text.chars().all(|character| character == ' ')
    }) {
        cell.pop();
    }
}

fn table_cell_text(cell: &[RenderSpan]) -> String {
    cell.iter().map(|span| span.text.as_str()).collect()
}

fn heading_rule(block: &RenderBlock) -> Option<String> {
    let glyph = match block.heading_level? {
        1 => '━',
        2 => '─',
        _ => return None,
    };
    let width = block
        .lines
        .first()
        .map(|line| unicode_width::UnicodeWidthStr::width(line.as_str()))
        .unwrap_or(0)
        .clamp(4, 48);
    Some(glyph.to_string().repeat(width))
}

fn colors_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}

fn marker_style(colors: bool) -> Style {
    if colors {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

fn selection_style() -> Style {
    if colors_enabled() {
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::REVERSED)
    }
}

fn block_style(kind: BlockKind, heading_level: Option<u8>, colors: bool) -> Style {
    match kind {
        BlockKind::Heading => match heading_level.unwrap_or(6) {
            1 => Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            2 => Style::default().add_modifier(Modifier::BOLD),
            3 => Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
            4 => Style::default().add_modifier(Modifier::BOLD),
            5 => Style::default().add_modifier(Modifier::ITALIC),
            _ => with_color(Style::default(), Color::DarkGray, colors)
                .add_modifier(Modifier::ITALIC | Modifier::DIM),
        },
        BlockKind::Blockquote => {
            with_color(Style::default(), Color::Gray, colors).add_modifier(Modifier::ITALIC)
        }
        BlockKind::Table => Style::default(),
        BlockKind::Footnote | BlockKind::Definition | BlockKind::ThematicBreak => {
            with_color(Style::default(), Color::DarkGray, colors).add_modifier(Modifier::DIM)
        }
        BlockKind::Html => with_color(Style::default(), Color::Yellow, colors),
        BlockKind::Math => {
            with_color(Style::default(), Color::Magenta, colors).add_modifier(Modifier::ITALIC)
        }
        BlockKind::Code => {
            with_color(Style::default(), Color::DarkGray, colors).add_modifier(Modifier::DIM)
        }
        BlockKind::Paragraph | BlockKind::List => Style::default(),
    }
}

fn inline_style(style: RenderStyle, colors: bool) -> Style {
    let mut output = Style::default();
    if style.bold {
        output = output.add_modifier(Modifier::BOLD);
    }
    if style.italic {
        output = output.add_modifier(Modifier::ITALIC);
    }
    if style.crossed_out {
        output = output.add_modifier(Modifier::CROSSED_OUT);
    }
    if style.link {
        output = output.add_modifier(Modifier::UNDERLINED);
    }
    if style.code {
        output = if colors {
            with_color(output, Color::Yellow, true).add_modifier(Modifier::BOLD)
        } else {
            output.add_modifier(Modifier::REVERSED)
        };
    }
    if style.dim {
        output = output.add_modifier(Modifier::DIM);
    }
    output
}

fn with_color(style: Style, color: Color, enabled: bool) -> Style {
    if enabled { style.fg(color) } else { style }
}

fn highlight_code<'a>(
    language: &str,
    lines: impl Iterator<Item = &'a String>,
) -> Vec<Vec<Span<'static>>> {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    static THEMES: OnceLock<ThemeSet> = OnceLock::new();

    if std::env::var_os("NO_COLOR").is_some() {
        return lines.map(|line| vec![Span::raw(line.to_owned())]).collect();
    }

    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines);
    let themes = THEMES.get_or_init(ThemeSet::load_defaults);
    let syntax = syntaxes
        .find_syntax_by_token(language)
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let theme = &themes.themes["base16-ocean.dark"];
    let mut highlighter = HighlightLines::new(syntax, theme);

    lines
        .map(|line| {
            highlighter
                .highlight_line(&format!("{line}\n"), syntaxes)
                .unwrap_or_else(|_| vec![(SyntectStyle::default(), line)])
                .into_iter()
                .map(|(style, text)| {
                    Span::styled(
                        text.trim_end_matches('\n').to_owned(),
                        terminal_style(style),
                    )
                })
                .collect()
        })
        .collect()
}

fn terminal_style(style: SyntectStyle) -> Style {
    let mut output = Style::default().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));
    if style.font_style.contains(FontStyle::BOLD) {
        output = output.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        output = output.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        output = output.add_modifier(Modifier::UNDERLINED);
    }
    output
}

fn safe_display(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\t' => "    ".chars().collect::<Vec<_>>(),
            '\u{1b}' => vec!['␛'],
            character if character.is_control() && character != '\n' => vec!['�'],
            character => vec![character],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        block_style, comment_editor_controls, comments_widget, document_scroll, document_widget,
        draw, focus_style, handle_key, handle_key_and_persist, handle_mouse, highlight_code,
        inline_style, marker_style, raw_cursor_position, raw_text, rendered_block_height,
        rendered_document_area, rendered_text, rendered_text_at_width, safe_display,
        shortcut_bar_text,
    };

    #[test]
    fn global_shortcut_bar_uses_the_active_context_without_cutting_off() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("Review this".into()))
            .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let document_bar = (0..120)
            .map(|x| terminal.backend().buffer()[(x, 23)].symbol())
            .collect::<String>();
        assert!(document_bar.contains("a Comment"));
        assert!(document_bar.contains("[/] Comments"));
        assert!(document_bar.contains("? Help"));

        app.apply(Command::ToggleCommentFocus).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let comment_bar = (0..120)
            .map(|x| terminal.backend().buffer()[(x, 23)].symbol())
            .collect::<String>();
        assert!(comment_bar.contains("j Jump"));
        assert!(comment_bar.contains("x Resolve"));
        assert!(comment_bar.contains("Esc Document"));
        assert!(comment_bar.contains("? Help"));

        for width in 30..80 {
            let controls = shortcut_bar_text(&app, width);
            assert!(unicode_width::UnicodeWidthStr::width(controls.as_str()) <= usize::from(width));
        }
    }

    #[test]
    fn question_mark_opens_a_complete_help_overlay_and_escape_closes_it() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        let mut save = |_: &str| Ok("sha256:saved".into());

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        assert!(app.help_visible());

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let visible = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(visible.contains("Keyboard shortcuts"));
        assert!(visible.contains("Rendered document"));
        assert!(visible.contains("Raw editor"));
        assert!(visible.contains("Comments"));

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        assert!(!app.help_visible());
    }

    #[test]
    fn raw_mode_keeps_question_mark_editable_and_uses_f1_for_help() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        let mut save = |_: &str| Ok("sha256:saved".into());

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        assert_eq!(app.source, "?# Heading\n");
        assert!(!app.help_visible());

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        assert!(app.help_visible());
    }

    #[test]
    fn minimum_terminal_shows_compact_help_without_silent_clipping() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleHelp).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(30, 6)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let rows = (0..6)
            .map(|y| {
                (0..30)
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert!(rows.iter().any(|row| row.contains("Compact help")));
        assert!(rows.iter().any(|row| row.contains("Resize to 76×18")));
        assert!(rows.iter().any(|row| row.contains("Esc/? closes help")));
    }

    #[test]
    fn help_stays_compact_until_the_full_content_fits() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleHelp).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(60, 18)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let visible = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(visible.contains("Compact help"));
        assert!(visible.contains("Resize to 76×18"));
        assert!(!visible.contains("Rendered document"));
    }

    #[test]
    fn comment_editor_uses_f1_for_help_and_advertises_word_navigation() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::BeginComment).unwrap();
        let mut save = |_: &str| Ok("sha256:saved".into());

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        assert!(app.help_visible());
        assert!(app.comment_draft.is_some());
        assert!(comment_editor_controls(80).contains("Option/Ctrl+←/→ words"));
        assert!(comment_editor_controls(80).contains("F1 help"));

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        assert!(!app.help_visible());
        assert!(app.comment_draft.is_some());
    }
    use crate::markdown::{BlockKind, RenderStyle};
    use crate::{
        annotations::{AnchorState, Annotation},
        app::{App, Command},
        markdown::parse,
    };
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::{
        Terminal,
        backend::TestBackend,
        layout::{Position, Rect},
        style::{Color, Modifier},
    };

    #[test]
    fn comment_editor_preserves_document_context_in_a_bottom_pane() {
        let source = vec!["X".repeat(79); 24].join("\n");
        let mut app = App::new(source.clone(), parse(&source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        app.apply(Command::BeginComment).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(2, 10)].symbol(), "X");
        let editor_title = (0..80)
            .map(|x| buffer[(x, 19)].symbol())
            .collect::<String>();
        let editor_footer = (0..80)
            .map(|x| buffer[(x, 23)].symbol())
            .collect::<String>();
        assert!(editor_title.contains("Comment"));
        assert!(editor_footer.contains("Enter save · Esc cancel"));
    }

    #[test]
    fn compact_comment_editor_preserves_document_context_at_minimum_height() {
        let source = "Visible context";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::BeginComment).unwrap();
        app.apply(Command::AppendCommentCharacter('D')).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(30, 6)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let buffer = terminal.backend().buffer();
        let visible = (0..6)
            .map(|y| (0..30).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>();
        assert!(visible[2].contains("Visible context"));
        assert!(visible[4].contains("Comment: D"));
        assert!(visible[5].contains("←/→ · Enter · Esc"));
        assert!(visible[5].contains("F1"));
    }

    #[test]
    fn comment_editor_keys_insert_at_and_show_the_draft_cursor() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::BeginComment).unwrap();
        for character in "draft".chars() {
            app.apply(Command::AppendCommentCharacter(character))
                .unwrap();
        }
        let mut save = |_: &str| Ok("sha256:saved".to_owned());
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        assert_eq!(app.comment_draft.as_deref(), Some("drafXt"));
        assert_eq!(terminal.backend().cursor_position(), Position::new(6, 20));
    }

    #[test]
    fn comment_editor_option_and_control_arrows_move_by_word() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::BeginComment).unwrap();
        for character in "one two".chars() {
            app.apply(Command::AppendCommentCharacter(character))
                .unwrap();
        }
        let mut save = |_: &str| Ok("sha256:saved".to_owned());

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Left, KeyModifiers::ALT),
            &mut save,
        )
        .unwrap();
        assert_eq!(app.comment_cursor(), 4);

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL),
            &mut save,
        )
        .unwrap();
        assert_eq!(app.comment_cursor(), 0);

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut save,
        )
        .unwrap();
        assert_eq!(app.comment_cursor(), 4);

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL),
            &mut save,
        )
        .unwrap();
        assert_eq!(app.comment_cursor(), 7);
    }

    #[test]
    fn comment_editor_scrolls_to_keep_a_wrapped_cursor_visible() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::BeginComment).unwrap();
        for _ in 0..200 {
            app.apply(Command::AppendCommentCharacter('x')).unwrap();
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        assert_eq!(terminal.backend().cursor_position(), Position::new(9, 22));
    }

    #[test]
    fn compact_comment_editor_scrolls_to_keep_the_cursor_visible() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::BeginComment).unwrap();
        for _ in 0..50 {
            app.apply(Command::AppendCommentCharacter('x')).unwrap();
        }
        let mut terminal = Terminal::new(TestBackend::new(30, 6)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        assert_eq!(terminal.backend().cursor_position(), Position::new(29, 4));
    }

    #[test]
    fn empty_comment_panel_leaves_actions_to_the_global_shortcut_bar() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        let mut terminal = Terminal::new(TestBackend::new(30, 8)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let visible = (1..7)
            .map(|y| (1..29).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        let footer = (0..30).map(|x| buffer[(x, 7)].symbol()).collect::<String>();
        assert!(visible.contains("No comments yet"));
        assert!(!visible.contains("Add comment"));
        assert!(!footer.contains("Comment"));
        assert!(shortcut_bar_text(&app, 30).contains("a Comment"));

        app.apply(Command::ToggleMode).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();
        let footer = (0..30)
            .map(|x| terminal.backend().buffer()[(x, 7)].symbol())
            .collect::<String>();
        assert!(!footer.contains("Ctrl+K"));
        assert!(shortcut_bar_text(&app, 80).contains("Ctrl+K Comment"));
    }

    #[test]
    fn long_comment_previews_wrap_inside_the_panel() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("one two three four five six".into()))
            .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(22, 7)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let visible = (1..6)
            .map(|y| (1..21).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(visible.contains("one two three four"));
        assert!(visible.contains("five six"));
    }

    #[test]
    fn focused_comment_shortcuts_are_width_aware_and_resolution_sensitive() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("Review this".into()))
            .unwrap();

        app.apply(Command::ToggleCommentFocus).unwrap();
        let wide = shortcut_bar_text(&app, 90);
        assert!(wide.contains("j Jump"));
        assert!(wide.contains("x Resolve"));
        assert!(wide.contains("d Delete"));
        let compact = shortcut_bar_text(&app, 30);
        assert!(compact.contains("? Help"));
        assert!(unicode_width::UnicodeWidthStr::width(compact.as_str()) <= 30);

        app.apply(Command::ToggleSelectedCommentResolution).unwrap();
        assert!(shortcut_bar_text(&app, 90).contains("x Reopen"));
    }

    #[test]
    fn comment_status_explains_feedback_inclusion() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("Review this".into()))
            .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(48, 7)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();
        let open = (0..48)
            .map(|x| terminal.backend().buffer()[(x, 1)].symbol())
            .collect::<String>();
        assert!(open.contains("Open (in feedback)"));

        app.apply(Command::ToggleSelectedCommentResolution).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();
        let resolved = (0..48)
            .map(|x| terminal.backend().buffer()[(x, 1)].symbol())
            .collect::<String>();
        assert!(resolved.contains("Resolved (not in feedback)"));
    }

    #[test]
    fn selected_comment_stays_visible_at_the_bottom_of_a_narrow_panel() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        for index in 0..4 {
            app.apply(Command::AddComment(format!(
                "Comment {index} has a preview long enough to exceed the panel width"
            )))
            .unwrap();
        }
        app.apply(Command::ToggleSelectedCommentResolution).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(22, 6)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let selected_row = (1..5)
            .find(|&y| buffer[(1, y)].symbol() == "›")
            .expect("selected comment marker should remain visible");
        let visible = (1..5)
            .map(|y| (1..21).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(visible.contains("4/4 · Resolved"));
        assert!(
            buffer[(1, selected_row)].bg == Color::Blue
                || buffer[(1, selected_row)]
                    .modifier
                    .contains(Modifier::REVERSED)
        );
    }

    #[test]
    fn comment_selection_moves_down_while_entries_fit_in_the_panel() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        for index in 0..3 {
            app.apply(Command::AddComment(format!("Comment {index}")))
                .unwrap();
        }
        let mut terminal = Terminal::new(TestBackend::new(22, 12)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(1, 7)].symbol(), "›");
        assert!(
            buffer[(1, 7)].bg == Color::Blue
                || buffer[(1, 7)].modifier.contains(Modifier::REVERSED)
        );
    }

    #[test]
    fn comment_preview_preserves_a_trailing_newline() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("first\nsecond\n".into()))
            .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(30, 5)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();

        let preview = (1..29)
            .map(|x| terminal.backend().buffer()[(x, 2)].symbol())
            .collect::<String>();
        assert!(preview.starts_with("  first ↵ second ↵ "));
    }

    #[test]
    fn rendered_block_selection_moves_down_while_blocks_fit_in_the_pane() {
        let source = "# One\n\nTwo\n\nThree\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::MoveNextBlock).unwrap();
        app.apply(Command::MoveNextBlock).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(30, 8)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                let scroll = document_scroll(&app, area, None);
                frame.render_widget(document_widget(&app, scroll, area.width), area);
            })
            .unwrap();

        assert_eq!(terminal.backend().buffer()[(1, 6)].symbol(), "›");
    }

    #[test]
    fn document_panel_keeps_state_in_the_title_without_pane_shortcuts() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        let mut terminal = Terminal::new(TestBackend::new(60, 7)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(document_widget(&app, 0, area.width), area);
            })
            .unwrap();
        let rendered_title = (0..60)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect::<String>();
        let rendered_footer = (0..60)
            .map(|x| terminal.backend().buffer()[(x, 6)].symbol())
            .collect::<String>();
        assert!(rendered_title.contains("Rendered · selected block: lines 1–1"));
        assert!(!rendered_title.contains("wheel"));
        assert!(!rendered_footer.contains("comment"));

        app.apply(Command::AddComment("Review this".into()))
            .unwrap();
        let mut compact_terminal = Terminal::new(TestBackend::new(40, 7)).unwrap();
        compact_terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(document_widget(&app, 0, area.width), area);
            })
            .unwrap();
        let compact_footer = (0..40)
            .map(|x| compact_terminal.backend().buffer()[(x, 6)].symbol())
            .collect::<String>();
        assert!(!compact_footer.contains("jump"));

        app.apply(Command::ToggleMode).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(document_widget(&app, 0, area.width), area);
            })
            .unwrap();
        let raw_title = (0..60)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect::<String>();
        let raw_footer = (0..60)
            .map(|x| terminal.backend().buffer()[(x, 6)].symbol())
            .collect::<String>();
        assert!(raw_title.contains("Raw"));
        assert!(!raw_title.contains("arrows"));
        assert!(!raw_footer.contains("Ctrl+S"));
    }

    #[test]
    fn raw_global_shortcut_bar_keeps_compact_actions_visible_at_minimum_width() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(30, 6)).unwrap();

        terminal.draw(|frame| draw(frame, &mut app)).unwrap();

        let footer = (0..30)
            .map(|x| terminal.backend().buffer()[(x, 5)].symbol())
            .collect::<String>();
        assert!(footer.contains("Arrows · Shift+↑↓ · ^S · F1"));
    }

    #[test]
    fn rendered_selected_block_is_visibly_highlighted() {
        let source = "# One\n\nTwo\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::MoveNextBlock).unwrap();

        let text = rendered_text(&app);
        let selected = &text.lines[3].spans[1];

        assert!(
            selected.style.bg == Some(Color::Blue)
                || selected.style.add_modifier.contains(Modifier::REVERSED)
        );
    }

    #[test]
    fn rendered_markers_distinguish_outdated_hints_from_orphans() {
        let source = "One\n\nTwo\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("Review one".into())).unwrap();
        app.apply(Command::MoveNextBlock).unwrap();
        app.comments[0].anchor_state = AnchorState::Orphaned;

        let orphaned = rendered_text(&app);
        assert_eq!(orphaned.lines[0].spans[0].content, "  ");

        app.comments[0].anchor_state = AnchorState::Outdated;
        app.comments[0].navigation_hint = Some(crate::annotations::NavigationHint {
            source_range: app.comments[0].anchor.source_range.clone(),
            document_fingerprint: app.document_fingerprint().to_owned(),
        });

        let outdated = rendered_text(&app);
        assert_eq!(outdated.lines[0].spans[0].content, "◌ ");
    }

    #[test]
    fn rendered_scroll_accounts_for_wrapped_rows() {
        let source = "one two three four five six seven eight\n\nSelected\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::MoveNextBlock).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                let scroll = document_scroll(&app, area, None);
                frame.render_widget(document_widget(&app, scroll, area.width), area);
            })
            .unwrap();

        assert_eq!(terminal.backend().buffer()[(1, 4)].symbol(), "›");
    }

    #[test]
    fn raw_cursor_moves_down_without_scrolling_while_lines_fit() {
        let source = "one\ntwo\nthree\nfour\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        app.apply(Command::MoveRawDown { select: false }).unwrap();
        app.apply(Command::MoveRawDown { select: false }).unwrap();

        let area = Rect::new(0, 0, 30, 8);
        assert_eq!(
            document_scroll(&app, area, Some(raw_cursor_position(&app, 28))),
            0
        );
    }

    #[test]
    fn raw_scroll_tracks_a_cursor_on_a_soft_wrapped_row() {
        let source = "12345 abcdef";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        for _ in 0..6 {
            app.apply(Command::MoveRawRight { select: false }).unwrap();
        }

        assert_eq!(raw_cursor_position(&app, 10), (1, 0));
        assert_eq!(
            document_scroll(&app, Rect::new(0, 0, 12, 3), Some((1, 0))),
            1
        );
    }

    #[test]
    fn raw_wrapped_cursor_position_handles_wide_unicode() {
        let source = "界界界 abcdef";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        for _ in 0..4 {
            app.apply(Command::MoveRawRight { select: false }).unwrap();
        }

        assert_eq!(raw_cursor_position(&app, 10), (1, 0));
    }

    #[test]
    fn raw_cursor_position_counts_rows_after_newlines() {
        let source = "abc\ndef";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        for _ in 0..4 {
            app.apply(Command::MoveRawRight { select: false }).unwrap();
        }

        assert_eq!(raw_cursor_position(&app, 10), (1, 0));
    }

    #[test]
    fn raw_cursor_position_preserves_the_line_after_a_final_newline() {
        let source = "abc\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        for _ in 0..4 {
            app.apply(Command::MoveRawRight { select: false }).unwrap();
        }

        assert_eq!(raw_cursor_position(&app, 10), (1, 0));
    }

    #[test]
    fn raw_cursor_probe_handles_a_long_unbroken_line_with_a_bounded_window() {
        let source = "a".repeat(10_000);
        let mut app = App::new(source.clone(), parse(&source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        app.cursor = 9_000;

        assert_eq!(raw_cursor_position(&app, 40), (225, 0));
    }

    #[test]
    fn highlights_known_fenced_code_languages() {
        let lines = ["fn main() {", "    println!(\"hello\");", "}"].map(str::to_owned);
        let highlighted = highlight_code("rust", lines.iter());

        assert_eq!(highlighted.len(), 3);
        if std::env::var_os("NO_COLOR").is_none() {
            assert!(highlighted[0].len() > 1);
            assert!(
                highlighted
                    .iter()
                    .flatten()
                    .any(|span| span.style.fg.is_some())
            );
        }
    }

    #[test]
    fn raw_selection_is_visibly_styled() {
        let source = "a🦀b";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        app.apply(Command::SetRawSelection(1..5)).unwrap();

        let text = raw_text(&app);
        assert_eq!(text.lines[0].spans[1].content, "🦀");
        if super::colors_enabled() {
            assert_eq!(text.lines[0].spans[1].style.bg, Some(Color::Blue));
        } else {
            assert!(
                text.lines[0].spans[1]
                    .style
                    .add_modifier
                    .contains(Modifier::REVERSED)
            );
        }
    }

    #[test]
    fn terminal_control_bytes_are_shown_as_safe_text() {
        assert_eq!(safe_display("\u{1b}[2J\t"), "␛[2J    ");
    }

    #[test]
    fn raw_letter_a_edits_text_and_control_k_starts_a_comment() {
        let source = "body";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        let mut save = |_: &str| Ok("sha256:saved".to_owned());

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        assert_eq!(app.source, "abody");
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT),
            &mut save,
        )
        .unwrap();
        assert_eq!(app.source, "aAbody");
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            &mut save,
        )
        .unwrap();
        assert!(app.comment_draft.is_some());
    }

    #[test]
    fn raw_mode_comment_focus_routes_shortcuts_away_from_the_editor() {
        let source = "body";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("first".into())).unwrap();
        app.apply(Command::AddComment("second".into())).unwrap();
        app.selected_comment = Some(0);
        app.apply(Command::ToggleMode).unwrap();
        let mut save = |_: &str| Ok("sha256:saved".to_owned());

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
            &mut save,
        )
        .unwrap();
        assert!(app.status.starts_with("Comments focused"));

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();

        assert_eq!(app.selected_comment, Some(1));
        assert_eq!(
            app.comments[1].status,
            crate::annotations::AnnotationStatus::Resolved
        );
        assert_eq!(app.source, source);

        let mut terminal = Terminal::new(TestBackend::new(48, 7)).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let title = (0..48).map(|x| buffer[(x, 0)].symbol()).collect::<String>();
        assert!(title.contains("Comments (2) · Focused"));
        assert!(shortcut_bar_text(&app, 80).contains("x Reopen"));

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();
        assert_eq!(app.source, "dbody");
    }

    #[test]
    fn failed_focused_action_still_persists_reanchors_from_a_raw_save() {
        let source = "# Heading\n\nBody\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("review".into())).unwrap();
        app.apply(Command::ToggleMode).unwrap();
        app.cursor = app.source.len();
        app.apply(Command::Insert('x')).unwrap();
        app.apply(Command::ToggleCommentFocus).unwrap();
        let mut source_saves = 0;
        let mut annotation_saves = 0;
        let mut save = |_: &str| {
            source_saves += 1;
            Ok("sha256:saved".to_owned())
        };
        let mut save_annotations = |_: &[Annotation], _: &str, _: &str| {
            annotation_saves += 1;
            Ok(())
        };

        handle_key_and_persist(
            &mut app,
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
            &mut save,
            &mut save_annotations,
        );

        assert_eq!(source_saves, 1);
        assert_eq!(annotation_saves, 1);
        assert!(app.status.starts_with("Action failed:"));
        assert!(!app.dirty);
    }

    #[test]
    fn comment_actions_save_dirty_source_before_persisting_annotations() {
        let source = "# Heading\n\nBody\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("review".into())).unwrap();
        app.apply(Command::ToggleMode).unwrap();
        app.apply(Command::Insert('x')).unwrap();
        app.apply(Command::ToggleMode).unwrap();
        let mut save_count = 0;
        let mut save = |_: &str| {
            save_count += 1;
            Ok("sha256:saved".to_owned())
        };

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();

        assert_eq!(save_count, 1);
        assert!(!app.dirty);
        assert_eq!(
            app.comments[0].status,
            crate::annotations::AnnotationStatus::Resolved
        );
    }

    #[test]
    fn rendered_up_key_moves_to_the_previous_block() {
        let source = "# Heading\n\nBody\n\nLast\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::MoveNextBlock).unwrap();
        let mut save = |_: &str| Ok("sha256:saved".to_owned());

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut save,
        )
        .unwrap();

        assert_eq!(app.selected_block, 0);
    }

    #[test]
    fn mouse_wheel_moves_the_rendered_block_selection() {
        let source = "# One\n\nTwo\n\nThree\n";
        let mut app = App::new(source.into(), parse(source).unwrap());

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 5,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 80, 24),
        )
        .unwrap();

        assert_eq!(app.selected_block, 1);

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 5,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 80, 24),
        )
        .unwrap();

        assert_eq!(app.selected_block, 0);
    }

    #[test]
    fn left_click_selects_the_rendered_block_under_the_pointer() {
        let source = "# One\n\nTwo\n\nThree\n";
        let mut app = App::new(source.into(), parse(source).unwrap());

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 40, 10),
        )
        .unwrap();

        assert_eq!(app.selected_block, 1);
    }

    #[test]
    fn left_click_maps_wrapped_blocks_with_nonzero_scroll() {
        let source = format!("One\n\n{}\n\nThree\n", "x".repeat(70));
        let mut app = App::new(source.clone(), parse(&source).unwrap());
        app.selected_block = 2;
        let terminal_area = Rect::new(0, 0, 30, 8);
        let document_area = rendered_document_area(&app, terminal_area).unwrap();
        assert!(document_scroll(&app, document_area, None) > 0);

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 2,
                modifiers: KeyModifiers::NONE,
            },
            terminal_area,
        )
        .unwrap();

        assert_eq!(app.selected_block, 1);
    }

    #[test]
    fn left_click_preserves_the_rendered_viewport() {
        let source = format!("One\n\n{}\n", "x".repeat(100));
        let mut app = App::new(source.clone(), parse(&source).unwrap());
        let terminal_area = Rect::new(0, 0, 30, 8);
        let document_area = rendered_document_area(&app, terminal_area).unwrap();
        assert_eq!(document_scroll(&app, document_area, None), 0);

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 4,
                modifiers: KeyModifiers::NONE,
            },
            terminal_area,
        )
        .unwrap();

        assert_eq!(app.selected_block, 1);
        assert_eq!(
            app.rendered_scroll_override(document_area.width, document_area.height),
            Some(0)
        );
        assert_eq!(document_scroll(&app, document_area, None), 0);
    }

    #[test]
    fn arrow_navigation_scrolls_only_after_selection_crosses_a_viewport_edge() {
        let source = "One\n\nTwo\n\nThree\n\nFour\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        let area = Rect::new(0, 0, 30, 8);
        app.select_rendered_block_from_click(1, 0, area.width, area.height);
        app.remember_rendered_scroll(0, area.width, area.height);

        app.apply(Command::MoveNextBlock).unwrap();
        assert_eq!(document_scroll(&app, area, None), 0);

        app.apply(Command::MoveNextBlock).unwrap();
        assert_eq!(document_scroll(&app, area, None), 1);
        app.remember_rendered_scroll(1, area.width, area.height);

        app.apply(Command::MovePreviousBlock).unwrap();
        assert_eq!(document_scroll(&app, area, None), 1);
        app.apply(Command::MovePreviousBlock).unwrap();
        assert_eq!(document_scroll(&app, area, None), 1);
        app.apply(Command::MovePreviousBlock).unwrap();
        assert_eq!(document_scroll(&app, area, None), 0);
    }

    #[test]
    fn left_click_selects_a_block_in_the_three_pane_document() {
        let source = "# One\n\nTwo\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("Review this".into()))
            .unwrap();

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 100, 24),
        )
        .unwrap();

        assert_eq!(app.selected_block, 1);
    }

    #[test]
    fn left_click_ignores_document_borders_spacers_and_the_comments_pane() {
        let source = "# One\n\nTwo\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("Review this".into()))
            .unwrap();
        let terminal_area = Rect::new(0, 0, 100, 24);

        for (column, row) in [(5, 1), (5, 4), (80, 5)] {
            handle_mouse(
                &mut app,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
                terminal_area,
            )
            .unwrap();
            assert_eq!(app.selected_block, 0);
        }
    }

    #[test]
    fn mouse_wheel_moves_the_raw_cursor_by_line() {
        let source = "One\nTwo\nThree\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::ToggleMode).unwrap();

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 5,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 80, 24),
        )
        .unwrap();

        assert_eq!(app.cursor, 4);
        assert_eq!(app.raw_selection, None);

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 5,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 80, 24),
        )
        .unwrap();

        assert_eq!(app.cursor, 0);
    }

    #[test]
    fn mouse_wheel_moves_comment_selection_when_comments_are_focused() {
        let source = "One\nTwo\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("first".into())).unwrap();
        app.apply(Command::AddComment("second".into())).unwrap();
        app.selected_comment = Some(0);
        app.apply(Command::ToggleMode).unwrap();
        app.apply(Command::ToggleCommentFocus).unwrap();

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 5,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 80, 24),
        )
        .unwrap();

        assert_eq!(app.selected_comment, Some(1));
        assert_eq!(app.cursor, 0);
    }

    #[test]
    fn mouse_wheel_does_not_move_the_document_while_a_comment_is_open() {
        let source = "# One\n\nTwo\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::BeginComment).unwrap();

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 5,
                row: 5,
                modifiers: KeyModifiers::NONE,
            },
            Rect::new(0, 0, 80, 24),
        )
        .unwrap();

        assert_eq!(app.selected_block, 0);
    }

    #[test]
    fn rendered_headings_have_terminal_emphasis() {
        let source = "# Heading\n";
        let app = App::new(source.into(), parse(source).unwrap());
        let text = rendered_text(&app);

        assert!(
            text.lines[0]
                .spans
                .iter()
                .any(|span| span.style.add_modifier.contains(Modifier::BOLD))
        );
    }

    #[test]
    fn rendered_heading_levels_have_distinct_visual_hierarchy() {
        let source = "# Top\n\n## Section\n\n### Topic\n\n###### Detail\n";
        let app = App::new(source.into(), parse(source).unwrap());
        let text = rendered_text(&app);

        assert!(
            text.lines
                .iter()
                .any(|line| { line.spans.iter().any(|span| span.content.contains('━')) })
        );
        assert!(
            text.lines
                .iter()
                .any(|line| { line.spans.iter().any(|span| span.content.contains('─')) })
        );
        let top = text
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .find(|span| span.content == "Top")
            .unwrap();
        let detail = text
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .find(|span| span.content == "Detail")
            .unwrap();
        assert!(top.style.add_modifier.contains(Modifier::BOLD));
        assert!(top.style.add_modifier.contains(Modifier::UNDERLINED));
        assert!(detail.style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn rendered_table_headers_are_bold() {
        let source = "| Key | Action |\n| --- | --- |\n| q | Quit |\n";
        let app = App::new(source.into(), parse(source).unwrap());
        let text = rendered_text(&app);

        assert!(text.lines[0].spans.iter().any(|span| {
            span.content == "Key" && span.style.add_modifier.contains(Modifier::BOLD)
        }));
    }

    #[test]
    fn short_tables_keep_visible_aligned_column_boundaries() {
        let source = "| Key | Action |\n| --- | --- |\n| q | Quit |\n";
        let app = App::new(source.into(), parse(source).unwrap());
        let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(document_widget(&app, 0, area.width), area);
            })
            .unwrap();

        let rows = (0..8)
            .map(|y| {
                (0..60)
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert!(rows.iter().any(|row| {
            row.contains("Key") && row.matches('│').count() >= 3 && row.contains("Action")
        }));
        assert!(
            rows.iter().any(|row| row.contains('q')
                && row.matches('│').count() >= 3
                && row.contains("Quit"))
        );
    }

    #[test]
    fn wide_tables_stack_cells_under_header_labels() {
        let source = "| Concern | TableDataGrid owns | Host owns |\n| --- | --- | --- |\n| Column schema and defaults | Header interpretation, default resolution, and AG Grid column translation | Header declarations for the available columns |\n| Current table configuration | Internal state when uncontrolled, config normalization, grid synchronization, and update events | Controlled current state when supplied |\n";
        let app = App::new(source.into(), parse(source).unwrap());
        let mut terminal = Terminal::new(TestBackend::new(80, 18)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(document_widget(&app, 0, area.width), area);
            })
            .unwrap();

        let visible = (0..18)
            .map(|y| {
                (0..80)
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(visible.contains("Concern: Column schema and defaults"));
        assert!(visible.contains("TableDataGrid owns: Header interpretation"));
        assert!(visible.contains("Host owns: Header declarations"));
        assert!(visible.contains("Concern: Current table configuration"));
    }

    #[test]
    fn stacked_tables_preserve_inline_styles_in_headers_and_cells() {
        let source = "| [Owner](owner.md) | Detail |\n| --- | --- |\n| *Package* | A long explanation that forces this table into its stacked layout. |\n";
        let app = App::new(source.into(), parse(source).unwrap());
        let text = rendered_text_at_width(&app, 36);

        let owner = text
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .find(|span| span.content == "Owner")
            .unwrap();
        assert!(owner.style.add_modifier.contains(Modifier::BOLD));
        assert!(owner.style.add_modifier.contains(Modifier::UNDERLINED));
        let package = text
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .find(|span| span.content == "Package")
            .unwrap();
        assert!(package.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn wide_header_only_tables_show_each_header_on_its_own_line() {
        let source = "| A very long first heading | Another long heading |\n| --- | --- |\n";
        let app = App::new(source.into(), parse(source).unwrap());
        let text = rendered_text_at_width(&app, 30);
        let visible = text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(
            visible
                .iter()
                .any(|line| line.contains("A very long first heading"))
        );
        assert!(
            visible
                .iter()
                .any(|line| line.contains("Another long heading"))
        );
        assert_eq!(visible.iter().filter(|line| line.contains('│')).count(), 0);
    }

    #[test]
    fn table_layout_height_matches_click_mapping_at_the_width_boundary() {
        let source = "| A | B |\n| --- | --- |\n| 12345678901234567 | 12345678901234567 |\n\nFollowing paragraph.\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        let terminal_area = Rect::new(0, 0, 40, 14);
        let document_area = rendered_document_area(&app, terminal_area).unwrap();
        let table_height = rendered_block_height(
            &app.rendered.blocks[0],
            document_area.width.saturating_sub(2),
        );
        assert_eq!(table_height, 2);

        handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: document_area.y + table_height as u16 + 2,
                modifiers: KeyModifiers::NONE,
            },
            terminal_area,
        )
        .unwrap();

        assert_eq!(app.selected_block, 1);
    }

    #[test]
    fn rendered_inline_styles_preserve_markdown_meaning() {
        let source = "**bold** *italic* ~~old~~ `code` [link](guide.md)";
        let app = App::new(source.into(), parse(source).unwrap());
        let text = rendered_text(&app);
        let spans = &text.lines[0].spans;

        assert!(spans.iter().any(|span| {
            span.content == "bold" && span.style.add_modifier.contains(Modifier::BOLD)
        }));
        assert!(spans.iter().any(|span| {
            span.content == "italic" && span.style.add_modifier.contains(Modifier::ITALIC)
        }));
        assert!(spans.iter().any(|span| {
            span.content == "old" && span.style.add_modifier.contains(Modifier::CROSSED_OUT)
        }));
        assert!(spans.iter().any(|span| {
            span.content == "code"
                && span
                    .style
                    .add_modifier
                    .contains(if super::colors_enabled() {
                        Modifier::BOLD
                    } else {
                        Modifier::REVERSED
                    })
        }));
        assert!(spans.iter().any(|span| {
            span.content == "link" && span.style.add_modifier.contains(Modifier::UNDERLINED)
        }));
    }

    #[test]
    fn links_and_headings_use_the_terminal_default_foreground() {
        let top_heading = block_style(BlockKind::Heading, Some(1), true);
        let section_heading = block_style(BlockKind::Heading, Some(2), true);
        let topic_heading = block_style(BlockKind::Heading, Some(3), true);
        let link = inline_style(
            RenderStyle {
                link: true,
                ..RenderStyle::default()
            },
            true,
        );

        assert_eq!(top_heading.fg, None);
        assert_eq!(section_heading.fg, None);
        assert_eq!(topic_heading.fg, None);
        assert_eq!(link.fg, None);
        assert!(link.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn no_color_styles_keep_semantics_without_terminal_colors() {
        let heading = block_style(BlockKind::Heading, Some(1), false);
        let link = inline_style(
            RenderStyle {
                link: true,
                ..RenderStyle::default()
            },
            false,
        );
        let code = inline_style(
            RenderStyle {
                code: true,
                ..RenderStyle::default()
            },
            false,
        );

        assert_eq!(heading.fg, None);
        assert_eq!(link.fg, None);
        assert_eq!(marker_style(false).fg, None);
        assert_eq!(focus_style(false).fg, None);
        assert!(heading.add_modifier.contains(Modifier::BOLD));
        assert!(focus_style(false).add_modifier.contains(Modifier::BOLD));
        assert!(link.add_modifier.contains(Modifier::UNDERLINED));
        assert!(code.add_modifier.contains(Modifier::REVERSED));
    }
}
