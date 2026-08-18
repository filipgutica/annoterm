use std::{sync::OnceLock, time::Duration};

use anyhow::{Result, anyhow};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseEvent, MouseEventKind,
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
    markdown::{BlockKind, RenderBlock, RenderStyle},
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
    G: FnMut(&[Annotation], &str) -> Result<()>,
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
                if let Err(error) = handle_mouse(app, mouse) {
                    app.status = format!("Action failed: {error}");
                }
                continue;
            }
            let Event::Key(key) = input else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let draft_was_open = app.comment_draft.is_some();
            if is_quit(key, app.mode) && !draft_was_open && !app.comments_focused() {
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
    G: FnMut(&[Annotation], &str) -> Result<()>,
{
    let comments_before = app.comments.clone();
    let transient_reanchor = app.dirty
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('r');
    let action_error = handle_key(app, key, save).err();
    let persistence_error = (!transient_reanchor && comments_before != app.comments)
        .then(|| save_annotations(&app.comments, app.document_fingerprint()).err())
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

fn handle_mouse(app: &mut App, mouse: MouseEvent) -> Result<()> {
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
        (Mode::Rendered, MouseEventKind::ScrollDown) => app.apply(Command::MoveNextBlock),
        (Mode::Rendered, MouseEventKind::ScrollUp) => app.apply(Command::MovePreviousBlock),
        (Mode::Raw, MouseEventKind::ScrollDown) => {
            app.apply(Command::MoveRawDown { select: false })
        }
        (Mode::Raw, MouseEventKind::ScrollUp) => app.apply(Command::MoveRawUp { select: false }),
        _ => Ok(()),
    }
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
    if app.comment_draft.is_some() {
        return match key.code {
            KeyCode::Esc => app.apply(Command::CancelComment),
            KeyCode::Enter => {
                save_source_if_dirty(app, save)?;
                app.apply(Command::SubmitComment)
            }
            KeyCode::Backspace => app.apply(Command::DeleteCommentCharacter),
            KeyCode::Char(character) if accepts_text(key.modifiers) => {
                app.apply(Command::AppendCommentCharacter(character))
            }
            _ => Ok(()),
        };
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
        KeyCode::Down | KeyCode::Char('j' | ']') => app.apply(Command::SelectNextComment),
        KeyCode::Up | KeyCode::Char('k' | '[') => app.apply(Command::SelectPreviousComment),
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

fn draw(frame: &mut Frame, app: &App) {
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
}

fn draw_document_only(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(frame.area());
    frame.render_widget(Paragraph::new(safe_display(&app.status)), chunks[0]);
    draw_document_pane(frame, app, chunks[1]);
    draw_comment_draft(frame, app);
}

fn draw_three_pane(frame: &mut Frame, app: &App) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(frame.area());
    let document_width = if app.comments.is_empty() { 82 } else { 72 };
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(document_width),
            Constraint::Percentage(100 - document_width),
        ])
        .split(vertical[1]);
    frame.render_widget(Paragraph::new(safe_display(&app.status)), vertical[0]);
    draw_document_pane(frame, app, horizontal[0]);
    frame.render_widget(comments_widget(app, horizontal[1]), horizontal[1]);
    draw_comment_draft(frame, app);
}

fn draw_document_pane(frame: &mut Frame, app: &App, area: Rect) {
    let raw_cursor = (app.mode == Mode::Raw)
        .then(|| raw_cursor_position(app, area.width.saturating_sub(2).max(1)));
    let scroll = document_scroll(app, area, raw_cursor);
    frame.render_widget(document_widget(app, scroll), area);
    place_raw_cursor(frame, app, area, raw_cursor, scroll);
}

fn draw_comment_draft(frame: &mut Frame, app: &App) {
    let Some(draft) = &app.comment_draft else {
        return;
    };
    let area = frame.area();
    let width = area.width.saturating_sub(8).min(60);
    let height = 5;
    let popup = ratatui::layout::Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(format!(
            "{}\n\nEnter saves comment · Esc cancels",
            safe_display(draft)
        ))
        .block(Block::default().borders(Borders::ALL).title("Comment"))
        .wrap(Wrap { trim: false }),
        popup,
    );
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
    let line_start = app.source[..app.cursor]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let line_end = app.source[app.cursor..]
        .find('\n')
        .map_or(app.source.len(), |index| app.cursor + index);
    let rows_before = if line_start == 0 {
        0
    } else {
        Paragraph::new(safe_display(&app.source[..line_start]))
            .wrap(Wrap { trim: false })
            .line_count(width)
    };
    let before = safe_display(&app.source[line_start..app.cursor]);
    let after = &app.source[app.cursor..line_end];
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

fn document_widget(app: &App, scroll: u16) -> Paragraph<'static> {
    let title = match app.mode {
        Mode::Rendered => app
            .rendered
            .blocks
            .get(app.selected_block)
            .map(|block| {
                format!(
                    "Rendered · selected block: lines {}–{} · ↑/↓/wheel move · a comment",
                    block.line_range.start(),
                    block.line_range.end()
                )
            })
            .unwrap_or_else(|| "Rendered · no selectable blocks".into()),
        Mode::Raw => "Raw · arrows/wheel move · Shift+arrows select · Ctrl+S save".into(),
    };
    let text = match app.mode {
        Mode::Rendered => rendered_text(app),
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
            if selected_height >= viewport_height {
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
    let mut lines = block
        .lines
        .iter()
        .map(|line| Line::from(format!("  {line}")))
        .collect::<Vec<_>>();
    if let Some(rule) = heading_rule(block) {
        lines.push(Line::from(format!("  {rule}")));
    }
    Paragraph::new(Text::from(lines))
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
                (AnnotationStatus::Resolved, _) => "Resolved",
                (_, AnchorState::Orphaned) => "Orphaned",
                _ => "Open",
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
    if app.comments.is_empty() {
        block = block.title_bottom(empty_comment_controls(app.mode, area.width));
    } else if app.mode == Mode::Raw && !app.comments_focused() {
        block = block.title_bottom(comment_focus_control(area.width));
    } else {
        block = block.title_bottom(comment_controls(app, area.width));
    }
    Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
}

fn comment_focus_control(width: u16) -> &'static str {
    if width >= 27 {
        "[Ctrl+W] Focus comments"
    } else {
        "[Ctrl+W] Comments"
    }
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

fn empty_comment_controls(mode: Mode, width: u16) -> &'static str {
    match (mode, width) {
        (Mode::Rendered, 28..) => "[a] Comment selected block",
        (Mode::Rendered, 11..) => "[a] Block",
        (Mode::Rendered, _) => "[a]",
        (Mode::Raw, 28..) => "[Ctrl+K] Comment selection",
        (Mode::Raw, 20..) => "[Ctrl+K] Selection",
        (Mode::Raw, _) => "[Ctrl+K]",
    }
}

fn comment_controls(app: &App, width: u16) -> String {
    if width >= 46 {
        let resolution_action = app
            .selected_comment
            .and_then(|index| app.comments.get(index))
            .map(|comment| match comment.status {
                AnnotationStatus::Open => "resolve",
                AnnotationStatus::Resolved => "reopen",
            })
            .unwrap_or("resolve");
        format!("[/] select · e edit · x {resolution_action} · d delete")
    } else if width >= 34 {
        "[/] select · e edit · d delete".into()
    } else if width >= 22 {
        "[/] select · e edit".into()
    } else {
        "[/] · e".into()
    }
}

fn rendered_text(app: &App) -> Text<'static> {
    let mut output = Vec::new();
    for (index, block) in app.rendered.blocks.iter().enumerate() {
        if index > 0 {
            output.push(Line::default());
        }
        let marker = if app.comments.iter().any(|comment| {
            let range = &comment.anchor.source_range;
            range.start.byte < block.source_range.end && range.end.byte > block.source_range.start
        }) {
            "● "
        } else if index == app.selected_block {
            "› "
        } else {
            "  "
        };
        output.extend(render_block_lines(
            block,
            marker,
            index == app.selected_block,
        ));
    }
    Text::from(output)
}

fn render_block_lines(
    block: &RenderBlock,
    marker: &'static str,
    selected: bool,
) -> Vec<Line<'static>> {
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
            1 => with_color(Style::default(), Color::Cyan, colors)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            2 => with_color(Style::default(), Color::Cyan, colors).add_modifier(Modifier::BOLD),
            3 => with_color(Style::default(), Color::Blue, colors).add_modifier(Modifier::BOLD),
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
        output = with_color(output, Color::Blue, colors).add_modifier(Modifier::UNDERLINED);
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
        block_style, comments_widget, document_scroll, document_widget, draw, focus_style,
        handle_key, handle_key_and_persist, handle_mouse, highlight_code, inline_style,
        marker_style, raw_cursor_position, raw_text, rendered_text, safe_display,
    };
    use crate::markdown::{BlockKind, RenderStyle};
    use crate::{
        annotations::Annotation,
        app::{App, Command},
        markdown::parse,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
    use ratatui::{
        Terminal,
        backend::TestBackend,
        layout::Rect,
        style::{Color, Modifier},
    };

    #[test]
    fn comment_modal_clears_the_document_behind_it() {
        let source = vec!["X".repeat(79); 24].join("\n");
        let mut app = App::new(source.clone(), parse(&source).unwrap());
        app.apply(Command::ToggleMode).unwrap();
        app.apply(Command::BeginComment).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        terminal.draw(|frame| draw(frame, &app)).unwrap();

        assert_eq!(terminal.backend().buffer()[(11, 10)].symbol(), " ");
    }

    #[test]
    fn empty_comment_panel_keeps_add_actions_in_the_footer() {
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
        assert!(footer.contains("[a] Comment selected block"));

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
        assert!(footer.contains("[Ctrl+K] Comment selection"));
        assert!(!footer.contains("[a]"));
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
    fn comment_panel_keeps_width_aware_controls_visible() {
        let source = "# Heading\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::AddComment("Review this".into()))
            .unwrap();

        for (width, expected, omitted) in [
            (48, "[/] select · e edit · x resolve · d delete", None),
            (28, "[/] select · e edit", Some("delete")),
        ] {
            let mut terminal = Terminal::new(TestBackend::new(width, 7)).unwrap();
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    frame.render_widget(comments_widget(&app, area), area);
                })
                .unwrap();

            let footer = (0..width)
                .map(|x| terminal.backend().buffer()[(x, 6)].symbol())
                .collect::<String>();
            assert!(footer.contains(expected));
            if let Some(omitted) = omitted {
                assert!(!footer.contains(omitted));
            }
        }

        app.apply(Command::ToggleSelectedCommentResolution).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(48, 7)).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(comments_widget(&app, area), area);
            })
            .unwrap();
        let footer = (0..48)
            .map(|x| terminal.backend().buffer()[(x, 6)].symbol())
            .collect::<String>();
        assert!(footer.contains("x reopen"));
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
                frame.render_widget(document_widget(&app, scroll), area);
            })
            .unwrap();

        assert_eq!(terminal.backend().buffer()[(1, 6)].symbol(), "›");
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
    fn rendered_scroll_accounts_for_wrapped_rows() {
        let source = "one two three four five six seven eight\n\nSelected\n";
        let mut app = App::new(source.into(), parse(source).unwrap());
        app.apply(Command::MoveNextBlock).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).unwrap();

        terminal
            .draw(|frame| {
                let area = frame.area();
                let scroll = document_scroll(&app, area, None);
                frame.render_widget(document_widget(&app, scroll), area);
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
        let footer = (0..48).map(|x| buffer[(x, 6)].symbol()).collect::<String>();
        assert!(title.contains("Comments (2) · Focused"));
        assert!(footer.contains("x reopen"));

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
        let mut save_annotations = |_: &[Annotation], _: &str| {
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
        )
        .unwrap();

        assert_eq!(app.selected_block, 0);
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
