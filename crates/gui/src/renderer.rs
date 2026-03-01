use std::collections::HashSet;

use debugger::{EvaluateResult, PausedFrame};
use eframe::egui::{self, Context, Key, Modifiers, Ui};
use transport::types::{StackFrame, Variable};

use crate::{
    DebuggerAppState, State, TabState,
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
}

impl<'s> Renderer<'s> {
    pub(crate) fn new(state: &'s mut DebuggerAppState) -> Self {
        Self { state }
    }

    pub(crate) fn render_ui(&mut self, ctx: &Context) {
        // Handle Ctrl+P to toggle file picker
        if ctx.input(|i| i.key_pressed(Key::P) && i.modifiers.matches_exact(Modifiers::CTRL)) {
            self.state.file_picker_open = !self.state.file_picker_open;
            if !self.state.file_picker_open {
                self.state.file_picker_input.clear();
                self.state.file_picker_cursor = 0;
            }
        }

        // Render file picker overlay if open
        if self.state.file_picker_open {
            if let FilePickerResult::Selected(path) = file_picker::show(ctx, self.state) {
                self.state.file_override = Some(path);
            }
        }

        let current_state = self.state.state.clone();
        match current_state {
            State::Initialising => {}
            State::Running => {
                if let Some(State::Paused {
                    stack,
                    paused_frame,
                    breakpoints,
                }) = self.state.previous_state.clone()
                {
                    self.render_paused_or_running_ui(
                        ctx,
                        &stack,
                        &paused_frame,
                        &breakpoints,
                        false,
                    );
                }
            }
            State::Paused {
                stack,
                paused_frame,
                breakpoints,
            } => {
                self.render_paused_or_running_ui(ctx, &stack, &paused_frame, &breakpoints, true);
            }
            State::Terminated => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("Program terminated");
                });
            }
        }
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
        original_breakpoints: &[debugger::Breakpoint],
        show_details: bool,
    ) {
        egui::SidePanel::left("left-panel").show(ctx, |ui| {
            self.render_sidepanel(ctx, ui, stack, original_breakpoints, show_details);
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
            self.render_code_panel(ctx, ui, paused_frame, original_breakpoints);
        });
    }

    fn render_controls_window(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("control_panel").show(ctx, |ui| {
            ui.heading("DAP Debugger");
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("▶ Continue").clicked() {
                    self.state
                        .bridge
                        .send(crate::async_bridge::UiCommand::Continue);
                }
                if ui.button("⏭ Step Over").clicked() {
                    self.state
                        .bridge
                        .send(crate::async_bridge::UiCommand::StepOver);
                }
                if ui.button("⏬ Step Into").clicked() {
                    self.state
                        .bridge
                        .send(crate::async_bridge::UiCommand::StepIn);
                }
                if ui.button("⏫ Step Out").clicked() {
                    self.state
                        .bridge
                        .send(crate::async_bridge::UiCommand::StepOut);
                }
            });
        });
    }

    fn render_sidepanel(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        stack: &[StackFrame],
        original_breakpoints: &[debugger::Breakpoint],
        show_details: bool,
    ) {
        ui.vertical(|ui| {
            ui.add(CallStack::new(stack, show_details, self.state));
            ui.separator();
            ui.add(Breakpoints::new(original_breakpoints, show_details));
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
        // We only have a frame id if we are paused. If we are running then there is no frame id,
        // so don't render the REPL.
        if let Some(frame_id) = self.state.current_frame_id {
            // output/history area
            ui.text_edit_multiline(repl_output);
            // input area
            if ui.text_edit_singleline(repl_input).lost_focus()
                && ui.input(|i| i.key_pressed(Key::Enter))
            {
                let expr = repl_input.clone();
                match self.state.bridge.send_sync(|reply| {
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

    fn render_code_panel(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        paused_frame: &PausedFrame,
        original_breakpoints: &[debugger::Breakpoint],
    ) {
        self.render_code_viewer(ctx, ui, paused_frame, original_breakpoints);
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
                        match self.state.bridge.send_sync(|reply| {
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
    fn render_code_viewer(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        paused_frame: &PausedFrame,
        original_breakpoints: &[debugger::Breakpoint],
    ) {
        let frame = &paused_frame.frame;
        let Some(debugger_path) = frame.source.as_ref().and_then(|s| s.path.as_ref()).cloned()
        else {
            ui.label("No source file available for current frame");
            return;
        };

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

        let mut breakpoints = HashSet::from_iter(
            original_breakpoints
                .iter()
                .filter(|b| display_path.as_path() == b.path)
                .cloned(),
        );

        let breakpoints_before = breakpoints.clone();
        let is_dark = ui.visuals().dark_mode;

        ui.add(CodeView::new(
            &contents,
            current_line,
            highlight_line,
            &mut breakpoints,
            &self.state.jump,
            display_path,
            is_dark,
        ));

        // Detect breakpoint changes from gutter clicks and sync with debugger
        for added in breakpoints.difference(&breakpoints_before) {
            let bp = added.clone();
            match self.state.bridge.send_sync(|reply| {
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
        for removed in breakpoints_before.difference(&breakpoints) {
            tracing::debug!(?removed, "breakpoint removed via gutter click");
            // Note: removal by Breakpoint struct (not by ID) requires looking up the ID.
            // For now we rely on the next pause event to refresh breakpoint state.
        }
    }
}
