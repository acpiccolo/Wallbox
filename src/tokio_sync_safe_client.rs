//! Thread-safe, stateful synchronous client for ABL eMH1 (EVCC2/3) wallboxes.
//!
//! The [`SafeClient`] wraps an [`AsciiClient`] within an `Arc<Mutex>`, allowing it
//! to be shared across threads safely.

use crate::{
    ascii_client::AsciiClient,
    protocol::{Evcc2Firmware, Evcc2Measurements, Evcc2ModbusSettings, Evcc2Status, Evcc2SystemFlags},
    tokio_sync::Evcc2,
};
use anyhow::Result;
use std::sync::{Arc, Mutex};

/// A thread-safe, stateful synchronous client for interacting with an ABL eMH1 wallbox.
#[derive(Clone)]
pub struct SafeClient {
    client: Arc<Mutex<AsciiClient>>,
}

impl SafeClient {
    /// Creates a new `SafeClient` wrapping an existing `AsciiClient`.
    pub fn new(client: AsciiClient) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
        }
    }

    /// Creates a new `SafeClient` from a shared `Arc<Mutex<AsciiClient>>`.
    pub fn from_shared(client: Arc<Mutex<AsciiClient>>) -> Self {
        Self { client }
    }

    /// Clones and returns the underlying shared client context.
    pub fn clone_shared(&self) -> Arc<Mutex<AsciiClient>> {
        self.client.clone()
    }

    /// Read firmware and hardware information (Register 0x0001).
    pub fn read_firmware(&mut self) -> Result<Evcc2Firmware> {
        let mut client = self.client.lock().unwrap();
        Evcc2::read_firmware(&mut client)
    }

    /// Read Modbus communication settings (Register 0x0003).
    pub fn read_modbus_settings(&mut self) -> Result<Evcc2ModbusSettings> {
        let mut client = self.client.lock().unwrap();
        Evcc2::read_modbus_settings(&mut client)
    }

    /// Read vehicle charging status (Register 0x0004).
    pub fn read_status(&mut self) -> Result<Evcc2Status> {
        let mut client = self.client.lock().unwrap();
        Evcc2::read_status(&mut client)
    }

    /// Read system flags and state machine pointer (Register 0x0006).
    pub fn read_system_flags(&mut self) -> Result<Evcc2SystemFlags> {
        let mut client = self.client.lock().unwrap();
        Evcc2::read_system_flags(&mut client)
    }

    /// Read currently configured charging current limit (Register 0x0014).
    pub fn read_configured_current(&mut self) -> Result<(f32, u16)> {
        let mut client = self.client.lock().unwrap();
        Evcc2::read_configured_current(&mut client)
    }

    /// Set charging current limit in Amperes per phase (Register 0x0014).
    pub fn set_current(&mut self, amperes: u16) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        Evcc2::set_current(&mut client, amperes)
    }

    /// Read measurements and 3-phase currents (Register 0x002E).
    pub fn read_measurements(&mut self) -> Result<Evcc2Measurements> {
        let mut client = self.client.lock().unwrap();
        Evcc2::read_measurements(&mut client)
    }

    /// Enable charging permission (sets charging current in Amperes per phase).
    pub fn enable(&mut self, amperes: Option<u16>) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        Evcc2::enable(&mut client, amperes)
    }

    /// Disable charging permission (halts charging).
    pub fn disable(&mut self) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        Evcc2::disable(&mut client)
    }

    /// Wake up sleeping vehicle via state E0 -> A1 Control Pilot sequence.
    pub fn wake_up(&mut self) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        Evcc2::wake_up(&mut client)
    }

    /// Reset wallbox charge controller (Register 0x0005, value 0x5A5A).
    pub fn reset(&mut self) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        Evcc2::reset(&mut client)
    }
}
