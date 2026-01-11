//! High level Debugger implementation
mod commands;
mod debugger;
mod internals;
mod pending_requests;
mod persistence;
pub(crate) mod state;
mod types;
pub mod utils;

// Async modules
mod async_debugger;
mod async_event;
mod async_internals;

pub use debugger::{Debugger, InitialiseArguments};
pub use internals::{FileSource, FollowUpRequest, StackTraceContext};
pub use state::{AttachArguments, Event, Language, LaunchArguments, ProgramState};
pub use types::{Breakpoint, EvaluateResult, PausedFrame};

// Export async types
pub use async_debugger::AsyncDebugger;
pub use async_event::AsyncEventReceiver;
