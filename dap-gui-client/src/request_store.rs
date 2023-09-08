use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
};

use crate::{responses, types};

/// Wraps the incoming request with a channel to reply back on
pub(crate) struct WaitingRequest {
    pub(crate) responder: mpsc::Sender<responses::Response>,
}

/// A container for the requests awaiting responses
pub(crate) type RequestStore = Arc<Mutex<HashMap<types::Seq, WaitingRequest>>>;
