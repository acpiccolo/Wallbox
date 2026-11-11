//! Asynchronous client and low-level functions for communicating with ABL eMH1 (EVCC2/3) wallboxes.
//!
//! Provides the asynchronous Modbus ASCII client [`AsyncAsciiClient`] and stateless helper struct [`Evcc2Async`],
//! supporting both direct TCP connections and asynchronous serial ports (`tokio_serial::SerialStream`).

use crate::protocol::{
    amperes_to_evcc2_duty_cycle, decode_ascii_frame, encode_ascii_frame, evcc2_registers,
    Evcc2Firmware, Evcc2Measurements, Evcc2ModbusSettings, Evcc2Status, Evcc2SystemFlags,
};
use anyhow::{bail, Context, Result};
use std::{net::SocketAddr, pin::Pin, task::{Context as TaskContext, Poll}, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadBuf};
use tokio::net::TcpStream;
use tokio_serial::SerialStream;

/// Asynchronous transport supporting either a network TCP stream or an asynchronous serial port.
#[derive(Debug)]
enum AsyncAsciiTransport {
    Tcp(TcpStream),
    Serial(SerialStream),
}

impl AsyncRead for AsyncAsciiTransport {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(t) => Pin::new(t).poll_read(cx, buf),
            Self::Serial(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for AsyncAsciiTransport {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Self::Tcp(t) => Pin::new(t).poll_write(cx, buf),
            Self::Serial(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(t) => Pin::new(t).poll_flush(cx),
            Self::Serial(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(t) => Pin::new(t).poll_shutdown(cx),
            Self::Serial(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

/// An asynchronous Modbus ASCII client for ABL EVCC2/3 controllers.
#[derive(Debug)]
pub struct AsyncAsciiClient {
    reader: BufReader<AsyncAsciiTransport>,
    slave_id: u8,
    delay: Duration,
}

impl AsyncAsciiClient {
    /// Connect via direct TCP to a transparent serial-to-ethernet server (e.g. `192.168.3.128:8899`).
    pub async fn connect_tcp(
        tcp_addr: &str,
        slave_id: u8,
        delay: Duration,
    ) -> Result<Self> {
        let socket_addr: SocketAddr = tcp_addr
            .parse()
            .with_context(|| format!("Cannot parse TCP socket address: {tcp_addr}"))?;
        let stream = TcpStream::connect(&socket_addr).await
            .with_context(|| format!("Failed to connect to async TCP server {tcp_addr}"))?;

        Ok(Self {
            reader: BufReader::new(AsyncAsciiTransport::Tcp(stream)),
            slave_id,
            delay,
        })
    }

    /// Connect via asynchronous serial port (e.g. `/dev/ttyUSB0`, 38400 8E1).
    pub fn connect_serial(
        device: &str,
        baud_rate: u32,
        parity: tokio_serial::Parity,
        stop_bits: tokio_serial::StopBits,
        slave_id: u8,
        delay: Duration,
    ) -> Result<Self> {
        let builder = tokio_serial::new(device, baud_rate)
            .parity(parity)
            .stop_bits(stop_bits)
            .data_bits(tokio_serial::DataBits::Eight);
        let stream = SerialStream::open(&builder)
            .with_context(|| format!("Failed to open async serial port {device}"))?;

        Ok(Self {
            reader: BufReader::new(AsyncAsciiTransport::Serial(stream)),
            slave_id,
            delay,
        })
    }

    /// Send a raw Modbus ASCII request and receive the verified response payload asynchronously.
    pub async fn transaction(&mut self, function_code: u8, request_payload: &[u8]) -> Result<Vec<u8>> {
        let frame = encode_ascii_frame(self.slave_id, function_code, request_payload, ':');
        let transport = self.reader.get_mut();
        transport
            .write_all(frame.as_bytes())
            .await
            .with_context(|| "Failed to write async Modbus ASCII request")?;
        transport
            .flush()
            .await
            .with_context(|| "Failed to flush async Modbus ASCII stream")?;

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .await
            .with_context(|| "Failed to read async Modbus ASCII response")?;

        let (dev_id, fc, payload) = decode_ascii_frame(&line)
            .with_context(|| format!("Failed to decode response frame: {:?}", line.trim()))?;

        if dev_id != self.slave_id {
            bail!(crate::Error::SlaveIdMismatch {
                expected: self.slave_id,
                received: dev_id,
            });
        }

        if fc != function_code {
            bail!(crate::Error::FunctionCodeMismatch {
                expected: function_code,
                received: fc,
            });
        }

        Ok(payload)
    }

    /// Read holding registers asynchronously with retry.
    pub async fn read_holding_registers(&mut self, start_reg: u16, qty: u16) -> Result<Vec<u8>> {
        let mut req = Vec::with_capacity(4);
        req.extend_from_slice(&start_reg.to_be_bytes());
        req.extend_from_slice(&qty.to_be_bytes());

        let mut last_err = None;
        for attempt in 1..=2 {
            if attempt > 1 {
                tokio::time::sleep(self.delay).await;
            }
            match self.transaction(0x03, &req).await {
                Ok(payload) => {
                    if payload.is_empty() {
                        bail!(crate::Error::EmptyPayload);
                    }
                    let byte_count = payload[0] as usize;
                    let data = &payload[1..];
                    if data.len() < byte_count {
                        bail!(crate::Error::IncompletePayload {
                            expected: byte_count,
                            received: data.len(),
                        });
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

    /// Write multiple registers asynchronously with retry.
    pub async fn write_multiple_registers(&mut self, start_reg: u16, words: &[u16]) -> Result<()> {
        let mut req = Vec::with_capacity(5 + words.len() * 2);
        req.extend_from_slice(&start_reg.to_be_bytes());
        req.extend_from_slice(&(words.len() as u16).to_be_bytes());
        req.push((words.len() * 2) as u8);
        for w in words {
            req.extend_from_slice(&w.to_be_bytes());
        }

        let mut last_err = None;
        for _ in 0..2 {
            match self.transaction(0x10, &req).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(self.delay).await;
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// Read firmware info asynchronously.
    pub async fn read_firmware(&mut self) -> Result<Evcc2Firmware> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_FIRMWARE, 2).await?;
        Evcc2Firmware::decode(&bytes).with_context(|| "Failed to parse EVCC2 firmware info")
    }

    /// Read Modbus settings asynchronously.
    pub async fn read_modbus_settings(&mut self) -> Result<Evcc2ModbusSettings> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_MODBUS_SETTINGS, 1).await?;
        Evcc2ModbusSettings::decode(&bytes).with_context(|| "Failed to parse EVCC2 Modbus settings")
    }

    /// Read vehicle charging status asynchronously.
    pub async fn read_status(&mut self) -> Result<Evcc2Status> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_STATUS, 1).await?;
        if bytes.len() < 2 {
            bail!("Invalid response length for status register");
        }
        Ok(Evcc2Status::from_raw(bytes[1]))
    }

    /// Read system flags asynchronously.
    pub async fn read_system_flags(&mut self) -> Result<Evcc2SystemFlags> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_SYSTEM_FLAGS, 2).await?;
        Evcc2SystemFlags::decode(&bytes).with_context(|| "Failed to parse EVCC2 system flags")
    }

    /// Read currently configured current limit asynchronously.
    pub async fn read_configured_current(&mut self) -> Result<(f32, u16)> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_AMPS_CONFIG, 1).await?;
        if bytes.len() < 2 {
            bail!("Invalid response length for current configuration register");
        }
        let duty = u16::from_be_bytes([bytes[0], bytes[1]]);
        let amps = if duty == evcc2_registers::AMPS_DISABLED || duty == 0 {
            0.0
        } else {
            (duty as f32 * 0.06 * 10.0).round() / 10.0
        };
        Ok((amps, duty))
    }

    /// Read measurements and 3-phase currents asynchronously.
    pub async fn read_measurements(&mut self) -> Result<Evcc2Measurements> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_STATUS_LONG, 5).await?;
        Evcc2Measurements::decode(&bytes).with_context(|| "Failed to parse EVCC2 measurement data")
    }

    /// Set charging current limit asynchronously.
    pub async fn set_current(&mut self, amperes: u16) -> Result<()> {
        let duty = amperes_to_evcc2_duty_cycle(amperes);
        self.write_multiple_registers(evcc2_registers::REG_AMPS_CONFIG, &[duty]).await
    }

    /// Enable charging asynchronously.
    pub async fn enable(&mut self, amperes: Option<u16>) -> Result<()> {
        let amps = amperes.unwrap_or(16);
        self.set_current(amps).await
    }

    /// Disable charging asynchronously.
    pub async fn disable(&mut self) -> Result<()> {
        self.set_current(0).await
    }

    /// Wake up vehicle asynchronously via Control Pilot sequence (E0 -> wait 3s -> A1).
    pub async fn wake_up(&mut self) -> Result<()> {
        self.write_multiple_registers(evcc2_registers::REG_MODIFY_STATE, &[0xE0E0]).await?;
        tokio::time::sleep(Duration::from_secs(3)).await;
        self.write_multiple_registers(evcc2_registers::REG_MODIFY_STATE, &[0xA1A1]).await?;
        Ok(())
    }

    /// Reset charge controller asynchronously.
    pub async fn reset(&mut self) -> Result<()> {
        self.write_multiple_registers(evcc2_registers::REG_MODIFY_STATE, &[0x5A5A]).await
    }
}
