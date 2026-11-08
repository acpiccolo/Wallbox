//! Synchronous Modbus operations for the ABL EVCC2/3 charge controller.

use crate::{
    protocol::{self as proto, ModbusParam},
    tokio_common::Result,
};
use tokio_modbus::prelude::{SyncReader, SyncWriter};

pub struct EVCC;

impl EVCC {
    /// Read device ID, firmware revision, and hardware config (Register 0x0001, 2 words)
    pub fn read_device_info(
        ctx: &mut tokio_modbus::client::sync::Context,
    ) -> Result<proto::DeviceIdAndFirmwareRevision> {
        let rsp = ctx.read_holding_registers(
            proto::DeviceIdAndFirmwareRevision::ADDRESS,
            proto::DeviceIdAndFirmwareRevision::QUANTITY,
        )??;
        Ok(proto::DeviceIdAndFirmwareRevision::decode_from_words(&rsp)?)
    }

    /// Read Modbus baud rate, parity, stop bits (Register 0x0003, 1 word)
    pub fn read_modbus_settings(
        ctx: &mut tokio_modbus::client::sync::Context,
    ) -> Result<proto::ModbusSettings> {
        let rsp = ctx.read_holding_registers(
            proto::ModbusSettings::ADDRESS,
            proto::ModbusSettings::QUANTITY,
        )??;
        Ok(proto::ModbusSettings::decode_from_words(&rsp)?)
    }

    /// Read system and diagnostic flags (Register 0x0006, 2 words)
    pub fn read_system_flags(
        ctx: &mut tokio_modbus::client::sync::Context,
    ) -> Result<proto::SystemFlags> {
        let rsp = ctx.read_holding_registers(
            proto::SystemFlags::ADDRESS,
            proto::SystemFlags::QUANTITY,
        )??;
        Ok(proto::SystemFlags::decode_from_words(&rsp)?)
    }

    /// Read EV current and status full resolution (Register 0x002E, 5 words)
    pub fn read_current_full(
        ctx: &mut tokio_modbus::client::sync::Context,
    ) -> Result<proto::ReadCurrentFull> {
        let rsp = ctx.read_holding_registers(
            proto::ReadCurrentFull::ADDRESS,
            proto::ReadCurrentFull::QUANTITY,
        )??;
        Ok(proto::ReadCurrentFull::decode_from_words(&rsp)?)
    }

    /// Read EV current and status 1A resolution (Register 0x0033, 3 words)
    pub fn read_current_amps(
        ctx: &mut tokio_modbus::client::sync::Context,
    ) -> Result<proto::ReadCurrentAmps> {
        let rsp = ctx.read_holding_registers(
            proto::ReadCurrentAmps::ADDRESS,
            proto::ReadCurrentAmps::QUANTITY,
        )??;
        Ok(proto::ReadCurrentAmps::decode_from_words(&rsp)?)
    }

    /// Set maximum charging current Icmax (Register 0x0014, 1 word)
    pub fn set_current(
        ctx: &mut tokio_modbus::client::sync::Context,
        icmax: proto::SetIcmax,
    ) -> Result<()> {
        let words = [icmax.encode_to_word()];
        Ok(ctx.write_multiple_registers(proto::SetIcmax::ADDRESS, &words)??)
    }

    /// Modify state (Register 0x0005, 1 word)
    pub fn set_state(
        ctx: &mut tokio_modbus::client::sync::Context,
        action: proto::ModifyStateAction,
    ) -> Result<()> {
        let words = [action.encode_to_word()];
        Ok(ctx.write_multiple_registers(proto::ModifyStateAction::ADDRESS, &words)??)
    }

    /// Set device ID and power-cycle behavior (Register 0x002C, 1 word)
    pub fn set_device_id(
        ctx: &mut tokio_modbus::client::sync::Context,
        config: proto::SetDeviceId,
    ) -> Result<()> {
        let words = [config.encode_to_word()];
        Ok(ctx.write_multiple_registers(proto::SetDeviceId::ADDRESS, &words)??)
    }
}
