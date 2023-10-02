use eframe::{
    egui::{self, TextEdit, TextFormat},
    epaint::{text::LayoutJob, Color32},
};

/// Code view that shows debugger related things
pub struct CodeView<'a> {
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
