use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, BottomTab, Focus, InputMode};
use crate::async_bridge::UiCommand;

/// Central keybinding dispatcher. Routes key events based on input mode and focus.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    match app.input_mode {
        InputMode::FilePicker => handle_file_picker_key(app, key),
        InputMode::Search => handle_search_key(app, key),
        InputMode::Normal => handle_normal_key(app, key),
    }
}

// ── File picker input mode ────────────────────────────────────────────────

fn handle_file_picker_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.file_picker.close();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            if let Some(path) = app.file_picker.select() {
                app.open_file(path);
            }
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Up => {
            app.file_picker.cursor_up();
        }
        KeyCode::Down => {
            app.file_picker.cursor_down();
        }
        KeyCode::Backspace => {
            app.file_picker.query.pop();
            app.file_picker.refilter();
        }
        KeyCode::Char(c) => {
            app.file_picker.query.push(c);
            app.file_picker.refilter();
        }
        _ => {}
    }
}

// ── Search input mode ─────────────────────────────────────────────────────

fn handle_search_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.search.active = false;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            // Commit search and go to normal mode (matches stay visible)
            app.input_mode = InputMode::Normal;
            // Jump to current match
            if let Some(line) = app.search.current_match_line() {
                app.code_view.cursor_line = line;
            }
        }
        KeyCode::Backspace => {
            app.search.query.pop();
            recompute_search(app);
        }
        KeyCode::Char(c) => {
            app.search.query.push(c);
            recompute_search(app);
        }
        _ => {}
    }
}

fn recompute_search(app: &mut App) {
    if let (Some(content), Some(path)) =
        (app.current_file_content(), app.code_view.file_path.clone())
    {
        // We need to work around the borrow checker: read content first, then update search.
        let content = content.to_string();
        app.search.update(&content, &path);
        // Jump cursor to current match
        if let Some(line) = app.search.current_match_line() {
            app.code_view.cursor_line = line;
        }
    }
}

// ── Normal mode ───────────────────────────────────────────────────────────

fn handle_normal_key(app: &mut App, key: KeyEvent) {
    // Global keybindings (always active in normal mode)
    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            return;
        }
        KeyCode::Char('?') => {
            app.show_help = !app.show_help;
            return;
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.file_picker.open();
            app.input_mode = InputMode::FilePicker;
            return;
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.search.active = true;
            app.input_mode = InputMode::Search;
            return;
        }
        KeyCode::Esc => {
            // Close help, dismiss search highlights
            if app.show_help {
                app.show_help = false;
                return;
            }
            if app.search.active {
                app.search.active = false;
                return;
            }
        }
        // Focus cycling
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.focus = app.focus.prev();
            return;
        }
        KeyCode::Tab => {
            app.focus = app.focus.next();
            return;
        }
        KeyCode::BackTab => {
            app.focus = app.focus.prev();
            return;
        }
        // Bottom tab switching
        KeyCode::Char('1') if key.modifiers.contains(KeyModifiers::ALT) => {
            app.bottom_tab = BottomTab::Variables;
            return;
        }
        KeyCode::Char('2') if key.modifiers.contains(KeyModifiers::ALT) => {
            app.bottom_tab = BottomTab::Output;
            return;
        }
        KeyCode::Char('3') if key.modifiers.contains(KeyModifiers::ALT) => {
            app.bottom_tab = BottomTab::Repl;
            return;
        }
        _ => {}
    }

    // Debugger keybindings
    match key.code {
        KeyCode::F(5) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift+F5: terminate / shutdown
            match app.mode {
                AppMode::Running | AppMode::Paused | AppMode::Initialising => {
                    app.stop_session();
                }
                AppMode::Terminated => {
                    app.shutdown_session();
                }
                AppMode::NoSession => {}
            }
            return;
        }
        KeyCode::F(5) => {
            // F5: start session or continue
            match app.mode {
                AppMode::NoSession | AppMode::Terminated => {
                    // Clean up old session first
                    if app.mode == AppMode::Terminated {
                        app.shutdown_session();
                    }
                    app.start_session();
                }
                AppMode::Paused => {
                    if let Some(session) = &app.session {
                        session.bridge.send(UiCommand::Continue);
                    }
                }
                _ => {}
            }
            return;
        }
        KeyCode::F(10) => {
            // F10: step over
            if app.mode == AppMode::Paused {
                if let Some(session) = &app.session {
                    session.bridge.send(UiCommand::StepOver);
                }
            }
            return;
        }
        KeyCode::F(11) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift+F11: step out
            if app.mode == AppMode::Paused {
                if let Some(session) = &app.session {
                    session.bridge.send(UiCommand::StepOut);
                }
            }
            return;
        }
        KeyCode::F(11) => {
            // F11: step in
            if app.mode == AppMode::Paused {
                if let Some(session) = &app.session {
                    session.bridge.send(UiCommand::StepIn);
                }
            }
            return;
        }
        _ => {}
    }

    // Focus-specific keybindings
    match app.focus {
        Focus::CodeView => handle_code_view_key(app, key),
        _ => {} // Other panes get keybindings in later phases
    }
}

fn handle_code_view_key(app: &mut App, key: KeyEvent) {
    match key.code {
        // Navigation
        KeyCode::Char('j') | KeyCode::Down => {
            app.code_view.move_cursor_down(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.code_view.move_cursor_up(1);
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.code_view.move_cursor_down(15); // half-page
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.code_view.move_cursor_up(15); // half-page
        }
        KeyCode::Char('g') => {
            app.code_view.go_to_top();
        }
        KeyCode::Char('G') => {
            app.code_view.go_to_bottom();
        }
        // Search navigation
        KeyCode::Char('n') => {
            app.search.next_match();
            if let Some(line) = app.search.current_match_line() {
                app.code_view.cursor_line = line;
            }
        }
        KeyCode::Char('N') => {
            app.search.prev_match();
            if let Some(line) = app.search.current_match_line() {
                app.code_view.cursor_line = line;
            }
        }
        KeyCode::Char('/') => {
            app.search.active = true;
            app.input_mode = InputMode::Search;
        }
        _ => {}
    }
}
