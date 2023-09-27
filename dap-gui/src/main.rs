use std::{
    env::current_dir,
    io::{BufRead, BufReader},
    net::{TcpListener, TcpStream},
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
    events::{self, OutputEventBody},
    requests::{self, Initialize},
    responses, Received,
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
}

#[derive(Debug)]
struct AppState {
    launch_config: LaunchConfiguration,
    working_directory: String,
    contents: String,

    debugger_status: DebuggerStatus,
}

impl AppState {
    fn handle_message(&mut self, message: Received) {
        match message {
            Received::Event(event) => self.handle_event(event),
            Received::Response(request, response) => self.handle_response(request, response),
        }
    }

    fn handle_event(&mut self, event: events::Event) {
        match event {
            events::Event::Initialized => todo!(),
            events::Event::Output(OutputEventBody { output, source, .. }) => {
                tracing::debug!(%output, ?source, "output event");
                self.debugger_status = DebuggerStatus::Initialized;
            }
            events::Event::Process(_) => todo!(),
            events::Event::Stopped(_) => todo!(),
            events::Event::Continued(_) => todo!(),
            events::Event::Thread(_) => todo!(),
            events::Event::Exited(_) => todo!(),
            events::Event::Module(_) => todo!(),
            events::Event::Terminated => todo!(),
            events::Event::Unknown(_) => todo!(),
            _ => todo!(),
        }
    }

    fn handle_response(&mut self, _request: requests::RequestBody, response: responses::Response) {
        if let Some(body) = response.body {
            match body {
                responses::ResponseBody::Initialize(_) => {
                    tracing::debug!("initialize response");
                    // TODO: store capabilities
                }
                responses::ResponseBody::SetFunctionBreakpoints(_) => todo!(),
                responses::ResponseBody::Continue(_) => todo!(),
                responses::ResponseBody::Threads(_) => todo!(),
                responses::ResponseBody::StackTrace(_) => todo!(),
                responses::ResponseBody::Scopes(_) => todo!(),
                responses::ResponseBody::Variables(_) => todo!(),
                responses::ResponseBody::ConfigurationDone => todo!(),
                responses::ResponseBody::Terminate => todo!(),
                responses::ResponseBody::Disconnect => todo!(),
                _ => todo!(),
            }
        }
    }

    fn render(&mut self, ctx: &egui::Context) {
        match self.debugger_status {
            DebuggerStatus::Running => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("Configuring");
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
        mut client: dap_gui_client::Client,
        client_events: Receiver<Received>,
    ) -> Self {
        // set up background thread watching events

        let filename = "test.py".to_string();
        let state = Arc::new(Mutex::new(AppState {
            debugger_status: DebuggerStatus::Running,
            launch_config: LaunchConfiguration::File {
                filename: filename.clone(),
            },
            working_directory: "/home/simon/dev/dap-gui".to_string(),
            contents: std::fs::read_to_string(&filename).unwrap(),
        }));
        /*
        let trigger_socket =
            TcpListener::bind("127.0.0.1:8989").expect("could not bind control socket");
        let control_state = Arc::clone(&state);
        control_worker(trigger_socket, control_state);
        */

        let background_state = Arc::clone(&state);
        thread::spawn(move || {
            for msg in client_events {
                handle_message(msg, Arc::clone(&background_state));
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

fn handle_message(msg: Received, state_m: Arc<Mutex<AppState>>) {
    let mut state = state_m.lock().unwrap();
    state.handle_message(msg);
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // TODO: locked for too long?
        let mut state = self.app_state.lock().unwrap();
        state.render(ctx);
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
