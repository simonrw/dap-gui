use anyhow::Result;
use eframe::egui;
use std::{
    io::{BufReader, BufWriter},
    net::TcpStream,
    sync::{mpsc, Arc, Mutex},
    thread,
};

mod syntax_highlighting;

use dap_gui_client::{events, responses, Message, Reader, Writer};

struct MyAppState {
    sender: Writer<TcpStream>,
}

#[derive(Clone)]
struct MyApp {
    state: Arc<Mutex<MyAppState>>,
    context: egui::Context,
}

impl MyApp {
    fn new(ctx: egui::Context) -> MyApp {
        let input_stream = TcpStream::connect("127.0.0.1:5678").unwrap();
        let output_stream = input_stream.try_clone().unwrap();

        let (tx, rx) = mpsc::channel();

        let mut reader = Reader::new(BufReader::new(input_stream), tx);
        let mut sender = Writer::new(BufWriter::new(output_stream));

        sender.send_initialize();

        let app = Self {
            state: Arc::new(Mutex::new(MyAppState { sender })),
            context: ctx,
        };
        thread::spawn(move || {
            reader.poll_loop();
        });

        let mut background_app = app.clone();
        thread::spawn(move || {
            for msg in rx {
                background_app.handle_message(msg);
            }
        });
        app
    }
    fn handle_message(&mut self, message: Message) {
        match message {
            Message::Event(m) => match m {
                events::Event::Initialized => {
                    eprintln!("server ready to receive breakpoint commands");

                    self.state
                        .lock()
                        .unwrap()
                        .sender
                        .send_set_function_breakpoints();
                }
                events::Event::Output(o) => {
                    eprintln!("{}", o.output);
                }
            },
            Message::Response(r) => {
                if let Some(body) = r.body {
                    match body {
                        responses::ResponseBody::Initialize(_init) => {
                            self.state.lock().unwrap().sender.send_launch();
                        }
                        responses::ResponseBody::SetFunctionBreakpoints(bps) => {
                            dbg!(bps);
                        }
                    }
                }
            }
        }
        self.context.request_repaint();
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::right("right_panel")
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("right panel");
                // split into
                // - variables
                // - stack frames
                // - breakpoints
            });
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.label("bottom panel");
            // split into
            // - repl
            // - output?
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("central panel");

            // code
            let example_code = include_str!("./main.rs");

            egui::ScrollArea::vertical().show(ui, |ui| {
                syntax_highlighting::code_view_ui(ui, example_code);
            });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        ..Default::default()
    };
    eframe::run_native(
        "DAP GUI",
        options,
        Box::new(|cc| Box::new(MyApp::new(cc.egui_ctx.clone()))),
    )
}
