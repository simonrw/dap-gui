use std::{
    cell::RefCell,
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
use state::StateManager;
use transport::types::{StackFrame, StackFrameId};

mod code_view;
mod renderer;
mod ui;

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

#[derive(PartialEq)]
enum TabState {
    Variables,
    Repl,
}

struct DebuggerAppState {
    state: State,
    debugger: Debugger,
    previous_state: Option<State>,
    current_frame_id: Option<StackFrameId>,

    // UI internals
    tab: RefCell<TabState>,
    repl_input: RefCell<String>,
    repl_output: RefCell<String>,
}

impl DebuggerAppState {
    #[tracing::instrument(skip(self))]
    fn handle_event(&mut self, event: &debugger::Event) -> eyre::Result<()> {
        tracing::debug!("handling event");
        self.previous_state = Some(self.state.clone());
        self.state = event.clone().into();
        if let State::Paused { paused_frame, .. } = &self.state {
            self.current_frame_id = Some(paused_frame.frame.id);
        } else if let State::Running = &self.state {
            self.current_frame_id = None;
        }
        Ok(())
    }
}

struct DebuggerApp {
    inner: Arc<Mutex<DebuggerAppState>>,
    _state_manager: StateManager,
}

impl DebuggerApp {
    fn new(args: Args, cc: &eframe::CreationContext<'_>) -> eyre::Result<Self> {
        let state_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("dapgui")
            .join("state.json");
        if !state_path.parent().unwrap().is_dir() {
            create_dir_all(state_path.parent().unwrap()).context("creating state directory")?;
        }
        let state_manager = StateManager::new(state_path)
            .wrap_err("loading state")?
            .save()
            .wrap_err("saving state")?;
        let _persisted_state = state_manager.current();

        let config = launch_configuration::load_from_path(&args.name, args.config_path)
            .wrap_err("loading configuration file")?
            .ok_or_eyre("finding named configuration")?;

        let mut debug_root_dir = std::env::current_dir().unwrap();

        let debugger = match config {
            LaunchConfiguration::Debugpy(Debugpy { request, cwd, .. }) => {
                if let Some(dir) = cwd {
                    debug_root_dir = dir;
                }
                let debugger = match request.as_str() {
                    "attach" => {
                        let launch_arguments = AttachArguments {
                            working_directory: debug_root_dir.clone(),
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

        if let Some(project_state) = state_manager
            .current()
            .projects
            .get(&debug_root_dir.display().to_string())
        {
            for breakpoint in &project_state.breakpoints {
                {
                    if !breakpoint.path.starts_with(&debug_root_dir) {
                        continue;
                    }

                    debugger
                        .add_breakpoint(breakpoint)
                        .context("adding breakpoint")?;
                }
            }
        }
        debugger.launch().context("launching debugee")?;

        let temp_state = DebuggerAppState {
            state: State::Initialising,
            previous_state: None,
            debugger,
            current_frame_id: None,
            tab: RefCell::new(TabState::Variables),
            repl_input: RefCell::new(String::new()),
            repl_output: RefCell::new(String::new()),
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

        Ok(Self {
            inner,
            _state_manager: state_manager,
        })
    }
}

impl eframe::App for DebuggerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |_ui| {
            let inner = self.inner.lock().unwrap();
            let mut user_interface = crate::renderer::Renderer::new(&inner);
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
