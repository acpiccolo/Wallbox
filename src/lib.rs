pub mod protocol;

#[cfg(any(
    feature = "tokio-rtu-sync",
    feature = "tokio-tcp-sync",
    feature = "tokio-rtu",
    feature = "tokio-tcp"
))]
pub mod tokio_common;

#[cfg(any(feature = "tokio-rtu-sync", feature = "tokio-tcp-sync"))]
pub mod tokio_sync;

#[cfg(all(
    feature = "safe-client-sync",
    any(feature = "tokio-rtu-sync", feature = "tokio-tcp-sync")
))]
pub mod tokio_sync_safe_client;

#[cfg(any(feature = "tokio-rtu", feature = "tokio-tcp"))]
pub mod tokio_async;

#[cfg(all(
    feature = "safe-client-async",
    any(feature = "tokio-rtu", feature = "tokio-tcp")
))]
pub mod tokio_async_safe_client;

pub use protocol::*;
