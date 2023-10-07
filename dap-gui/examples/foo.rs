use anyhow::Result;
use eframe::egui;
use std::collections::HashSet;

use dap_gui::code_view::CodeView;

struct MyApp {
    content: &'static str,
    breakpoints: HashSet<usize>,
}

impl MyApp {
    fn new(breakpoints: HashSet<usize>) -> Self {
        Self {
            content: include_str!("../../test.py"),
            breakpoints,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let code_view = CodeView::new(self.content, 7, true, &mut self.breakpoints);
            ui.add(code_view);
        });
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let initial_breakpoints = [1].into_iter().collect();

    if let Err(e) = eframe::run_native(
        "Test",
        Default::default(),
        Box::new(move |_| Box::new(MyApp::new(initial_breakpoints))),
    ) {
        anyhow::bail!("eframe error: {e}");
    }

    Ok(())
}
