use std::{
    collections::HashSet,
    env::current_dir,
    net::TcpStream,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use anyhow::{Context, Result};
use clap::Parser;
use eframe::egui::{self, TextEdit, Visuals};
use gui::code_view::CodeView;
use gui::debug_server::{DebugServerConfig, PythonDebugServer};
use serde::{Deserialize, Serialize};
use transport::{
    bindings::get_random_tcp_port,
    events::{self, OutputEventBody, StoppedEventBody},
    requests::{self, Initialize, PathFormat},
    responses,
    types::{self, Source, SourceBreakpoint, ThreadId},
    Client, Received,
};

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
    line: Option<usize>,

    current_thread_id: Option<ThreadId>,
    debugger_status: DebuggerStatus,
    should_quit: bool,

    capabilities: Option<responses::Capabilities>,

    breakpoints: Vec<types::Breakpoint>,

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

                // work out where to show the code window
                send!(
                    client,
                    "stackTrace",
                    requests::RequestBody::StackTrace(requests::StackTrace {
                        thread_id: *thread_id,
                        start_frame: Some(0),
                        levels: Some(1),
                        ..Default::default()
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
                let breakpoints: Vec<_> = vec![1, 4, 8, 11, 20]
                    .iter()
                    .map(|line| SourceBreakpoint {
                        line: *line,
                        ..Default::default()
                    })
                    .collect();
                send!(
                    client,
                    "setBreakpoints",
                    requests::RequestBody::SetBreakpoints(requests::SetBreakpoints {
                        source: Source {
                            name: Some("test.py".to_string()),
                            path: Some(self.working_directory.join("test.py")),
                            ..Default::default()
                        },
                        // deprecated api
                        lines: Some(
                            breakpoints
                                .iter()
                                .map(|breakpoint| breakpoint.line)
                                .collect()
                        ),
                        breakpoints: Some(breakpoints),
                        ..Default::default()
                    })
                );

                if self
                    .capabilities
                    .as_ref()
                    .and_then(|cs| cs.supports_loaded_sources_request)
                    .unwrap_or_default()
                {
                    send!(
                        client,
                        "loadedSources",
                        requests::RequestBody::LoadedSources
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
                        "launch",
                        requests::RequestBody::Launch(requests::Launch {
                            program: self.working_directory.join("test.py"),
                            launch_arguments: None,
                        })
                    );
                    // send!(
                    //     client,
                    //     "attach",
                    //     requests::RequestBody::Attach(requests::Attach {
                    //         connect: requests::ConnectInfo {
                    //             host: "localhost".to_string(),
                    //             port: 5678,
                    //         },
                    //         path_mappings: vec![],
                    //         just_my_code: false,
                    //         workspace_folder: self.working_directory.clone(),
                    //     })
                    // );
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
                responses::ResponseBody::SetBreakpoints(responses::SetBreakpoints {
                    breakpoints,
                }) => {
                    self.breakpoints = breakpoints;
                    // TODO: handle multiple breakpoints
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
                    self.stack_frames = stack_frames;

                    // the first stack frame is the file to show
                    let current_frame = &self.stack_frames[0];
                    let current_file = current_frame
                        .source
                        .as_ref()
                        .unwrap()
                        .path
                        .as_ref()
                        .unwrap();
                    self.contents =
                        std::fs::read_to_string(current_file).expect("reading file contents");
                    self.line = Some(current_frame.line);
                    tracing::debug!(file = %current_file.display(), "inspecting stack frames");

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
                        if ui.button(format!("{} {}", frame.id, frame.name)).clicked() {
                            self.contents = std::fs::read_to_string(
                                frame.source.as_ref().unwrap().path.as_ref().unwrap(),
                            )
                            .unwrap();
                            self.line = Some(frame.line);
                            ctx.request_repaint();
                            return;
                        }
                    }
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Paused");

                    let mut breakpoint_positions: HashSet<usize> = self
                        .breakpoints
                        .iter()
                        .filter_map(|breakpoint| breakpoint.line.map(|line| line as usize))
                        .collect();

                    if let Some(ref current_line) = self.line {
                        ui.add(CodeView::new(
                            &self.contents,
                            *current_line,
                            true,
                            &mut breakpoint_positions,
                        ));
                    } else {
                        ui.add(CodeView::new(
                            &self.contents,
                            0,
                            false,
                            &mut breakpoint_positions,
                        ));
                    }

                    // TODO: update our breakpoint positons

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
    client: transport::Client,
    app_state: Arc<Mutex<AppState>>,
}

impl MyApp {
    pub fn new(
        ctx: egui::Context,
        client: transport::Client,
        client_events: crossbeam_channel::Receiver<Received>,
    ) -> Self {
        // set up background thread watching events

        let filename = "test.py".to_string();
        let state = Arc::new(Mutex::new(AppState {
            debugger_status: DebuggerStatus::Running,
            launch_config: LaunchConfiguration::File {
                filename: filename.clone(),
            },
            working_directory: std::env::current_dir().unwrap(),
            contents: std::fs::read_to_string(&filename).unwrap(),
            line: None,
            current_thread_id: None,
            should_quit: false,
            capabilities: None,
            stack_frames: Default::default(),
            breakpoints: Vec::new(),
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
            loop {
                let msg = client_events.recv().unwrap();
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
            lines_start_at_one: true,
            path_format: PathFormat::Path,
            supports_start_debugging_request: true,
            supports_variable_type: true,
            supports_variable_paging: true,
            supports_progress_reporting: true,
            supports_memory_event: true,
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

    #[clap(long, default_value = "false")]
    light: bool,

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
    let light_mode = match &args {
        Arguments::Launch(LaunchArguments { light, .. }) => *light,
        Arguments::Attach(_) => todo!(),
    };

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
    let (tx, rx) = crossbeam_channel::unbounded();
    let stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    let client = transport::Client::new(stream, tx).unwrap();

    let res = eframe::run_native(
        "DAP GUI",
        options,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            if light_mode {
                ctx.set_visuals(Visuals::light());
            } else {
                ctx.set_visuals(Visuals::dark());
            };
            Box::new(MyApp::new(ctx, client, rx))
        }),
    );

    tracing::info!("exiting");

    if let Err(e) = res {
        anyhow::bail!("error running GUI: {e}");
    }

    Ok(())
}
