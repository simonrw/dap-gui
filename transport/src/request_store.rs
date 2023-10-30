use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{requests, responses, types};

/// Wraps the incoming request with a channel to reply back on
pub(crate) struct WaitingRequest(
    pub(crate) requests::RequestBody,
    pub(crate) Option<oneshot::Sender<responses::ResponseBody>>,
);

/// A container for the requests awaiting responses
pub(crate) type RequestStore = Arc<Mutex<HashMap<types::Seq, WaitingRequest>>>;
