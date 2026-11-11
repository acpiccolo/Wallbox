//! A library for controlling ABL eMH1 / eMH2 series wallboxes via Modbus ASCII.
//!
//! This crate provides clients and protocol implementations for communicating with
//! ABL eMH1 / eMH2 (EVCC2/3) wallboxes over Modbus ASCII:
//!
//! - [`tokio_sync::AsciiClient`]: Synchronous client for serial and direct TCP connections.
//! - [`tokio_async::AsyncAsciiClient`]: Asynchronous client for serial and direct TCP connections.

/// Contains error types for the library.
mod error;
pub mod protocol;

pub use error::Error;

#[cfg(feature = "tokio-sync")]
pub mod tokio_sync;

#[cfg(feature = "tokio-async")]
pub mod tokio_async;

#[cfg(feature = "tokio-sync")]
pub use tokio_sync::AsciiClient;

#[cfg(feature = "tokio-async")]
pub use tokio_async::AsyncAsciiClient;

pub use protocol::*;
