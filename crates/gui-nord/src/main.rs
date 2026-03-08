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
use eyre::WrapErr;
use launch_configuration::{ChosenLaunchConfiguration, Debugpy, LaunchConfiguration};
use state::StateManager;

type StackFrameId = i64;

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
    variables_cache: HashMap<i64, Vec<dap_types::Variable>>,

    // Persistent breakpoint state for the UI (survives across frames)
    ui_breakpoints: HashSet<debugger::Breakpoint>,

    // Status bar state
    status: crate::ui::status_bar::StatusState,

    // Persistence
    state_manager: StateManager,
    debug_root_dir: PathBuf,
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
        if let State::Paused {
            paused_frame,
            breakpoints,
            ..
        } = &self.state
        {
            self.current_frame_id = Some(paused_frame.frame.id);
            // Refresh UI breakpoints from the debugger's authoritative state
            self.ui_breakpoints = breakpoints.iter().cloned().collect();
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
    // Keep the debug server process alive for the lifetime of the app.
    // Dropping this handle terminates the server and closes the transport.
    _server: Option<Box<dyn server::Server + Send>>,
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

        let mut debug_root_dir = std::env::current_dir()
            .and_then(|p| std::fs::canonicalize(&p))
            .unwrap();

        // Create a tokio runtime for async initialization
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .map_err(|e| eyre::eyre!("failed to create tokio runtime: {e}"))?;

        // Server handle must live beyond the async block so the debug server
        // process is not killed before we finish initialization.
        let mut _server_handle: Option<Box<dyn server::Server + Send>> = None;

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
                        debug_root_dir =
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

                            Ok::<_, eyre::Report>((debugger, vec![]))
                        }
                        "launch" => {
                            let Some(program) = program else {
                                eyre::bail!("'program' is a required setting");
                            };

                            // Canonicalize so breakpoint paths match what the
                            // debug adapter returns in frame.source.path.
                            let program = std::fs::canonicalize(&program).unwrap_or(program);

                            let port = server::DEFAULT_DAP_PORT;
                            _server_handle = Some(
                                server::for_implementation_on_port(
                                    server::Implementation::Debugpy,
                                    port,
                                )
                                .context("creating background server process")?,
                            );

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
                let normalised = debugger::utils::normalise_path(&bp.path).into_owned();
                bp.path = std::fs::canonicalize(&normalised).unwrap_or(normalised);
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
            ui_breakpoints: all_breakpoints.into_iter().collect(),
            status: Default::default(),
            state_manager,
            debug_root_dir,
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
            _server: _server_handle,
        })
    }
}

impl eframe::App for DebuggerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
    }
}

fn main() -> eyre::Result<()> {
    setup_sentry!();
    let _ = tracing_subscriber::fmt::try_init();
    let _ = color_eyre::install();

    let args = Args::parse();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "DAP Debugger (Nord)",
        native_options,
        Box::new(|cc| {
            let is_dark = match dark_light::detect() {
                dark_light::Mode::Dark | dark_light::Mode::Default => true,
                dark_light::Mode::Light => false,
            };
            let style = if is_dark {
                use egui::{Color32, CornerRadius, Shadow, Stroke, vec2};
                // Nord Polar Night
                let nord0 = Color32::from_rgb(46, 52, 64); // #2e3440
                let nord1 = Color32::from_rgb(59, 66, 82); // #3b4252
                let nord2 = Color32::from_rgb(67, 76, 94); // #434c5e
                let nord3 = Color32::from_rgb(76, 86, 106); // #4c566a
                // Nord Snow Storm
                let nord4 = Color32::from_rgb(216, 222, 233); // #d8dee9
                let nord5 = Color32::from_rgb(229, 233, 240); // #e5e9f0
                let _nord6 = Color32::from_rgb(236, 239, 244); // #eceff4
                // Nord Frost
                let nord7 = Color32::from_rgb(143, 188, 187); // #8fbcbb
                let nord8 = Color32::from_rgb(136, 192, 208); // #88c0d0
                let nord9 = Color32::from_rgb(129, 161, 193); // #81a1c1
                let nord10 = Color32::from_rgb(94, 129, 172); // #5e81ac
                // Nord Aurora
                let nord11 = Color32::from_rgb(191, 97, 106); // #bf616a (red)
                let corner_radius = CornerRadius::same(6);
                egui::Style {
                    visuals: Visuals {
                        dark_mode: true,
                        override_text_color: Some(nord4),
                        panel_fill: nord0,
                        window_fill: nord1,
                        faint_bg_color: nord1,
                        extreme_bg_color: Color32::from_rgb(36, 40, 50),
                        window_corner_radius: corner_radius,
                        window_stroke: Stroke::new(1.0, nord3),
                        window_shadow: Shadow {
                            offset: [0, 2],
                            blur: 8,
                            spread: 0,
                            color: Color32::from_black_alpha(50),
                        },
                        menu_corner_radius: corner_radius,
                        selection: egui::style::Selection {
                            bg_fill: nord9.linear_multiply(0.4),
                            stroke: Stroke::new(1.0, nord8),
                        },
                        hyperlink_color: nord8,
                        widgets: egui::style::Widgets {
                            noninteractive: egui::style::WidgetVisuals {
                                bg_fill: nord1,
                                weak_bg_fill: nord1,
                                bg_stroke: Stroke::new(1.0, nord3),
                                fg_stroke: Stroke::new(1.0, nord4),
                                corner_radius,
                                expansion: 0.0,
                            },
                            inactive: egui::style::WidgetVisuals {
                                bg_fill: nord2,
                                weak_bg_fill: nord2,
                                bg_stroke: Stroke::NONE,
                                fg_stroke: Stroke::new(1.0, nord5),
                                corner_radius,
                                expansion: 0.0,
                            },
                            hovered: egui::style::WidgetVisuals {
                                bg_fill: nord3,
                                weak_bg_fill: nord3,
                                bg_stroke: Stroke::new(1.0, nord8),
                                fg_stroke: Stroke::new(1.5, nord5),
                                corner_radius,
                                expansion: 1.0,
                            },
                            active: egui::style::WidgetVisuals {
                                bg_fill: nord10,
                                weak_bg_fill: nord10,
                                bg_stroke: Stroke::NONE,
                                fg_stroke: Stroke::new(2.0, Color32::WHITE),
                                corner_radius,
                                expansion: 0.0,
                            },
                            open: egui::style::WidgetVisuals {
                                bg_fill: nord2,
                                weak_bg_fill: nord2,
                                bg_stroke: Stroke::new(1.0, nord3),
                                fg_stroke: Stroke::new(1.0, nord5),
                                corner_radius,
                                expansion: 0.0,
                            },
                        },
                        popup_shadow: Shadow {
                            offset: [0, 2],
                            blur: 8,
                            spread: 0,
                            color: Color32::from_black_alpha(50),
                        },
                        resize_corner_size: 12.0,
                        ..Visuals::dark()
                    },
                    spacing: egui::Spacing {
                        button_padding: vec2(10.0, 5.0),
                        item_spacing: vec2(8.0, 5.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            } else {
                use egui::{Color32, CornerRadius, Shadow, Stroke, vec2};
                let snow0 = Color32::from_rgb(216, 222, 233); // #d8dee9
                let snow1 = Color32::from_rgb(229, 233, 240); // #e5e9f0
                let snow2 = Color32::from_rgb(236, 239, 244); // #eceff4
                let polar0 = Color32::from_rgb(46, 52, 64); // #2e3440
                let polar1 = Color32::from_rgb(59, 66, 82); // #3b4252
                let frost8 = Color32::from_rgb(136, 192, 208); // #88c0d0
                let frost9 = Color32::from_rgb(129, 161, 193); // #81a1c1
                let frost10 = Color32::from_rgb(94, 129, 172); // #5e81ac
                let corner_radius = CornerRadius::same(6);
                egui::Style {
                    visuals: Visuals {
                        dark_mode: false,
                        override_text_color: Some(polar0),
                        panel_fill: snow2,
                        window_fill: snow1,
                        faint_bg_color: snow1,
                        extreme_bg_color: Color32::WHITE,
                        window_corner_radius: corner_radius,
                        window_stroke: Stroke::new(1.0, snow0),
                        window_shadow: Shadow {
                            offset: [0, 1],
                            blur: 6,
                            spread: 0,
                            color: Color32::from_black_alpha(25),
                        },
                        menu_corner_radius: corner_radius,
                        selection: egui::style::Selection {
                            bg_fill: frost9.linear_multiply(0.3),
                            stroke: Stroke::new(1.0, frost10),
                        },
                        hyperlink_color: frost10,
                        widgets: egui::style::Widgets {
                            noninteractive: egui::style::WidgetVisuals {
                                bg_fill: snow1,
                                weak_bg_fill: snow1,
                                bg_stroke: Stroke::new(1.0, snow0),
                                fg_stroke: Stroke::new(1.0, polar1),
                                corner_radius,
                                expansion: 0.0,
                            },
                            inactive: egui::style::WidgetVisuals {
                                bg_fill: snow0,
                                weak_bg_fill: snow0,
                                bg_stroke: Stroke::NONE,
                                fg_stroke: Stroke::new(1.0, polar0),
                                corner_radius,
                                expansion: 0.0,
                            },
                            hovered: egui::style::WidgetVisuals {
                                bg_fill: Color32::WHITE,
                                weak_bg_fill: Color32::WHITE,
                                bg_stroke: Stroke::new(1.0, frost8),
                                fg_stroke: Stroke::new(1.5, polar0),
                                corner_radius,
                                expansion: 1.0,
                            },
                            active: egui::style::WidgetVisuals {
                                bg_fill: frost10,
                                weak_bg_fill: frost10,
                                bg_stroke: Stroke::NONE,
                                fg_stroke: Stroke::new(2.0, Color32::WHITE),
                                corner_radius,
                                expansion: 0.0,
                            },
                            open: egui::style::WidgetVisuals {
                                bg_fill: snow0,
                                weak_bg_fill: snow0,
                                bg_stroke: Stroke::new(1.0, snow0),
                                fg_stroke: Stroke::new(1.0, polar0),
                                corner_radius,
                                expansion: 0.0,
                            },
                        },
                        popup_shadow: Shadow {
                            offset: [0, 1],
                            blur: 6,
                            spread: 0,
                            color: Color32::from_black_alpha(25),
                        },
                        resize_corner_size: 12.0,
                        ..Visuals::light()
                    },
                    spacing: egui::Spacing {
                        button_padding: vec2(10.0, 5.0),
                        item_spacing: vec2(8.0, 5.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            };
            cc.egui_ctx.set_style(style);
            let app = DebuggerApp::new(args, cc)
                .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| eyre::eyre!("running gui mainloop: {e}"))
}
