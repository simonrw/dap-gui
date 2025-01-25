use std::{
    cell::RefCell,
    fs::create_dir_all,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use clap::Parser;
use debugger::{AttachArguments, Debugger, LaunchArguments, PausedFrame, ProgramDescription};
use eframe::egui::{self, Visuals};
use eyre::WrapErr;
use launch_configuration::{ChosenLaunchConfiguration, Debugpy, LaunchConfiguration};
use state::StateManager;
use transport::types::{StackFrame, StackFrameId};

mod code_view;
mod renderer;
mod ui;

#[derive(Parser)]
struct Args {
    config_path: PathBuf,

    #[clap(short, long)]
    name: Option<String>,

    #[clap(short, long)]
    breakpoints: Vec<usize>,
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
        paused_frame: Box<PausedFrame>,
        breakpoints: Vec<debugger::Breakpoint>,
    },
    Terminated,
}

impl From<debugger::Event> for State {
    fn from(event: debugger::Event) -> Self {
        match event {
            debugger::Event::Initialised => State::Running,
            debugger::Event::Paused(ProgramDescription {
                stack,
                paused_frame,
                breakpoints,
            }) => State::Paused {
                stack,
                paused_frame: Box::new(paused_frame),
                breakpoints,
            },
            debugger::Event::Running => State::Running,
            debugger::Event::Ended => State::Terminated,
            debugger::Event::Uninitialised => State::Initialising,
            debugger::Event::ScopeChange(ProgramDescription {
                stack,
                breakpoints,
                paused_frame,
            }) => State::Paused {
                stack,
                breakpoints,
                paused_frame: Box::new(paused_frame),
            },
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
    jump: bool,
}

impl DebuggerAppState {
    pub(crate) fn change_scope(&self, stack_frame_id: StackFrameId) -> eyre::Result<()> {
        self.debugger
            .change_scope(stack_frame_id)
            .wrap_err("changing scope")
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_event(&mut self, event: &debugger::Event) -> eyre::Result<()> {
        tracing::debug!("handling event");
        self.previous_state = Some(self.state.clone());
        self.state = event.clone().into();
        if let State::Paused { paused_frame, .. } = &self.state {
            self.current_frame_id = Some(paused_frame.frame.id);
        } else if let State::Running = &self.state {
            self.current_frame_id = None;
        }

        // if we have just been paused then jump the editor to the nearest point
        if let (State::Paused { .. }, Some(State::Running)) =
            (&mut self.state, &self.previous_state)
        {
            self.jump = true;
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
        tracing::debug!(state_path = %state_path.display(), "loading state");
        if !state_path.parent().unwrap().is_dir() {
            create_dir_all(state_path.parent().unwrap()).context("creating state directory")?;
        }
        let state_manager = StateManager::new(state_path)
            .wrap_err("loading state")?
            .save()
            .wrap_err("saving state")?;
        let persisted_state = state_manager.current();
        tracing::trace!(state = ?persisted_state, "loaded state");

        let config =
            match launch_configuration::load_from_path(args.name.as_ref(), args.config_path)
                .wrap_err("loading launch configuration")?
            {
                ChosenLaunchConfiguration::Specific(config) => config,
                ChosenLaunchConfiguration::NotFound => {
                    eyre::bail!("no matching configuration found")
                }
                ChosenLaunchConfiguration::ToBeChosen(configurations) => {
                    eprintln!("Configuration name not specified");
                    eprintln!("Available options:");
                    for config in &configurations {
                        eprintln!("- {config}");
                    }
                    // TODO: best option?
                    std::process::exit(1);
                }
            };

        let mut debug_root_dir = std::env::current_dir().unwrap();

        let debugger = match config {
            LaunchConfiguration::Debugpy(Debugpy {
                request,
                cwd,
                connect,
                path_mappings,
                program,
                ..
            }) => {
                if let Some(dir) = cwd {
                    debug_root_dir = debugger::utils::normalise_path(&dir).into_owned();
                }
                let debugger = match request.as_str() {
                    "attach" => {
                        let launch_arguments = AttachArguments {
                            working_directory: debug_root_dir.to_owned().to_path_buf(),
                            port: connect.map(|c| c.port),
                            language: debugger::Language::DebugPy,
                            path_mappings,
                        };

                        tracing::debug!(?launch_arguments, "generated launch configuration");

                        Debugger::new(launch_arguments).context("creating internal debugger")?
                    }
                    "launch" => {
                        let Some(program) = program else {
                            eyre::bail!("'program' is a required setting");
                        };
                        let launch_arguments = LaunchArguments {
                            program: program.clone(),
                            working_directory: Some(debug_root_dir.to_owned().to_path_buf()),
                            language: debugger::Language::DebugPy,
                        };

                        tracing::debug!(?launch_arguments, "generated launch configuration");
                        let debugger = debugger::Debugger::new(launch_arguments)
                            .context("creating internal debugger")?;

                        for line in args.breakpoints {
                            let breakpoint = debugger::Breakpoint {
                                path: program.clone(),
                                line,
                                ..Default::default()
                            };
                            debugger
                                .add_breakpoint(&breakpoint)
                                .context("adding breakpoint")?;
                        }

                        debugger
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
            .iter()
            .find(|p| debugger::utils::normalise_path(&p.path) == debug_root_dir)
        {
            tracing::debug!("got project state");
            for breakpoint in &project_state.breakpoints {
                {
                    let breakpoint_path = debugger::utils::normalise_path(&breakpoint.path);
                    if !breakpoint_path.starts_with(&debug_root_dir) {
                        continue;
                    }
                    tracing::debug!(?breakpoint, "adding breakpoint from state file");

                    let mut breakpoint = breakpoint.clone();
                    breakpoint.path = debugger::utils::normalise_path(&breakpoint.path)
                        .into_owned()
                        .to_path_buf();

                    debugger
                        .add_breakpoint(&breakpoint)
                        .context("adding breakpoint")?;
                }
            }
        } else {
            tracing::warn!("missing project state");
        }

        tracing::debug!("launching debugee");
        debugger.start().context("launching debugee")?;

        let temp_state = DebuggerAppState {
            state: State::Initialising,
            previous_state: None,
            debugger,
            current_frame_id: None,
            jump: false,
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
            let mut inner = self.inner.lock().unwrap();
            let mut user_interface = crate::renderer::Renderer::new(&inner);
            user_interface.render_ui(ctx);
            if inner.jump {
                inner.jump = false;
            }
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
        "DAP Debugger",
        native_options,
        Box::new(|cc| {
            let style = egui::Style {
                visuals: match dark_light::detect() {
                    dark_light::Mode::Dark | dark_light::Mode::Default => Visuals::dark(),
                    dark_light::Mode::Light => Visuals::light(),
                },
                ..Default::default()
            };
            cc.egui_ctx.set_style(style);
            let app = DebuggerApp::new(args, cc).expect("creating main application");
            Box::new(app)
        }),
    )
    .map_err(|e| eyre::eyre!("running gui mainloop: {e}"))
}
