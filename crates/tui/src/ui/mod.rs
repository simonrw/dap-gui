pub mod breakpoints;
pub mod call_stack;
pub mod code_view;
pub mod controls_bar;
pub mod evaluate_popup;
pub mod file_browser;
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

use crate::app::{App, AppMode, BottomTab, Focus};

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

    if app.zen_mode {
        // Zen mode: code view fills everything except the status bar
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),    // code view (full height)
                Constraint::Length(1), // status bar
            ])
            .split(size);

        code_view::render(app, frame, outer[0]);
        status_bar::render(app, frame, outer[1]);
    } else {
        // Normal layout:
        //   [controls bar: 3 rows]
        //   [middle area: fill]
        //   [bottom tabbed panel: 8 rows]
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
    }

    // Overlays (rendered last so they draw on top)
    if app.file_picker.open {
        file_picker::render(app, frame);
    }
    if app.evaluate_popup_open {
        evaluate_popup::render(app, frame);
    }
    if app.show_help {
        help::render(frame, &app.keybindings);
    }
}

/// Render the left sidebar: file browser (no-session) or call stack + breakpoints + threads.
fn render_sidebar(app: &mut App, frame: &mut Frame, area: Rect) {
    if app.mode == AppMode::NoSession {
        // Lazy-load files for the browser
        app.ensure_file_browser_loaded();
        file_browser::render(app, frame, area);
    } else {
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
}

/// Render the bottom tabbed panel (variables / output / REPL).
fn render_bottom_panel(app: &App, frame: &mut Frame, area: Rect) {
    match app.bottom_tab {
        BottomTab::Variables => variables::render(app, frame, area),
        BottomTab::Output => output::render(app, frame, area),
        BottomTab::Repl => repl::render(app, frame, area),
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use crate::app::test_helpers::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Render the full UI into a TestBackend and return the Display string.
    fn render_to_string(width: u16, height: u16, f: impl FnOnce(&mut App)) -> String {
        let dir = tempfile::tempdir().expect("tempdir");
        let state_path = dir.path().join("state.json");
        let state_manager = state::StateManager::new(&state_path).expect("StateManager");
        let (wakeup_tx, _) = crossbeam_channel::unbounded();

        let mut app = crate::app::App::new(
            vec![],
            vec![],
            0,
            std::path::PathBuf::from("/tmp/test"),
            std::path::PathBuf::from("/tmp/test"),
            state_manager,
            wakeup_tx,
            vec![],
            Default::default(),
        );
        // Prevent the file browser from loading real git files.
        app.file_browser_loaded = true;

        f(&mut app);

        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&mut app, frame)).unwrap();
        terminal.backend().to_string()
    }

    /// Same as above but with config names.
    fn render_to_string_with_configs(
        width: u16,
        height: u16,
        config_names: Vec<String>,
        f: impl FnOnce(&mut App),
    ) -> String {
        let dir = tempfile::tempdir().expect("tempdir");
        let state_path = dir.path().join("state.json");
        let state_manager = state::StateManager::new(&state_path).expect("StateManager");
        let (wakeup_tx, _) = crossbeam_channel::unbounded();

        let configs: Vec<launch_configuration::LaunchConfiguration> = config_names
            .iter()
            .map(|name| {
                let json = serde_json::json!({
                    "name": name,
                    "type": "python",
                    "request": "launch",
                    "program": "/tmp/test.py"
                });
                serde_json::from_value(json).expect("test config")
            })
            .collect();

        let mut app = crate::app::App::new(
            configs,
            config_names,
            0,
            std::path::PathBuf::from("/tmp/test"),
            std::path::PathBuf::from("/tmp/test"),
            state_manager,
            wakeup_tx,
            vec![],
            Default::default(),
        );
        // Prevent the file browser from loading real git files.
        app.file_browser_loaded = true;

        f(&mut app);

        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&mut app, frame)).unwrap();
        terminal.backend().to_string()
    }

    // ── Full layout snapshots ─────────────────────────────────────────

    #[test]
    fn snapshot_no_session_empty() {
        let output = render_to_string(80, 24, |_app| {});
        insta::assert_snapshot!("no_session_empty", output);
    }

    #[test]
    fn snapshot_no_session_with_configs() {
        let output = render_to_string_with_configs(
            80,
            24,
            vec!["Debug Python".into(), "Debug Rust".into()],
            |_app| {},
        );
        insta::assert_snapshot!("no_session_with_configs", output);
    }

    #[test]
    fn snapshot_no_session_with_file() {
        let (_dir, file_path) = write_temp_file("def hello():\n    print('hi')\n\nhello()\n");

        let output = render_to_string(80, 24, |app| {
            app.open_file(file_path);
        });
        insta::assert_snapshot!("no_session_with_file", output);
    }

    #[test]
    fn snapshot_running_mode() {
        let output = render_to_string_with_configs(80, 24, vec!["Test".into()], |app| {
            app.mode = crate::app::AppMode::Running;
            app.status_message = Some("Running...".to_string());
        });
        insta::assert_snapshot!("running_mode", output);
    }

    #[test]
    fn snapshot_terminated_mode() {
        let output = render_to_string_with_configs(80, 24, vec!["Test".into()], |app| {
            app.mode = crate::app::AppMode::Terminated;
            app.status_message = Some("Debugee terminated".to_string());
        });
        insta::assert_snapshot!("terminated_mode", output);
    }

    #[test]
    fn snapshot_help_overlay() {
        let output = render_to_string(80, 24, |app| {
            app.show_help = true;
        });
        insta::assert_snapshot!("help_overlay", output);
    }

    #[test]
    fn snapshot_output_tab_with_lines() {
        let output = render_to_string(80, 24, |app| {
            app.mode = crate::app::AppMode::Running;
            app.bottom_tab = crate::app::BottomTab::Output;
            app.output_lines
                .push(("stdout".into(), "Hello from program".into()));
            app.output_lines
                .push(("stderr".into(), "Warning: something".into()));
            app.output_lines
                .push(("console".into(), "Debugger message".into()));
        });
        insta::assert_snapshot!("output_tab_with_lines", output);
    }

    #[test]
    fn snapshot_repl_tab() {
        let output = render_to_string(80, 24, |app| {
            app.mode = crate::app::AppMode::Running;
            app.bottom_tab = crate::app::BottomTab::Repl;
            app.repl_history
                .push(("x + 1".to_string(), "42".to_string(), false));
            app.repl_input = "some_var".to_string();
        });
        insta::assert_snapshot!("repl_tab", output);
    }

    #[test]
    fn snapshot_breakpoints_panel() {
        let output = render_to_string(80, 24, |app| {
            app.mode = crate::app::AppMode::Running;
            app.ui_breakpoints.insert(debugger::Breakpoint {
                name: None,
                path: std::path::PathBuf::from("/home/user/project/main.py"),
                line: 10,
            });
            app.ui_breakpoints.insert(debugger::Breakpoint {
                name: None,
                path: std::path::PathBuf::from("/home/user/project/utils.py"),
                line: 25,
            });
        });
        insta::assert_snapshot!("breakpoints_in_running_mode", output);
    }

    #[test]
    fn snapshot_status_bar_with_error() {
        let output = render_to_string(80, 24, |app| {
            app.status_error = Some("Connection refused".to_string());
        });
        insta::assert_snapshot!("status_bar_with_error", output);
    }

    #[test]
    fn snapshot_zen_mode_empty() {
        let output = render_to_string(80, 24, |app| {
            app.zen_mode = true;
        });
        insta::assert_snapshot!("zen_mode_empty", output);
    }

    #[test]
    fn snapshot_zen_mode_with_file() {
        let (_dir, file_path) = write_temp_file("def hello():\n    print('hi')\n\nhello()\n");

        let output = render_to_string(80, 24, |app| {
            app.zen_mode = true;
            app.open_file(file_path);
        });
        insta::assert_snapshot!("zen_mode_with_file", output);
    }
}
