use eframe::egui;

#[derive(Default)]
struct App {}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {}
}

fn main() {
    eframe::run_native(
        "App",
        eframe::NativeOptions::default(),
        Box::new(|_cc| {
            let app = App::default();
            Box::new(app)
        }),
    )
    .unwrap();
}
