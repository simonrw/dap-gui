use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use dap_types::{StackFrame, Variable};
use debugger::{EvaluateResult, PausedFrame};
use eframe::egui::{self, Context, Key, Modifiers, Ui};

use crate::{
    DebuggerAppState, Session, State, TabState,
    code_view::CodeView,
    ui::{
        breakpoints::Breakpoints,
        call_stack::CallStack,
        file_picker::{self, FilePickerResult},
        status_bar::StatusBar,
    },
};

use config::keybindings::KeyName;

/// ID for the search input field so we can request focus
const SEARCH_INPUT_ID: &str = "code_view_search_input";

/// All keys that can appear in a configurable debug action binding.
///
/// This list must cover every key that [`egui_to_key_name`] can translate,
/// so that user-configured bindings to any supported key work in the GUI.
const DEBUG_POLL_KEYS: [Key; 63] = [
    // Function keys
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
    // Letters
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::G,
    Key::H,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::Q,
    Key::R,
    Key::S,
    Key::T,
    Key::U,
    Key::V,
    Key::W,
    Key::X,
    Key::Y,
    Key::Z,
    // Digits
    Key::Num0,
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
    // Navigation
    Key::ArrowUp,
    Key::ArrowDown,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::Home,
    Key::End,
    Key::PageUp,
    Key::PageDown,
    // Editing / special
    Key::Enter,
    Key::Escape,
    Key::Backspace,
    Key::Delete,
    Key::Insert,
    Key::Tab,
    Key::Space,
];

fn egui_to_key_name(key: Key) -> Option<KeyName> {
    match key {
        Key::F1 => Some(KeyName::F1),
        Key::F2 => Some(KeyName::F2),
        Key::F3 => Some(KeyName::F3),
        Key::F4 => Some(KeyName::F4),
        Key::F5 => Some(KeyName::F5),
        Key::F6 => Some(KeyName::F6),
        Key::F7 => Some(KeyName::F7),
        Key::F8 => Some(KeyName::F8),
        Key::F9 => Some(KeyName::F9),
        Key::F10 => Some(KeyName::F10),
        Key::F11 => Some(KeyName::F11),
        Key::F12 => Some(KeyName::F12),
        Key::A => Some(KeyName::A),
        Key::B => Some(KeyName::B),
        Key::C => Some(KeyName::C),
        Key::D => Some(KeyName::D),
        Key::E => Some(KeyName::E),
        Key::F => Some(KeyName::F),
        Key::G => Some(KeyName::G),
        Key::H => Some(KeyName::H),
        Key::I => Some(KeyName::I),
        Key::J => Some(KeyName::J),
        Key::K => Some(KeyName::K),
        Key::L => Some(KeyName::L),
        Key::M => Some(KeyName::M),
        Key::N => Some(KeyName::N),
        Key::O => Some(KeyName::O),
        Key::P => Some(KeyName::P),
        Key::Q => Some(KeyName::Q),
        Key::R => Some(KeyName::R),
        Key::S => Some(KeyName::S),
        Key::T => Some(KeyName::T),
        Key::U => Some(KeyName::U),
        Key::V => Some(KeyName::V),
        Key::W => Some(KeyName::W),
        Key::X => Some(KeyName::X),
        Key::Y => Some(KeyName::Y),
        Key::Z => Some(KeyName::Z),
        Key::Num0 => Some(KeyName::Digit0),
        Key::Num1 => Some(KeyName::Digit1),
        Key::Num2 => Some(KeyName::Digit2),
        Key::Num3 => Some(KeyName::Digit3),
        Key::Num4 => Some(KeyName::Digit4),
        Key::Num5 => Some(KeyName::Digit5),
        Key::Num6 => Some(KeyName::Digit6),
        Key::Num7 => Some(KeyName::Digit7),
        Key::Num8 => Some(KeyName::Digit8),
        Key::Num9 => Some(KeyName::Digit9),
        Key::ArrowUp => Some(KeyName::Up),
        Key::ArrowDown => Some(KeyName::Down),
        Key::ArrowLeft => Some(KeyName::Left),
        Key::ArrowRight => Some(KeyName::Right),
        Key::Home => Some(KeyName::Home),
        Key::End => Some(KeyName::End),
        Key::PageUp => Some(KeyName::PageUp),
        Key::PageDown => Some(KeyName::PageDown),
        Key::Enter => Some(KeyName::Enter),
        Key::Escape => Some(KeyName::Escape),
        Key::Backspace => Some(KeyName::Backspace),
        Key::Delete => Some(KeyName::Delete),
        Key::Insert => Some(KeyName::Insert),
        Key::Tab => Some(KeyName::Tab),
        Key::Space => Some(KeyName::Space),
        _ => None,
    }
}

pub(crate) struct Renderer<'a> {
    state: &'a mut DebuggerAppState,
    app_state: &'a Arc<Mutex<DebuggerAppState>>,
    egui_ctx: &'a Context,
}

impl<'s> Renderer<'s> {
    pub(crate) fn new(
        state: &'s mut DebuggerAppState,
        app_state: &'s Arc<Mutex<DebuggerAppState>>,
        egui_ctx: &'s Context,
    ) -> Self {
        Self {
            state,
            app_state,
            egui_ctx,
        }
    }

    pub(crate) fn render_ui(&mut self, ctx: &Context) {
        // Handle Ctrl/Cmd+= to increase code font size
        if ctx.input(|i| i.key_pressed(Key::Equals) && i.modifiers.command) {
            self.state.code_font_size = (self.state.code_font_size + 1.0).min(32.0);
            if let Err(e) = self
                .state
                .state_manager
                .set_code_font_size(self.state.code_font_size)
            {
                tracing::warn!(error = %e, "failed to persist font size");
            }
        }
        // Handle Ctrl/Cmd+- to decrease code font size
        if ctx.input(|i| i.key_pressed(Key::Minus) && i.modifiers.command) {
            self.state.code_font_size = (self.state.code_font_size - 1.0).max(8.0);
            if let Err(e) = self
                .state
                .state_manager
                .set_code_font_size(self.state.code_font_size)
            {
                tracing::warn!(error = %e, "failed to persist font size");
            }
        }

        // Handle Ctrl+P to toggle file picker
        if ctx.input(|i| i.key_pressed(Key::P) && i.modifiers.matches_exact(Modifiers::CTRL)) {
            if self.state.file_picker.open {
                self.state.file_picker.close();
            } else {
                self.state
                    .file_picker
                    .open(&self.state.debug_root_dir.clone());
            }
        }

        // Configurable debug keybindings
        let debug_action = ctx.input(|i| {
            for key in DEBUG_POLL_KEYS {
                if i.key_pressed(key) {
                    if let Some(kn) = egui_to_key_name(key) {
                        let m = i.modifiers;
                        if let Some(action) = self.state.keybindings.match_action(
                            kn,
                            m.shift,
                            m.ctrl || m.command,
                            m.alt,
                        ) {
                            return Some(action);
                        }
                    }
                }
            }
            None
        });
        if let Some(action) = debug_action {
            use config::keybindings::DebugAction;
            match action {
                DebugAction::ContinueOrStart => self.handle_f5(),
                DebugAction::Stop => {
                    if let Some(session) = self.state.session.take() {
                        session
                            .bridge
                            .send(crate::async_bridge::UiCommand::Terminate);
                        self.state.variables_cache.clear();
                        *self.state.repl_output.borrow_mut() = String::new();
                    }
                }
                DebugAction::Restart => {
                    // Stop then start
                    if self.state.session.is_some() {
                        self.state.session.take();
                    }
                    self.start_session();
                }
                DebugAction::StepOver => {
                    if let Some(session) = &self.state.session {
                        session
                            .bridge
                            .send(crate::async_bridge::UiCommand::StepOver);
                    }
                }
                DebugAction::StepInto => {
                    if let Some(session) = &self.state.session {
                        session.bridge.send(crate::async_bridge::UiCommand::StepIn);
                    }
                }
                DebugAction::StepOut => {
                    if let Some(session) = &self.state.session {
                        session.bridge.send(crate::async_bridge::UiCommand::StepOut);
                    }
                }
            }
        }

        // Handle Cmd+F / Ctrl+F to toggle search
        let find_shortcut = ctx.input(|i| {
            i.key_pressed(Key::F)
                && (i.modifiers.matches_exact(Modifiers::COMMAND)
                    || i.modifiers.matches_exact(Modifiers::CTRL))
        });
        if find_shortcut {
            let search = &mut self.state.search_state;
            if search.active {
                // Re-focus if already open
                search.request_focus = true;
            } else {
                search.active = true;
                search.request_focus = true;
            }
        }

        // Render file picker overlay if open
        if self.state.file_picker.open {
            if let FilePickerResult::Selected(path) = file_picker::show(ctx, self.state) {
                self.state.file_override = Some(std::fs::canonicalize(&path).unwrap_or(path));
            }
        }

        let has_session = self.state.session.is_some();
        if !has_session {
            self.render_no_session(ctx);
            return;
        }

        let current_state = self.state.session.as_ref().unwrap().state.clone();
        match current_state {
            State::Running => {
                if let Some(State::Paused {
                    stack,
                    paused_frame,
                    ..
                }) = self.state.session.as_ref().unwrap().previous_state.clone()
                {
                    self.render_paused_or_running_ui(ctx, &stack, &paused_frame, false);
                } else {
                    self.render_controls_window(ctx);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.label("Program running...");
                        });
                    });
                }
            }
            State::Paused {
                stack,
                paused_frame,
                ..
            } => {
                self.render_paused_or_running_ui(ctx, &stack, &paused_frame, true);
            }
            State::Terminated => {
                self.render_controls_window(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label("Program terminated");
                        ui.add_space(10.0);
                        if ui.button("⟳ Restart").clicked() {
                            self.start_session();
                        }
                        ui.label(format!(
                            "or press {}",
                            self.state
                                .keybindings
                                .label(config::keybindings::DebugAction::ContinueOrStart)
                                .unwrap_or("?")
                        ));
                    });
                });
            }
        }
    }

    fn handle_f5(&mut self) {
        if let Some(session) = &self.state.session {
            match &session.state {
                State::Paused { .. } => {
                    session
                        .bridge
                        .send(crate::async_bridge::UiCommand::Continue);
                }
                State::Terminated => {
                    self.start_session();
                }
                _ => {} // Running or Initialising: no-op
            }
        } else {
            self.start_session();
        }
    }

    fn start_session(&mut self) {
        // Gracefully terminate existing session first
        if let Some(session) = self.state.session.take() {
            session
                .bridge
                .send(crate::async_bridge::UiCommand::Terminate);
        }
        self.state.variables_cache.clear();
        *self.state.repl_output.borrow_mut() = String::new();

        let persisted_bps = self.state.collect_persisted_breakpoints();
        let mut all_bps: Vec<_> = self.state.ui_breakpoints.iter().cloned().collect();
        all_bps.extend(persisted_bps);
        self.state.ui_breakpoints = all_bps.iter().cloned().collect();

        let config = self.state.configs[self.state.selected_config_index].clone();
        let app_state_clone = Arc::clone(self.app_state);
        match Session::start(
            &config,
            &all_bps,
            &mut self.state.debug_root_dir,
            self.egui_ctx,
            app_state_clone,
        ) {
            Ok(session) => {
                self.state.session = Some(session);
            }
            Err(e) => {
                self.state
                    .status
                    .push_error(format!("Failed to start session: {e}"));
            }
        }
    }

    fn render_no_session(&mut self, ctx: &Context) {
        // Update debug_root_dir from the selected config's cwd
        let config = &self.state.configs[self.state.selected_config_index];
        if let Some(cwd) = config.cwd() {
            let normalised = debugger::utils::normalise_path(cwd);
            let resolved = std::fs::canonicalize(normalised.as_ref())
                .unwrap_or_else(|_| normalised.into_owned());
            if resolved != self.state.debug_root_dir {
                self.state.debug_root_dir = resolved;
                self.state.file_picker.git_files_loaded = false;
                self.state.file_override = None;
                self.state.file_cache.clear();
            }
        }

        // Lazy-load git files for the file picker
        self.state
            .file_picker
            .ensure_loaded(&self.state.debug_root_dir);

        // Handle keyboard for file picker navigation
        ctx.input(|i| {
            if i.key_pressed(Key::ArrowDown) {
                self.state.file_picker.cursor_down();
            }
            if i.key_pressed(Key::ArrowUp) {
                self.state.file_picker.cursor_up();
            }
            if i.key_pressed(Key::Enter) {
                if let Some(path) = self.state.file_picker.select() {
                    self.state.file_override = Some(path);
                }
            }
        });

        egui::TopBottomPanel::top("control_panel").show(ctx, |ui| {
            ui.heading("DAP Debugger");
            ui.separator();
            ui.horizontal(|ui| {
                self.render_config_selector(ui);
                if ui.button("▶ Start").clicked() {
                    self.start_session();
                }
            });
        });
        egui::TopBottomPanel::bottom("status-bar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.add(StatusBar::new("Ready", &mut self.state.status));
            });

        // File browser sidebar
        egui::SidePanel::left("browse-file-picker")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Files");
                ui.separator();

                let _input_response = ui.add(
                    egui::TextEdit::singleline(&mut self.state.file_picker.query)
                        .hint_text("Search files...")
                        .desired_width(f32::INFINITY),
                );

                self.state.file_picker.refilter();

                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, m) in self.state.file_picker.results.iter().enumerate() {
                        let is_selected = i == self.state.file_picker.cursor;
                        let path_str = m.file.relative_path.to_string_lossy();

                        let mut job = eframe::epaint::text::LayoutJob::default();
                        let base_color = if is_selected {
                            egui::Color32::WHITE
                        } else {
                            ui.visuals().text_color()
                        };
                        let match_color = egui::Color32::from_rgb(255, 200, 50);

                        for (ci, ch) in path_str.char_indices() {
                            let color = if m.matched_indices.contains(&ci) {
                                match_color
                            } else {
                                base_color
                            };
                            let mut buf = [0u8; 4];
                            job.append(
                                ch.encode_utf8(&mut buf),
                                0.0,
                                egui::TextFormat {
                                    color,
                                    ..Default::default()
                                },
                            );
                        }

                        let bg = if is_selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            egui::Color32::TRANSPARENT
                        };

                        let frame = egui::Frame::new().fill(bg).inner_margin(4.0);
                        let response = frame
                            .show(ui, |ui| {
                                ui.label(job);
                            })
                            .response;

                        if response.clicked() {
                            self.state.file_override = Some(
                                std::fs::canonicalize(&m.file.absolute_path)
                                    .unwrap_or_else(|_| m.file.absolute_path.clone()),
                            );
                            self.state.file_picker.cursor = i;
                        }
                    }
                });
            });

        // Central panel: code viewer or welcome
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(ref display_path) = self.state.file_override else {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.heading("Select a configuration and press Start or F5");
                    ui.add_space(10.0);
                    ui.label("Browse files in the sidebar to set breakpoints");
                });
                return;
            };
            let display_path = display_path.clone();

            let display_name = display_path
                .strip_prefix(&self.state.debug_root_dir)
                .unwrap_or(&display_path)
                .to_string_lossy()
                .to_string();
            ui.label(&display_name);
            ui.separator();

            // Always-visible search bar for browsing
            let search = &mut self.state.search_state;
            if !search.active {
                search.active = true;
            }
            self.render_search_bar(ui);

            let contents = self
                .state
                .file_cache
                .get_or_load(&display_path)
                .unwrap_or("")
                .to_string();

            // Update search matches if query or file changed
            if self.state.search_state.active {
                self.state.search_state.update(&contents, &display_path);
            }

            let mut file_breakpoints: HashSet<_> = self
                .state
                .ui_breakpoints
                .iter()
                .filter(|b| b.path == display_path.as_path())
                .cloned()
                .collect();

            let breakpoints_before = file_breakpoints.clone();
            let is_dark = ui.visuals().dark_mode;
            let jump = false;

            let scroll_to_line = if self.state.search_state.scroll_to_match {
                self.state.search_state.scroll_to_match = false;
                self.state.search_state.current_match_line().map(|l| l + 1)
            } else {
                None
            };

            let search_matches = if self.state.search_state.active {
                &self.state.search_state.matches
            } else {
                &[][..]
            };
            let current_search_match = self.state.search_state.current_match;

            ui.add(CodeView::new(
                &contents,
                1,
                false,
                &mut file_breakpoints,
                &jump,
                display_path,
                is_dark,
                self.state.code_font_size,
                search_matches,
                current_search_match,
                scroll_to_line,
            ));

            for added in file_breakpoints.difference(&breakpoints_before) {
                self.state.ui_breakpoints.insert(added.clone());
            }
            for removed in breakpoints_before.difference(&file_breakpoints) {
                self.state.ui_breakpoints.remove(removed);
            }

            if file_breakpoints != breakpoints_before {
                self.state.persist_breakpoints();
            }
        });
    }

    /// Render both the paused and running UIs
    ///
    /// The only difference is that the running UI hides
    /// * the variables
    /// * the breakpoints
    /// * the call stack
    pub fn render_paused_or_running_ui(
        &mut self,
        ctx: &Context,
        stack: &[StackFrame],
        paused_frame: &PausedFrame,
        show_details: bool,
    ) {
        egui::SidePanel::left("left-panel").show(ctx, |ui| {
            self.render_sidepanel(ctx, ui, stack, show_details);
        });
        egui::TopBottomPanel::bottom("status-bar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                let state_label = if show_details { "Paused" } else { "Running" };
                ui.add(StatusBar::new(state_label, &mut self.state.status));
            });
        egui::TopBottomPanel::bottom("bottom-panel")
            .min_height(200.0)
            .show(ctx, |ui| {
                self.render_bottom_panel(ctx, ui, paused_frame, show_details);
            });
        self.render_controls_window(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_code_panel(ui, paused_frame);
        });
    }

    fn render_config_selector(&mut self, ui: &mut Ui) {
        let selected_name = self.state.config_names[self.state.selected_config_index].clone();
        egui::ComboBox::from_id_salt("config_selector")
            .selected_text(&selected_name)
            .show_ui(ui, |ui| {
                for (i, name) in self.state.config_names.iter().enumerate() {
                    ui.selectable_value(&mut self.state.selected_config_index, i, name);
                }
            });
    }

    fn render_controls_window(&mut self, ctx: &Context) {
        let has_session = self.state.session.is_some();
        let is_terminated = self
            .state
            .session
            .as_ref()
            .is_some_and(|s| matches!(s.state, State::Terminated));

        egui::TopBottomPanel::top("control_panel").show(ctx, |ui| {
            ui.heading("DAP Debugger");
            ui.separator();

            ui.horizontal(|ui| {
                // Config selector (disabled during active session)
                ui.add_enabled_ui(!has_session || is_terminated, |ui| {
                    self.render_config_selector(ui);
                });

                if !has_session || is_terminated {
                    if ui.button("▶ Start").clicked() {
                        self.start_session();
                    }
                }

                if has_session && !is_terminated {
                    if ui.button("⏹ Stop").clicked() {
                        if let Some(session) = self.state.session.take() {
                            session
                                .bridge
                                .send(crate::async_bridge::UiCommand::Terminate);
                        }
                        self.state.variables_cache.clear();
                        *self.state.repl_output.borrow_mut() = String::new();
                    }
                }

                // Stepping controls only when session is active and not terminated
                if let Some(session) = &self.state.session {
                    if !matches!(session.state, State::Terminated) {
                        if ui.button("▶ Continue").clicked() {
                            session
                                .bridge
                                .send(crate::async_bridge::UiCommand::Continue);
                        }
                        if ui.button("⏭ Step Over").clicked() {
                            session
                                .bridge
                                .send(crate::async_bridge::UiCommand::StepOver);
                        }
                        if ui.button("⏬ Step Into").clicked() {
                            session.bridge.send(crate::async_bridge::UiCommand::StepIn);
                        }
                        if ui.button("⏫ Step Out").clicked() {
                            session.bridge.send(crate::async_bridge::UiCommand::StepOut);
                        }
                    }
                }
            });
        });
    }

    fn render_sidepanel(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        stack: &[StackFrame],
        show_details: bool,
    ) {
        let bp_list: Vec<_> = self.state.ui_breakpoints.iter().cloned().collect();
        ui.vertical(|ui| {
            ui.add(CallStack::new(stack, show_details, self.state));
            ui.separator();
            ui.add(Breakpoints::new(&bp_list, show_details));

            // Text input for adding breakpoints via file:line
            ui.separator();
            let hint = "file:line";
            let text_edit = egui::TextEdit::singleline(&mut self.state.breakpoint_input)
                .hint_text(hint)
                .text_color_opt(if self.state.breakpoint_input_error {
                    Some(egui::Color32::RED)
                } else {
                    None
                });
            let response = ui.add(text_edit);
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let input = self.state.breakpoint_input.clone();
                if !input.trim().is_empty() {
                    match debugger::Breakpoint::parse(&input, &self.state.debug_root_dir) {
                        Ok(bp) => {
                            self.state.ui_breakpoints.insert(bp.clone());
                            if let Some(session) = &self.state.session {
                                match session.bridge.send_sync(|reply| {
                                    crate::async_bridge::UiCommand::AddBreakpoint {
                                        breakpoint: bp,
                                        reply,
                                    }
                                }) {
                                    Ok(_id) => {}
                                    Err(e) => {
                                        self.state
                                            .status
                                            .push_error(format!("Failed to add breakpoint: {e}"));
                                    }
                                }
                            }
                            self.state.persist_breakpoints();
                            self.state.breakpoint_input.clear();
                            self.state.breakpoint_input_error = false;
                        }
                        Err(_) => {
                            self.state.breakpoint_input_error = true;
                        }
                    }
                }
            }
            if response.changed() {
                self.state.breakpoint_input_error = false;
            }
        });
    }

    fn render_bottom_panel(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        paused_frame: &PausedFrame,
        show_details: bool,
    ) {
        {
            let mut tab = self.state.tab.borrow_mut();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut *tab, TabState::Variables, "Variables");
                ui.selectable_value(&mut *tab, TabState::Repl, "Repl");
            });
        }
        let current_tab = *self.state.tab.borrow();
        match current_tab {
            TabState::Variables => self.render_variables(ctx, ui, paused_frame, show_details),
            TabState::Repl => self.render_repl(ctx, ui),
        }
    }

    fn render_repl(&mut self, _ctx: &Context, ui: &mut Ui) {
        let repl_input = &mut *self.state.repl_input.borrow_mut();
        let repl_output = &mut *self.state.repl_output.borrow_mut();
        let current_frame_id = self.state.session.as_ref().and_then(|s| s.current_frame_id);
        if let Some(frame_id) = current_frame_id {
            // output/history area
            ui.text_edit_multiline(repl_output);
            // input area
            if ui.text_edit_singleline(repl_input).lost_focus()
                && ui.input(|i| i.key_pressed(Key::Enter))
            {
                let expr = repl_input.clone();
                if let Some(session) = &self.state.session {
                    match session.bridge.send_sync(|reply| {
                        crate::async_bridge::UiCommand::Evaluate {
                            expression: expr,
                            frame_id,
                            reply,
                        }
                    }) {
                        Ok(EvaluateResult { output, error }) => {
                            if error {
                                *repl_output += &format!("\n{repl_input}\n!! {output}\n");
                            } else {
                                *repl_output += &format!("\n{repl_input}\n=> {output}\n");
                            }
                            repl_input.clear();
                        }
                        Err(e) => {
                            self.state.status.push_error(format!("Eval failed: {e}"));
                            repl_input.clear();
                        }
                    }
                }
            }
        }
    }

    fn render_code_panel(&mut self, ui: &mut Ui, paused_frame: &PausedFrame) {
        self.render_search_bar(ui);
        self.render_code_viewer(ui, paused_frame);
    }

    fn render_search_bar(&mut self, ui: &mut Ui) {
        if !self.state.search_state.active {
            return;
        }

        let search = &mut self.state.search_state;
        let id = egui::Id::new(SEARCH_INPUT_ID);

        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut search.query)
                    .id(id)
                    .desired_width(200.0)
                    .hint_text("Search..."),
            );

            if search.request_focus {
                response.request_focus();
                // Select all text when focusing
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(search.query.len()),
                        )));
                    state.store(ui.ctx(), id);
                }
                search.request_focus = false;
            }

            // Handle Enter/Shift+Enter for navigation
            if response.has_focus() {
                let enter = ui.input(|i| i.key_pressed(Key::Enter));
                let shift = ui.input(|i| i.modifiers.shift);
                if enter {
                    if shift {
                        search.prev_match();
                    } else {
                        search.next_match();
                    }
                }

                // Handle Escape to close
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    search.active = false;
                    search.query.clear();
                    search.recompute_for_new_file();
                }
            }

            // Match count label
            if !search.query.is_empty() {
                let total = search.matches.len();
                if total > 0 {
                    ui.label(format!("{} of {}", search.current_match + 1, total));
                } else {
                    ui.label("0 of 0");
                }
            }

            // Prev/Next buttons
            if ui.button("<").clicked() {
                search.prev_match();
            }
            if ui.button(">").clicked() {
                search.next_match();
            }
        });
        ui.separator();
    }

    fn render_variables(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        paused_frame: &PausedFrame,
        show_details: bool,
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Variables");
            if show_details {
                let variables = paused_frame.variables.clone();
                for var in &variables {
                    self.render_variable(ui, var, 0);
                }
            }
        });
    }

    fn render_variable(&mut self, ui: &mut Ui, var: &Variable, depth: usize) {
        let value = var.value.clone().unwrap_or_default();
        let label = match &var.r#type {
            Some(t) => format!("{}: {} = {}", var.name, t, value),
            None => format!("{} = {}", var.name, value),
        };

        let has_children = var.variables_reference.is_some_and(|r| r != 0);

        if has_children {
            let var_ref = var.variables_reference.unwrap();
            let id = ui.make_persistent_id(format!("var_{}_{}", depth, var.name));
            egui::CollapsingHeader::new(label)
                .id_salt(id)
                .show(ui, |ui| {
                    // Fetch children on first expand
                    if !self.state.variables_cache.contains_key(&var_ref) {
                        if let Some(session) = &self.state.session {
                            match session.bridge.send_sync(|reply| {
                                crate::async_bridge::UiCommand::FetchVariables {
                                    reference: var_ref,
                                    reply,
                                }
                            }) {
                                Ok(children) => {
                                    self.state.variables_cache.insert(var_ref, children);
                                }
                                Err(e) => {
                                    self.state
                                        .status
                                        .push_error(format!("Failed to fetch variables: {e}"));
                                    return;
                                }
                            }
                        }
                    }

                    if let Some(children) = self.state.variables_cache.get(&var_ref).cloned() {
                        for child in &children {
                            self.render_variable(ui, child, depth + 1);
                        }
                    }
                });
        } else {
            ui.label(label);
        }
    }

    fn render_code_viewer(&mut self, ui: &mut Ui, paused_frame: &PausedFrame) {
        let frame = &paused_frame.frame;
        let Some(debugger_path) = frame.source.as_ref().and_then(|s| s.path.as_ref()).cloned()
        else {
            ui.label("No source file available for current frame");
            return;
        };
        let debugger_path = std::fs::canonicalize(&debugger_path).unwrap_or(debugger_path);

        // Determine which file to display
        let (display_path, highlight_line, current_line) =
            if let Some(ref override_path) = self.state.file_override {
                (override_path.clone(), false, 1)
            } else {
                (debugger_path, true, frame.line)
            };

        // File breadcrumb
        let display_name = display_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| display_path.to_string_lossy().to_string());
        ui.label(&display_name);
        ui.separator();

        // Read file contents with caching
        let contents = self
            .state
            .file_cache
            .get_or_load(&display_path)
            .unwrap_or("")
            .to_string();

        // Update search matches if query or file changed
        if self.state.search_state.active {
            self.state.search_state.update(&contents, &display_path);
        }

        // Filter ui_breakpoints to current file for the code view
        let mut file_breakpoints: HashSet<_> = self
            .state
            .ui_breakpoints
            .iter()
            .filter(|b| b.path == display_path.as_path())
            .cloned()
            .collect();

        let breakpoints_before = file_breakpoints.clone();
        let is_dark = ui.visuals().dark_mode;

        // Compute scroll target for search match navigation
        let scroll_to_line = if self.state.search_state.scroll_to_match {
            self.state.search_state.scroll_to_match = false;
            self.state.search_state.current_match_line().map(|l| l + 1)
        } else {
            None
        };

        let search_matches = if self.state.search_state.active {
            &self.state.search_state.matches
        } else {
            &[][..]
        };
        let current_search_match = self.state.search_state.current_match;

        ui.add(CodeView::new(
            &contents,
            current_line,
            highlight_line,
            &mut file_breakpoints,
            &self.state.jump,
            display_path,
            is_dark,
            self.state.code_font_size,
            search_matches,
            current_search_match,
            scroll_to_line,
        ));

        // Detect breakpoint changes from gutter clicks and sync with debugger
        for added in file_breakpoints.difference(&breakpoints_before) {
            let bp = added.clone();
            self.state.ui_breakpoints.insert(bp.clone());
            if let Some(session) = &self.state.session {
                match session.bridge.send_sync(|reply| {
                    crate::async_bridge::UiCommand::AddBreakpoint {
                        breakpoint: bp,
                        reply,
                    }
                }) {
                    Ok(_id) => {}
                    Err(e) => {
                        self.state
                            .status
                            .push_error(format!("Failed to add breakpoint: {e}"));
                    }
                }
            }
        }
        for removed in breakpoints_before.difference(&file_breakpoints) {
            self.state.ui_breakpoints.remove(removed);
            tracing::debug!(?removed, "breakpoint removed via gutter click");
        }

        if file_breakpoints != breakpoints_before {
            self.state.persist_breakpoints();
        }
    }
}
