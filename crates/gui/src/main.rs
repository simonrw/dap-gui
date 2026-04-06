use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use clap::Parser;
use eframe::egui::{self, Visuals};
use eyre::Context;
use launch_configuration::LaunchConfiguration;
use state::StateManager;
use ui_core::bootstrap::{self, Args};
use ui_core::session_state::SessionState;

type StackFrameId = i64;

mod async_bridge;
mod code_view;
mod fonts;
mod renderer;
mod ui;

use async_bridge::AsyncBridge;

/// Re-export SessionState as State for GUI compatibility.
type State = SessionState;

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
        let mut connected =
            ui_core::session_bootstrap::connect_debugger(config, breakpoints, debug_root_dir)?;

        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let event_receiver = connected.debugger.take_events();
        let egui_ctx = egui_ctx.clone();

        let egui_ctx_for_wakeup = egui_ctx.clone();
        let wakeup: Box<dyn Fn() + Send + Sync + 'static> =
            Box::new(move || egui_ctx_for_wakeup.request_repaint());
        let bridge = AsyncBridge::spawn(
            connected.debugger,
            event_receiver,
            event_tx.clone(),
            wakeup,
            connected.runtime,
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
            state: State::Running,
            previous_state: None,
            current_frame_id: None,
            _server: connected.server,
        })
    }

    fn handle_event(&mut self, event: &debugger::Event) {
        tracing::debug!("handling event");
        self.previous_state = Some(self.state.clone());
        self.state = self.state.clone().apply(event);
        if let State::Paused { paused_frame, .. } = &self.state {
            self.current_frame_id = Some(paused_frame.frame.id);
        } else if self.state.is_running() {
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

    // File picker state (shared)
    file_picker: ui_core::file_picker::FilePickerState,

    // File override (when user manually opens a file via picker)
    file_override: Option<PathBuf>,

    // File content cache to avoid repeated disk reads
    file_cache: ui_core::file_cache::FileCache,

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
    search_state: crate::code_view::GuiSearchState,

    // Keybindings
    keybindings: ui_core::keybindings::KeybindingConfig,

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
        ui_core::breakpoints::persist_breakpoints(
            &mut self.state_manager,
            &self.debug_root_dir,
            &self.ui_breakpoints,
        );
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
        ui_core::breakpoints::collect_persisted_breakpoints(
            &self.state_manager,
            &self.debug_root_dir,
        )
    }
}

struct DebuggerApp {
    inner: Arc<Mutex<DebuggerAppState>>,
}

impl DebuggerApp {
    fn new(args: Args, cc: &eframe::CreationContext<'_>) -> eyre::Result<Self> {
        let boot = bootstrap::bootstrap(&args)?;
        let persisted_state = boot.state_manager.current();

        let app_state = DebuggerAppState {
            session: None,
            jump: false,
            tab: RefCell::new(TabState::Variables),
            repl_input: RefCell::new(String::new()),
            repl_output: RefCell::new(String::new()),
            file_picker: Default::default(),
            file_override: None,
            file_cache: Default::default(),
            variables_cache: HashMap::new(),
            ui_breakpoints: boot.initial_breakpoints.into_iter().collect(),
            breakpoint_input: String::new(),
            breakpoint_input_error: false,
            code_font_size: persisted_state.code_font_size.unwrap_or(14.0),
            status: Default::default(),
            search_state: Default::default(),
            keybindings: boot.keybindings,
            state_manager: boot.state_manager,
            debug_root_dir: boot.debug_root_dir,
            configs: boot.configs,
            config_names: boot.config_names,
            selected_config_index: boot.selected_config_index,
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
