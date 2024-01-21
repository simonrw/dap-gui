use eframe::egui::{self, Visuals};

#[derive(Default)]
struct App {}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Hello world");
        });
    }
}

fn main() {
    let _ = color_eyre::install();
    let _ = tracing_subscriber::fmt::try_init();

    eframe::run_native(
        "App",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            let style = egui::Style {
                visuals: match dark_light::detect() {
                    dark_light::Mode::Dark | dark_light::Mode::Default => {
                        tracing::debug!("choosing dark mode");
                        Visuals::dark()
                    }
                    dark_light::Mode::Light => {
                        tracing::debug!("choosing light mode");
                        Visuals::light()
                    }
                },
                ..Default::default()
            };
            cc.egui_ctx.set_style(style);
            let app = App::default();
            Box::new(app)
        }),
    )
    .unwrap();
}
