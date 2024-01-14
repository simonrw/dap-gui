use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use debugger::PausedFrame;
use eframe::egui::{self, Button, Context, Ui};
use transport::types::StackFrame;

use crate::{code_view::CodeView, DebuggerAppState, State};

pub(crate) struct Renderer<'a> {
    state: &'a DebuggerAppState,
}

impl<'s> Renderer<'s> {
    pub(crate) fn new(state: &'s DebuggerAppState) -> Self {
        Self { state }
    }

    pub(crate) fn render_ui(&self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| match &self.state.state {
            State::Initialising => {}
            State::Running => {
                let previous_state = &self.state.previous_state;
                if let Some(State::Paused {
                    stack,
                    paused_frame,
                    breakpoints,
                }) = previous_state
                {
                    self.render_paused_or_running_ui(
                        ctx,
                        ui,
                        stack,
                        paused_frame,
                        breakpoints,
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
        &self,
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
            self.render_sidepanel(ctx, ui, stack, show_details);
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

    fn render_controls_window(&self, ctx: &Context, _ui: &mut Ui) {
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
        &self,
        ctx: &Context,
        ui: &mut Ui,
        stack: &[StackFrame],
        show_details: bool,
    ) {
        ui.vertical(|ui| {
            self.render_call_stack(ctx, ui, stack, show_details);
            self.render_breakpoints(ctx, ui, show_details);
        });
    }

    fn render_bottom_panel(
        &self,
        ctx: &Context,
        ui: &mut Ui,
        paused_frame: &PausedFrame,
        show_details: bool,
    ) {
        // TODO: tabbed interface with repl
        self.render_variables(ctx, ui, paused_frame, show_details);
    }

    fn render_code_panel(
        &self,
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
        &self,
        _ctx: &Context,
        ui: &mut Ui,
        stack: &[StackFrame],
        show_details: bool,
    ) {
        ui.label("Call Stack");
        if show_details {
            for frame in stack {
                ui.label(frame.name.to_string());
            }
        }
    }
    fn render_breakpoints(&self, _ctx: &Context, _ui: &mut Ui, _show_details: bool) {}
    fn render_variables(
        &self,
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
        &self,
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
