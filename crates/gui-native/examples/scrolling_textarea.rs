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
            let output = egui::ScrollArea::vertical().show(ui, |ui| {
                TextEdit::multiline(&mut self.contents)
                    .desired_width(f32::INFINITY)
                    .code_editor()
                    .show(ui)
            });

            let mut state = output.state;
            if let Some(position) = &self.click_position {
                match position {
                    Position::Top => state.offset.y = 0.0,
                    Position::Mid => {
                        state.offset.y = (output.content_size.y - output.inner_rect.max.y) / 2.0
                    }
                    Position::Bottom => {
                        state.offset.y = output.content_size.y - output.inner_rect.max.y
                    }
                }
            }
            state.store(ui.ctx(), output.id);
        });
    }
}

fn main() {
    let _ = color_eyre::install();
    let _ = tracing_subscriber::fmt::try_init();

    eframe::run_native(
        "App (Native)",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            let is_dark = match dark_light::detect() {
                dark_light::Mode::Dark | dark_light::Mode::Default => {
                    tracing::debug!("choosing dark mode");
                    true
                }
                dark_light::Mode::Light => {
                    tracing::debug!("choosing light mode");
                    false
                }
            };
            let mut visuals = if is_dark {
                Visuals::dark()
            } else {
                Visuals::light()
            };
            if is_dark {
                visuals.panel_fill = egui::Color32::from_rgb(30, 30, 30);
                visuals.window_fill = egui::Color32::from_rgb(42, 42, 42);
            } else {
                visuals.panel_fill = egui::Color32::from_rgb(246, 246, 246);
            }
            visuals.window_corner_radius = egui::CornerRadius::same(8);
            visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
            visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
            visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
            let style = egui::Style {
                visuals,
                ..Default::default()
            };
            cc.egui_ctx.set_style(style);
            let app = App {
                contents: include_str!("./scrolling_textarea.rs").to_string(),
                ..Default::default()
            };
            Ok(Box::new(app))
        }),
    )
    .unwrap();
}
