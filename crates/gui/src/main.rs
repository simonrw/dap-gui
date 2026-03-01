use std::{
    cell::RefCell,
    collections::HashMap,
    fs::create_dir_all,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use clap::Parser;
use debugger::{PausedFrame, ProgramState};
use eframe::egui::{self, Visuals};
use eyre::WrapErr;
use launch_configuration::{ChosenLaunchConfiguration, Debugpy, LaunchConfiguration};
use state::StateManager;
use transport::types::{StackFrame, StackFrameId};

mod async_bridge;
mod code_view;
mod renderer;
mod ui;

use async_bridge::AsyncBridge;

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
            debugger::Event::Paused(ProgramState {
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
            debugger::Event::ScopeChange(ProgramState {
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

#[derive(Clone, Copy, PartialEq)]
enum TabState {
    Variables,
    Repl,
}

struct DebuggerAppState {
    state: State,
    bridge: AsyncBridge,
    previous_state: Option<State>,
    current_frame_id: Option<StackFrameId>,

    // UI internals
    tab: RefCell<TabState>,
    repl_input: RefCell<String>,
    repl_output: RefCell<String>,
    jump: bool,

    // File picker state
    file_picker_open: bool,
    file_picker_input: String,
    file_picker_cursor: usize,
    file_picker_results: Vec<fuzzy::FuzzyMatch>,
    git_files: Vec<fuzzy::TrackedFile>,
    git_files_loaded: bool,

    // File override (when user manually opens a file via picker)
    file_override: Option<PathBuf>,

    // File content cache to avoid repeated disk reads
    file_cache: HashMap<PathBuf, String>,

    // Cache for child variables fetched via variablesReference
    variables_cache: HashMap<i64, Vec<transport::types::Variable>>,

    // Status bar state
    status: crate::ui::status_bar::StatusState,
}

impl DebuggerAppState {
    pub(crate) fn change_scope(&self, stack_frame_id: StackFrameId) -> eyre::Result<()> {
        self.bridge
            .send_sync(|reply| async_bridge::UiCommand::ChangeScope {
                frame_id: stack_frame_id,
                reply,
            })
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
            self.file_override = None;
            self.variables_cache.clear();
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

        // Create a tokio runtime for async initialization
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .map_err(|e| eyre::eyre!("failed to create tokio runtime: {e}"))?;

        let (mut debugger, initial_breakpoints) = rt.block_on(async {
            match config {
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

                    match request.as_str() {
                        "attach" => {
                            let attach_args = debugger::AttachArguments {
                                working_directory: debug_root_dir.to_owned(),
                                port: connect.as_ref().map(|c| c.port),
                                host: connect.map(|c| c.host),
                                language: debugger::Language::DebugPy,
                                path_mappings,
                                just_my_code: None,
                            };

                            tracing::debug!(?attach_args, "generated attach configuration");

                            let port = attach_args.port.unwrap_or(transport::DEFAULT_DAP_PORT);
                            let debugger = debugger::TcpAsyncDebugger::attach_staged(
                                port,
                                debugger::Language::DebugPy,
                                &attach_args,
                            )
                            .await
                            .context("creating async debugger (attach)")?;

                            Ok::<_, eyre::Report>((debugger, vec![]))
                        }
                        "launch" => {
                            let Some(program) = program else {
                                eyre::bail!("'program' is a required setting");
                            };

                            let port = transport::DEFAULT_DAP_PORT;
                            let _server = server::for_implementation_on_port(
                                server::Implementation::Debugpy,
                                port,
                            )
                            .context("creating background server process")?;

                            // Small delay to let the server start
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                            let launch_args = debugger::LaunchArguments {
                                program: Some(program.clone()),
                                module: None,
                                args: None,
                                env: None,
                                working_directory: Some(debug_root_dir.to_owned()),
                                language: debugger::Language::DebugPy,
                                just_my_code: None,
                                stop_on_entry: None,
                            };

                            tracing::debug!(?launch_args, "generated launch configuration");

                            let debugger = debugger::TcpAsyncDebugger::connect_staged(
                                port,
                                debugger::Language::DebugPy,
                                &launch_args,
                                false,
                            )
                            .await
                            .context("creating async debugger (launch)")?;

                            // Collect breakpoints from CLI args
                            let initial_bps: Vec<debugger::Breakpoint> = args
                                .breakpoints
                                .iter()
                                .map(|&line| debugger::Breakpoint {
                                    path: program.clone(),
                                    line,
                                    ..Default::default()
                                })
                                .collect();

                            Ok((debugger, initial_bps))
                        }
                        other => eyre::bail!("unsupported request type: {other}"),
                    }
                }
                other => eyre::bail!("unsupported configuration: {other:?}"),
            }
        })?;

        // Configure breakpoints (from CLI and persisted state) before starting
        let mut all_breakpoints = initial_breakpoints;
        if let Some(project_state) = state_manager
            .current()
            .projects
            .iter()
            .find(|p| debugger::utils::normalise_path(&p.path) == debug_root_dir)
        {
            tracing::debug!("got project state");
            for breakpoint in &project_state.breakpoints {
                let breakpoint_path = debugger::utils::normalise_path(&breakpoint.path);
                if !breakpoint_path.starts_with(&debug_root_dir) {
                    continue;
                }
                tracing::debug!(?breakpoint, "adding breakpoint from state file");
                let mut bp = breakpoint.clone();
                bp.path = debugger::utils::normalise_path(&bp.path)
                    .into_owned()
                    .to_path_buf();
                all_breakpoints.push(bp);
            }
        } else {
            tracing::warn!("missing project state");
        }

        rt.block_on(async {
            debugger
                .configure_breakpoints(&all_breakpoints)
                .await
                .context("configuring breakpoints")?;

            tracing::debug!("launching debugee");
            debugger.start().await.context("launching debugee")
        })?;

        // Set up event forwarding from async debugger to GUI thread
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let event_receiver = debugger.take_events();

        let egui_context = cc.egui_ctx.clone();

        // Create the async bridge which takes ownership of the debugger
        let bridge = AsyncBridge::spawn(
            debugger,
            event_receiver,
            event_tx.clone(),
            egui_context.clone(),
            rt,
        )
        .context("creating async bridge")?;

        let temp_state = DebuggerAppState {
            state: State::Initialising,
            previous_state: None,
            bridge,
            current_frame_id: None,
            jump: false,
            tab: RefCell::new(TabState::Variables),
            repl_input: RefCell::new(String::new()),
            repl_output: RefCell::new(String::new()),
            file_picker_open: false,
            file_picker_input: String::new(),
            file_picker_cursor: 0,
            file_picker_results: Vec::new(),
            git_files: Vec::new(),
            git_files_loaded: false,
            file_override: None,
            file_cache: HashMap::new(),
            variables_cache: HashMap::new(),
            status: Default::default(),
        };

        let inner = Arc::new(Mutex::new(temp_state));
        let background_inner = Arc::clone(&inner);

        std::thread::spawn(move || {
            loop {
                if let Ok(event) = event_rx.recv() {
                    if let Err(e) = background_inner.lock().unwrap().handle_event(&event) {
                        tracing::warn!(error = %e, "handling debugger event");
                    }
                    egui_context.request_repaint();
                }
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

            // Drain any async command errors into the status bar
            for err in inner.bridge.drain_errors() {
                inner
                    .status
                    .push_error(format!("{} failed: {}", err.operation, err.error));
            }

            let mut user_interface = crate::renderer::Renderer::new(&mut inner);
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
            let app = DebuggerApp::new(args, cc)
                .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| eyre::eyre!("running gui mainloop: {e}"))
}
