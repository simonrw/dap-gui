use eframe::{
    egui::{self, Style},
    epaint::FontId,
};

#[derive(Debug)]
struct App {
    src: String,
}

impl Default for App {
    fn default() -> Self {
        let src = include_str!("../../test.py");
        Self {
            src: src.to_string(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // use dap_gui_ui::syntax_highlighting;
            // syntax_highlighting::code_view_ui(ui, &self.src);

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.src)
                        .font(egui::TextStyle::Monospace) // for cursor height
                        .code_editor()
                        .desired_rows(1)
                        .lock_focus(true)
                        .interactive(false),
                    // .layouter(&mut layouter),
                );
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
        Box::new(move |cc| {
            let style = Style {
                // temporarily increase font size
                override_font_id: Some(FontId::monospace(24.0)),
                ..Style::default()
            };
            cc.egui_ctx.set_style(style);
            Box::new(App::default())
        }),
    )
}
