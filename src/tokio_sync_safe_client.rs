//! Thread-safe synchronous client wrapper for the ABL EVCC1 charge controller.

use crate::{
    protocol::{self as proto},
    tokio_common::{AllValues, Result},
    tokio_sync::EVCC,
};
use std::{sync::Mutex, time::Duration};

pub struct SafeClient {
    context: Mutex<tokio_modbus::client::sync::Context>,
}

impl SafeClient {
    pub fn new(context: tokio_modbus::client::sync::Context) -> Self {
        Self {
            context: Mutex::new(context),
        }
    }

    /// Read vehicle charging status (Register 256 / 0x0100)
    pub fn read_status(&self) -> Result<proto::ChargeStatus> {
        let mut ctx = self.context.lock().unwrap();
        EVCC::read_status(&mut ctx)
    }

    /// Read maximum charging current setting / status (Register 257 / 0x0101)
    pub fn read_current(&self) -> Result<proto::ChargingCurrent> {
        let mut ctx = self.context.lock().unwrap();
        EVCC::read_current(&mut ctx)
    }

    /// Set maximum charging current in Amperes (Register 257 / 0x0101)
    pub fn set_current(&self, current: proto::ChargingCurrent) -> Result<()> {
        let mut ctx = self.context.lock().unwrap();
        EVCC::set_current(&mut ctx, current)
    }

    /// Read active power or current (Register 258 / 0x0102)
    pub fn read_power_or_current(&self) -> Result<proto::PowerOrCurrent> {
        let mut ctx = self.context.lock().unwrap();
        EVCC::read_power_or_current(&mut ctx)
    }

    /// Read all measurement values in batch
    pub fn read_all(&self, delay: &Duration) -> Result<AllValues> {
        let mut ctx = self.context.lock().unwrap();
        EVCC::read_all(&mut ctx, delay)
    }
}
