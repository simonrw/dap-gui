use std::f32;

use eframe::egui::{self, TextEdit, Visuals};

enum Position {
    Top,
    Mid,
    Bottom,
}

#[derive(Default)]
struct App {
    contents: String,
    click_position: Option<Position>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.click_position = None;

        egui::SidePanel::left("left-panel").show(ctx, |ui| {
            if ui.button("scroll to top").clicked() {
                self.click_position = Some(Position::Top);
            } else if ui.button("scroll to midpoint").clicked() {
                self.click_position = Some(Position::Mid);
            } else if ui.button("scroll to bottom").clicked() {
                self.click_position = Some(Position::Bottom);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let output = TextEdit::multiline(&mut self.contents)
                    .desired_width(f32::INFINITY)
                    .code_editor()
                    .show(ui);

                if let Some(position) = &self.click_position {
                    let text_edit_id = output.response.id;
                    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), text_edit_id) {
                        let ccursor = match position {
                            Position::Top => egui::text::CCursor::new(0),
                            Position::Mid => {
                                egui::text::CCursor::new(self.contents.chars().count() / 2)
                            }
                            Position::Bottom => {
                                egui::text::CCursor::new(self.contents.chars().count())
                            }
                        };
                        state.set_ccursor_range(Some(egui::text::CCursorRange::one(ccursor)));
                        state.store(ui.ctx(), text_edit_id);
                        ui.ctx().memory_mut(|m| {
                            m.request_focus(text_edit_id);
                        });
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
