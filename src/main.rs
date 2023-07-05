use eframe::egui;
use std::net;
use std::io::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[derive(Default)]
struct InitializeArguments {
    adapter_id: String,
}


#[derive(Serialize)]
enum Command {
    Initialize(InitializeArguments),
}

#[derive(Deserialize)]
struct InitializeResponse {
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Response {
    Initialize(InitializeResponse),
}


fn main() {
    let cmd = Command::Initialize(InitializeArguments {
        adapter_id: "dap-gui".to_string(),
        ..Default::default()
    });
    let msg_ser = serde_json::to_string(&cmd).unwrap();
    let mut conn = net::TcpStream::connect("127.0.0.1:5678").unwrap();
    write!(conn, "{}", msg_ser).unwrap();

    let mut buf = [0u8; 1024];
    let n = conn.read(&mut buf[..]).unwrap();
    let reply_ser = &buf[..n];

    let resp: 

}


fn egui_main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(320.0, 240.0)),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|_cc| Box::<MyApp>::default()),
    )
}

struct MyApp {
    name: String,
    age: u32,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: "Arthur".to_owned(),
            age: 42,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            if ui.button("Click each year").clicked() {
                self.age += 1;
            }
            ui.label(format!("Hello '{}', age {}", self.name, self.age));
        });
    }
}
