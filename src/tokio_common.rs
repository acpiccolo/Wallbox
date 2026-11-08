//! This module provides common data structures and error types for the `tokio`
//! based clients.
//!
//! It defines the `Error` enum, which encapsulates all possible communication
//! errors, and the `AllSettings` and `AllValues` structs, which are used to
//! return all the settings and values from the EVCC controller in one go.

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

/// A struct containing all configuration and settings of the EVCC controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllSettings {
    pub device_info: proto::DeviceIdAndFirmwareRevision,
    pub modbus_settings: proto::ModbusSettings,
    pub system_flags: proto::SystemFlags,
}

impl std::fmt::Display for AllSettings {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(fmt, "Device Info: {}", self.device_info)?;
        writeln!(fmt, "Modbus Settings: {}", self.modbus_settings)?;
        write!(fmt, "System Flags: State Pointer={:#04X}", self.system_flags.state_machine_pointer)?;
        Ok(())
    }
}

/// A struct containing all measurement and state values of the EVCC controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllValues {
    pub current_full: proto::ReadCurrentFull,
    pub current_amps: proto::ReadCurrentAmps,
}

impl std::fmt::Display for AllValues {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(fmt, "Full: {}", self.current_full)?;
        write!(fmt, "Amps: {}", self.current_amps)?;
        Ok(())
    }
}
