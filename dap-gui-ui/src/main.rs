use anyhow::{Context, Result};
use eframe::{
    egui::{self, Style, Visuals},
    epaint::FontId,
};
use std::{
    io::{BufReader, BufWriter},
    net::TcpStream,
    sync::{mpsc, Arc, Mutex},
    thread,
};

mod syntax_highlighting;

use dap_gui_client::{responses, Message, Reader, Writer};

#[derive(Debug, Clone, Copy)]
enum AppStatus {
    Starting,
    Started,
    Paused,
    Finished,
}

struct MyAppState {
    sender: Writer<TcpStream>,
    status: AppStatus,
    current_thread_id: Option<i64>,
}

impl MyAppState {
    pub fn new(sender: Writer<TcpStream>) -> Self {
        Self {
            sender,
            status: AppStatus::Starting,
            current_thread_id: None,
        }
    }

    fn set_state(&mut self, state: AppStatus) {
        log::debug!("changing state to {:?}", state);
        self.status = state;
    }
}

#[derive(Clone)]
struct MyApp {
    state: Arc<Mutex<MyAppState>>,
    context: egui::Context,
}

impl MyApp {
    fn new(ctx: egui::Context) -> Result<MyApp> {
        let input_stream =
            TcpStream::connect("127.0.0.1:5678").context("connecting to DAP server")?;
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
        Ok(app)
    }

    fn set_state(&mut self, state: AppStatus) {
        self.state.lock().unwrap().set_state(state);
    }

    fn handle_message(&mut self, message: Message) {
        use dap_gui_client::events::Event::*;
        match message {
            Message::Response(r) => {
                use responses::ResponseBody::*;

                if let Some(body) = r.body {
                    match body {
                        Initialize(_init) => {
                            log::debug!("received initialize response");
                            self.set_state(AppStatus::Started)
                        }
                        SetFunctionBreakpoints(_bps) => {
                            log::debug!("received set function breakpoints response");
                            let mut state = self.state.lock().unwrap();
                            state.sender.send_configuration_done();
                        }
                        Continue(body) => {
                            log::debug!("received continue response {body:?}");
                            self.set_state(AppStatus::Started);
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
                Process(_body) => {
                    log::debug!("received process event");
                }
                Stopped(body) => {
                    log::debug!("received stopped event, body: {:?}", body);
                    {
                        let mut state = self.state.lock().unwrap();
                        state.current_thread_id = Some(body.thread_id);
                    }
                    self.set_state(AppStatus::Paused);
                }
                Continued(body) => {
                    log::debug!("received continued event {body:?}");
                }
                Thread(_thread_info) => {
                    log::debug!("received thread event");
                }
                Exited(_body) => {
                    log::debug!("received exited event");
                    self.set_state(AppStatus::Finished);
                }
                Terminated => {
                    log::debug!("received terminated event");
                    self.set_state(AppStatus::Finished);
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

                        if let Some(thread_id) = state.current_thread_id {
                            state.sender.send_continue(thread_id);
                        };
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

        /*
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
        */
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
            Box::new(MyApp::new(cc.egui_ctx.clone()).unwrap())
        }),
    )
}
