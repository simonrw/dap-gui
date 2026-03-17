use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fs::create_dir_all,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use clap::Parser;
use dap_types::StackFrame;
use debugger::{PausedFrame, ProgramState};
use eframe::egui::{self, Visuals};
use eyre::Context;
use launch_configuration::{Debugpy, LaunchConfiguration};
use state::StateManager;

type StackFrameId = i64;

mod async_bridge;
mod code_view;
mod fonts;
mod renderer;
mod ui;

use async_bridge::AsyncBridge;

#[derive(Parser)]
struct Args {
    config_path: PathBuf,

    #[clap(short, long)]
    name: Option<String>,

    #[clap(short, long)]
    breakpoints: Vec<String>,
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

/// An active debug session, bundling the async bridge and server process.
///
/// Dropping this struct tears down the session: the bridge is dropped (which
/// cancels the debugger tasks), the server process is killed, and the event
/// forwarding thread exits when its channel closes.
struct Session {
    bridge: AsyncBridge,
    state: State,
    previous_state: Option<State>,
    current_frame_id: Option<StackFrameId>,
    _server: Option<Box<dyn server::Server + Send>>,
}

impl Session {
    /// Start a new debug session from a launch configuration.
    ///
    /// This spawns the server (for launch mode), connects to the debugger,
    /// configures breakpoints, and starts execution.
    fn start(
        config: &LaunchConfiguration,
        breakpoints: &[debugger::Breakpoint],
        debug_root_dir: &mut PathBuf,
        egui_ctx: &egui::Context,
        app_state: Arc<Mutex<DebuggerAppState>>,
    ) -> eyre::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .map_err(|e| eyre::eyre!("failed to create tokio runtime: {e}"))?;

        let mut server_handle: Option<Box<dyn server::Server + Send>> = None;

        let mut debugger = rt.block_on(async {
            match config {
                LaunchConfiguration::Python(debugpy) | LaunchConfiguration::Debugpy(debugpy) => {
                    let Debugpy {
                        request,
                        cwd,
                        connect,
                        path_mappings,
                        program,
                        ..
                    } = debugpy.clone();
                    if let Some(dir) = cwd {
                        *debug_root_dir =
                            std::fs::canonicalize(debugger::utils::normalise_path(&dir).as_ref())
                                .unwrap_or_else(|_| {
                                    debugger::utils::normalise_path(&dir).into_owned()
                                });
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

                            let port = attach_args.port.unwrap_or(server::DEFAULT_DAP_PORT);
                            let debugger = debugger::TcpAsyncDebugger::attach_staged(
                                port,
                                debugger::Language::DebugPy,
                                &attach_args,
                            )
                            .await
                            .context("creating async debugger (attach)")?;

                            Ok::<_, eyre::Report>(debugger)
                        }
                        "launch" => {
                            let Some(program) = program else {
                                eyre::bail!("'program' is a required setting");
                            };

                            let program = std::fs::canonicalize(&program).unwrap_or(program);

                            let port = server::DEFAULT_DAP_PORT;
                            server_handle = Some(
                                server::for_implementation_on_port(
                                    server::Implementation::Debugpy,
                                    port,
                                )
                                .context("creating background server process")?,
                            );

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

                            Ok(debugger)
                        }
                        other => eyre::bail!("unsupported request type: {other}"),
                    }
                }
                other => eyre::bail!("unsupported configuration: {other:?}"),
            }
        })?;

        rt.block_on(async {
            debugger
                .configure_breakpoints(breakpoints)
                .await
                .context("configuring breakpoints")?;

            tracing::debug!("launching debugee");
            debugger.start().await.context("launching debugee")
        })?;

        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let event_receiver = debugger.take_events();
        let egui_ctx = egui_ctx.clone();

        let bridge = AsyncBridge::spawn(
            debugger,
            event_receiver,
            event_tx.clone(),
            egui_ctx.clone(),
            rt,
        )
        .context("creating async bridge")?;

        // Spawn event forwarding thread for this session
        std::thread::spawn(move || {
            loop {
                match event_rx.recv() {
                    Ok(event) => {
                        let mut state = app_state.lock().unwrap();
                        state.handle_session_event(&event);
                        drop(state);
                        egui_ctx.request_repaint();
                    }
                    Err(_) => break,
                }
            }
            tracing::debug!("event forwarding thread ended");
        });

        Ok(Self {
            bridge,
            state: State::Initialising,
            previous_state: None,
            current_frame_id: None,
            _server: server_handle,
        })
    }

    fn handle_event(&mut self, event: &debugger::Event) {
        tracing::debug!("handling event");
        self.previous_state = Some(self.state.clone());
        self.state = event.clone().into();
        if let State::Paused { paused_frame, .. } = &self.state {
            self.current_frame_id = Some(paused_frame.frame.id);
        } else if let State::Running = &self.state {
            self.current_frame_id = None;
        }
    }
}

struct DebuggerAppState {
    session: Option<Session>,

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
    variables_cache: HashMap<i64, Vec<dap_types::Variable>>,

    // Persistent breakpoint state for the UI (survives across sessions)
    ui_breakpoints: HashSet<debugger::Breakpoint>,

    // Text input for adding breakpoints via file:line
    breakpoint_input: String,
    breakpoint_input_error: bool,

    // Code view font size
    code_font_size: f32,

    // Status bar state
    status: crate::ui::status_bar::StatusState,

    // Search state for the code view
    search_state: crate::code_view::SearchState,

    // Persistence
    state_manager: StateManager,
    debug_root_dir: PathBuf,

    // Launch configurations
    configs: Vec<LaunchConfiguration>,
    config_names: Vec<String>,
    selected_config_index: usize,
    #[allow(dead_code)]
    config_path: PathBuf,
}

impl DebuggerAppState {
    pub(crate) fn persist_breakpoints(&mut self) {
        let breakpoints: Vec<_> = self.ui_breakpoints.iter().cloned().collect();
        if let Err(e) = self
            .state_manager
            .set_project_breakpoints(self.debug_root_dir.clone(), breakpoints)
        {
            tracing::warn!(error = %e, "failed to persist breakpoints");
        }
    }

    pub(crate) fn change_scope(&self, stack_frame_id: StackFrameId) -> eyre::Result<()> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| eyre::eyre!("no active session"))?;
        session
            .bridge
            .send_sync(|reply| async_bridge::UiCommand::ChangeScope {
                frame_id: stack_frame_id,
                reply,
            })
    }

    fn handle_session_event(&mut self, event: &debugger::Event) {
        if let Some(session) = &mut self.session {
            session.handle_event(event);
            // Refresh UI breakpoints from debugger's authoritative state
            if let State::Paused { breakpoints, .. } = &session.state {
                self.ui_breakpoints = breakpoints.iter().cloned().collect();
            }
            // Jump + clear overrides when transitioning to paused
            if let (State::Paused { .. }, Some(State::Running)) =
                (&session.state, &session.previous_state)
            {
                self.jump = true;
                self.file_override = None;
                self.variables_cache.clear();
            }
        }
    }

    /// Collect breakpoints from persisted state for the current project.
    fn collect_persisted_breakpoints(&self) -> Vec<debugger::Breakpoint> {
        let mut bps = Vec::new();
        if let Some(project_state) = self
            .state_manager
            .current()
            .projects
            .iter()
            .find(|p| debugger::utils::normalise_path(&p.path) == self.debug_root_dir)
        {
            tracing::debug!("got project state");
            for breakpoint in &project_state.breakpoints {
                let breakpoint_path = debugger::utils::normalise_path(&breakpoint.path);
                if !breakpoint_path.starts_with(&self.debug_root_dir) {
                    continue;
                }
                tracing::debug!(?breakpoint, "adding breakpoint from state file");
                let mut bp = breakpoint.clone();
                let normalised = debugger::utils::normalise_path(&bp.path).into_owned();
                bp.path = std::fs::canonicalize(&normalised).unwrap_or(normalised);
                bps.push(bp);
            }
        } else {
            tracing::warn!("missing project state");
        }
        bps
    }
}

struct DebuggerApp {
    inner: Arc<Mutex<DebuggerAppState>>,
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
        let state_manager = StateManager::new(state_path).wrap_err("loading state")?;
        state_manager.save().wrap_err("saving state")?;
        let persisted_state = state_manager.current();
        tracing::trace!(state = ?persisted_state, "loaded state");

        let debug_root_dir = std::env::current_dir()
            .and_then(|p| std::fs::canonicalize(&p))
            .unwrap();

        // Load all configurations
        let configs = launch_configuration::load_all_from_path(&args.config_path)
            .wrap_err("loading launch configurations")?;
        if configs.is_empty() {
            eyre::bail!("no configurations found in launch.json");
        }
        let config_names: Vec<String> = configs.iter().map(|c| c.name().to_string()).collect();

        // Pre-select config if --name provided
        let selected_config_index = if let Some(ref name) = args.name {
            config_names
                .iter()
                .position(|n| n == name)
                .ok_or_else(|| eyre::eyre!("no configuration named '{name}' found"))?
        } else {
            0
        };

        // Parse CLI breakpoints
        let initial_breakpoints: Vec<debugger::Breakpoint> = args
            .breakpoints
            .iter()
            .map(|bp_str| debugger::Breakpoint::parse(bp_str, &debug_root_dir))
            .collect::<eyre::Result<Vec<_>>>()
            .wrap_err("parsing --breakpoint arguments")?;

        let app_state = DebuggerAppState {
            session: None,
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
            ui_breakpoints: initial_breakpoints.into_iter().collect(),
            breakpoint_input: String::new(),
            breakpoint_input_error: false,
            code_font_size: persisted_state.code_font_size.unwrap_or(14.0),
            status: Default::default(),
            search_state: Default::default(),
            state_manager,
            debug_root_dir,
            configs,
            config_names,
            selected_config_index,
            config_path: args.config_path,
        };

        let inner = Arc::new(Mutex::new(app_state));

        // Auto-start if --name and breakpoints were both provided on the CLI
        if args.name.is_some() && !args.breakpoints.is_empty() {
            let mut state = inner.lock().unwrap();
            let persisted_bps = state.collect_persisted_breakpoints();
            let mut all_bps: Vec<_> = state.ui_breakpoints.iter().cloned().collect();
            all_bps.extend(persisted_bps);
            state.ui_breakpoints = all_bps.iter().cloned().collect();

            let config = state.configs[state.selected_config_index].clone();
            let egui_ctx = cc.egui_ctx.clone();
            let app_state_clone = Arc::clone(&inner);
            match Session::start(
                &config,
                &all_bps,
                &mut state.debug_root_dir,
                &egui_ctx,
                app_state_clone,
            ) {
                Ok(session) => {
                    state.session = Some(session);
                }
                Err(e) => {
                    state
                        .status
                        .push_error(format!("Failed to start session: {e}"));
                }
            }
        }

        Ok(Self { inner })
    }
}

impl eframe::App for DebuggerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut inner = self.inner.lock().unwrap();

        // Drain any async command errors into the status bar
        if let Some(session) = &inner.session {
            for err in session.bridge.drain_errors() {
                inner
                    .status
                    .push_error(format!("{} failed: {}", err.operation, err.error));
            }
        }

        let mut user_interface = crate::renderer::Renderer::new(&mut inner, &self.inner, ctx);
        user_interface.render_ui(ctx);
        if inner.jump {
            inner.jump = false;
        }
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
        Box::new(move |cc| {
            let style = egui::Style {
                visuals: match dark_light::detect() {
                    dark_light::Mode::Dark | dark_light::Mode::Default => Visuals::dark(),
                    dark_light::Mode::Light => Visuals::light(),
                },
                ..Default::default()
            };
            cc.egui_ctx.set_style(style);
            crate::fonts::install_lilex(&cc.egui_ctx);
            let app = DebuggerApp::new(args, cc)
                .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
            Ok(Box::new(app) as Box<dyn eframe::App>)
        }),
    )
    .map_err(|e| eyre::eyre!("running gui mainloop: {e}"))
}

