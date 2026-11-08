//! Common types, errors, and serial builder for tokio-based Modbus connections.

use crate::protocol as proto;

/// Represents errors that can occur during Modbus communication with the EVCC controller.
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

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "tokio-rtu-sync")]
pub const DATA_BITS: &tokio_serial::DataBits = &tokio_serial::DataBits::Eight;

#[cfg(feature = "tokio-rtu-sync")]
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
