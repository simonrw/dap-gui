struct App {}

impl App {
    fn new(_cc: &eframe::CreationContext) -> Self {
        App {}
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        todo!()
    }
}

fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "DAP Debugger",
        native_options,
        Box::new(|cc| {
            let app = App::new(cc);
            Box::new(app)
        }),
    )
    .unwrap();
}
