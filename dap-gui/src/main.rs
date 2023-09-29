use std::{
    env::current_dir,
    net::TcpStream,
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread,
};

use anyhow::{Context, Result};
use clap::Parser;
use dap_gui::debug_server::{DebugServerConfig, PythonDebugServer};
use dap_gui_client::{
    bindings::get_random_tcp_port,
    events::{self, OutputEventBody, StoppedEventBody},
    requests::{self, Initialize},
    responses,
    types::{self, ThreadId},
    Client, Received,
};
use eframe::egui::{self, TextEdit};
use serde::{Deserialize, Serialize};

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

#[derive(Debug)]
enum DebuggerStatus {
    Running,
    Initialized,
    Paused,
}

#[derive(Debug)]
struct AppState {
    launch_config: LaunchConfiguration,
    working_directory: PathBuf,
    contents: String,

    current_thread_id: Option<ThreadId>,
    debugger_status: DebuggerStatus,
    should_quit: bool,

    // variables
    stack_frames: Vec<types::StackFrame>,
}

enum HandleNext {
    Continue,
    Break,
}

// macro to simplify sending messages handling the error response
macro_rules! send {
    ($client:expr, $name:literal, $body:expr) => {{
        if let Err(e) = $client.send($body) {
            tracing::warn!(error = %e, concat!("sending ", $name, " request"));
        }
    }}
}

impl AppState {
    fn handle_message(&mut self, client: &Client, message: Received) -> HandleNext {
        match message {
            Received::Event(event) => self.handle_event(client, event),
            Received::Response(request, response) => {
                self.handle_response(client, request, response)
            }
        }
    }

    #[tracing::instrument(skip(self, client))]
    fn handle_event(&mut self, client: &Client, event: events::Event) -> HandleNext {
        tracing::trace!("got event");
        match event {
            events::Event::Output(OutputEventBody { output, source, .. }) => {
                tracing::debug!(%output, ?source, "output event");
            }
            events::Event::Stopped(StoppedEventBody { ref thread_id, .. }) => {
                self.current_thread_id = Some(*thread_id);
                self.debugger_status = DebuggerStatus::Paused;

                send!(
                    client,
                    "stackTrace",
                    requests::RequestBody::StackTrace(requests::StackTrace {
                        thread_id: *thread_id,
                    })
                );
            }
            events::Event::Thread(events::ThreadEventBody { reason, thread_id }) => {
                let span = tracing::debug_span!("thread event", ?reason, %thread_id);
                let _guard = span.enter();

                tracing::debug!("thread status");
            }
            events::Event::Initialized => {
                self.debugger_status = DebuggerStatus::Initialized;

                // configure
                send!(
                    client,
                    "setFunctionBreakpoints",
                    requests::RequestBody::SetFunctionBreakpoints(
                        requests::SetFunctionBreakpoints {
                            breakpoints: vec![requests::Breakpoint {
                                name: "foo".to_string(),
                            }],
                        },
                    )
                );
                }
            }
            events::Event::Continued(_) => {
                self.debugger_status = DebuggerStatus::Running;
            }
            events::Event::Terminated => {
                tracing::debug!("debugee ended");
                self.should_quit = true;
                return HandleNext::Break;
            }
            _ => tracing::warn!("todo"),
        }
        HandleNext::Continue
    }

    #[tracing::instrument(skip(self, client, request))]
    fn handle_response(
        &mut self,
        client: &Client,
        request: requests::RequestBody,
        response: responses::Response,
    ) -> HandleNext {
        if !response.success {
            tracing::warn!(request = ?request, "unsuccessful request");
            return HandleNext::Continue;
        }

        if let Some(body) = response.body {
            match body {
                responses::ResponseBody::Initialize(capabilities) => {
                    tracing::debug!("initialize response");
                    self.capabilities = Some(capabilities);

                    // send launch
                    send!(
                        client,
                        "attach",
                        requests::RequestBody::Attach(requests::Attach {
                            connect: requests::ConnectInfo {
                                host: "localhost".to_string(),
                                port: 5678,
                            },
                            path_mappings: vec![],
                            just_my_code: false,
                            workspace_folder: self.working_directory.clone(),
                        })
                    );
                }
                responses::ResponseBody::SetFunctionBreakpoints(_) => {
                    tracing::debug!("set function breakpoints response");
                    // TODO: handle multiple line/function breakpoints
                    // for now only one breakpoint has been set
                    send!(
                        client,
                        "configurationDone",
                        requests::RequestBody::ConfigurationDone
                    );
                }
                responses::ResponseBody::ConfigurationDone => {
                    tracing::debug!("configuration done");
                    self.debugger_status = DebuggerStatus::Running;
                }
                responses::ResponseBody::StackTrace(responses::StackTraceResponse {
                    stack_frames,
                }) => {
                    tracing::debug!(?stack_frames, "inspecting stack frames");
                    self.stack_frames = stack_frames;

                    for frame in &self.stack_frames {
                        send!(
                            client,
                            "scopes",
                            requests::RequestBody::Scopes(requests::Scopes { frame_id: frame.id })
                        );
                    }
                }
                responses::ResponseBody::Scopes(responses::ScopesResponse { .. }) => {
                    tracing::debug!("scopes response");
                }
                _ => tracing::warn!("todo"),
            }
        } else {
            tracing::warn!("no body specified");
        }

        HandleNext::Continue
    }

    fn render(&mut self, client: &Client, ctx: &egui::Context) {
        match self.debugger_status {
            DebuggerStatus::Running => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Program running");
                });
            }
            DebuggerStatus::Paused => {
                // sidebar
                egui::SidePanel::left("left-panel").show(ctx, |ui| {
                    ui.heading("Variables");

                    ui.heading("Stack Frames");
                    for frame in &self.stack_frames {
                        // TODO: change position in the stack
                        ui.label(format!("{} {}", frame.id, frame.name));
                    }
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Paused");

                    // TODO: show the current breakpoint position

                    ui.add(
                        TextEdit::multiline(&mut self.contents)
                            .code_editor()
                            .interactive(false),
                    );

                    if ui.button("Continue").clicked() {
                        // TODO: how to trigger continue cleanly
                        send!(
                            client,
                            "continue",
                            requests::RequestBody::Continue(requests::Continue {
                                thread_id: self.current_thread_id.take().unwrap(),
                                single_thread: false,
                            })
                        );
                    }
                });
            }
            DebuggerStatus::Initialized => {
                egui::CentralPanel::default().show(ctx, |ui| match self.launch_config {
                    LaunchConfiguration::File { ref filename } => {
                        ui.label(filename);
                        ui.add(
                            TextEdit::multiline(&mut self.contents)
                                .code_editor()
                                .interactive(false),
                        );
                    }
                });
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
#[non_exhaustive]
enum LaunchConfiguration {
    File { filename: String },
    // Module { module: String },
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "command")]
#[non_exhaustive]
enum Command {
    Launch {
        language: Language,
        #[serde(flatten)]
        launch_config: LaunchConfiguration,
        working_directory: String,
    },
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[non_exhaustive]
enum Language {
    Python,
}

/*
fn _control_worker(listener: TcpListener, state: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        loop {
            if let Ok((socket, addr)) = listener.accept() {
                tracing::debug!(?addr, "got connection");
                // read instruction
                let mut reader = BufReader::new(socket);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();

                match serde_json::from_str::<Command>(&line) {
                    Ok(command) => {
                        tracing::debug!(?command, "got valid command");

                        match command {
                            Command::Launch {
                                launch_config,
                                working_directory,
                                ..
                            } => {
                                // TODO: dispatch on language
                                // update state accordingly
                                let mut unlocked_state = state.lock().unwrap();
                                *unlocked_state = AppState {
                                    debugger_status: DebuggerStatus::Running,
                                    launch_config,
                                    working_directory,
                                };
                            }
                        }
                        // TODO: trigger refresh
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "parsing command");
                    }
                }
            };
        }
    });
}
*/

#[allow(dead_code)]
struct MyApp {
    client: dap_gui_client::Client,
    app_state: Arc<Mutex<AppState>>,
}

impl MyApp {
    pub fn new(
        ctx: egui::Context,
        client: dap_gui_client::Client,
        client_events: Receiver<Received>,
    ) -> Self {
        // set up background thread watching events

        let filename = "test.py".to_string();
        let state = Arc::new(Mutex::new(AppState {
            debugger_status: DebuggerStatus::Running,
            launch_config: LaunchConfiguration::File {
                filename: filename.clone(),
            },
            working_directory: PathBuf::from("/home/simon/dev/dap-gui"),
            contents: std::fs::read_to_string(&filename).unwrap(),
            current_thread_id: None,
            should_quit: false,
            stack_frames: Default::default(),
        }));
        /*
        let trigger_socket =
            TcpListener::bind("127.0.0.1:8989").expect("could not bind control socket");
        let control_state = Arc::clone(&state);
        control_worker(trigger_socket, control_state);
        */

        let background_state = Arc::clone(&state);
        let background_client = client.clone();
        thread::spawn(move || {
            for msg in client_events {
                if let HandleNext::Break =
                    handle_message(msg, &background_client, Arc::clone(&background_state))
                {
                    tracing::debug!("background message thread shutting down");
                    break;
                }
                // refresh the UI
                ctx.request_repaint();
            }
        });

        // send initialize
        let req = requests::RequestBody::Initialize(Initialize {
            adapter_id: "dap gui".to_string(),
        });
        tracing::info!("initializing debug adapter");
        client.send(req).unwrap();

        Self {
            client,
            app_state: state,
        }
    }
}

fn handle_message(msg: Received, client: &Client, state_m: Arc<Mutex<AppState>>) -> HandleNext {
    let mut state = state_m.lock().unwrap();
    state.handle_message(client, msg)
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // TODO: locked for too long?
        let mut state = self.app_state.lock().unwrap();
        if state.should_quit {
            frame.close();
        }
        state.render(&self.client, ctx);
    }
}

#[derive(Debug, Parser)]
struct AttachArguments {}

#[derive(Debug, Parser)]
struct LaunchArguments {
    #[clap(short, long)]
    working_directory: Option<PathBuf>,

    #[clap(short, long)]
    file: PathBuf,
}

#[derive(Debug, Parser)]
enum Arguments {
    Attach(AttachArguments),
    Launch(LaunchArguments),
}

fn main() -> Result<()> {
    setup_sentry!();
    tracing_subscriber::fmt::init();

    let args = Arguments::parse();

    // start debug server in the background
    let port = get_random_tcp_port().context("no free ports available")?;
    let _debug_server = match args {
        Arguments::Launch(args @ LaunchArguments { .. }) => {
            PythonDebugServer::new(DebugServerConfig {
                working_dir: args
                    .working_directory
                    .unwrap_or_else(|| current_dir().unwrap()),
                filename: args.file,
                port,
            })
            .context("launching debugpy")?
        }
        Arguments::Attach(_) => todo!(),
    };

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1024.0, 768.0)),
        ..Default::default()
    };
    // TODO: connect to DAP server once language is known
    let (tx, rx) = mpsc::channel();
    let stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    let client = dap_gui_client::Client::new(stream, tx).unwrap();

    let res = eframe::run_native(
        "DAP GUI",
        options,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            Box::new(MyApp::new(ctx, client, rx))
        }),
    );

    tracing::info!("exiting");

    if let Err(e) = res {
        anyhow::bail!("error running GUI: {e}");
    }

    Ok(())
}
