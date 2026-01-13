use std::path::PathBuf;
use std::sync::Arc;

use debugger::TcpAsyncDebugger;
use iced::Task;

use crate::message::Message;

/// Commands that can be sent to the debugger.
#[derive(Debug, Clone, Copy)]
pub enum DebuggerCommand {
    Continue,
    StepOver,
    StepIn,
    StepOut,
    Stop,
}

/// Send a command to the debugger asynchronously.
pub fn send_command(debugger: Arc<TcpAsyncDebugger>, cmd: DebuggerCommand) -> Task<Message> {
    Task::perform(
        async move {
            match cmd {
                DebuggerCommand::Continue => debugger.continue_().await,
                DebuggerCommand::StepOver => debugger.step_over().await,
                DebuggerCommand::StepIn => debugger.step_in().await,
                DebuggerCommand::StepOut => debugger.step_out().await,
                DebuggerCommand::Stop => debugger.terminate().await,
            }
            .map_err(|e| e.to_string())
        },
        Message::CommandResult,
    )
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
