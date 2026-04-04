use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, BottomTab, Focus, InputMode};
use crate::async_bridge::UiCommand;
use crate::session::DebuggerState;

/// Central keybinding dispatcher. Routes key events based on input mode and focus.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    match app.input_mode {
        InputMode::FilePicker => handle_file_picker_key(app, key),
        InputMode::Search => handle_search_key(app, key),
        InputMode::BreakpointInput => handle_breakpoint_input_key(app, key),
        InputMode::FileBrowser => handle_file_browser_input_key(app, key),
        InputMode::EvaluatePopup => handle_evaluate_popup_key(app, key),
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

// ── Breakpoint input mode ─────────────────────────────────────────────────

fn handle_breakpoint_input_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.breakpoint_input = None;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            if let Some(input) = app.breakpoint_input.take() {
                let input = input.trim().to_string();
                if !input.is_empty() {
                    app.add_breakpoint_from_str(&input);
                }
            }
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            if let Some(ref mut input) = app.breakpoint_input {
                input.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut input) = app.breakpoint_input {
                input.push(c);
            }
        }
        _ => {}
    }
}

// ── File browser input mode (typing in sidebar search) ────────────────────

fn handle_file_browser_input_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.file_browser_query.clear();
            app.refilter_file_browser();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.select_file_browser_item();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Up => {
            app.file_browser_cursor = app.file_browser_cursor.saturating_sub(1);
        }
        KeyCode::Down => {
            if !app.file_browser_results.is_empty() {
                app.file_browser_cursor =
                    (app.file_browser_cursor + 1).min(app.file_browser_results.len() - 1);
            }
        }
        KeyCode::Backspace => {
            app.file_browser_query.pop();
            app.refilter_file_browser();
        }
        KeyCode::Char(c) => {
            app.file_browser_query.push(c);
            app.refilter_file_browser();
        }
        _ => {}
    }
}

// ── Evaluate popup input mode ─────────────────────────────────────────────

fn handle_evaluate_popup_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.close_evaluate_popup();
        }
        KeyCode::Enter => {
            app.evaluate_popup_expression();
        }
        KeyCode::Up => {
            // Navigate shared input history
            app.repl_history_up();
            app.evaluate_input = app.repl_input.clone();
        }
        KeyCode::Down => {
            app.repl_history_down();
            app.evaluate_input = app.repl_input.clone();
        }
        KeyCode::Backspace => {
            app.evaluate_input.pop();
        }
        KeyCode::Char(c) => {
            app.evaluate_input.push(c);
        }
        _ => {}
    }
}

// ── Normal mode ───────────────────────────────────────────────────────────

fn handle_normal_key(app: &mut App, key: KeyEvent) {
    // Global keybindings (always active in normal mode)
    match key.code {
        KeyCode::Char('q') => {
            // Clean quit: shut down session if active, then exit
            if app.session.is_some() {
                app.shutdown_session();
            }
            app.should_quit = true;
            return;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl+C: same as q
            if app.session.is_some() {
                app.shutdown_session();
            }
            app.should_quit = true;
            return;
        }
        KeyCode::Char('?') => {
            app.show_help = !app.show_help;
            return;
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.file_picker.open(&app.debug_root_dir);
            app.input_mode = InputMode::FilePicker;
            return;
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.search.active = true;
            app.input_mode = InputMode::Search;
            return;
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.mode == AppMode::Paused {
                app.open_evaluate_popup();
            }
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

    // Config cycling (h/l when no active session)
    if matches!(app.mode, AppMode::NoSession | AppMode::Terminated) && !app.config_names.is_empty()
    {
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => {
                if app.selected_config_index == 0 {
                    app.selected_config_index = app.config_names.len() - 1;
                } else {
                    app.selected_config_index -= 1;
                }
                return;
            }
            KeyCode::Char('l') | KeyCode::Right => {
                app.selected_config_index =
                    (app.selected_config_index + 1) % app.config_names.len();
                return;
            }
            _ => {}
        }
    }

    // Debugger keybindings
    match key.code {
        // Ctrl+Shift+F5: restart session
        KeyCode::F(5)
            if key
                .modifiers
                .contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT) =>
        {
            app.restart_session();
            return;
        }
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
        Focus::Breakpoints => {
            if app.mode == AppMode::NoSession {
                handle_file_browser_focus_key(app, key);
            } else {
                handle_breakpoints_key(app, key);
            }
        }
        Focus::Variables => handle_variables_key(app, key),
        Focus::CallStack => {
            if app.mode == AppMode::NoSession {
                handle_file_browser_focus_key(app, key);
            } else {
                handle_call_stack_key(app, key);
            }
        }
        Focus::Output => handle_output_key(app, key),
        Focus::Repl => handle_repl_focus_key(app, key),
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
        // Breakpoint toggle
        KeyCode::Char('b') => {
            app.toggle_breakpoint_at_cursor();
        }
        // Visual line selection
        KeyCode::Char('v') => {
            if app.code_view.selection_anchor.is_some() {
                app.code_view.selection_anchor = None;
            } else {
                app.code_view.selection_anchor = Some(app.code_view.cursor_line);
            }
        }
        KeyCode::Esc => {
            if app.code_view.selection_anchor.is_some() {
                app.code_view.selection_anchor = None;
            }
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
        // Inline evaluation
        KeyCode::Char('e') => {
            app.evaluate_inline();
        }
        KeyCode::Char('x') => {
            app.inline_evaluations.clear();
        }
        _ => {}
    }
}

fn handle_breakpoints_key(app: &mut App, key: KeyEvent) {
    match key.code {
        // Navigation
        KeyCode::Char('j') | KeyCode::Down => {
            let count = app.ui_breakpoints.len();
            if count > 0 {
                app.breakpoints_cursor = (app.breakpoints_cursor + 1).min(count - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.breakpoints_cursor = app.breakpoints_cursor.saturating_sub(1);
        }
        // Add breakpoint
        KeyCode::Char('a') => {
            app.breakpoint_input = Some(String::new());
            app.input_mode = InputMode::BreakpointInput;
        }
        // Delete breakpoint
        KeyCode::Char('d') | KeyCode::Delete => {
            if !app.ui_breakpoints.is_empty() {
                app.remove_breakpoint_by_index(app.breakpoints_cursor);
                // Clamp cursor
                let count = app.ui_breakpoints.len();
                if count > 0 {
                    app.breakpoints_cursor = app.breakpoints_cursor.min(count - 1);
                } else {
                    app.breakpoints_cursor = 0;
                }
            }
        }
        // Jump to breakpoint location in code view
        KeyCode::Enter => {
            let mut bps: Vec<_> = app.ui_breakpoints.iter().cloned().collect();
            bps.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
            if let Some(bp) = bps.get(app.breakpoints_cursor) {
                let path = bp.path.clone();
                let line = bp.line;
                app.open_file(path);
                app.code_view.cursor_line = line.saturating_sub(1);
                app.focus = Focus::CodeView;
            }
        }
        _ => {}
    }
}

fn handle_variables_key(app: &mut App, key: KeyEvent) {
    // Build flat variable list for navigation
    let vars = get_flat_variables(app);
    let count = vars.len();
    if count == 0 {
        return;
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.variables_cursor = (app.variables_cursor + 1).min(count - 1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.variables_cursor = app.variables_cursor.saturating_sub(1);
        }
        // Expand/collapse
        KeyCode::Enter | KeyCode::Char('l') => {
            if let Some(var) = vars.get(app.variables_cursor) {
                if let Some(ref_id) = var.variables_reference {
                    if ref_id > 0 {
                        if app.variables_cache.contains_key(&ref_id) {
                            // Toggle collapse by removing from cache
                            app.variables_cache.remove(&ref_id);
                        } else {
                            app.fetch_variables(ref_id);
                        }
                    }
                }
            }
        }
        KeyCode::Char('h') => {
            // Collapse: remove from cache if expanded
            if let Some(var) = vars.get(app.variables_cursor) {
                if let Some(ref_id) = var.variables_reference {
                    if ref_id > 0 {
                        app.variables_cache.remove(&ref_id);
                    }
                }
            }
        }
        // Yank value
        KeyCode::Char('y') => {
            if let Some(var) = vars.get(app.variables_cursor) {
                if let Some(ref value) = var.value {
                    app.yank_variable_value(value);
                    app.status_message = Some("Yanked value (OSC 52)".to_string());
                }
            }
        }
        _ => {}
    }
}

/// Get the flat list of variables currently visible (top-level + expanded children).
fn get_flat_variables(app: &App) -> Vec<dap_types::Variable> {
    let Some(session) = &app.session else {
        return Vec::new();
    };
    let DebuggerState::Paused { paused_frame, .. } = &session.state else {
        return Vec::new();
    };

    let mut flat: Vec<dap_types::Variable> = Vec::new();
    for var in &paused_frame.variables {
        flat.push(var.clone());
        // If expanded, add children
        if let Some(ref_id) = var.variables_reference {
            if ref_id > 0 {
                if let Some(children) = app.variables_cache.get(&ref_id) {
                    for child in children {
                        flat.push(child.clone());
                    }
                }
            }
        }
    }
    flat
}

fn handle_call_stack_key(app: &mut App, key: KeyEvent) {
    let stack_len = app
        .session
        .as_ref()
        .and_then(|s| {
            if let DebuggerState::Paused { ref stack, .. } = s.state {
                Some(stack.len())
            } else {
                None
            }
        })
        .unwrap_or(0);

    if stack_len == 0 {
        return;
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            app.call_stack_cursor = (app.call_stack_cursor + 1).min(stack_len - 1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.call_stack_cursor = app.call_stack_cursor.saturating_sub(1);
        }
        // Change scope to selected frame
        KeyCode::Enter => {
            if let Some(session) = &app.session {
                if let DebuggerState::Paused { ref stack, .. } = session.state {
                    if let Some(frame) = stack.get(app.call_stack_cursor) {
                        let frame_id = frame.id;
                        session.bridge.send(UiCommand::ChangeScope {
                            frame_id,
                            reply: tokio::sync::oneshot::channel().0,
                        });
                    }
                }
            }
        }
        _ => {}
    }
}

fn handle_output_key(app: &mut App, key: KeyEvent) {
    match key.code {
        // Scroll
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.output_auto_scroll {
                app.output_scroll_offset = app.output_scroll_offset.saturating_add(1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if !app.output_auto_scroll {
                app.output_scroll_offset = app.output_scroll_offset.saturating_sub(1);
            }
        }
        // Toggle auto-scroll
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.output_auto_scroll = !app.output_auto_scroll;
            if app.output_auto_scroll {
                app.status_message = Some("Output auto-scroll ON".to_string());
            } else {
                app.status_message = Some("Output auto-scroll OFF".to_string());
            }
        }
        _ => {}
    }
}

fn handle_repl_focus_key(app: &mut App, key: KeyEvent) {
    let is_paused = app.mode == AppMode::Paused;

    match key.code {
        // Evaluate expression
        KeyCode::Enter => {
            if is_paused {
                app.evaluate_repl();
            }
        }
        // History navigation
        KeyCode::Up => {
            app.repl_history_up();
        }
        KeyCode::Down => {
            app.repl_history_down();
        }
        // Editing
        KeyCode::Backspace => {
            app.repl_input.pop();
        }
        // Type directly into the REPL input
        KeyCode::Char(c) => {
            // Don't capture global shortcuts that use modifiers
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                app.repl_input.push(c);
            }
        }
        _ => {}
    }
}

fn handle_file_browser_focus_key(app: &mut App, key: KeyEvent) {
    match key.code {
        // Start typing to filter
        KeyCode::Char('/') | KeyCode::Char('i') => {
            app.input_mode = InputMode::FileBrowser;
        }
        // Navigation
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.file_browser_results.is_empty() {
                app.file_browser_cursor =
                    (app.file_browser_cursor + 1).min(app.file_browser_results.len() - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.file_browser_cursor = app.file_browser_cursor.saturating_sub(1);
        }
        // Select file
        KeyCode::Enter => {
            app.select_file_browser_item();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_helpers::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// Helper: create a KeyEvent with no modifiers.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Helper: create a KeyEvent with modifiers.
    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    // ── Global keys (normal mode) ─────────────────────────────────────

    #[test]
    fn q_sets_should_quit() {
        with_test_app(|app| {
            handle_key(app, key(KeyCode::Char('q')));
            assert!(app.should_quit);
        });
    }

    #[test]
    fn ctrl_c_sets_should_quit() {
        with_test_app(|app| {
            handle_key(app, key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL));
            assert!(app.should_quit);
        });
    }

    #[test]
    fn question_mark_toggles_help() {
        with_test_app(|app| {
            assert!(!app.show_help);
            handle_key(app, key(KeyCode::Char('?')));
            assert!(app.show_help);
            handle_key(app, key(KeyCode::Char('?')));
            assert!(!app.show_help);
        });
    }

    #[test]
    fn esc_dismisses_help() {
        with_test_app(|app| {
            app.show_help = true;
            handle_key(app, key(KeyCode::Esc));
            assert!(!app.show_help);
        });
    }

    #[test]
    fn esc_dismisses_search() {
        with_test_app(|app| {
            app.search.active = true;
            handle_key(app, key(KeyCode::Esc));
            assert!(!app.search.active);
        });
    }

    // ── Focus cycling ─────────────────────────────────────────────────

    #[test]
    fn tab_cycles_focus_forward() {
        with_test_app(|app| {
            assert_eq!(app.focus, Focus::CodeView);
            handle_key(app, key(KeyCode::Tab));
            assert_eq!(app.focus, Focus::CallStack);
            handle_key(app, key(KeyCode::Tab));
            assert_eq!(app.focus, Focus::Breakpoints);
        });
    }

    #[test]
    fn backtab_cycles_focus_backward() {
        with_test_app(|app| {
            assert_eq!(app.focus, Focus::CodeView);
            handle_key(app, key(KeyCode::BackTab));
            assert_eq!(app.focus, Focus::Repl);
        });
    }

    #[test]
    fn shift_tab_cycles_focus_backward() {
        with_test_app(|app| {
            assert_eq!(app.focus, Focus::CodeView);
            handle_key(app, key_mod(KeyCode::Tab, KeyModifiers::SHIFT));
            assert_eq!(app.focus, Focus::Repl);
        });
    }

    // ── Bottom tab switching ──────────────────────────────────────────

    #[test]
    fn alt_1_switches_to_variables_tab() {
        with_test_app(|app| {
            app.bottom_tab = BottomTab::Output;
            handle_key(app, key_mod(KeyCode::Char('1'), KeyModifiers::ALT));
            assert_eq!(app.bottom_tab, BottomTab::Variables);
        });
    }

    #[test]
    fn alt_2_switches_to_output_tab() {
        with_test_app(|app| {
            handle_key(app, key_mod(KeyCode::Char('2'), KeyModifiers::ALT));
            assert_eq!(app.bottom_tab, BottomTab::Output);
        });
    }

    #[test]
    fn alt_3_switches_to_repl_tab() {
        with_test_app(|app| {
            handle_key(app, key_mod(KeyCode::Char('3'), KeyModifiers::ALT));
            assert_eq!(app.bottom_tab, BottomTab::Repl);
        });
    }

    // ── Ctrl+P opens file picker ──────────────────────────────────────

    #[test]
    fn ctrl_p_opens_file_picker() {
        with_test_app(|app| {
            handle_key(app, key_mod(KeyCode::Char('p'), KeyModifiers::CONTROL));
            assert_eq!(app.input_mode, InputMode::FilePicker);
            assert!(app.file_picker.open);
        });
    }

    // ── Ctrl+F opens search ───────────────────────────────────────────

    #[test]
    fn ctrl_f_opens_search() {
        with_test_app(|app| {
            handle_key(app, key_mod(KeyCode::Char('f'), KeyModifiers::CONTROL));
            assert_eq!(app.input_mode, InputMode::Search);
            assert!(app.search.active);
        });
    }

    // ── Config cycling ────────────────────────────────────────────────

    #[test]
    fn h_l_cycle_configs_in_no_session_mode() {
        with_test_app_configs(vec!["cfg0".into(), "cfg1".into(), "cfg2".into()], |app| {
            assert_eq!(app.selected_config_index, 0);

            // l (or Right) cycles forward
            handle_key(app, key(KeyCode::Char('l')));
            assert_eq!(app.selected_config_index, 1);

            handle_key(app, key(KeyCode::Char('l')));
            assert_eq!(app.selected_config_index, 2);

            // Wraps around
            handle_key(app, key(KeyCode::Char('l')));
            assert_eq!(app.selected_config_index, 0);

            // h (or Left) cycles backward
            handle_key(app, key(KeyCode::Char('h')));
            assert_eq!(app.selected_config_index, 2); // wraps to end

            handle_key(app, key(KeyCode::Char('h')));
            assert_eq!(app.selected_config_index, 1);
        });
    }

    #[test]
    fn config_cycling_disabled_when_running() {
        with_test_app_configs(vec!["cfg0".into(), "cfg1".into()], |app| {
            app.mode = AppMode::Running;
            handle_key(app, key(KeyCode::Char('l')));
            assert_eq!(app.selected_config_index, 0); // unchanged
        });
    }

    // ── Code view navigation ──────────────────────────────────────────

    #[test]
    fn j_k_move_cursor_in_code_view() {
        let (_dir, file_path) = write_temp_file("l1\nl2\nl3\nl4\nl5\n");

        with_test_app(|app| {
            app.open_file(file_path);
            assert_eq!(app.code_view.cursor_line, 0);

            handle_key(app, key(KeyCode::Char('j')));
            assert_eq!(app.code_view.cursor_line, 1);

            handle_key(app, key(KeyCode::Char('j')));
            assert_eq!(app.code_view.cursor_line, 2);

            handle_key(app, key(KeyCode::Char('k')));
            assert_eq!(app.code_view.cursor_line, 1);
        });
    }

    #[test]
    fn g_goes_to_top_and_shift_g_goes_to_bottom() {
        let (_dir, file_path) = write_temp_file("l1\nl2\nl3\nl4\nl5\n");

        with_test_app(|app| {
            app.open_file(file_path);
            app.code_view.cursor_line = 2;

            handle_key(app, key(KeyCode::Char('G')));
            assert_eq!(app.code_view.cursor_line, 4); // 5 lines, 0-indexed

            handle_key(app, key(KeyCode::Char('g')));
            assert_eq!(app.code_view.cursor_line, 0);
        });
    }

    #[test]
    fn ctrl_d_and_ctrl_u_half_page_scroll() {
        let content = (0..50)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (_dir, file_path) = write_temp_file(&content);

        with_test_app(|app| {
            app.open_file(file_path);

            handle_key(app, key_mod(KeyCode::Char('d'), KeyModifiers::CONTROL));
            assert_eq!(app.code_view.cursor_line, 15);

            handle_key(app, key_mod(KeyCode::Char('u'), KeyModifiers::CONTROL));
            assert_eq!(app.code_view.cursor_line, 0);
        });
    }

    // ── Breakpoint toggle via 'b' ─────────────────────────────────────

    #[test]
    fn b_toggles_breakpoint_in_code_view() {
        let (_dir, file_path) = write_temp_file("line1\nline2\nline3\n");

        with_test_app(|app| {
            app.open_file(file_path);
            app.code_view.cursor_line = 1;

            handle_key(app, key(KeyCode::Char('b')));
            assert_eq!(app.ui_breakpoints.len(), 1);

            handle_key(app, key(KeyCode::Char('b')));
            assert!(app.ui_breakpoints.is_empty());
        });
    }

    // ── Visual selection via 'v' ──────────────────────────────────────

    #[test]
    fn v_toggles_visual_selection() {
        let (_dir, file_path) = write_temp_file("line1\nline2\nline3\n");

        with_test_app(|app| {
            app.open_file(file_path);
            app.code_view.cursor_line = 1;

            // Start selection
            handle_key(app, key(KeyCode::Char('v')));
            assert_eq!(app.code_view.selection_anchor, Some(1));

            // Cancel selection
            handle_key(app, key(KeyCode::Char('v')));
            assert_eq!(app.code_view.selection_anchor, None);
        });
    }

    #[test]
    fn esc_cancels_visual_selection_in_code_view() {
        let (_dir, file_path) = write_temp_file("line1\nline2\n");

        with_test_app(|app| {
            app.open_file(file_path);
            app.code_view.selection_anchor = Some(0);

            // Focus must be CodeView for this handler
            app.focus = Focus::CodeView;
            handle_key(app, key(KeyCode::Esc));
            assert_eq!(app.code_view.selection_anchor, None);
        });
    }

    // ── Slash enters search mode ──────────────────────────────────────

    #[test]
    fn slash_enters_search_mode_from_code_view() {
        with_test_app(|app| {
            app.focus = Focus::CodeView;
            handle_key(app, key(KeyCode::Char('/')));
            assert_eq!(app.input_mode, InputMode::Search);
            assert!(app.search.active);
        });
    }

    // ── Search input mode ─────────────────────────────────────────────

    #[test]
    fn search_mode_typing_and_esc() {
        let (_dir, file_path) = write_temp_file("hello world\nfoo bar\nhello again\n");

        with_test_app(|app| {
            app.open_file(file_path);
            app.input_mode = InputMode::Search;
            app.search.active = true;

            // Type "hello"
            for c in "hello".chars() {
                handle_key(app, key(KeyCode::Char(c)));
            }
            assert_eq!(app.search.query, "hello");

            // Backspace
            handle_key(app, key(KeyCode::Backspace));
            assert_eq!(app.search.query, "hell");

            // Esc exits search mode
            handle_key(app, key(KeyCode::Esc));
            assert_eq!(app.input_mode, InputMode::Normal);
            assert!(!app.search.active);
        });
    }

    #[test]
    fn search_enter_commits_and_returns_to_normal() {
        let (_dir, file_path) = write_temp_file("hello world\nfoo bar\nhello again\n");

        with_test_app(|app| {
            app.open_file(file_path);
            app.input_mode = InputMode::Search;
            app.search.active = true;

            for c in "hello".chars() {
                handle_key(app, key(KeyCode::Char(c)));
            }

            handle_key(app, key(KeyCode::Enter));
            assert_eq!(app.input_mode, InputMode::Normal);
            // Search stays active (matches remain visible)
        });
    }

    // ── File picker input mode ────────────────────────────────────────

    #[test]
    fn file_picker_esc_closes() {
        with_test_app(|app| {
            app.file_picker.open = true;
            app.input_mode = InputMode::FilePicker;

            handle_key(app, key(KeyCode::Esc));
            assert_eq!(app.input_mode, InputMode::Normal);
            assert!(!app.file_picker.open);
        });
    }

    #[test]
    fn file_picker_typing_updates_query() {
        with_test_app(|app| {
            app.input_mode = InputMode::FilePicker;
            app.file_picker.open = true;

            handle_key(app, key(KeyCode::Char('t')));
            handle_key(app, key(KeyCode::Char('e')));
            assert_eq!(app.file_picker.query, "te");

            handle_key(app, key(KeyCode::Backspace));
            assert_eq!(app.file_picker.query, "t");
        });
    }

    // ── Breakpoint input mode ─────────────────────────────────────────

    #[test]
    fn breakpoint_input_typing_and_esc() {
        with_test_app(|app| {
            app.input_mode = InputMode::BreakpointInput;
            app.breakpoint_input = Some(String::new());

            handle_key(app, key(KeyCode::Char('f')));
            handle_key(app, key(KeyCode::Char('o')));
            assert_eq!(app.breakpoint_input.as_deref(), Some("fo"));

            // Esc cancels
            handle_key(app, key(KeyCode::Esc));
            assert!(app.breakpoint_input.is_none());
            assert_eq!(app.input_mode, InputMode::Normal);
        });
    }

    #[test]
    fn breakpoint_input_enter_submits() {
        with_test_app(|app| {
            app.input_mode = InputMode::BreakpointInput;
            app.breakpoint_input = Some("/tmp/test.py:10".to_string());

            handle_key(app, key(KeyCode::Enter));

            assert_eq!(app.input_mode, InputMode::Normal);
            assert!(app.breakpoint_input.is_none());
            assert_eq!(app.ui_breakpoints.len(), 1);
        });
    }

    // ── Breakpoint panel keys ─────────────────────────────────────────

    #[test]
    fn breakpoints_panel_j_k_navigate() {
        with_test_app(|app| {
            // Must have an active session mode for breakpoint panel focus
            app.mode = AppMode::Running;
            app.focus = Focus::Breakpoints;

            // Add breakpoints
            app.ui_breakpoints.insert(debugger::Breakpoint {
                name: None,
                path: std::path::PathBuf::from("/a.py"),
                line: 1,
            });
            app.ui_breakpoints.insert(debugger::Breakpoint {
                name: None,
                path: std::path::PathBuf::from("/b.py"),
                line: 2,
            });

            assert_eq!(app.breakpoints_cursor, 0);
            handle_key(app, key(KeyCode::Char('j')));
            assert_eq!(app.breakpoints_cursor, 1);

            // Clamps at end
            handle_key(app, key(KeyCode::Char('j')));
            assert_eq!(app.breakpoints_cursor, 1);

            handle_key(app, key(KeyCode::Char('k')));
            assert_eq!(app.breakpoints_cursor, 0);
        });
    }

    #[test]
    fn breakpoints_panel_a_opens_input_mode() {
        with_test_app(|app| {
            app.mode = AppMode::Running;
            app.focus = Focus::Breakpoints;

            handle_key(app, key(KeyCode::Char('a')));
            assert_eq!(app.input_mode, InputMode::BreakpointInput);
            assert_eq!(app.breakpoint_input, Some(String::new()));
        });
    }

    #[test]
    fn breakpoints_panel_d_deletes() {
        with_test_app(|app| {
            app.mode = AppMode::Running;
            app.focus = Focus::Breakpoints;

            app.ui_breakpoints.insert(debugger::Breakpoint {
                name: None,
                path: std::path::PathBuf::from("/a.py"),
                line: 1,
            });

            handle_key(app, key(KeyCode::Char('d')));
            assert!(app.ui_breakpoints.is_empty());
        });
    }

    // ── REPL focus keys ───────────────────────────────────────────────

    #[test]
    fn repl_focus_typing_appends_to_input() {
        with_test_app(|app| {
            app.focus = Focus::Repl;

            handle_key(app, key(KeyCode::Char('h')));
            handle_key(app, key(KeyCode::Char('i')));
            assert_eq!(app.repl_input, "hi");

            handle_key(app, key(KeyCode::Backspace));
            assert_eq!(app.repl_input, "h");
        });
    }

    #[test]
    fn repl_focus_up_down_navigate_history() {
        with_test_app(|app| {
            app.focus = Focus::Repl;
            app.repl_input_history = vec!["old1".to_string(), "old2".to_string()];

            handle_key(app, key(KeyCode::Up));
            assert_eq!(app.repl_input, "old2");

            handle_key(app, key(KeyCode::Up));
            assert_eq!(app.repl_input, "old1");

            handle_key(app, key(KeyCode::Down));
            assert_eq!(app.repl_input, "old2");
        });
    }

    // ── Output focus keys ─────────────────────────────────────────────

    #[test]
    fn output_scroll_when_not_auto() {
        with_test_app(|app| {
            app.focus = Focus::Output;
            app.output_auto_scroll = false;

            handle_key(app, key(KeyCode::Char('j')));
            assert_eq!(app.output_scroll_offset, 1);

            handle_key(app, key(KeyCode::Char('j')));
            assert_eq!(app.output_scroll_offset, 2);

            handle_key(app, key(KeyCode::Char('k')));
            assert_eq!(app.output_scroll_offset, 1);
        });
    }

    #[test]
    fn output_ctrl_s_toggles_auto_scroll() {
        with_test_app(|app| {
            app.focus = Focus::Output;
            assert!(app.output_auto_scroll);

            handle_key(app, key_mod(KeyCode::Char('s'), KeyModifiers::CONTROL));
            assert!(!app.output_auto_scroll);

            handle_key(app, key_mod(KeyCode::Char('s'), KeyModifiers::CONTROL));
            assert!(app.output_auto_scroll);
        });
    }

    // ── Evaluate popup input mode ─────────────────────────────────────

    #[test]
    fn evaluate_popup_typing_and_esc() {
        with_test_app(|app| {
            app.input_mode = InputMode::EvaluatePopup;
            app.evaluate_popup_open = true;

            handle_key(app, key(KeyCode::Char('x')));
            handle_key(app, key(KeyCode::Char('y')));
            assert_eq!(app.evaluate_input, "xy");

            handle_key(app, key(KeyCode::Backspace));
            assert_eq!(app.evaluate_input, "x");

            // Esc closes
            handle_key(app, key(KeyCode::Esc));
            assert!(!app.evaluate_popup_open);
            assert_eq!(app.input_mode, InputMode::Normal);
        });
    }

    // ── Inline evaluation 'x' clears ──────────────────────────────────

    #[test]
    fn x_clears_inline_evaluations() {
        with_test_app(|app| {
            app.focus = Focus::CodeView;
            app.inline_evaluations.insert(0, "= 42".to_string());
            app.inline_evaluations.insert(1, "= 99".to_string());

            handle_key(app, key(KeyCode::Char('x')));
            assert!(app.inline_evaluations.is_empty());
        });
    }

    // ── F5 in NoSession with no configs ───────────────────────────────

    #[test]
    fn f5_in_no_session_tries_start_session() {
        with_test_app(|app| {
            assert_eq!(app.mode, AppMode::NoSession);
            handle_key(app, key(KeyCode::F(5)));
            // No configs -> error, mode stays NoSession
            assert_eq!(app.mode, AppMode::NoSession);
            assert!(app.status_error.is_some());
        });
    }
}
