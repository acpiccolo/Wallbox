//! A library for controlling ABL eMH1 / eMH2 series wallboxes via Modbus ASCII.
//!
//! This crate provides two main ways to interact with the ABL eMH1 charge controller (EVCC2/3):
//!
//! 1. **High-Level, Safe Clients**: Stateful, thread-safe clients that are easy to share and use
//!    in concurrent applications. This is the recommended approach for most users. See
//!    [`tokio_sync_safe_client::SafeClient`] (blocking) and [`tokio_async_safe_client::SafeClient`] (`async`).
//!
//! 2. **Low-Level, Stateless Functions**: A set of stateless functions that directly map to the
//!    device's Modbus ASCII commands. This API offers maximum flexibility but requires manual management
//!    of the underlying context. See the [`tokio_sync`] and [`tokio_async`] modules.

/// Contains error types for the library.
mod error;
pub mod ascii_client;
pub mod protocol;

pub use error::Error;

#[cfg(feature = "tokio-sync")]
pub mod tokio_sync;

#[cfg(feature = "safe-client-sync")]
pub mod tokio_sync_safe_client;

#[cfg(feature = "tokio-async")]
pub mod tokio_async;

#[cfg(feature = "safe-client-async")]
pub mod tokio_async_safe_client;

pub use ascii_client::AsciiClient;
pub use protocol::*;
