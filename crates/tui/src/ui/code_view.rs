use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Focus, InputMode};
use crate::session::DebuggerState;
use crate::syntax::SyntaxHighlighter;

use super::border_style;

// Persistent SyntaxHighlighter stored in thread-local so it caches parse
// checkpoints across frames.
thread_local! {
    static HIGHLIGHTER: std::cell::RefCell<SyntaxHighlighter> = std::cell::RefCell::new(SyntaxHighlighter::new());
    static LAST_FILE: std::cell::RefCell<Option<(std::path::PathBuf, &'static str)>> = std::cell::RefCell::new(None);
}

pub fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style_for_code(app));

    let content = app.current_file_content();
    let file_path = app.code_view.file_path.clone();

    match (content, &file_path) {
        (Some(content), Some(path)) => {
            // Compute title from filename
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Code".to_string());

            let block = block.title(format!(" {file_name} "));

            // Inner area (accounting for borders)
            let inner = block.inner(area);
            frame.render_widget(block, area);

            // Reserve space for search bar if active
            let (code_area, search_area) = if app.search.active {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(inner);
                (chunks[0], Some(chunks[1]))
            } else {
                (inner, None)
            };

            let viewport_height = code_area.height as usize;

            // Update search matches
            let path_clone = path.clone();
            // We need to clone content to avoid borrow issues since we mutate app.search
            let content_owned = content.to_string();
            app.search.update(&content_owned, &path_clone);

            // Ensure cursor visible
            app.code_view.ensure_cursor_visible(viewport_height);

            let start_line = app.code_view.scroll_offset;
            let end_line = (start_line + viewport_height).min(app.code_view.total_lines);

            // Gutter width based on total line count
            let gutter_width = format!("{}", app.code_view.total_lines).len();

            // Syntax highlight the visible lines
            let syntect_theme = app.theme.syntect_theme;
            LAST_FILE.with(|last| {
                let mut last = last.borrow_mut();
                let key = (path.clone(), syntect_theme);
                if last.as_ref() != Some(&key) {
                    HIGHLIGHTER.with(|h| h.borrow_mut().set_file(path));
                    *last = Some(key);
                }
            });

            let highlighted = HIGHLIGHTER.with(|h| {
                h.borrow_mut()
                    .highlight_lines(&content_owned, start_line, end_line, syntect_theme)
            });

            // Determine execution line (0-indexed) if paused at current file
            let exec_line: Option<usize> = app.session.as_ref().and_then(|session| {
                if let DebuggerState::Paused { paused_frame, .. } = &session.state {
                    let frame = &paused_frame.frame;
                    if let Some(source) = &frame.source {
                        if source.path.as_ref() == Some(path) {
                            return Some((frame.line as usize).saturating_sub(1));
                        }
                    }
                }
                None
            });

            // Collect breakpoint lines (1-indexed) for the current file
            let breakpoint_lines: std::collections::HashSet<usize> = app
                .ui_breakpoints
                .iter()
                .filter(|bp| bp.path == *path)
                .map(|bp| bp.line)
                .collect();

            // Visual selection range
            let selection_range = app.code_view.selection_range();

            let lines = SyntaxHighlighter::build_lines(
                &highlighted,
                start_line,
                gutter_width,
                app.code_view.cursor_line,
                &app.search.matches,
                app.search.current_match,
                exec_line,
                &breakpoint_lines,
                selection_range,
                &app.inline_evaluations,
                &app.theme,
            );

            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph, code_area);

            // Render search bar
            if let Some(search_area) = search_area {
                render_search_bar(app, frame, search_area);
            }
        }
        _ => {
            let block = block.title(" Code ");
            let paragraph = Paragraph::new("(no file open)")
                .alignment(Alignment::Center)
                .block(block);
            frame.render_widget(paragraph, area);
        }
    }
}

fn render_search_bar(app: &App, frame: &mut Frame, area: Rect) {
    let match_info = if app.search.matches.is_empty() {
        if app.search.query.is_empty() {
            String::new()
        } else {
            " (no matches)".to_string()
        }
    } else {
        format!(
            " ({}/{})",
            app.search.current_match + 1,
            app.search.matches.len()
        )
    };

    let is_active = app.input_mode == InputMode::Search;
    let base_style = Style::default()
        .fg(app.theme.text)
        .add_modifier(Modifier::BOLD);

    let mut spans = vec![Span::styled("/", Style::default().fg(app.theme.accent))];
    if is_active {
        spans.extend(app.search_editor.render_spans(base_style));
    } else {
        spans.push(Span::styled(app.search.query.as_str(), base_style));
    }
    spans.push(Span::styled(
        match_info,
        Style::default().fg(app.theme.text_muted),
    ));
    let line = Line::from(spans);

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn border_style_for_code(app: &App) -> Style {
    border_style(app, Focus::CodeView)
}
