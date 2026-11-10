//! This module provides common data structures and error types for the `tokio`
//! based clients.
//!
//! It defines the `Error` enum, which encapsulates all possible communication
//! errors, and the `AllValues` struct, which is used to
//! return all values from the EVCC1 controller in one go.

use crate::protocol as proto;

/// Represents all possible errors that can occur during Modbus communication.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An error originating from protocol logic or payload decoding.
    #[error(transparent)]
    Protocol(#[from] proto::Error),

    /// A Modbus exception response from the device.
    #[error(transparent)]
    ModbusException(#[from] tokio_modbus::ExceptionCode),

    /// A transport error from tokio-modbus.
    #[error(transparent)]
    Modbus(#[from] tokio_modbus::Error),

    /// An I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[cfg(any(feature = "tokio-rtu-sync", feature = "tokio-rtu"))]
pub const DATA_BITS: &tokio_serial::DataBits = &tokio_serial::DataBits::Eight;

#[cfg(any(feature = "tokio-rtu-sync", feature = "tokio-rtu"))]
pub fn serial_port_builder(
    device: &str,
    baud_rate: u32,
    parity: tokio_serial::Parity,
    stop_bits: tokio_serial::StopBits,
) -> tokio_serial::SerialPortBuilder {
    tokio_serial::new(device, baud_rate)
        .parity(parity)
        .stop_bits(stop_bits)
        .data_bits(*DATA_BITS)
        .flow_control(tokio_serial::FlowControl::None)
}

/// A struct containing all measurement and state values of the EVCC1 controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllValues {
    /// Register 256: Charging status (vehicle status)
    pub status: proto::ChargeStatus,
    /// Register 257: Maximum charging current (setpoint / enable status) in Amperes
    pub max_current: proto::ChargingCurrent,
    /// Register 258: Active charging power or current
    pub power_or_current: proto::PowerOrCurrent,
}

impl std::fmt::Display for AllValues {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(fmt, "Charging status: {}", self.status)?;
        writeln!(fmt, "Max. current: {}", self.max_current)?;
        write!(fmt, "Charging power / current: {}", self.power_or_current)?;
        Ok(())
    }
}
