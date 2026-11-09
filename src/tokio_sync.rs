//! Synchronous Modbus operations for the ABL EVCC1 charge controller.

use crate::{
    protocol::{self as proto, ModbusParam},
    tokio_common::{AllValues, Result},
};
use std::time::Duration;
use tokio_modbus::prelude::{SyncReader, SyncWriter};

pub struct EVCC;

impl EVCC {
    /// Read vehicle charging status (Register 256 / 0x0100, 1 word)
    pub fn read_status(
        ctx: &mut tokio_modbus::client::sync::Context,
    ) -> Result<proto::ChargeStatus> {
        let rsp = ctx.read_holding_registers(
            proto::ChargeStatus::ADDRESS,
            proto::ChargeStatus::QUANTITY,
        )??;
        Ok(proto::ChargeStatus::decode_from_words(&rsp)?)
    }

    /// Read maximum charging current setting / status (Register 257 / 0x0101, 1 word)
    pub fn read_current(
        ctx: &mut tokio_modbus::client::sync::Context,
    ) -> Result<proto::ChargingCurrent> {
        let rsp = ctx.read_holding_registers(
            proto::ChargingCurrent::ADDRESS,
            proto::ChargingCurrent::QUANTITY,
        )??;
        Ok(proto::ChargingCurrent::decode_from_words(&rsp)?)
    }

    /// Set maximum charging current in Amperes (Register 257 / 0x0101, 1 word)
    pub fn set_current(
        ctx: &mut tokio_modbus::client::sync::Context,
        current: proto::ChargingCurrent,
    ) -> Result<()> {
        let words = [current.encode_to_word()];
        Ok(ctx.write_multiple_registers(proto::ChargingCurrent::ADDRESS, &words)??)
    }

    /// Read active power or current (Register 258 / 0x0102, 1 word)
    pub fn read_power_or_current(
        ctx: &mut tokio_modbus::client::sync::Context,
    ) -> Result<proto::PowerOrCurrent> {
        let rsp = ctx.read_holding_registers(
            proto::PowerOrCurrent::ADDRESS,
            proto::PowerOrCurrent::QUANTITY,
        )??;
        Ok(proto::PowerOrCurrent::decode_from_words(&rsp)?)
    }

    /// Read all measurement and status values in batch
    pub fn read_all(
        ctx: &mut tokio_modbus::client::sync::Context,
        delay: &Duration,
    ) -> Result<AllValues> {
        let status = Self::read_status(ctx)?;
        std::thread::sleep(*delay);
        let max_current = Self::read_current(ctx)?;
        std::thread::sleep(*delay);
        let power_or_current = Self::read_power_or_current(ctx)?;
        Ok(AllValues {
            status,
            max_current,
            power_or_current,
        })
    }
}
