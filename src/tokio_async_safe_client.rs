//! Thread-safe asynchronous client wrapper for the ABL EVCC1 charge controller.

use crate::{
    protocol::{self as proto},
    tokio_async::EVCC,
    tokio_common::{AllValues, Result},
};
use std::time::Duration;
use tokio::sync::Mutex;

pub struct SafeClient {
    context: Mutex<tokio_modbus::client::Context>,
}

impl SafeClient {
    pub fn new(context: tokio_modbus::client::Context) -> Self {
        Self {
            context: Mutex::new(context),
        }
    }

    /// Read vehicle charging status (Register 256 / 0x0100)
    pub async fn read_status(&self) -> Result<proto::ChargeStatus> {
        let mut ctx = self.context.lock().await;
        EVCC::read_status(&mut ctx).await
    }

    /// Read maximum charging current setting / status (Register 257 / 0x0101)
    pub async fn read_current(&self) -> Result<proto::ChargingCurrent> {
        let mut ctx = self.context.lock().await;
        EVCC::read_current(&mut ctx).await
    }

    /// Set maximum charging current in Amperes (Register 257 / 0x0101)
    pub async fn set_current(&self, current: proto::ChargingCurrent) -> Result<()> {
        let mut ctx = self.context.lock().await;
        EVCC::set_current(&mut ctx, current).await
    }

    /// Read active power or current (Register 258 / 0x0102)
    pub async fn read_power_or_current(&self) -> Result<proto::PowerOrCurrent> {
        let mut ctx = self.context.lock().await;
        EVCC::read_power_or_current(&mut ctx).await
    }

    /// Read all measurement values in batch asynchronously
    pub async fn read_all(&self, delay: &Duration) -> Result<AllValues> {
        let mut ctx = self.context.lock().await;
        EVCC::read_all(&mut ctx, delay).await
    }
}
