//! Tracking pending requests and matching responses
//!
//! This module provides infrastructure for tracking DAP requests that are
//! waiting for responses. When a request is sent, it's added to the pending
//! map. When a response arrives, it's matched by sequence number and sent
//! to the waiting caller.

use std::collections::HashMap;
use transport::{responses, types::Seq};

use crate::internals::FollowUpRequest;

/// Type of pending item - either a command request or a follow-up request
pub(crate) enum PendingItem {
    /// A command from the main thread waiting for a response
    Command(oneshot::Sender<eyre::Result<responses::Response>>),
    /// A follow-up request from event processing
    FollowUp(FollowUpRequest),
}

/// Tracker for pending DAP requests
///
/// This structure maintains a map of sequence numbers to pending items.
/// When a response arrives, it can be matched to the waiting request.
pub(crate) struct PendingRequests {
    pending: HashMap<Seq, PendingItem>,
}

impl PendingRequests {
    /// Create a new pending requests tracker
    pub(crate) fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Add a pending command request with the provided response sender
    pub(crate) fn add_command_with_sender(
        &mut self,
        seq: Seq,
        response_tx: oneshot::Sender<eyre::Result<responses::Response>>,
    ) {
        self.pending.insert(seq, PendingItem::Command(response_tx));
    }

    /// Add a pending follow-up request
    pub(crate) fn add_follow_up(&mut self, seq: Seq, follow_up: FollowUpRequest) {
        self.pending.insert(seq, PendingItem::FollowUp(follow_up));
    }

    /// Handle an incoming response
    ///
    /// Returns the pending item if found, None otherwise
    pub(crate) fn take(&mut self, request_seq: Seq) -> Option<PendingItem> {
        self.pending.remove(&request_seq)
    }

    /// Get the number of pending requests
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.pending.len()
    }

    /// Check if there are any pending requests
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}
