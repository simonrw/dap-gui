//! Tracking pending requests and matching responses
//!
//! This module provides infrastructure for tracking DAP requests that are
//! waiting for responses. When a request is sent, it's added to the pending
//! map. When a response arrives, it's matched by sequence number and sent
//! to the waiting caller.

use std::collections::HashMap;
use transport::{responses, types::Seq};

/// Tracker for pending DAP requests
///
/// This structure maintains a map of sequence numbers to response channels.
/// When a response arrives, it can be matched to the waiting request.
pub(crate) struct PendingRequests {
    pending: HashMap<Seq, oneshot::Sender<responses::Response>>,
}

impl PendingRequests {
    /// Create a new pending requests tracker
    pub(crate) fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Add a pending request
    ///
    /// Returns the response receiver that will receive the response when it arrives
    pub(crate) fn add(&mut self, seq: Seq) -> oneshot::Receiver<responses::Response> {
        let (tx, rx) = oneshot::channel();
        self.pending.insert(seq, tx);
        rx
    }

    /// Handle an incoming response
    ///
    /// If this response matches a pending request, sends it to the waiter and returns true.
    /// Otherwise returns false.
    pub(crate) fn handle_response(&mut self, response: responses::Response) -> bool {
        if let Some(tx) = self.pending.remove(&response.request_seq) {
            let _ = tx.send(response);
            true
        } else {
            false
        }
    }

    /// Get the number of pending requests
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.pending.len()
    }
}
