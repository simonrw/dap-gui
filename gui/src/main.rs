use std::{
    fs::create_dir_all,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use clap::Parser;
use debugger::{AttachArguments, Debugger, PausedFrame};
use eframe::egui;
use eyre::{OptionExt, WrapErr};
use launch_configuration::{Debugpy, LaunchConfiguration};
use transport::types::StackFrame;

mod code_view;
mod renderer;

#[derive(Parser)]
struct Args {
    config_path: PathBuf,

    #[clap(short, long)]
    name: String,
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

#[derive(Clone)]
enum State {
    Initialising,
    Running,
    Paused {
        stack: Vec<StackFrame>,
        paused_frame: PausedFrame,
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
                paused_frame,
                breakpoints,
            } => State::Paused {
                stack,
                paused_frame,
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
    previous_state: Option<State>,
}

impl DebuggerAppState {
    #[tracing::instrument(skip(self))]
    fn handle_event(&mut self, event: &debugger::Event) -> eyre::Result<()> {
        tracing::debug!("handling event");
        self.previous_state = Some(self.state.clone());
        self.state = event.clone().into();
        Ok(())
    }
}

struct DebuggerApp {
    inner: Arc<Mutex<DebuggerAppState>>,
}

impl DebuggerApp {
    fn new(args: Args, cc: &eframe::CreationContext<'_>) -> eyre::Result<Self> {
        let state_path = dirs::state_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("dapgui")
            .join("state.json");
        if !state_path.is_dir() {
            create_dir_all(state_path.parent().unwrap()).context("creating state directory")?;
        }
        let persisted_state = state::read_from(&state_path).unwrap_or_default();
        state::save_to(&persisted_state, &state_path).context("persisting initial state")?;

        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        let cwd = std::env::current_dir().unwrap();

        let config = launch_configuration::load_from_path(&args.name, args.config_path)
            .wrap_err("loading configuration file")?
            .ok_or_eyre("finding named configuration")?;

        let debugger = match config {
            LaunchConfiguration::Debugpy(Debugpy { request, .. }) => {
                let debugger = match request.as_str() {
                    "attach" => {
                        let launch_arguments = AttachArguments {
                            working_directory: cwd,
                            port: None,
                            language: debugger::Language::DebugPy,
                        };

                        Debugger::new(launch_arguments).context("creating internal debugger")?
                    }
                    _ => todo!(),
                };
                debugger
            }
        };

        let events = debugger.events();

        debugger.wait_for_event(|e| matches!(e, debugger::Event::Initialised));

        // TEMP
        for line_no in [1, 27, 16, 34] {
            debugger
                .add_breakpoint(debugger::Breakpoint {
                    path: PathBuf::from("./attach.py"),
                    line: line_no,
                    ..Default::default()
                })
                .context("adding temp breakpoint")?;
        }
        debugger.launch().context("launching debugee")?;

        let temp_state = DebuggerAppState {
            state: State::Initialising,
            previous_state: None,
            debugger,
        };

        let inner = Arc::new(Mutex::new(temp_state));
        let background_inner = Arc::clone(&inner);
        let egui_context = cc.egui_ctx.clone();

        thread::spawn(move || loop {
            if let Ok(event) = events.recv() {
                if let Err(e) = background_inner.lock().unwrap().handle_event(&event) {
                    tracing::warn!(error = %e, "handling debugger event");
                }
                egui_context.request_repaint();
            }
        });

        Ok(Self { inner })
    }
}

impl eframe::App for DebuggerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |_ui| {
            let inner = self.inner.lock().unwrap();
            let user_interface = crate::renderer::Renderer::new(&inner);
            user_interface.render_ui(ctx);
        });
    }
}

fn main() -> eyre::Result<()> {
    setup_sentry!();
    let _ = tracing_subscriber::fmt::try_init();
    let _ = color_eyre::install();

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
