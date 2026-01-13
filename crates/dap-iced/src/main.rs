use std::path::PathBuf;
use std::sync::OnceLock;

use clap::Parser;
use iced::widget::{column, container, row};
use iced::{Element, Fill, Subscription, Task, Theme};
use launch_configuration::{ChosenLaunchConfiguration, Debugpy, LaunchConfiguration};

mod debugger_bridge;
mod message;
mod state;
mod widgets;

use debugger::{Language, LaunchArguments};
use debugger_bridge::DebuggerHandle;
use message::Message;
use state::AppState;

/// Global storage for CLI args (parsed once at startup).
static ARGS: OnceLock<Args> = OnceLock::new();

#[derive(Parser, Clone, Debug)]
struct Args {
    /// Path to a source file to display (for testing without debugger)
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// Path to launch configuration file (e.g., .vscode/launch.json)
    config_path: Option<PathBuf>,

    /// Name of the configuration to use
    #[clap(short, long)]
    name: Option<String>,
}

struct App {
    state: AppState,
    /// Handle for sending commands to the debugger
    debugger_handle: Option<DebuggerHandle>,
    /// Launch configuration if provided
    launch_config: Option<ParsedLaunchConfig>,
    /// Subscription ID counter (incremented to restart subscription)
    subscription_id: usize,
}

#[derive(Clone)]
struct ParsedLaunchConfig {
    language: Language,
    launch_args: LaunchArguments,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let args = ARGS.get().expect("Args not initialized");

        let mut app = Self {
            state: AppState::default(),
            debugger_handle: None,
            launch_config: None,
            subscription_id: 0,
        };

        // Parse launch configuration if provided
        if let Some(config_path) = &args.config_path {
            match Self::parse_launch_config(config_path, args.name.as_ref()) {
                Ok(config) => {
                    app.launch_config = Some(config);
                    app.state
                        .console_output
                        .push("Launch configuration loaded".into());
                }
                Err(e) => {
                    app.state
                        .console_output
                        .push(format!("Config error: {}", e));
                }
            }
        }

        // Determine initial task
        let task = if let Some(file) = &args.file {
            // Just load a file for viewing (no debugger)
            app.state.current_file = Some(file.clone());
            debugger_bridge::load_source_file(file.clone())
        } else if app.launch_config.is_some() {
            // Start debug server automatically
            app.state
                .console_output
                .push("Starting debug server...".into());
            debugger_bridge::spawn_debug_server(Language::DebugPy)
        } else {
            app.state
                .console_output
                .push("No configuration provided. Use --file to view a source file or provide a launch.json config.".into());
            Task::none()
        };

        (app, task)
    }

    fn parse_launch_config(
        config_path: &PathBuf,
        name: Option<&String>,
    ) -> eyre::Result<ParsedLaunchConfig> {
        let config = match launch_configuration::load_from_path(name, config_path)? {
            ChosenLaunchConfiguration::Specific(config) => config,
            ChosenLaunchConfiguration::NotFound => {
                eyre::bail!("No matching configuration found")
            }
            ChosenLaunchConfiguration::ToBeChosen(configurations) => {
                eyre::bail!(
                    "Please specify a configuration name with --name. Available: {:?}",
                    configurations
                )
            }
        };

        match config {
            LaunchConfiguration::Debugpy(Debugpy {
                request,
                cwd,
                program,
                ..
            }) => {
                if request != "launch" {
                    eyre::bail!("Only 'launch' request type is supported, got: {}", request);
                }

                let program = program.ok_or_else(|| eyre::eyre!("'program' is required"))?;
                let working_directory = cwd.or_else(|| program.parent().map(|p| p.to_path_buf()));

                Ok(ParsedLaunchConfig {
                    language: Language::DebugPy,
                    launch_args: LaunchArguments {
                        program,
                        working_directory,
                        language: Language::DebugPy,
                    },
                })
            }
            other => eyre::bail!("Unsupported configuration type: {:?}", other),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Debugger control commands
            Message::Continue => self.send_command(debugger_bridge::DebuggerCommand::Continue),
            Message::StepOver => self.send_command(debugger_bridge::DebuggerCommand::StepOver),
            Message::StepIn => self.send_command(debugger_bridge::DebuggerCommand::StepIn),
            Message::StepOut => self.send_command(debugger_bridge::DebuggerCommand::StepOut),
            Message::Stop => self.send_command(debugger_bridge::DebuggerCommand::Stop),

            Message::ToggleBreakpoint(line) => {
                if self.state.breakpoints.contains(&line) {
                    self.state.breakpoints.remove(&line);
                } else {
                    self.state.breakpoints.insert(line);
                }
                // TODO: Send breakpoint update to debugger
                Task::none()
            }

            // Debugger lifecycle
            Message::StartDebugSession => {
                if self.launch_config.is_some() {
                    self.state
                        .console_output
                        .push("Starting debug server...".into());
                    debugger_bridge::spawn_debug_server(Language::DebugPy)
                } else {
                    self.state
                        .console_output
                        .push("No launch configuration".into());
                    Task::none()
                }
            }

            Message::DebugServerStarted(port) => {
                self.state
                    .console_output
                    .push(format!("Debug server started on port {}", port));

                if let Some(config) = &self.launch_config {
                    self.state
                        .console_output
                        .push("Connecting to debugger...".into());
                    debugger_bridge::connect_and_run(
                        port,
                        config.language,
                        config.launch_args.clone(),
                    )
                } else {
                    Task::none()
                }
            }

            Message::DebuggerReady(handle) => {
                self.state.connected = true;
                self.state.console_output.push("Debugger ready!".into());
                self.debugger_handle = Some(handle);
                self.subscription_id += 1; // Trigger subscription refresh
                Task::none()
            }

            Message::DebuggerEvent(event) => self.handle_debugger_event(event),

            Message::DebuggerError(e) => {
                self.state.console_output.push(format!("Error: {}", e));
                Task::none()
            }

            Message::DebuggerDisconnected => {
                self.state.connected = false;
                self.state.is_running = false;
                self.debugger_handle = None;
                self.state
                    .console_output
                    .push("Debugger disconnected".into());
                Task::none()
            }

            // Source file operations
            Message::LoadSource(path) => {
                self.state.current_file = Some(path.clone());
                debugger_bridge::load_source_file(path)
            }

            Message::SourceLoaded(result) => {
                match result {
                    Ok(content) => {
                        self.state.source_content = content;
                    }
                    Err(e) => {
                        self.state.console_output.push(format!("Error: {}", e));
                    }
                }
                Task::none()
            }
        }
    }

    fn send_command(&self, cmd: debugger_bridge::DebuggerCommand) -> Task<Message> {
        if let Some(handle) = &self.debugger_handle {
            handle.send(cmd);
        }
        Task::none()
    }

    fn handle_debugger_event(&mut self, event: debugger::Event) -> Task<Message> {
        use debugger::Event;

        match event {
            Event::Paused(program_state) => {
                self.state.is_running = false;
                self.state.console_output.push("Paused".into());

                // Update stack frames
                self.state.stack_frames = program_state
                    .stack
                    .iter()
                    .map(|f| state::StackFrame {
                        name: f.name.clone(),
                        file: f
                            .source
                            .as_ref()
                            .and_then(|s| s.path.as_ref())
                            .map(|p| p.display().to_string())
                            .unwrap_or_default(),
                        line: f.line,
                    })
                    .collect();

                // Update current line and load source if needed
                let frame = &program_state.paused_frame.frame;
                self.state.current_line = Some(frame.line);

                if let Some(source) = &frame.source {
                    if let Some(path) = &source.path {
                        // Load source file if it's different
                        if self.state.current_file.as_ref() != Some(path) {
                            return Task::done(Message::LoadSource(path.clone()));
                        }
                    }
                }

                Task::none()
            }

            Event::Running => {
                self.state.is_running = true;
                self.state.current_line = None;
                self.state.console_output.push("Running...".into());
                Task::none()
            }

            Event::Ended => {
                self.state.connected = false;
                self.state.is_running = false;
                self.state.console_output.push("Debug session ended".into());
                Task::none()
            }

            Event::Initialised => {
                self.state
                    .console_output
                    .push("Debugger initialized".into());
                Task::none()
            }

            Event::ScopeChange(_) | Event::Uninitialised => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            // Control bar at top
            widgets::control_bar::control_bar(self.state.is_running, self.state.connected),
            // Main content area
            row![
                // Left: Call stack (placeholder)
                widgets::call_stack::call_stack_panel(&self.state.stack_frames),
                // Center: Source view (main focus)
                container(widgets::source_view::source_view(
                    &self.state.source_content,
                    self.state.current_line,
                    &self.state.breakpoints,
                ))
                .width(Fill),
                // Right: Variables (placeholder)
                widgets::variables::variables_panel(&self.state.variables),
            ]
            .height(Fill),
            // Bottom: Console (placeholder)
            widgets::console::console_panel(&self.state.console_output),
        ]
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        // No subscriptions needed - events come through the channel in connect_and_run
        Subscription::none()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    // Parse and store args globally so boot fn can access them
    ARGS.set(Args::parse()).expect("Args already set");

    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        .title("DAP Debugger")
        .run()
}
