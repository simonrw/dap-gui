//! DAP Client
//!
//! This crate contains code to create a DAP client.
pub mod bindings;
mod client;
pub mod events;
mod parse;
pub(crate) mod reader;
mod request_store;
pub mod requests;
pub mod responses;
pub mod types;

pub use client::Client;
pub use client::Message;
pub use client::Received;
pub use reader::Reader;

/// The default port the DAP protocol listens on
pub const DEFAULT_DAP_PORT: u16 = 5678;
