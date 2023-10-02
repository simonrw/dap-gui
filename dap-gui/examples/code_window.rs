use eframe::{
    egui::{self, Slider, TextEdit, TextFormat},
    epaint::{text::LayoutJob, Color32},
};

#[derive(Default)]
struct MyEguiApp {
    content: String,
    current_line: usize,
    highlight_line: bool,
    breakpoint_positions: Vec<usize>,
}

/// Code view that shows debugger related things
struct CodeView<'a> {
    /// Read-only view into the text content
    content: &'a str,
    /// Optionally highlight the line the debugger has stopped on
    current_line: usize,
    highlight_line: bool,
    /// Line numbers to add breakpoint markers to
    breakpoint_positions: &'a [usize],
}

impl<'a> CodeView<'a> {
    /// Create a new code view
    pub fn new(
        content: &'a str,
        current_line: usize,
        highlight_line: bool,
        breakpoint_positions: &'a [usize],
    ) -> Self {
        Self {
            content,
            current_line,
            highlight_line,
            breakpoint_positions,
        }
    }
}

impl<'a> egui::Widget for CodeView<'a> {
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        // closure that defines the layout drop
        let mut layouter = |ui: &egui::Ui, s: &str, _wrap_width: f32| {
            let mut layout_job = LayoutJob::default();
            let indent = 16.0;
            for (i, line) in s.lines().enumerate() {
                if self.breakpoint_positions.contains(&i) {
                    // marker
                    layout_job.append(
                        "•",
                        0.0,
                        TextFormat {
                            color: Color32::from_rgb(255, 0, 0),
                            ..Default::default()
                        },
                    );
                };
                if self.highlight_line && self.current_line == i {
                    // highlighted line
                    layout_job.append(
                        line,
                        indent,
                        TextFormat {
                            background: Color32::from_gray(128),
                            ..Default::default()
                        },
                    );
                } else {
                    layout_job.append(line, indent, TextFormat::default());
                }
                layout_job.append("\n", indent, TextFormat::default());
            }

            ui.fonts(|f| f.layout_job(layout_job))
        };
        ui.add(
            TextEdit::multiline(&mut self.content)
                .interactive(false)
                .layouter(&mut layouter),
        )
    }
}

impl MyEguiApp {
    pub fn new(_c: &eframe::CreationContext<'_>) -> Self {
        let content = include_str!("../../test.py");
        Self {
            content: content.to_string(),
            current_line: 10,
            highlight_line: true,
            breakpoint_positions: vec![3, 10],
        }
    }
}

impl eframe::App for MyEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.checkbox(&mut self.highlight_line, "Highlight?");
            ui.add(
                Slider::new(&mut self.current_line, 0..=self.content.lines().count() - 1)
                    .text("Highlight line")
                    .integer(),
            );
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add(CodeView::new(
                    &self.content,
                    self.current_line,
                    self.highlight_line,
                    &self.breakpoint_positions,
                ));
            });
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
