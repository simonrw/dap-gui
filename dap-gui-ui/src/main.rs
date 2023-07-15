use anyhow::Result;
use eframe::egui;

mod syntax_highlighting;

#[derive(Default)]
struct MyApp {}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::right("right_panel")
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("right panel");
                // split into
                // - variables
                // - stack frames
                // - breakpoints
            });
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.label("bottom panel");
            // split into
            // - repl
            // - output?
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("central panel");

            // code
            let example_code = include_str!("./main.rs");

            egui::ScrollArea::vertical().show(ui, |ui| {
                syntax_highlighting::code_view_ui(ui, example_code);
            });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        ..Default::default()
    };
    eframe::run_native(
        "DAP GUI",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    )
}
