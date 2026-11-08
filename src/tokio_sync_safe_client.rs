//! Thread-safe synchronous client wrapper for the ABL EVCC2/3 charge controller.

use crate::{
    protocol as proto,
    tokio_common::Result,
    tokio_sync::EVCC,
};
use std::sync::{Arc, Mutex};
use tokio_modbus::client::sync::Context;

#[derive(Clone)]
pub struct SafeClient {
    ctx: Arc<Mutex<Context>>,
}

impl SafeClient {
    /// Creates a new `SafeClient` wrapping a synchronous Modbus context.
    pub fn new(ctx: Context) -> Self {
        Self {
            ctx: Arc::new(Mutex::new(ctx)),
        }
    }

    /// Read device ID, firmware revision, hardware config
    pub fn read_device_info(&mut self) -> Result<proto::DeviceIdAndFirmwareRevision> {
        let mut ctx = self.ctx.lock().unwrap();
        EVCC::read_device_info(&mut ctx)
    }

    /// Read Modbus baud rate, parity, stop bits
    pub fn read_modbus_settings(&mut self) -> Result<proto::ModbusSettings> {
        let mut ctx = self.ctx.lock().unwrap();
        EVCC::read_modbus_settings(&mut ctx)
    }

    /// Read system flags
    pub fn read_system_flags(&mut self) -> Result<proto::SystemFlags> {
        let mut ctx = self.ctx.lock().unwrap();
        EVCC::read_system_flags(&mut ctx)
    }

    /// Read current and status (full 0.1A resolution)
    pub fn read_current_full(&mut self) -> Result<proto::ReadCurrentFull> {
        let mut ctx = self.ctx.lock().unwrap();
        EVCC::read_current_full(&mut ctx)
    }

    /// Read current and status (1A resolution)
    pub fn read_current_amps(&mut self) -> Result<proto::ReadCurrentAmps> {
        let mut ctx = self.ctx.lock().unwrap();
        EVCC::read_current_amps(&mut ctx)
    }

    /// Set charging current Icmax
    pub fn set_current(&mut self, icmax: proto::SetIcmax) -> Result<()> {
        let mut ctx = self.ctx.lock().unwrap();
        EVCC::set_current(&mut ctx, icmax)
    }

    /// Modify state
    pub fn set_state(&mut self, action: proto::ModifyStateAction) -> Result<()> {
        let mut ctx = self.ctx.lock().unwrap();
        EVCC::set_state(&mut ctx, action)
    }

    /// Set device ID
    pub fn set_device_id(&mut self, config: proto::SetDeviceId) -> Result<()> {
        let mut ctx = self.ctx.lock().unwrap();
        EVCC::set_device_id(&mut ctx, config)
    }
}
