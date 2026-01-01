//! Command infrastructure for debugger background thread communication
//!
//! This module provides the command pattern for communication between the main thread
//! and the debugger's background thread. Commands allow the main thread to request
//! operations that need to be performed by the background thread in coordination with
//! event processing.

use transport::{requests, responses, types::Seq};

/// Commands sent from the main thread to the background thread
///
/// This enum represents all possible commands that can be sent to the background
/// thread for processing. Each command variant includes the data needed to perform
/// the operation and a channel to send back the result.
#[derive(Debug)]
pub(crate) enum Command {
    /// Send a request and wait for a response
    ///
    /// The background thread will send the request, track the sequence number,
    /// and send back the response when it arrives.
    SendRequest {
        body: requests::RequestBody,
        response_tx: oneshot::Sender<eyre::Result<responses::Response>>,
    },

    /// Send a request without waiting for a response (fire-and-forget)
    ///
    /// Used for commands like Continue, Step, etc. where we don't need to wait
    /// for the response. The result channel only indicates whether the request
    /// was successfully sent.
    SendExecute {
        body: requests::RequestBody,
        response_tx: oneshot::Sender<eyre::Result<()>>,
    },

    /// Gracefully shutdown the background thread
    Shutdown,
}

/// State of the background thread
///
/// This enum tracks what the background thread is currently doing. It's useful
/// for debugging and understanding the thread's lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackgroundThreadState {
    /// Waiting for work (events or commands)
    Idle,

    /// Processing a transport event
    ProcessingEvent,

    /// Processing a command from the main thread
    ProcessingCommand,

    /// Shutting down
    Shutdown,
}
