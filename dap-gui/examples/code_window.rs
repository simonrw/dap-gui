use eframe::{
    egui::{self, Slider, TextEdit, TextFormat},
    epaint::{text::LayoutJob, Color32},
};

#[derive(Default)]
struct MyEguiApp {
    content: String,
    breakpoint_line: usize,
    should_highlight: bool,
}

impl MyEguiApp {
    pub fn new(_c: &eframe::CreationContext<'_>) -> Self {
        let content = include_str!("../../test.py");
        Self {
            content: content.to_string(),
            breakpoint_line: 0,
            should_highlight: false,
        }
    }

    fn render_text_element(&mut self, ui: &mut egui::Ui) {
        let mut layouter = |ui: &egui::Ui, s: &str, _wrap_width: f32| {
            let mut layout_job = LayoutJob::default();
            for (i, line) in s.lines().enumerate() {
                if self.should_highlight && self.breakpoint_line == i {
                    layout_job.append(
                        line,
                        0.0,
                        TextFormat {
                            background: Color32::from_gray(128),
                            ..Default::default()
                        },
                    );
                } else {
                    layout_job.append(line, 0.0, TextFormat::default());
                }
                layout_job.append("\n", 0.0, TextFormat::default());
            }

            ui.fonts(|f| f.layout_job(layout_job))
        };
        ui.add(
            TextEdit::multiline(&mut self.content)
                .interactive(false)
                .layouter(&mut layouter),
        );
    }
}

impl eframe::App for MyEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.checkbox(&mut self.should_highlight, "Enable line highlighting");
            ui.add(
                Slider::new(
                    &mut self.breakpoint_line,
                    0..=self.content.lines().count() - 1,
                )
                .text("Highlight line")
                .integer(),
            );
            self.render_text_element(ui);
        });
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "My egui App",
        native_options,
        Box::new(|cc| Box::new(MyEguiApp::new(cc))),
    )
}
