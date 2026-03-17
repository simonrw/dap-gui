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
            self.state.file_picker_open = !self.state.file_picker_open;
            if !self.state.file_picker_open {
                self.state.file_picker_input.clear();
                self.state.file_picker_cursor = 0;
            }
        }

        // Handle F5: start session or continue
        if ctx.input(|i| i.key_pressed(Key::F5) && i.modifiers.is_none()) {
            self.handle_f5();
        }

        // Handle Shift+F5: stop session
        if ctx.input(|i| i.key_pressed(Key::F5) && i.modifiers.matches_exact(Modifiers::SHIFT)) {
            if self.state.session.is_some() {
                self.state.session = None;
                self.state.variables_cache.clear();
                *self.state.repl_output.borrow_mut() = String::new();
            }
        }

        // Render file picker overlay if open
        if self.state.file_picker_open {
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
            State::Initialising => {
                self.render_controls_window(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label("Initialising debugger...");
                    });
                });
            }
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
                        ui.label("or press F5");
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
        // Drop existing session first
        self.state.session = None;
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
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading("Select a configuration and press Start or F5");
            });
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
            self.render_code_panel(ctx, ui, paused_frame);
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
                        self.state.session = None;
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

    fn render_code_panel(&mut self, ctx: &Context, ui: &mut Ui, paused_frame: &PausedFrame) {
        self.render_code_viewer(ctx, ui, paused_frame);
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
    fn render_code_viewer(&mut self, _ctx: &Context, ui: &mut Ui, paused_frame: &PausedFrame) {
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
            .entry(display_path.clone())
            .or_insert_with(|| {
                std::fs::read_to_string(&display_path)
                    .unwrap_or_else(|e| format!("Error reading file: {e}"))
            })
            .clone();

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

        ui.add(CodeView::new(
            &contents,
            current_line,
            highlight_line,
            &mut file_breakpoints,
            &self.state.jump,
            display_path,
            is_dark,
            self.state.code_font_size,
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
