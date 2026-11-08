//! Thread-safe asynchronous client wrapper for the ABL EVCC2/3 charge controller.

use crate::{
    protocol as proto,
    tokio_async::EVCC,
    tokio_common::Result,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_modbus::client::Context;

#[derive(Clone)]
pub struct SafeClient {
    ctx: Arc<Mutex<Context>>,
}

impl SafeClient {
    /// Creates a new asynchronous `SafeClient`.
    pub fn new(ctx: Context) -> Self {
        Self {
            ctx: Arc::new(Mutex::new(ctx)),
        }
    }

    /// Read device ID, firmware revision, hardware config
    pub async fn read_device_info(&mut self) -> Result<proto::DeviceIdAndFirmwareRevision> {
        let mut ctx = self.ctx.lock().await;
        EVCC::read_device_info(&mut ctx).await
    }

    /// Read Modbus baud rate, parity, stop bits
    pub async fn read_modbus_settings(&mut self) -> Result<proto::ModbusSettings> {
        let mut ctx = self.ctx.lock().await;
        EVCC::read_modbus_settings(&mut ctx).await
    }

    /// Read system flags
    pub async fn read_system_flags(&mut self) -> Result<proto::SystemFlags> {
        let mut ctx = self.ctx.lock().await;
        EVCC::read_system_flags(&mut ctx).await
    }

    /// Read current and status (full 0.1A resolution)
    pub async fn read_current_full(&mut self) -> Result<proto::ReadCurrentFull> {
        let mut ctx = self.ctx.lock().await;
        EVCC::read_current_full(&mut ctx).await
    }

    /// Read current and status (1A resolution)
    pub async fn read_current_amps(&mut self) -> Result<proto::ReadCurrentAmps> {
        let mut ctx = self.ctx.lock().await;
        EVCC::read_current_amps(&mut ctx).await
    }

    /// Set charging current Icmax
    pub async fn set_current(&mut self, icmax: proto::SetIcmax) -> Result<()> {
        let mut ctx = self.ctx.lock().await;
        EVCC::set_current(&mut ctx, icmax).await
    }

    /// Modify state
    pub async fn set_state(&mut self, action: proto::ModifyStateAction) -> Result<()> {
        let mut ctx = self.ctx.lock().await;
        EVCC::set_state(&mut ctx, action).await
    }

    /// Set device ID
    pub async fn set_device_id(&mut self, config: proto::SetDeviceId) -> Result<()> {
        let mut ctx = self.ctx.lock().await;
        EVCC::set_device_id(&mut ctx, config).await
    }
}
