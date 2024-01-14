use std::collections::HashSet;

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
            State::Running => {}
            State::Paused {
                stack,
                paused_frame,
                breakpoints,
            } => self.render_paused_ui(ctx, ui, stack, paused_frame, breakpoints),
            State::Terminated => {
                ui.label("Program terminated");
            }
        });
    }

    pub fn render_paused_ui(
        &self,
        ctx: &Context,
        ui: &mut Ui,
        stack: &[StackFrame],
        paused_frame: &PausedFrame,
        original_breakpoints: &[debugger::Breakpoint],
    ) {
        self.render_controls_window(ctx, ui);

        egui::SidePanel::left("left-panel").show_inside(ui, |ui| {
            self.render_sidepanel(ctx, ui, stack);
        });
        ui.vertical(|ui| {
            egui::TopBottomPanel::bottom("bottom-panel")
                .min_height(200.0)
                .show_inside(ui, |ui| {
                    self.render_bottom_panel(ctx, ui, paused_frame);
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
                    let button = Button::new("▶️").small();
                    if ui.add(button).clicked() {
                        self.state.debugger.r#continue().unwrap();
                    }
                });
            });
    }

    fn render_sidepanel(&self, ctx: &Context, ui: &mut Ui, stack: &[StackFrame]) {
        ui.vertical(|ui| {
            self.render_call_stack(ctx, ui, stack);
            self.render_breakpoints(ctx, ui);
        });
    }

    fn render_bottom_panel(&self, ctx: &Context, ui: &mut Ui, paused_frame: &PausedFrame) {
        // TODO: tabbed interface with repl
        self.render_variables(ctx, ui, paused_frame);
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

    fn render_call_stack(&self, _ctx: &Context, ui: &mut Ui, stack: &[StackFrame]) {
        for frame in stack {
            ui.label(frame.name.to_string());
        }
    }
    fn render_breakpoints(&self, _ctx: &Context, _ui: &mut Ui) {}
    fn render_variables(&self, _ctx: &Context, ui: &mut Ui, paused_frame: &PausedFrame) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Variables");
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
