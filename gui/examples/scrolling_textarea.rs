use std::f32;

use eframe::{
    egui::{self, TextEdit, Visuals},
    epaint::text::cursor::RCursor,
};

enum Position {
    Top,
    Mid,
    Bottom,
}

#[derive(Default)]
struct App {
    contents: String,
    last_cursor: Option<RCursor>,
}

impl App {
    fn scroll_to(&mut self, _position: Position) {}
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("left-panel").show(ctx, |ui| {
            if ui.button("scroll to top").clicked() {
                self.scroll_to(Position::Top);
            }

            if ui.button("scroll to midpoint").clicked() {
                self.scroll_to(Position::Mid);
            }

            if ui.button("scroll to bottom").clicked() {
                self.scroll_to(Position::Bottom);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let output = TextEdit::multiline(&mut self.contents)
                    .desired_width(f32::INFINITY)
                    .code_editor()
                    .show(ui);

                // tracing::trace!(cursor = ?output.cursor_range.map(|r| r.primary), "cursor position");

                if let Some(new_cursor) = output.cursor_range.map(|r| r.primary.rcursor) {
                    match self.last_cursor.as_mut() {
                        Some(last_cursor) => {
                            if new_cursor != *last_cursor {
                                *last_cursor = new_cursor;
                                tracing::debug!(cursor = ?*last_cursor, "new cursor position");
                            }
                        }
                        None => self.last_cursor = Some(new_cursor),
                    }
                }

                output.response
            });
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
            let app = App {
                contents: include_str!("./scrolling_textarea.rs").to_string(),
                ..Default::default()
            };
            Box::new(app)
        }),
    )
    .unwrap();
}
