use anyhow::Result;
use eframe::{
    egui::{self, Style, TextStyle, Visuals},
    epaint::FontId,
};
use std::{
    io::{BufReader, BufWriter},
    net::TcpStream,
    sync::{mpsc, Arc, Mutex},
    thread,
};

mod syntax_highlighting;

use dap_gui_client::{events, responses, Message, Reader, Writer};

enum AppStatus {
    Starting,
    Started,
    Paused,
    Finished,
}

struct MyAppState {
    sender: Writer<TcpStream>,
    status: AppStatus,
}

impl MyAppState {
    pub fn new(sender: Writer<TcpStream>) -> Self {
        Self {
            sender,
            status: AppStatus::Starting,
        }
    }
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
            state: Arc::new(Mutex::new(MyAppState::new(sender))),
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
        use dap_gui_client::events::Event::*;
        match message {
            Message::Response(r) => {
                if let Some(body) = r.body {
                    match body {
                        responses::ResponseBody::Initialize(_init) => {
                            log::debug!("received initialize response");
                            let mut state = self.state.lock().unwrap();
                            state.status = AppStatus::Started;
                        }
                        responses::ResponseBody::SetFunctionBreakpoints(_bps) => {
                            log::debug!("received set function breakpoints response");
                            let mut state = self.state.lock().unwrap();
                            state.sender.send_configuration_done();
                        }
                    }
                }
            }
            Message::Event(m) => match m {
                Initialized => {
                    log::debug!("received initialize event");

                    self.state
                        .lock()
                        .unwrap()
                        .sender
                        .send_set_function_breakpoints();
                }
                Output(o) => {
                    log::debug!("received output event: {}", o.output);
                }
                Process => {
                    log::debug!("received process event");
                }
                Stopped(_body) => {
                    log::debug!("received stopped event");
                    self.state.lock().unwrap().status = AppStatus::Paused;
                }
            },
        }
        self.context.request_repaint();
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut state = self.state.lock().unwrap();
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("dap-gui");

            match state.status {
                AppStatus::Started => {
                    ui.horizontal(|ui| {
                        if ui.button("Launch").clicked() {
                            state.sender.send_launch();
                        }
                    });
                }
                AppStatus::Paused => {
                    if ui.button("Continue").clicked() {
                        // TODO: move this into response handler
                        // state.status = AppStatus::Started;

                        state.sender.send_continue();
                    }
                }
                AppStatus::Starting => {
                    ui.label("STARTING");
                }
                AppStatus::Finished => {
                    ui.label("FINISHED");
                }
            }
        });

        return;
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
    env_logger::init();
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        ..Default::default()
    };
    eframe::run_native(
        "DAP GUI",
        options,
        Box::new(|cc| {
            let style = Style {
                visuals: Visuals::dark(),
                // temporarily increase font size
                override_font_id: Some(FontId::monospace(24.0)),
                ..Style::default()
            };
            cc.egui_ctx.set_style(style);
            Box::new(MyApp::new(cc.egui_ctx.clone()))
        }),
    )
}
