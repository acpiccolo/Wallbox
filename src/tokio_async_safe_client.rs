//! Thread-safe asynchronous client for ABL eMH1 (EVCC2/3) wallboxes.

use crate::{
    protocol::{Evcc2Firmware, Evcc2Measurements, Evcc2ModbusSettings, Evcc2Status, Evcc2SystemFlags},
    tokio_async::{AsyncAsciiClient, Evcc2Async},
};
use anyhow::Result;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

/// A thread-safe, asynchronous client for interacting with an ABL eMH1 wallbox over TCP or Serial.
#[derive(Clone)]
pub struct SafeClient {
    client: Arc<Mutex<AsyncAsciiClient>>,
}

impl SafeClient {
    /// Creates a new `SafeClient` wrapping an existing `AsyncAsciiClient`.
    pub fn new(client: AsyncAsciiClient) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
        }
    }

    /// Creates a new `SafeClient` from a shared `Arc<Mutex<AsyncAsciiClient>>`.
    pub fn from_shared(client: Arc<Mutex<AsyncAsciiClient>>) -> Self {
        Self { client }
    }

    /// Clones and returns the underlying shared client context.
    pub fn clone_shared(&self) -> Arc<Mutex<AsyncAsciiClient>> {
        self.client.clone()
    }

    /// Connects via direct TCP and creates a new `SafeClient`.
    pub async fn connect_tcp(tcp_addr: &str, slave_id: u8, delay: Duration) -> Result<Self> {
        let client = AsyncAsciiClient::connect_tcp(tcp_addr, slave_id, delay).await?;
        Ok(Self::new(client))
    }

    /// Connects via asynchronous serial port and creates a new `SafeClient`.
    pub fn connect_serial(
        device: &str,
        baud_rate: u32,
        parity: tokio_serial::Parity,
        stop_bits: tokio_serial::StopBits,
        slave_id: u8,
        delay: Duration,
    ) -> Result<Self> {
        let client = AsyncAsciiClient::connect_serial(device, baud_rate, parity, stop_bits, slave_id, delay)?;
        Ok(Self::new(client))
    }

    /// Read firmware information asynchronously.
    pub async fn read_firmware(&mut self) -> Result<Evcc2Firmware> {
        let mut client = self.client.lock().await;
        Evcc2Async::read_firmware(&mut client).await
    }

    /// Read Modbus settings asynchronously.
    pub async fn read_modbus_settings(&mut self) -> Result<Evcc2ModbusSettings> {
        let mut client = self.client.lock().await;
        Evcc2Async::read_modbus_settings(&mut client).await
    }

    /// Read charging status asynchronously.
    pub async fn read_status(&mut self) -> Result<Evcc2Status> {
        let mut client = self.client.lock().await;
        Evcc2Async::read_status(&mut client).await
    }

    /// Read system flags asynchronously.
    pub async fn read_system_flags(&mut self) -> Result<Evcc2SystemFlags> {
        let mut client = self.client.lock().await;
        Evcc2Async::read_system_flags(&mut client).await
    }

    /// Read currently configured current limit asynchronously.
    pub async fn read_configured_current(&mut self) -> Result<(f32, u16)> {
        let mut client = self.client.lock().await;
        Evcc2Async::read_configured_current(&mut client).await
    }

    /// Read measurements and 3-phase currents asynchronously.
    pub async fn read_measurements(&mut self) -> Result<Evcc2Measurements> {
        let mut client = self.client.lock().await;
        Evcc2Async::read_measurements(&mut client).await
    }

    /// Set charging current limit asynchronously.
    pub async fn set_current(&mut self, amperes: u16) -> Result<()> {
        let mut client = self.client.lock().await;
        Evcc2Async::set_current(&mut client, amperes).await
    }

    /// Enable charging asynchronously.
    pub async fn enable(&mut self, amperes: Option<u16>) -> Result<()> {
        let mut client = self.client.lock().await;
        Evcc2Async::enable(&mut client, amperes).await
    }

    /// Disable charging asynchronously.
    pub async fn disable(&mut self) -> Result<()> {
        let mut client = self.client.lock().await;
        Evcc2Async::disable(&mut client).await
    }

    /// Wake up vehicle asynchronously.
    pub async fn wake_up(&mut self) -> Result<()> {
        let mut client = self.client.lock().await;
        Evcc2Async::wake_up(&mut client).await
    }

    /// Reset charge controller asynchronously.
    pub async fn reset(&mut self) -> Result<()> {
        let mut client = self.client.lock().await;
        Evcc2Async::reset(&mut client).await
    }
}
