//! Synchronous client and low-level functions for communicating with ABL eMH1 (EVCC2/3) wallboxes.
//!
//! Provides the Modbus ASCII client [`AsciiClient`] and stateless helper struct [`Evcc2`].

use crate::protocol::{
    amperes_to_evcc2_duty_cycle, decode_ascii_frame, encode_ascii_frame, evcc2_registers,
    Evcc2Firmware, Evcc2Measurements, Evcc2ModbusSettings, Evcc2Status, Evcc2SystemFlags,
};
use anyhow::{bail, Context, Result};
use log::*;
use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

pub enum AsciiTransport {
    Serial(Box<dyn tokio_serial::SerialPort>),
    Tcp(TcpStream),
}

impl From<Box<dyn tokio_serial::SerialPort>> for AsciiTransport {
    fn from(s: Box<dyn tokio_serial::SerialPort>) -> Self {
        Self::Serial(s)
    }
}

impl From<TcpStream> for AsciiTransport {
    fn from(t: TcpStream) -> Self {
        Self::Tcp(t)
    }
}

impl std::io::Read for AsciiTransport {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Serial(s) => s.read(buf),
            Self::Tcp(t) => t.read(buf),
        }
    }
}

impl std::io::Write for AsciiTransport {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Serial(s) => s.write(buf),
            Self::Tcp(t) => t.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Serial(s) => s.flush(),
            Self::Tcp(t) => t.flush(),
        }
    }
}

/// A synchronous client for communicating with ABL EVCC controllers via Modbus ASCII.
pub struct AsciiClient {
    reader: BufReader<AsciiTransport>,
    slave_id: u8,
    _timeout: Duration,
    delay: Duration,
}

impl AsciiClient {
    /// Connect via serial device (e.g. `/dev/ttyUSB0`, default 38400 8E1).
    pub fn connect_serial(
        device: &str,
        baud_rate: u32,
        parity: tokio_serial::Parity,
        stop_bits: tokio_serial::StopBits,
        slave_id: u8,
        timeout: Duration,
        delay: Duration,
    ) -> Result<Self> {
        trace!("Opening Modbus ASCII serial port {device} at {baud_rate} baud");
        let port = tokio_serial::new(device, baud_rate)
            .parity(parity)
            .stop_bits(stop_bits)
            .data_bits(tokio_serial::DataBits::Eight)
            .timeout(timeout)
            .open()
            .with_context(|| format!("Failed to open serial device {device}"))?;

        Ok(Self {
            reader: BufReader::new(AsciiTransport::Serial(port)),
            slave_id,
            _timeout: timeout,
            delay,
        })
    }

    /// Connect via TCP (e.g. to serial-to-ethernet bridge like Elfin EW11 at `192.168.3.128:8899`).
    pub fn connect_tcp(
        tcp_addr: &str,
        slave_id: u8,
        timeout: Duration,
        delay: Duration,
    ) -> Result<Self> {
        trace!("Connecting to Modbus ASCII TCP bridge at {tcp_addr}");
        let socket_addr: SocketAddr = tcp_addr
            .parse()
            .with_context(|| format!("Cannot parse TCP socket address: {tcp_addr}"))?;

        let stream = TcpStream::connect_timeout(&socket_addr, timeout)
            .with_context(|| format!("Failed to connect to TCP server {tcp_addr}"))?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;

        Ok(Self {
            reader: BufReader::new(AsciiTransport::Tcp(stream)),
            slave_id,
            _timeout: timeout,
            delay,
        })
    }

    /// Send a raw Modbus ASCII request and receive the verified response payload.
    pub fn transaction(&mut self, function_code: u8, request_payload: &[u8]) -> Result<Vec<u8>> {
        let frame = encode_ascii_frame(self.slave_id, function_code, request_payload, ':');
        trace!("Modbus ASCII send: {:?}", frame.trim());

        let transport = self.reader.get_mut();
        transport
            .write_all(frame.as_bytes())
            .with_context(|| "Failed to write Modbus ASCII request")?;
        transport
            .flush()
            .with_context(|| "Failed to flush Modbus ASCII stream")?;

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .with_context(|| "Failed to read Modbus ASCII response (timed out or connection closed)")?;

        trace!("Modbus ASCII recv: {:?}", line.trim());
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

    /// Read holding registers (Function Code 0x03) with retry.
    pub fn read_holding_registers(&mut self, start_reg: u16, qty: u16) -> Result<Vec<u8>> {
        let mut req = Vec::with_capacity(4);
        req.extend_from_slice(&start_reg.to_be_bytes());
        req.extend_from_slice(&qty.to_be_bytes());

        let mut last_err = None;
        for attempt in 1..=2 {
            if attempt > 1 {
                std::thread::sleep(self.delay);
            }
            match self.transaction(0x03, &req) {
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

    /// Write multiple holding registers (Function Code 0x10) with retry.
    pub fn write_multiple_registers(&mut self, start_reg: u16, words: &[u16]) -> Result<()> {
        let mut req = Vec::with_capacity(5 + words.len() * 2);
        req.extend_from_slice(&start_reg.to_be_bytes());
        req.extend_from_slice(&(words.len() as u16).to_be_bytes());
        req.push((words.len() * 2) as u8);
        for w in words {
            req.extend_from_slice(&w.to_be_bytes());
        }

        let mut last_err = None;
        for _ in 0..2 {
            match self.transaction(0x10, &req) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last_err = Some(e);
                    std::thread::sleep(self.delay);
                }
            }
        }

        Err(last_err.unwrap())
    }

    /// Read EVCC2 firmware and hardware information (Register 0x0001, 2 words).
    pub fn read_firmware(&mut self) -> Result<Evcc2Firmware> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_FIRMWARE, 2)?;
        Evcc2Firmware::decode(&bytes).with_context(|| "Failed to parse EVCC2 firmware info")
    }

    /// Read EVCC2 charging status (Register 0x0004, 1 word).
    pub fn read_status(&mut self) -> Result<Evcc2Status> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_STATUS, 1)?;
        if bytes.len() < 2 {
            bail!("Invalid response length for status register");
        }
        Ok(Evcc2Status::from_raw(bytes[1]))
    }

    /// Read EVCC2 complete measurements and currents (Register 0x002E, 5 words).
    pub fn read_measurements(&mut self) -> Result<Evcc2Measurements> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_STATUS_LONG, 5)?;
        Evcc2Measurements::decode(&bytes).with_context(|| "Failed to parse EVCC2 measurement data")
    }

    /// Read currently configured charging current limit / duty cycle (Register 0x0014, 1 word).
    pub fn read_configured_current(&mut self) -> Result<(f32, u16)> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_AMPS_CONFIG, 1)?;
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

    /// Set charging current limit in Amperes per phase (6..=80A or 0 to stop/disable).
    pub fn set_current(&mut self, amperes: u16) -> Result<()> {
        let duty = amperes_to_evcc2_duty_cycle(amperes);
        trace!("Setting charging current {amperes}A per phase -> duty cycle code {duty}");
        self.write_multiple_registers(evcc2_registers::REG_AMPS_CONFIG, &[duty])
            .with_context(|| format!("Failed to set charging current to {amperes}A"))
    }

    /// Enable charging (resumes charging at specified Amperes, or 16A default).
    pub fn enable(&mut self, amperes: Option<u16>) -> Result<()> {
        let amps = amperes.unwrap_or(16);
        self.set_current(amps)
    }

    /// Disable charging (halts charging / sets duty cycle to disabled 0x03E8).
    pub fn disable(&mut self) -> Result<()> {
        self.set_current(0)
    }

    /// Read Modbus communication settings (Register 0x0003, 1 word).
    pub fn read_modbus_settings(&mut self) -> Result<Evcc2ModbusSettings> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_MODBUS_SETTINGS, 1)?;
        Evcc2ModbusSettings::decode(&bytes)
            .with_context(|| "Failed to parse EVCC2 Modbus settings")
    }

    /// Read system flags and state machine pointer (Register 0x0006, 2 words).
    pub fn read_system_flags(&mut self) -> Result<Evcc2SystemFlags> {
        let bytes = self.read_holding_registers(evcc2_registers::REG_SYSTEM_FLAGS, 2)?;
        Evcc2SystemFlags::decode(&bytes)
            .with_context(|| "Failed to parse EVCC2 system flags")
    }

    /// Reset wallbox charge controller (Register 0x0005, value 0x5A5A).
    pub fn reset(&mut self) -> Result<()> {
        trace!("Sending reset command (0x5A5A) to EVCC2 controller");
        self.write_multiple_registers(evcc2_registers::REG_MODIFY_STATE, &[0x5A5A])
            .with_context(|| "Failed to send reset command to wallbox")
    }

    /// Wake up the vehicle by toggling state E0 (outlet disabled) -> A1 (waiting for EV).
    pub fn wake_up(&mut self) -> Result<()> {
        trace!("Sending wake-up sequence: jump to state E0E0");
        self.write_multiple_registers(evcc2_registers::REG_MODIFY_STATE, &[0xE0E0])?;
        std::thread::sleep(Duration::from_secs(3));
        trace!("Jump back to state A1A1");
        self.write_multiple_registers(evcc2_registers::REG_MODIFY_STATE, &[0xA1A1])?;
        Ok(())
    }
}

/// Stateless synchronous functions for interacting with an ABL eMH1 / EVCC2 wallbox.
pub struct Evcc2;

impl Evcc2 {
    /// Read firmware and hardware information (Register 0x0001).
    pub fn read_firmware(client: &mut AsciiClient) -> Result<Evcc2Firmware> {
        client.read_firmware()
    }

    /// Read Modbus communication settings (Register 0x0003).
    pub fn read_modbus_settings(client: &mut AsciiClient) -> Result<Evcc2ModbusSettings> {
        client.read_modbus_settings()
    }

    /// Read vehicle charging status (Register 0x0004).
    pub fn read_status(client: &mut AsciiClient) -> Result<Evcc2Status> {
        client.read_status()
    }

    /// Read system flags and state machine pointer (Register 0x0006).
    pub fn read_system_flags(client: &mut AsciiClient) -> Result<Evcc2SystemFlags> {
        client.read_system_flags()
    }

    /// Read currently configured charging current and duty cycle (Register 0x0014).
    pub fn read_configured_current(client: &mut AsciiClient) -> Result<(f32, u16)> {
        client.read_configured_current()
    }

    /// Set charging current limit in Amperes per phase (Register 0x0014).
    pub fn set_current(client: &mut AsciiClient, amperes: u16) -> Result<()> {
        client.set_current(amperes)
    }

    /// Read measurements and 3-phase currents (Register 0x002E).
    pub fn read_measurements(client: &mut AsciiClient) -> Result<Evcc2Measurements> {
        client.read_measurements()
    }

    /// Enable charging permission (sets charging current in Amperes per phase).
    pub fn enable(client: &mut AsciiClient, amperes: Option<u16>) -> Result<()> {
        client.enable(amperes)
    }

    /// Disable charging permission (halts charging).
    pub fn disable(client: &mut AsciiClient) -> Result<()> {
        client.disable()
    }

    /// Wake up sleeping vehicle via state E0 -> A1 Control Pilot sequence.
    pub fn wake_up(client: &mut AsciiClient) -> Result<()> {
        client.wake_up()
    }

    /// Reset wallbox charge controller (Register 0x0005, value 0x5A5A).
    pub fn reset(client: &mut AsciiClient) -> Result<()> {
        client.reset()
    }
}
