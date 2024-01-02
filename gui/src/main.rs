use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use clap::{Parser, Subcommand};
use code_view::CodeView;
use debugger::{AttachArguments, Debugger, FileSource};
use eframe::egui;
use eyre::WrapErr;
use transport::types::StackFrame;

mod code_view;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Attach,
    Launch,
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

enum State {
    Initialising,
    Running,
    Paused {
        stack: Vec<StackFrame>,
        source: FileSource,
        breakpoints: Vec<debugger::Breakpoint>,
    },
    Terminated,
}

impl From<debugger::Event> for State {
    fn from(event: debugger::Event) -> Self {
        match event {
            debugger::Event::Initialised => State::Running,
            debugger::Event::Paused {
                stack,
                source,
                breakpoints,
            } => State::Paused {
                stack,
                source,
                breakpoints,
            },
            debugger::Event::Running => State::Running,
            debugger::Event::Ended => State::Terminated,
            debugger::Event::Uninitialised => State::Initialising,
        }
    }
}

struct DebuggerAppState {
    state: State,
    debugger: Debugger,
}

impl DebuggerAppState {
    #[tracing::instrument(skip(self))]
    fn handle_event(&mut self, event: &debugger::Event) -> eyre::Result<()> {
        tracing::debug!("handling event");
        self.state = event.clone().into();
        Ok(())
    }
}

struct DebuggerApp {
    inner: Arc<Mutex<DebuggerAppState>>,
}

impl DebuggerApp {
    fn new(args: Args, _cc: &eframe::CreationContext<'_>) -> eyre::Result<Self> {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        let cwd = std::env::current_dir().unwrap();
        let debugger = match args.command {
            Command::Attach => {
                let launch_arguments = AttachArguments {
                    working_directory: cwd,
                    port: None,
                    language: debugger::Language::DebugPy,
                };

                Debugger::new(launch_arguments).context("creating internal debugger")?
            }
            Command::Launch => todo!(),
        };

        let events = debugger.events();

        debugger.wait_for_event(|e| matches!(e, debugger::Event::Initialised));

        // TEMP
        debugger
            .add_breakpoint(debugger::Breakpoint {
                path: PathBuf::from("./attach.py"),
                line: 9,
                ..Default::default()
            })
            .context("adding temp breakpoint")?;
        debugger.launch().context("launching debugee")?;

        let temp_state = DebuggerAppState {
            state: State::Initialising,
            debugger,
        };

        let inner = Arc::new(Mutex::new(temp_state));
        let background_inner = Arc::clone(&inner);

        thread::spawn(move || loop {
            if let Ok(event) = events.recv() {
                if let Err(e) = background_inner.lock().unwrap().handle_event(&event) {
                    tracing::warn!(error = %e, "handling debugger event");
                }
            }
        });

        Ok(Self { inner })
    }
}

impl eframe::App for DebuggerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Debugger");
            let inner = self.inner.lock().unwrap();

            match &inner.state {
                State::Initialising => {}
                State::Running => {}
                State::Paused {
                    stack: _stack,
                    source,
                    breakpoints: original_breakpoints,
                } => {
                    egui::SidePanel::left("left-panel").show(ctx, |ui| {
                        ui.heading("Variables");
                        ui.heading("Stack Frames");
                    });

                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.heading("Paused");

                        let contents = if let Some(ref path) = source.file_path {
                            // TODO: Result
                            std::fs::read_to_string(path)
                                .expect("reading source contents from file")
                        } else {
                            unreachable!("no file source specified");
                        };
                        let mut breakpoints = HashSet::from_iter(
                            original_breakpoints
                                .iter()
                                .filter(|b| {
                                    source
                                        .file_path
                                        .as_ref()
                                        .map(|s| s == b.path.as_path())
                                        .unwrap_or(false)
                                })
                                .cloned(),
                        );

                        ui.add(CodeView::new(
                            &contents,
                            source.line,
                            true,
                            &mut breakpoints,
                        ));

                        if ui.button("continue").clicked() {
                            inner.debugger.r#continue().unwrap();
                        }
                    });
                }
                State::Terminated => {
                    ui.label("Program terminated");
                }
            }
        });
    }
}

fn main() -> eyre::Result<()> {
    setup_sentry!();
    let _ = tracing_subscriber::fmt::try_init();

    let args = Args::parse();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "My egui App",
        native_options,
        Box::new(|cc| {
            let app = DebuggerApp::new(args, cc).expect("creating main application");
            Box::new(app)
        }),
    )
    .map_err(|e| eyre::eyre!("running gui mainloop: {e}"))
}
