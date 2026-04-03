pub mod breakpoints;
pub mod call_stack;
pub mod code_view;
pub mod controls_bar;
pub mod file_picker;
pub mod help;
pub mod output;
pub mod repl;
pub mod status_bar;
pub mod threads;
pub mod variables;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
};

use crate::app::{App, BottomTab, Focus};

/// Border style for the currently focused pane.
fn focused_border() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Border style for unfocused panes.
fn unfocused_border() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Return the appropriate border style for a pane.
pub fn border_style(app: &App, pane: Focus) -> Style {
    if app.focus == pane {
        focused_border()
    } else {
        unfocused_border()
    }
}

/// Render the full application layout.
pub fn render(app: &mut App, frame: &mut Frame) {
    let size = frame.area();

    // Top-level vertical split:
    //   [controls bar: 3 rows]
    //   [middle area: fill]
    //   [bottom tabbed panel: 25%]
    //   [status bar: 1 row]
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // controls bar
            Constraint::Min(8),    // middle (sidebar + code)
            Constraint::Length(8), // bottom panel
            Constraint::Length(1), // status bar
        ])
        .split(size);

    // Controls bar
    controls_bar::render(app, frame, outer[0]);

    // Middle: sidebar | code view
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24), // sidebar
            Constraint::Min(20),    // code view
        ])
        .split(outer[1]);

    render_sidebar(app, frame, middle[0]);

    // Code view needs &mut App for scroll/search state updates
    code_view::render(app, frame, middle[1]);

    // Bottom tabbed panel
    render_bottom_panel(app, frame, outer[2]);

    // Status bar
    status_bar::render(app, frame, outer[3]);

    // Overlays (rendered last so they draw on top)
    if app.file_picker.open {
        file_picker::render(app, frame);
    }
    if app.show_help {
        help::render(frame);
    }
}

/// Render the left sidebar: call stack, breakpoints, threads stacked vertically.
fn render_sidebar(app: &App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // call stack
            Constraint::Percentage(40), // breakpoints
            Constraint::Percentage(20), // threads
        ])
        .split(area);

    call_stack::render(app, frame, chunks[0]);
    breakpoints::render(app, frame, chunks[1]);
    threads::render(app, frame, chunks[2]);
}

/// Render the bottom tabbed panel (variables / output / REPL).
fn render_bottom_panel(app: &App, frame: &mut Frame, area: Rect) {
    match app.bottom_tab {
        BottomTab::Variables => variables::render(app, frame, area),
        BottomTab::Output => output::render(app, frame, area),
        BottomTab::Repl => repl::render(app, frame, area),
    }
}
