//! Thread-safe asynchronous client for ABL eMH1 (EVCC2/3) wallboxes.

use crate::{
    protocol::{Evcc2Firmware, Evcc2Measurements, Evcc2ModbusSettings, Evcc2Status, Evcc2SystemFlags},
    tokio_async::Evcc2Async,
};
use anyhow::Result;
use std::{sync::Arc, time::Duration};
use tokio::{io::BufReader, net::TcpStream, sync::Mutex};

/// A thread-safe, asynchronous client for interacting with an ABL eMH1 wallbox over TCP.
#[derive(Clone)]
pub struct SafeClient {
    stream: Arc<Mutex<BufReader<TcpStream>>>,
    slave_id: u8,
    delay: Duration,
}

impl SafeClient {
    /// Connects to a remote TCP serial server and creates a new `SafeClient`.
    pub async fn connect(tcp_addr: &str, slave_id: u8, delay: Duration) -> Result<Self> {
        let stream = TcpStream::connect(tcp_addr).await?;
        Ok(Self {
            stream: Arc::new(Mutex::new(BufReader::new(stream))),
            slave_id,
            delay,
        })
    }

    /// Read firmware information asynchronously.
    pub async fn read_firmware(&mut self) -> Result<Evcc2Firmware> {
        let mut stream = self.stream.lock().await;
        Evcc2Async::read_firmware(&mut stream, self.slave_id, self.delay).await
    }

    /// Read Modbus settings asynchronously.
    pub async fn read_modbus_settings(&mut self) -> Result<Evcc2ModbusSettings> {
        let mut stream = self.stream.lock().await;
        Evcc2Async::read_modbus_settings(&mut stream, self.slave_id, self.delay).await
    }

    /// Read charging status asynchronously.
    pub async fn read_status(&mut self) -> Result<Evcc2Status> {
        let mut stream = self.stream.lock().await;
        Evcc2Async::read_status(&mut stream, self.slave_id, self.delay).await
    }

    /// Read system flags asynchronously.
    pub async fn read_system_flags(&mut self) -> Result<Evcc2SystemFlags> {
        let mut stream = self.stream.lock().await;
        Evcc2Async::read_system_flags(&mut stream, self.slave_id, self.delay).await
    }

    /// Read measurements and 3-phase currents asynchronously.
    pub async fn read_measurements(&mut self) -> Result<Evcc2Measurements> {
        let mut stream = self.stream.lock().await;
        Evcc2Async::read_measurements(&mut stream, self.slave_id, self.delay).await
    }

    /// Set charging current limit asynchronously.
    pub async fn set_current(&mut self, amperes: u16) -> Result<()> {
        let mut stream = self.stream.lock().await;
        Evcc2Async::set_current(&mut stream, self.slave_id, amperes, self.delay).await
    }
}
