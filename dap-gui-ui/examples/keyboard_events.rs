use eframe::egui;
use egui::*;

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Keyboard events",
        options,
        Box::new(|_cc| Box::<Content>::default()),
    )
}

#[derive(Default)]
struct Content {
    text: String,
    space_previously_pressed: bool,
}

impl eframe::App for Content {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.space_previously_pressed {

                ui.heading("Command mode: press space to exit");


                if ctx.input(|i| i.key_released(Key::Space)) {
                    // cancel command mode
                    self.space_previously_pressed = false;
                }
                if ctx.input(|i| i.key_down(Key::P)) {
                    ui.label("Finding file");
                }
            } else {
                ui.heading("Press/Hold/Release example. Press A to test.");
                if ui.button("Clear").clicked() {
                    self.text.clear();
                }
                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.label(&self.text);
                    });
                if ctx.input(|i| i.key_released(Key::Space)) {
                    self.space_previously_pressed = true;
                    return;
                }

                if ctx.input(|i| i.key_pressed(Key::A)) {
                    self.text.push_str("\nPressed");
                }
                if ctx.input(|i| i.key_down(Key::A)) {
                    self.text.push_str("\nHeld");
                    ui.ctx().request_repaint(); // make sure we note the holding.
                }
                if ctx.input(|i| i.key_released(Key::A)) {
                    self.text.push_str("\nReleased");
                }
            }
        });
    }
}
