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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_response(request_seq: i64) -> responses::Response {
        responses::Response {
            request_seq,
            success: true,
            message: None,
            body: None,
        }
    }

    #[test]
    fn add_and_match_response() {
        let mut pending = PendingRequests::new();
        let rx = pending.add(1);

        assert_eq!(pending.len(), 1);

        let response = make_response(1);
        let matched = pending.handle_response(response);

        assert!(matched);
        assert_eq!(pending.len(), 0);

        let received = rx.try_recv().unwrap();
        assert_eq!(received.request_seq, 1);
        assert!(received.success);
    }

    #[test]
    fn multiple_concurrent_requests_out_of_order() {
        let mut pending = PendingRequests::new();
        let rx1 = pending.add(1);
        let rx2 = pending.add(2);
        let rx3 = pending.add(3);

        assert_eq!(pending.len(), 3);

        // Respond out of order: 3, 1, 2
        assert!(pending.handle_response(make_response(3)));
        assert!(pending.handle_response(make_response(1)));
        assert!(pending.handle_response(make_response(2)));

        assert_eq!(pending.len(), 0);

        assert_eq!(rx1.try_recv().unwrap().request_seq, 1);
        assert_eq!(rx2.try_recv().unwrap().request_seq, 2);
        assert_eq!(rx3.try_recv().unwrap().request_seq, 3);
    }

    #[test]
    fn unmatched_response_returns_false() {
        let mut pending = PendingRequests::new();
        let _rx = pending.add(1);

        let matched = pending.handle_response(make_response(99));
        assert!(!matched);
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn empty_tracker_returns_false() {
        let mut pending = PendingRequests::new();
        assert_eq!(pending.len(), 0);

        let matched = pending.handle_response(make_response(1));
        assert!(!matched);
    }

    #[test]
    fn duplicate_seq_replaces_previous() {
        let mut pending = PendingRequests::new();
        let rx1 = pending.add(1);
        let rx2 = pending.add(1); // same seq, replaces

        assert_eq!(pending.len(), 1);

        assert!(pending.handle_response(make_response(1)));

        // rx2 should get the response, rx1's sender was dropped
        assert_eq!(rx2.try_recv().unwrap().request_seq, 1);
        assert!(rx1.try_recv().is_err());
    }
}
