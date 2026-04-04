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
            app.file_picker.open();
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
