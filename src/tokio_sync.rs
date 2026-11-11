//! Synchronous, low-level functions for communicating with ABL eMH1 (EVCC2/3) wallboxes.
//!
//! The [`Evcc2`] struct provides direct, stateless functions that take an [`AsciiClient`]
//! reference to execute Modbus ASCII register operations.

use crate::{
    ascii_client::AsciiClient,
    protocol::{
        Evcc2Firmware, Evcc2Measurements, Evcc2ModbusSettings, Evcc2Status,
        Evcc2SystemFlags,
    },
};
use anyhow::Result;

/// Stateless synchronous functions for interacting with an ABL eMH1 / EVCC2 wallbox.
pub struct Evcc2;

impl Evcc2 {
    /// Read firmware and hardware information (Register 0x0001).
    pub fn read_firmware(client: &mut AsciiClient) -> Result<Evcc2Firmware> {
        client.read_firmware()
    }

    /// Read Modbus communication settings (Register 0x0003).
    pub fn read_modbus_settings(client: &mut AsciiClient) -> Result<Evcc2ModbusSettings> {
        client.read_modbus_settings()
    }

    /// Read vehicle charging status (Register 0x0004).
    pub fn read_status(client: &mut AsciiClient) -> Result<Evcc2Status> {
        client.read_status()
    }

    /// Read system flags and state machine pointer (Register 0x0006).
    pub fn read_system_flags(client: &mut AsciiClient) -> Result<Evcc2SystemFlags> {
        client.read_system_flags()
    }

    /// Read currently configured charging current and duty cycle (Register 0x0014).
    pub fn read_configured_current(client: &mut AsciiClient) -> Result<(f32, u16)> {
        client.read_configured_current()
    }

    /// Set charging current limit in Amperes per phase (Register 0x0014).
    pub fn set_current(client: &mut AsciiClient, amperes: u16) -> Result<()> {
        client.set_current(amperes)
    }

    /// Read measurements and 3-phase currents (Register 0x002E).
    pub fn read_measurements(client: &mut AsciiClient) -> Result<Evcc2Measurements> {
        client.read_measurements()
    }

    /// Enable charging permission (sets charging current in Amperes per phase).
    pub fn enable(client: &mut AsciiClient, amperes: Option<u16>) -> Result<()> {
        client.enable(amperes)
    }

    /// Disable charging permission (halts charging).
    pub fn disable(client: &mut AsciiClient) -> Result<()> {
        client.disable()
    }

    /// Wake up sleeping vehicle via state E0 -> A1 Control Pilot sequence.
    pub fn wake_up(client: &mut AsciiClient) -> Result<()> {
        client.wake_up()
    }

    /// Reset wallbox charge controller (Register 0x0005, value 0x5A5A).
    pub fn reset(client: &mut AsciiClient) -> Result<()> {
        client.reset()
    }
}
