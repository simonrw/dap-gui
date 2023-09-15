//! DAP Client
//!
//! This crate contains code to create a DAP client.
pub mod events;
pub mod requests;
pub mod responses;
pub mod types;
mod client;
mod request_store;

pub use client::Client;
pub use client::Received;
