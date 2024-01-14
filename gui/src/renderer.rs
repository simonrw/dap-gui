use std::{collections::HashSet, ops::Deref};

use debugger::PausedFrame;
use eframe::egui::{self, Button, Context, Ui};
use transport::types::StackFrame;

use crate::{code_view::CodeView, ui::call_stack::CallStack, DebuggerAppState, State, TabState};

pub(crate) struct Renderer<'a> {
    state: &'a DebuggerAppState,
}

impl<'s> Renderer<'s> {
    pub(crate) fn new(state: &'s DebuggerAppState) -> Self {
        Self { state }
    }

    pub(crate) fn render_ui(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| match &self.state.state {
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
                        ui,
                        &stack,
                        &paused_frame,
                        &breakpoints,
                        false,
                    );
                } else {
                    tracing::warn!("no previous state found");
                }
            }
            State::Paused {
                stack,
                paused_frame,
                breakpoints,
            } => {
                self.render_paused_or_running_ui(ctx, ui, stack, paused_frame, breakpoints, true);
            }
            State::Terminated => {
                ui.label("Program terminated");
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
        ui: &mut Ui,
        stack: &[StackFrame],
        paused_frame: &PausedFrame,
        original_breakpoints: &[debugger::Breakpoint],
        show_details: bool,
    ) {
        if show_details {
            self.render_controls_window(ctx, ui);
        }

        egui::SidePanel::left("left-panel").show_inside(ui, |ui| {
            self.render_sidepanel(ctx, ui, stack, original_breakpoints, show_details);
        });
        ui.vertical(|ui| {
            egui::TopBottomPanel::bottom("bottom-panel")
                .min_height(200.0)
                .show_inside(ui, |ui| {
                    self.render_bottom_panel(ctx, ui, paused_frame, show_details);
                });
            egui::CentralPanel::default().show_inside(ui, |ui| {
                self.render_code_panel(ctx, ui, paused_frame, original_breakpoints);
            });
        });
    }

    fn render_controls_window(&mut self, ctx: &Context, _ui: &mut Ui) {
        egui::Window::new("Controls")
            .anchor(egui::Align2::RIGHT_TOP, (10., 10.))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.add(Button::new("▶️").small()).clicked() {
                        self.state.debugger.r#continue().unwrap();
                    }
                    if ui.add(Button::new("step-over").small()).clicked() {
                        self.state.debugger.step_over().unwrap();
                    }
                    if ui.add(Button::new("step-in").small()).clicked() {
                        self.state.debugger.step_in().unwrap();
                    }
                    if ui.add(Button::new("step-out").small()).clicked() {
                        self.state.debugger.step_out().unwrap();
                    }
                });
            });
    }

    fn render_sidepanel(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        stack: &[StackFrame],
        original_breakpoints: &[debugger::Breakpoint],
        show_details: bool,
    ) {
        ui.vertical(|ui| {
            self.render_call_stack(ctx, ui, stack, show_details);
            ui.separator();
            self.render_breakpoints(ctx, ui, original_breakpoints, show_details);
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

    fn render_repl(&mut self, _ctx: &Context, _ui: &mut Ui) {}

    fn render_code_panel(
        &mut self,
        ctx: &Context,
        ui: &mut Ui,
        paused_frame: &PausedFrame,
        original_breakpoints: &[debugger::Breakpoint],
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.render_code_viewer(ctx, ui, paused_frame, original_breakpoints);
        });
    }

    fn render_call_stack(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        stack: &[StackFrame],
        show_details: bool,
    ) {
        ui.add(CallStack::new(stack, show_details));
    }

    fn render_breakpoints(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        breakpoints: &[debugger::Breakpoint],
        show_details: bool,
    ) {
        ui.label("Breakpoints");
        if !show_details {
            return;
        }

        for breakpoint in breakpoints {
            if let Some(name) = &breakpoint.name {
                ui.label(format!(
                    "{path}:{line} ({name})",
                    path = breakpoint.path.display(),
                    line = breakpoint.line,
                    name = name
                ));
            } else {
                ui.label(format!(
                    "{path}:{line}",
                    path = breakpoint.path.display(),
                    line = breakpoint.line,
                ));
            }
        }
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
    }
    fn render_code_viewer(
        &mut self,
        _ctx: &Context,
        ui: &mut Ui,
        paused_frame: &PausedFrame,
        original_breakpoints: &[debugger::Breakpoint],
    ) {
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

        ui.add(CodeView::new(&contents, frame.line, true, &mut breakpoints));
    }
}
