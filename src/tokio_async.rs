//! Asynchronous, low-level functions for communicating with ABL eMH1 (EVCC2/3) wallboxes.
//!
//! Provides direct, asynchronous functions for interacting with an ABL eMH1 over Modbus ASCII.

use crate::protocol::{
    amperes_to_evcc2_duty_cycle, decode_ascii_frame, encode_ascii_frame, evcc2_registers,
    Evcc2Firmware, Evcc2Measurements, Evcc2ModbusSettings, Evcc2Status, Evcc2SystemFlags,
};
use anyhow::{bail, Context, Result};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// Stateless asynchronous functions for interacting with an ABL eMH1 wallbox over TCP.
pub struct Evcc2Async;

impl Evcc2Async {
    /// Send a raw Modbus ASCII transaction asynchronously over a Tokio TCP stream.
    pub async fn transaction(
        reader: &mut BufReader<TcpStream>,
        slave_id: u8,
        function_code: u8,
        request_payload: &[u8],
    ) -> Result<Vec<u8>> {
        let frame = encode_ascii_frame(slave_id, function_code, request_payload, ':');
        let transport = reader.get_mut();
        transport.write_all(frame.as_bytes()).await
            .with_context(|| "Failed to write async Modbus ASCII request")?;
        transport.flush().await
            .with_context(|| "Failed to flush async Modbus ASCII stream")?;

        let mut line = String::new();
        reader.read_line(&mut line).await
            .with_context(|| "Failed to read async Modbus ASCII response")?;

        let (dev_id, fc, payload) = decode_ascii_frame(&line)
            .with_context(|| format!("Failed to decode response frame: {:?}", line.trim()))?;

        if dev_id != slave_id {
            bail!("Mismatched slave ID: expected {}, got {}", slave_id, dev_id);
        }
        if fc != function_code {
            bail!("Mismatched function code: expected {:#04X}, got {:#04X}", function_code, fc);
        }

        Ok(payload)
    }

    /// Read holding registers asynchronously.
    pub async fn read_holding_registers(
        reader: &mut BufReader<TcpStream>,
        slave_id: u8,
        start_reg: u16,
        qty: u16,
        delay: Duration,
    ) -> Result<Vec<u8>> {
        let mut req = Vec::with_capacity(4);
        req.extend_from_slice(&start_reg.to_be_bytes());
        req.extend_from_slice(&qty.to_be_bytes());

        let mut last_err = None;
        for attempt in 1..=2 {
            if attempt > 1 {
                tokio::time::sleep(delay).await;
            }
            match Self::transaction(reader, slave_id, 0x03, &req).await {
                Ok(payload) => {
                    if payload.is_empty() {
                        bail!("Empty payload received for ReadHoldingRegisters");
                    }
                    let byte_count = payload[0] as usize;
                    let data = &payload[1..];
                    if data.len() < byte_count {
                        bail!("Incomplete data payload: expected {} bytes, got {}", byte_count, data.len());
                    }
                    return Ok(data[..byte_count].to_vec());
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// Write multiple registers asynchronously.
    pub async fn write_multiple_registers(
        reader: &mut BufReader<TcpStream>,
        slave_id: u8,
        start_reg: u16,
        words: &[u16],
        delay: Duration,
    ) -> Result<()> {
        let mut req = Vec::with_capacity(5 + words.len() * 2);
        req.extend_from_slice(&start_reg.to_be_bytes());
        req.extend_from_slice(&(words.len() as u16).to_be_bytes());
        req.push((words.len() * 2) as u8);
        for w in words {
            req.extend_from_slice(&w.to_be_bytes());
        }

        let mut last_err = None;
        for _ in 0..2 {
            match Self::transaction(reader, slave_id, 0x10, &req).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(delay).await;
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// Read firmware info asynchronously.
    pub async fn read_firmware(reader: &mut BufReader<TcpStream>, slave_id: u8, delay: Duration) -> Result<Evcc2Firmware> {
        let bytes = Self::read_holding_registers(reader, slave_id, evcc2_registers::REG_FIRMWARE, 2, delay).await?;
        Evcc2Firmware::decode(&bytes).with_context(|| "Failed to parse EVCC2 firmware info")
    }

    /// Read Modbus settings asynchronously.
    pub async fn read_modbus_settings(reader: &mut BufReader<TcpStream>, slave_id: u8, delay: Duration) -> Result<Evcc2ModbusSettings> {
        let bytes = Self::read_holding_registers(reader, slave_id, evcc2_registers::REG_MODBUS_SETTINGS, 1, delay).await?;
        Evcc2ModbusSettings::decode(&bytes).with_context(|| "Failed to parse EVCC2 Modbus settings")
    }

    /// Read vehicle status asynchronously.
    pub async fn read_status(reader: &mut BufReader<TcpStream>, slave_id: u8, delay: Duration) -> Result<Evcc2Status> {
        let bytes = Self::read_holding_registers(reader, slave_id, evcc2_registers::REG_STATUS, 1, delay).await?;
        if bytes.len() < 2 {
            bail!("Invalid response length for status register");
        }
        Ok(Evcc2Status::from_raw(bytes[1]))
    }

    /// Read system flags asynchronously.
    pub async fn read_system_flags(reader: &mut BufReader<TcpStream>, slave_id: u8, delay: Duration) -> Result<Evcc2SystemFlags> {
        let bytes = Self::read_holding_registers(reader, slave_id, evcc2_registers::REG_SYSTEM_FLAGS, 2, delay).await?;
        Evcc2SystemFlags::decode(&bytes).with_context(|| "Failed to parse EVCC2 system flags")
    }

    /// Read measurements and currents asynchronously.
    pub async fn read_measurements(reader: &mut BufReader<TcpStream>, slave_id: u8, delay: Duration) -> Result<Evcc2Measurements> {
        let bytes = Self::read_holding_registers(reader, slave_id, evcc2_registers::REG_STATUS_LONG, 5, delay).await?;
        Evcc2Measurements::decode(&bytes).with_context(|| "Failed to parse EVCC2 measurement data")
    }

    /// Set charging current limit asynchronously.
    pub async fn set_current(reader: &mut BufReader<TcpStream>, slave_id: u8, amperes: u16, delay: Duration) -> Result<()> {
        let duty = amperes_to_evcc2_duty_cycle(amperes);
        Self::write_multiple_registers(reader, slave_id, evcc2_registers::REG_AMPS_CONFIG, &[duty], delay).await
    }
}
