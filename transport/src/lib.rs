//! DAP Client
//!
//! This crate contains code to create a DAP client.
pub mod bindings;
mod client;
pub mod events;
mod request_store;
pub mod requests;
pub mod responses;
pub mod types;

pub use client::Client;
pub use client::Received;

/// The default port the DAP protocol listens on
pub const DEFAULT_DAP_PORT: u16 = 5678;
