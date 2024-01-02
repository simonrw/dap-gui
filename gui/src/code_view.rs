use std::collections::HashSet;

use eframe::{
    egui::{self, Response, TextEdit, TextFormat},
    epaint::{text::LayoutJob, Color32},
};

/// Code view that shows debugger related things
///
/// Note: we assume that breakpoints have been filtered for the file that `content` is read from
pub struct CodeView<'a> {
    /// Read-only view into the text content
    content: &'a str,
    /// Optionally highlight the line the debugger has stopped on (1-indexed)
    current_line: usize,
    highlight_line: bool,
    /// Line numbers to add breakpoint markers to (1-indexed)
    breakpoints: &'a mut HashSet<debugger::Breakpoint>,
}

impl<'a> CodeView<'a> {
    /// Create a new code view
    pub fn new(
        content: &'a str,
        current_line: usize,
        highlight_line: bool,
        breakpoints: &'a mut HashSet<debugger::Breakpoint>,
    ) -> Self {
        Self {
            content,
            current_line,
            highlight_line,
            breakpoints,
        }
    }

    fn breakpoint_positions(&self) -> HashSet<usize> {
        HashSet::from_iter(self.breakpoints.iter().map(|b| b.line))
    }
}

impl<'a> egui::Widget for CodeView<'a> {
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        let breakpoint_positions = self.breakpoint_positions();
        // closure that defines the layout drop
        let mut layouter = |ui: &egui::Ui, s: &str, _wrap_width: f32| {
            let mut layout_job = LayoutJob::default();
            let indent = 16.0;
            for (i, line) in s.lines().enumerate() {
                if breakpoint_positions.contains(&(i + 1)) {
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
                if self.highlight_line && i == (self.current_line - 1) {
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
        let response = ui.add(TextEdit::multiline(&mut self.content).layouter(&mut layouter));

        self.update_breakpoints(&response);

        response
    }
}

impl<'a> CodeView<'a> {
    fn update_breakpoints(&mut self, response: &Response) {
        // TODO
        return;

        /*
        if response.clicked_by(egui::PointerButton::Primary) {
            // unwrap ok because we know we were clicked
            let pos = response.interact_pointer_pos().unwrap();
            // dbg!(&pos);
            if pos.x >= 0.0 && pos.x < 16.0 {
                // click in the margin
                // TODO: calculate line height properly
                // line number 1-indexed
                let line = (pos.y / 16.0).floor() as usize;
                if self.breakpoints.contains(&line) {
                    // remove the breakpoint
                    self.breakpoints.remove(&line);
                } else {
                    // add the breakpoint
                    self.breakpoints.insert(line);
                }
            }
        }
        */
    }
}
