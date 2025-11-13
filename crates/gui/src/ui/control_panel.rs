use debugger::Debugger;
use eframe::egui::{self, Button, Context, Response, Widget};

pub(crate) struct ControlPanel<'s> {
    debugger: &'s Debugger,
    ctx: &'s Context,
}

impl<'s> ControlPanel<'s> {
    pub(crate) fn new(debugger: &'s Debugger, ctx: &'s Context) -> Self {
        Self { debugger, ctx }
    }
}

impl Widget for ControlPanel<'_> {
    fn ui(self, _ui: &mut eframe::egui::Ui) -> Response {
        egui::Window::new("Controls")
            .anchor(egui::Align2::RIGHT_TOP, (10., 10.))
            .show(self.ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.add(Button::new("▶️").small()).clicked() {
                        self.debugger.r#continue().unwrap();
                    }
                    if ui.add(Button::new("step-over").small()).clicked() {
                        self.debugger.step_over().unwrap();
                    }
                    if ui.add(Button::new("step-in").small()).clicked() {
                        self.debugger.step_in().unwrap();
                    }
                    if ui.add(Button::new("step-out").small()).clicked() {
                        self.debugger.step_out().unwrap();
                    }
                })
                .response
            })
            .unwrap()
            .inner
            .unwrap()
    }
}
