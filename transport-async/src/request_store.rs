use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{requests, responses::Response, types};

/// Wraps the incoming request with a channel to reply back on
pub(crate) struct WaitingRequest(
    pub(crate) requests::RequestBody,
    pub(crate) tokio::sync::oneshot::Sender<Response>,
);

/// A container for the requests awaiting responses
pub(crate) type RequestStore = Arc<Mutex<HashMap<types::Seq, WaitingRequest>>>;
