use std::{collections::HashSet, ops::Deref};

use debugger::{EvaluateResult, PausedFrame};
use eframe::egui::{self, Context, Key, Ui};
use transport::types::StackFrame;

use crate::{
    code_view::CodeView,
    ui::{breakpoints::Breakpoints, call_stack::CallStack, control_panel::ControlPanel},
    DebuggerAppState, State, TabState,
};

pub(crate) struct Renderer<'a> {
    state: &'a DebuggerAppState,
}

impl<'s> Renderer<'s> {
    pub(crate) fn new(state: &'s DebuggerAppState) -> Self {
        Self { state }
    }

    pub(crate) fn render_ui(&mut self, ctx: &Context) {
        match &self.state.state {
            State::Initialising => {}
            State::Running => {
                let DebuggerAppState { previous_state, .. } = &self.state;
                if let Some(State::Paused {
                    stack,
                    paused_frame,
                    breakpoints,
                }) = previous_state.clone()
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
                self.render_paused_or_running_ui(ctx, stack, paused_frame, breakpoints, true);
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
        egui::TopBottomPanel::bottom("bottom-panel")
            .min_height(200.0)
            .show(ctx, |ui| {
                self.render_bottom_panel(ctx, ui, paused_frame, show_details);
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_code_panel(ctx, ui, paused_frame, original_breakpoints);
            if show_details {
                self.render_controls_window(ctx, ui);
            }
        });
    }

    fn render_controls_window(&mut self, ctx: &Context, ui: &mut Ui) {
        ui.add(ControlPanel::new(&self.state.debugger, ctx));
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
        match self.state.tab.borrow().deref() {
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
                // TODO: handle the error case
                if let Ok(Some(EvaluateResult {
                    output,
                    error: _error,
                })) = self.state.debugger.evaluate(repl_input, frame_id)
                {
                    *repl_output += &("\n".to_string() + repl_input + "\n=> " + &output + "\n");
                    repl_input.clear();
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
        _ui: &mut Ui,
        _paused_frame: &PausedFrame,
        _show_details: bool,
    ) {
        /*
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Variables");
            if show_details {
                for var in &paused_frame.variables {
                    match &var.r#type {
                        Some(t) => {
                            ui.label(format!(
                                "{name}: {typ} = {value}",
                                name = var.name,
                                typ = t,
                                value = var.value,
                            ));
                        }
                        None => {
                            ui.label(format!(
                                "{name} = {value}",
                                name = var.name,
                                value = var.value,
                            ));
                        }
                    }
                }
            }
        });
        */
    }
    fn render_code_viewer(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        paused_frame: &PausedFrame,
        original_breakpoints: &[debugger::Breakpoint],
    ) {
        // let DebuggerAppState { ref mut jump, .. } = self.state;
        let frame = &paused_frame.frame;
        let file_path = frame
            .source
            .as_ref()
            .and_then(|s| s.path.as_ref())
            .expect("no file source given");
        let contents =
            std::fs::read_to_string(file_path).expect("reading source from given file path");
        let mut breakpoints = HashSet::from_iter(
            original_breakpoints
                .iter()
                .filter(|b| file_path.as_path() == b.path)
                .cloned(),
        );

        ui.add(CodeView::new(
            &contents,
            frame.line,
            true,
            &mut breakpoints,
            &self.state.jump,
        ));
    }
}
