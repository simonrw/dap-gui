use anyhow::{Context, Result};
use clap::Parser;
use eframe::{
    egui::{self, Style},
    epaint::FontId,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    io::{BufReader, Read},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Arc, Mutex, MutexGuard},
    thread,
};

use dap_gui_client::{
    requests::{self, RequestBody},
    responses::{self},
    types::{self, Breakpoint, Variable},
    Message, Reader, Reply, Writer, WriterProxy,
};

// argument parsing
#[derive(Debug, Parser)]
struct Args {
    #[clap(short, long, default_value = "false")]
    large: bool,
}

#[derive(Debug, Clone)]
struct Position {
    _x: usize,
    _y: usize,
}

#[derive(Debug, Clone)]
struct PausedState {
    current_thread_id: types::ThreadId,
    threads: Vec<types::Thread>,
    stack_frames: HashMap<i64, Vec<types::StackFrame>>,
    scopes: Option<HashMap<types::StackFrameId, Vec<types::Scope>>>,
    variables: Option<HashMap<types::VariablesReference, Vec<types::Variable>>>,
    _current_position: Position,
}

#[derive(Debug, Clone)]
enum AppStatus {
    WaitingForConnection,
    Starting,
    Started,
    Paused(PausedState),
    Running,
    Finished,
}

#[derive(Debug)]
struct MyAppState {
    sender: WriterProxy,
    status: AppStatus,
    breakpoints: Vec<Breakpoint>,
    source: String,
}

impl MyAppState {
    pub fn new(sender: WriterProxy) -> Self {
        let source = include_str!("../../test.py");

        tracing::debug!("creating initial state");
        Self {
            sender,
            status: AppStatus::WaitingForConnection,
            breakpoints: Vec::new(),
            source: source.to_string(),
        }
    }

    fn set_state(&mut self, state: AppStatus) {
        tracing::debug!("changing state to {:?}", state);
        self.status = state;
    }
}

#[derive(Debug, Clone)]
struct MyApp {
    state: Arc<Mutex<MyAppState>>,
    context: egui::Context,
}

#[derive(Debug, Deserialize)]
struct ControlMessage {
}

impl MyApp {
    fn new(ctx: egui::Context) -> Result<MyApp> {
        let input_stream =
            TcpStream::connect("127.0.0.1:5678").context("connecting to DAP server")?;
        let output_stream = input_stream.try_clone().unwrap();

        let (tx, rx) = mpsc::channel();

        let store = Arc::new(Mutex::new(HashMap::new()));
        let mut reader = Reader::new(BufReader::new(input_stream), tx, Arc::clone(&store));
        let mut sender = Writer::new(output_stream, Arc::clone(&store));
        let (wtx, wrx) = mpsc::channel();
        let writer_proxy = WriterProxy::new(wtx);

        thread::spawn(move || {
            for msg in wrx {
                match sender.send(msg) {
                    Ok(_) => {}
                    Err(e) => tracing::warn!("sending message to writer: {e}"),
                }
            }
        });

        let app = Self {
            state: Arc::new(Mutex::new(MyAppState::new(writer_proxy))),
            context: ctx,
        };

        // wait for control events
        let control_app = app.clone();
        let listener = TcpListener::bind("127.0.0.1:7777").expect("cannot bind to control socket");
        thread::spawn(move || {
            tracing::debug!("waiting for control events");
            for stream in listener.incoming() {
                let mut stream = stream.expect("loading stream");
                tracing::debug!("got control event");

                let mut buf = Vec::new();
                let _ = stream.read_to_end(&mut buf).unwrap();
                let message_s = std::str::from_utf8(&buf).unwrap();

                match serde_json::from_str::<ControlMessage>(message_s) {
                    Ok(message) => {
                        tracing::info!(?message, "received message");

                        let mut state = control_app.state.lock().unwrap();
                        if let AppStatus::WaitingForConnection = state.status {
                            // launch app
                            state.set_state(AppStatus::Starting);
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(?e, "invalid message, skipping");
                    }
                }
            }
        });

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

    fn handle_message(&mut self, reply: Reply) {
        use dap_gui_client::events::Event::*;
        match reply.message {
            Message::Response(r) => {
                use responses::ResponseBody::*;

                if let Some(body) = r.body {
                    match body {
                        Initialize(_init) => {
                            tracing::debug!("received initialize response");
                            self.set_state(AppStatus::Started)
                        }
                        SetFunctionBreakpoints(body) => {
                            tracing::debug!("received set function breakpoints response: {body:?}");
                            let mut state = self.state.lock().unwrap();

                            state.breakpoints = body.breakpoints;

                            state.sender.send_configuration_done();
                        }
                        Continue(body) => {
                            tracing::debug!("received continue response {body:?}");
                            self.set_state(AppStatus::Running);
                        }
                        Threads(body) => {
                            tracing::debug!("received threads response {body:?}");
                            let mut state = self.state.lock().unwrap();
                            match state.status {
                                AppStatus::Paused(PausedState {
                                    ref mut threads,
                                    current_thread_id,
                                    ..
                                }) => {
                                    let mut thread_ids = Vec::new();
                                    for thread in body.threads {
                                        thread_ids.push(thread.id);
                                        threads.push(thread);
                                    }
                                    state.sender.send_stacktrace_request(current_thread_id);
                                }
                                _ => unreachable!("invalid state"),
                            }
                        }
                        StackTrace(body) => {
                            let request = &reply.request.expect("no request found");
                            tracing::debug!(
                                "received threads response {body:?} with request {request:?}"
                            );
                            let mut state = self.state.lock().unwrap();
                            match state.status {
                                AppStatus::Paused(PausedState {
                                    ref mut stack_frames,
                                    ..
                                }) => match request.body {
                                    RequestBody::StackTrace(requests::StackTrace { thread_id }) => {
                                        stack_frames.insert(thread_id, body.stack_frames);
                                    }
                                    _ => unreachable!("invalid request type"),
                                },
                                _ => unreachable!("invalid state"),
                            }
                        }
                        Scopes(body) => {
                            let request = &reply.request.expect("no request found");
                            tracing::debug!(
                                "received scopes response {body:?} with request {request:?}"
                            );
                            let mut state = self.state.lock().unwrap();
                            match state.status {
                                AppStatus::Paused(PausedState { ref mut scopes, .. }) => {
                                    match request.body {
                                        RequestBody::Scopes(requests::Scopes { frame_id }) => {
                                            match scopes {
                                                Some(scopes) => {
                                                    scopes.insert(frame_id, body.scopes);
                                                }
                                                None => {
                                                    let mut hm = HashMap::new();
                                                    hm.insert(frame_id, body.scopes);
                                                    *scopes = Some(hm);
                                                }
                                            }
                                        }
                                        _ => unreachable!("invalid request type"),
                                    }
                                }
                                _ => unreachable!("invalid state"),
                            }
                        }
                        Variables(body) => {
                            let request = &reply.request.expect("no request found");
                            tracing::debug!(
                                "received variables response {body:?} with request {request:?}"
                            );
                            let mut state = self.state.lock().unwrap();
                            match state.status {
                                AppStatus::Paused(PausedState {
                                    ref mut variables, ..
                                }) => match request.body {
                                    RequestBody::Variables(requests::Variables {
                                        variables_reference,
                                    }) => match variables {
                                        Some(variables) => {
                                            if variables.contains_key(&variables_reference) {
                                                tracing::warn!("already found variables reference {variables_reference}");
                                                // TODO
                                            }
                                            variables.insert(variables_reference, body.variables);
                                        }
                                        None => {
                                            let mut hm = HashMap::new();
                                            hm.insert(variables_reference, body.variables);
                                            *variables = Some(hm);
                                        }
                                    },
                                    _ => unreachable!("invalid request type"),
                                },
                                _ => unreachable!("invalid state"),
                            }
                        }
                        b => tracing::warn!("unhandled response: {b:?}"),
                    }
                }
            }
            Message::Event(m) => match m {
                Initialized => {
                    tracing::debug!("received initialize event");

                    let breakpoints = vec![requests::FunctionBreakpoint {
                        name: "foo".to_string(),
                    }];

                    self.state
                        .lock()
                        .unwrap()
                        .sender
                        .send_set_function_breakpoints(breakpoints);
                }
                Output(o) => {
                    tracing::debug!("received output event: {}", o.output);
                }
                Process(body) => {
                    tracing::debug!("received process event: {:?}", body);
                    self.set_state(AppStatus::Running);
                }
                Stopped(body) => {
                    tracing::debug!("received stopped event, body: {:?}", body);
                    {
                        let state = self.state.lock().unwrap();
                        state.sender.send_threads_request();
                        state.sender.send_stacktrace_request(body.thread_id);
                    }

                    self.set_state(AppStatus::Paused(PausedState {
                        current_thread_id: body.thread_id,
                        threads: Vec::new(),
                        stack_frames: HashMap::new(),
                        scopes: None,
                        variables: None,
                        _current_position: Position { _x: 0, _y: 0 },
                    }));
                }
                Continued(body) => {
                    tracing::debug!("received continued event {body:?}");
                }
                Thread(_thread_info) => {
                    tracing::debug!("received thread event");
                }
                Exited(_body) => {
                    tracing::debug!("received exited event");
                    self.set_state(AppStatus::Finished);
                }
                Terminated => {
                    tracing::debug!("received terminated event");
                    self.set_state(AppStatus::Finished);
                }
                e => tracing::warn!("unhandled event {e:?}"),
            },
        }
        self.context.request_repaint();
    }

    fn render_central_panel(&self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label("dap-gui");

        let state = self.state.lock().unwrap();
        tracing::trace!("{:?}", state.status);
        // TODO: clone here is to work around duplicate borrow, we should not need to clone in
        // this case.
        match state.status.clone() {
            AppStatus::Started => {
                self.render_started_state(ui, state);
            }
            AppStatus::Paused(ref paused_state) => {
                self.render_paused_state(ui, ctx, state, paused_state);
            }
            AppStatus::Starting => {
                ui.label("STARTING");
            }
            AppStatus::Finished => {
                ui.label("FINISHED");
            }
            AppStatus::Running => {
                ui.label("Running...");
            }
            AppStatus::WaitingForConnection => {
                ui.label("Waiting for connection");
            }
        }
    }

    fn render_started_state(&self, ui: &mut egui::Ui, state: MutexGuard<'_, MyAppState>) {
        ui.horizontal(|ui| {
            if ui.button("Launch").clicked() {
                state.sender.send_launch();
            }
        });
    }

    fn render_paused_state(
        &self,
        _ui: &mut egui::Ui,
        ctx: &egui::Context,
        state: MutexGuard<'_, MyAppState>,
        paused_state: &PausedState,
    ) {
        tracing::trace!("app state: {paused_state:?}");

        egui::SidePanel::left("sidebar").show(ctx, |ui| {
            for thread in &paused_state.threads {
                ui.label(format!("thread {}", thread.name));
                ui.separator();
                if let Some(frames) = paused_state.stack_frames.get(&thread.id) {
                    for frame in frames {
                        if ui
                            .collapsing(format!("\t{}", frame.name), |ui| {
                                if let Some(scopes) =
                                    paused_state.scopes.as_ref().and_then(|s| s.get(&frame.id))
                                {
                                    for scope in scopes {
                                        if ui
                                            .collapsing(scope.name.to_string(), |ui| {
                                                if let Some(variables) = paused_state
                                                    .variables
                                                    .as_ref()
                                                    .and_then(|v| v.get(&scope.variables_reference))
                                                {
                                                    for variable in variables {
                                                        present_variable(variable, ui);
                                                    }
                                                }
                                            })
                                            .header_response
                                            .clicked()
                                        {
                                            tracing::debug!("uncollapsed");
                                            state.sender.send_variables(scope.variables_reference);
                                        };
                                    }
                                }
                            })
                            .header_response
                            .clicked()
                        {
                            // TODO: only first time
                            tracing::debug!("uncollapsed");
                            state.sender.send_scopes(frame.id);
                        }
                    }
                }
            }

            if ui.button("Continue").clicked() {
                state.sender.send_continue(paused_state.current_thread_id);
            }
        });

        // source code
        egui::CentralPanel::default().show(ctx, |ui| {
            dap_gui_ui::syntax_highlighting::code_view_ui(ui, &state.source);
        });
    }
}

fn present_variable(variable: &Variable, ui: &mut egui::Ui) {
    if let Some(variable_type) = variable.r#type.clone() {
        if !variable_type.is_empty() {
            ui.label(format!(
                "{} ({}) = {}",
                variable.name, variable_type, variable.value
            ));
            ui.label(format!("{} {}", variable.name, variable.value));
        } else {
            ui.label(format!("{} = {}", variable.name, variable.value));
        }
    } else {
        ui.label(format!("{} = {}", variable.name, variable.value));
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_central_panel(ui, ctx);
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

#[cfg(feature = "sentry")]
macro_rules! setup_sentry {
    () => {
        tracing::info!("setting up sentry for crash reporting");
        let _guard = sentry::init((
            "https://f08b65bc9944ecbb855f1ebb2cadcb92@o366030.ingest.sentry.io/4505663159926784",
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ));
    };
}

#[cfg(not(feature = "sentry"))]
macro_rules! setup_sentry {
    () => {};
}

fn main() -> Result<(), eframe::Error> {
    tracing_subscriber::fmt::init();

    setup_sentry!();

    let args = Args::parse();
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        ..Default::default()
    };
    eframe::run_native(
        "DAP GUI",
        options,
        Box::new(move |cc| {
            if args.large {
                let style = Style {
                    // temporarily increase font size
                    override_font_id: Some(FontId::monospace(24.0)),
                    ..Style::default()
                };
                cc.egui_ctx.set_style(style);
            }
            Box::new(MyApp::new(cc.egui_ctx.clone()).unwrap())
        }),
    )
}
