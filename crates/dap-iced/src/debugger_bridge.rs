use std::path::PathBuf;

use debugger::{Language, LaunchArguments, TcpAsyncDebugger};
use iced::futures::SinkExt;
use iced::futures::channel::mpsc::Sender;
use iced::{Task, stream};
use server::Server;
use tokio::sync::mpsc;

use crate::message::Message;

/// Commands that can be sent to the debugger.
#[derive(Debug, Clone)]
pub enum DebuggerCommand {
    Continue,
    StepOver,
    StepIn,
    StepOut,
    Stop,
}

/// Handle for sending commands to the debugger task.
#[derive(Clone, Debug)]
pub struct DebuggerHandle {
    command_tx: mpsc::UnboundedSender<DebuggerCommand>,
}

impl DebuggerHandle {
    /// Send a command to the debugger.
    pub fn send(&self, cmd: DebuggerCommand) {
        let _ = self.command_tx.send(cmd);
    }
}

/// Load a source file from disk asynchronously.
pub fn load_source_file(path: PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Failed to load {}: {}", path.display(), e))
        },
        Message::SourceLoaded,
    )
}

/// Spawn a debug server process and return the port.
pub fn spawn_debug_server(language: Language) -> Task<Message> {
    Task::perform(
        async move {
            let port = transport::DEFAULT_DAP_PORT;
            match language {
                Language::DebugPy => {
                    server::debugpy::DebugpyServer::on_port(port)
                        .map_err(|e| format!("Failed to start debugpy: {}", e))?;
                }
                Language::Delve => {
                    return Err("Delve not yet supported".to_string());
                }
            }
            Ok(port)
        },
        |result: Result<u16, String>| match result {
            Ok(port) => Message::DebugServerStarted(port),
            Err(e) => Message::DebuggerError(e),
        },
    )
}

/// Connect to a debugger and run it, returning a stream of messages.
/// This function connects to the debugger, starts the session, and returns
/// a Task that streams debugger events and command results.
pub fn connect_and_run(
    port: u16,
    language: Language,
    launch_args: LaunchArguments,
) -> Task<Message> {
    // Create the command channel
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let handle = DebuggerHandle { command_tx };

    // Return a stream-based task that connects, starts, and runs the debugger
    Task::stream(stream::channel(100, move |mut output: Sender<Message>| {
        async move {
            // First, send the handle back to the UI
            let _ = output.send(Message::DebuggerReady(handle)).await;

            // Connect to the debugger
            let mut debugger: TcpAsyncDebugger =
                match TcpAsyncDebugger::connect(port, language, &launch_args, true).await {
                    Ok(d) => d,
                    Err(e) => {
                        let _ = output
                            .send(Message::DebuggerError(format!("Connect failed: {}", e)))
                            .await;
                        return;
                    }
                };

            // Start the debug session
            if let Err(e) = debugger.start().await {
                let _ = output
                    .send(Message::DebuggerError(format!("Start failed: {}", e)))
                    .await;
                return;
            }

            // Run the event loop
            let mut command_rx = command_rx;
            loop {
                tokio::select! {
                    // Handle commands from UI
                    Some(cmd) = command_rx.recv() => {
                        let result: eyre::Result<()> = match cmd {
                            DebuggerCommand::Continue => debugger.continue_().await,
                            DebuggerCommand::StepOver => debugger.step_over().await,
                            DebuggerCommand::StepIn => debugger.step_in().await,
                            DebuggerCommand::StepOut => debugger.step_out().await,
                            DebuggerCommand::Stop => debugger.terminate().await,
                        };

                        if let Err(e) = result {
                            let _ = output.send(Message::DebuggerError(e.to_string())).await;
                        }
                    }

                    // Handle events from debugger
                    Some(event) = debugger.events().recv() => {
                        let _ = output.send(Message::DebuggerEvent(event)).await;
                    }

                    else => break,
                }
            }

            // Debugger disconnected
            let _ = output.send(Message::DebuggerDisconnected).await;
        }
    }))
}
