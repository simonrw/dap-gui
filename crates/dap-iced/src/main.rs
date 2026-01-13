use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use clap::Parser;
use iced::widget::{column, container, row};
use iced::{Element, Fill, Subscription, Task, Theme};

mod debugger_bridge;
mod message;
mod state;
mod widgets;

use debugger::TcpAsyncDebugger;
use message::Message;
use state::AppState;

/// Global storage for CLI args (parsed once at startup).
static ARGS: OnceLock<Args> = OnceLock::new();

#[derive(Parser, Clone, Debug)]
struct Args {
    /// Path to a source file to display (for testing without debugger)
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// Path to launch configuration file
    config_path: Option<PathBuf>,

    /// Name of the configuration to use
    #[clap(short, long)]
    name: Option<String>,
}

struct App {
    state: AppState,
    debugger: Option<Arc<TcpAsyncDebugger>>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let args = ARGS.get().expect("Args not initialized");

        let mut app = Self {
            state: AppState::default(),
            debugger: None,
        };

        // If a file was specified, load it for testing
        let task = if let Some(file) = &args.file {
            app.state.current_file = Some(file.clone());
            debugger_bridge::load_source_file(file.clone())
        } else {
            Task::none()
        };

        (app, task)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Continue => {
                self.send_debugger_command(debugger_bridge::DebuggerCommand::Continue)
            }
            Message::StepOver => {
                self.send_debugger_command(debugger_bridge::DebuggerCommand::StepOver)
            }
            Message::StepIn => self.send_debugger_command(debugger_bridge::DebuggerCommand::StepIn),
            Message::StepOut => {
                self.send_debugger_command(debugger_bridge::DebuggerCommand::StepOut)
            }
            Message::Stop => self.send_debugger_command(debugger_bridge::DebuggerCommand::Stop),

            Message::ToggleBreakpoint(line) => {
                if self.state.breakpoints.contains(&line) {
                    self.state.breakpoints.remove(&line);
                } else {
                    self.state.breakpoints.insert(line);
                }
                // TODO: Send breakpoint update to debugger
                Task::none()
            }

            Message::DebuggerConnected => {
                self.state.connected = true;
                self.state.console_output.push("Debugger connected".into());
                Task::none()
            }

            Message::DebuggerEvent(event) => self.handle_debugger_event(event),

            Message::CommandResult(result) => {
                if let Err(e) = result {
                    self.state.console_output.push(format!("Error: {}", e));
                }
                Task::none()
            }

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

    fn send_debugger_command(&self, cmd: debugger_bridge::DebuggerCommand) -> Task<Message> {
        if let Some(debugger) = &self.debugger {
            debugger_bridge::send_command(debugger.clone(), cmd)
        } else {
            Task::none()
        }
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
        // TODO: Wire up debugger event subscription when connected
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
