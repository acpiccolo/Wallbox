//! This module defines the core data structures, protocol framing, and register mappings
//! for ABL eMH1 / eMH2 (EVCC2/3) wallboxes using Modbus ASCII.

/// Represents errors that can occur within the EVCC protocol logic or Modbus ASCII framing.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// The provided LRC checksum did not match the calculated checksum.
    #[error("LRC checksum error: expected {expected:#04X}, calculated {calculated:#04X}")]
    InvalidLrc { expected: u8, calculated: u8 },

    /// The frame format is invalid or malformed.
    #[error("Invalid Modbus ASCII frame format")]
    InvalidFrame,

    /// Failed to parse hexadecimal representation.
    #[error("Failed to decode hex string")]
    HexDecodeError,

    /// An exception response was returned by the slave device.
    #[error("Modbus exception response: function code {function_code:#04X}, exception code {exception_code:#04X}")]
    ExceptionResponse { function_code: u8, exception_code: u8 },

    /// The provided Device ID is outside the permissible range (0x01..0x10 or 0x00 for broadcast).
    #[error("Device ID {0} is out of permissible range (1..=16)")]
    DeviceIdOutOfRange(u8),

    /// The provided current is out of permissible range.
    #[error("Charging current {0}A is out of permissible range (0 or 6..=80A)")]
    CurrentOutOfRange(u16),

    /// An unexpected or invalid value was received in a register field.
    #[error("Invalid register value encountered")]
    InvalidValue,

    /// The number of words provided is incorrect for the target register or structure.
    #[error("Words count error for register decoding")]
    WordsCountError,
}

/// Calculates the Modbus ASCII Longitudinal Redundancy Check (LRC) checksum over binary bytes.
///
/// The LRC is calculated by summing all bytes (wrapping u8 addition),
/// and taking the two's complement of the resulting sum (i.e. `wrapping_neg()`).
pub fn calculate_lrc(bytes: &[u8]) -> u8 {
    let sum: u8 = bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    sum.wrapping_neg()
}

/// Encodes a Modbus ASCII request or response frame.
///
/// Returns an ASCII string starting with `start_delimiter` (default `:` or `>`),
/// followed by hex-encoded payload (device_id, function_code, payload), 2-digit hex LRC checksum, and `\r\n`.
pub fn encode_ascii_frame(device_id: u8, function_code: u8, payload: &[u8], start_delimiter: char) -> String {
    let mut raw_bytes = Vec::with_capacity(2 + payload.len());
    raw_bytes.push(device_id);
    raw_bytes.push(function_code);
    raw_bytes.extend_from_slice(payload);

    let lrc = calculate_lrc(&raw_bytes);

    let mut frame = String::new();
    frame.push(start_delimiter);
    for b in &raw_bytes {
        frame.push_str(&format!("{:02X}", b));
    }
    frame.push_str(&format!("{:02X}", lrc));
    frame.push_str("\r\n");
    frame
}

/// Decodes a Modbus ASCII frame string into `(device_id, function_code, payload)`.
///
/// Verifies the LRC checksum and returns an error if the frame is malformed or invalid.
pub fn decode_ascii_frame(frame: &str) -> Result<(u8, u8, Vec<u8>), Error> {
    let trimmed = frame.trim();
    if trimmed.len() < 7 {
        return Err(Error::InvalidFrame);
    }

    // Strip leading delimiter (':' or '>')
    let hex_part = if trimmed.starts_with(':') || trimmed.starts_with('>') {
        &trimmed[1..]
    } else {
        trimmed
    };

    if hex_part.len() % 2 != 0 {
        return Err(Error::InvalidFrame);
    }

    let mut bytes = Vec::with_capacity(hex_part.len() / 2);
    for i in (0..hex_part.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex_part[i..i + 2], 16).map_err(|_| Error::HexDecodeError)?;
        bytes.push(byte);
    }

    if bytes.len() < 3 {
        return Err(Error::InvalidFrame);
    }

    let lrc_idx = bytes.len() - 1;
    let received_lrc = bytes[lrc_idx];
    let data_bytes = &bytes[..lrc_idx];

    let calculated_lrc = calculate_lrc(data_bytes);
    if received_lrc != calculated_lrc {
        return Err(Error::InvalidLrc {
            expected: received_lrc,
            calculated: calculated_lrc,
        });
    }

    let device_id = data_bytes[0];
    let function_code = data_bytes[1];
    let payload = data_bytes[2..].to_vec();

    // Check if exception response (function code highest bit set)
    if (function_code & 0x80) != 0 {
        let exception_code = if !payload.is_empty() { payload[0] } else { 0 };
        return Err(Error::ExceptionResponse {
            function_code,
            exception_code,
        });
    }

    Ok((device_id, function_code, payload))
}

// =============================================================================
// ABL EVCC2/3 (eMH1 / eMH2) Modbus ASCII Protocol (docs/Schnittstellenbeschreibung_Modbus-ASCII.pdf)
// =============================================================================

/// Register addresses for ABL EVCC2/3 charge controllers
pub mod evcc2_registers {
    pub const REG_FIRMWARE: u16 = 0x0001;     // 2 words
    pub const REG_MODBUS_SETTINGS: u16 = 0x0003; // 1 word
    pub const REG_STATUS: u16 = 0x0004;       // 1 word
    pub const REG_MODIFY_STATE: u16 = 0x0005; // 1 word
    pub const REG_SYSTEM_FLAGS: u16 = 0x0006; // 2 words
    pub const REG_ENABLED: u16 = 0x000F;      // 5 words
    pub const REG_AMPS_CONFIG: u16 = 0x0014;  // 1 word
    pub const REG_STATUS_LONG: u16 = 0x002E;  // 5 words
    pub const REG_STATUS_AMPS: u16 = 0x0033;  // 3 words

    pub const AMPS_DISABLED: u16 = 0x03E8; // 1000 = disabled / 100% duty cycle
}

/// EVCC2/3 state code (as reported in register 0x0004 and 0x002E).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Evcc2Status {
    /// 0xA1: Waiting for EV (IEC State A)
    WaitingForEv,
    /// 0xB1: EV is asking for charging (IEC State B)
    EvAskingForCharging,
    /// 0xB2: EV has the permission to charge (IEC State B)
    EvPermissionToCharge,
    /// 0xC2: EV is charged (IEC State C)
    Charging,
    /// 0xC3: C2, reduced current (error F16, F17)
    ChargingReducedF16F17,
    /// 0xC4: C2, reduced current (imbalance F15)
    ChargingReducedImbalance,
    /// 0xE0: Outlet disabled
    OutletDisabled,
    /// 0xE1: Production test
    ProductionTest,
    /// 0xE2: EVCC setup mode
    SetupMode,
    /// 0xE3: Bus idle
    BusIdle,
    /// 0xF1: Unintended closed contact (Welding)
    WeldingDetected,
    /// 0xF2: Internal error
    InternalError,
    /// 0xF3: DC residual current detected
    ResidualCurrentDetected,
    /// 0xF4: Upstream communication timeout
    CommunicationTimeout,
    /// 0xF5: Lock of socket failed
    SocketLockFailed,
    /// 0xF6: CS out of range
    CableSenseOutOfRange,
    /// 0xF7: State D requested by EV
    VentilationRequested,
    /// 0xF8: CP out of range
    ControlPilotOutOfRange,
    /// 0xF9: Overcurrent detected
    OvercurrentDetected,
    /// 0xFA: Temperature outside limits
    TemperatureOutsideLimits,
    /// 0xFB: Unintended opened contact
    ContactorOpenedError,
    /// Other / vendor specific
    Other(u8),
}

impl Evcc2Status {
    pub fn from_raw(code: u8) -> Self {
        match code {
            0xA1 => Self::WaitingForEv,
            0xB1 => Self::EvAskingForCharging,
            0xB2 => Self::EvPermissionToCharge,
            0xC2 => Self::Charging,
            0xC3 => Self::ChargingReducedF16F17,
            0xC4 => Self::ChargingReducedImbalance,
            0xE0 => Self::OutletDisabled,
            0xE1 => Self::ProductionTest,
            0xE2 => Self::SetupMode,
            0xE3 => Self::BusIdle,
            0xF1 => Self::WeldingDetected,
            0xF2 => Self::InternalError,
            0xF3 => Self::ResidualCurrentDetected,
            0xF4 => Self::CommunicationTimeout,
            0xF5 => Self::SocketLockFailed,
            0xF6 => Self::CableSenseOutOfRange,
            0xF7 => Self::VentilationRequested,
            0xF8 => Self::ControlPilotOutOfRange,
            0xF9 => Self::OvercurrentDetected,
            0xFA => Self::TemperatureOutsideLimits,
            0xFB => Self::ContactorOpenedError,
            other => Self::Other(other),
        }
    }

    pub fn raw_code(&self) -> u8 {
        match self {
            Self::WaitingForEv => 0xA1,
            Self::EvAskingForCharging => 0xB1,
            Self::EvPermissionToCharge => 0xB2,
            Self::Charging => 0xC2,
            Self::ChargingReducedF16F17 => 0xC3,
            Self::ChargingReducedImbalance => 0xC4,
            Self::OutletDisabled => 0xE0,
            Self::ProductionTest => 0xE1,
            Self::SetupMode => 0xE2,
            Self::BusIdle => 0xE3,
            Self::WeldingDetected => 0xF1,
            Self::InternalError => 0xF2,
            Self::ResidualCurrentDetected => 0xF3,
            Self::CommunicationTimeout => 0xF4,
            Self::SocketLockFailed => 0xF5,
            Self::CableSenseOutOfRange => 0xF6,
            Self::VentilationRequested => 0xF7,
            Self::ControlPilotOutOfRange => 0xF8,
            Self::OvercurrentDetected => 0xF9,
            Self::TemperatureOutsideLimits => 0xFA,
            Self::ContactorOpenedError => 0xFB,
            Self::Other(c) => *c,
        }
    }

    /// Single letter status according to IEC 61851-1 ('A', 'B', 'C', 'D', 'E', 'F')
    pub fn as_char(&self) -> char {
        match self {
            Self::WaitingForEv => 'A',
            Self::EvAskingForCharging | Self::EvPermissionToCharge => 'B',
            Self::Charging | Self::ChargingReducedF16F17 | Self::ChargingReducedImbalance => 'C',
            Self::VentilationRequested => 'D',
            Self::OutletDisabled | Self::ProductionTest | Self::SetupMode | Self::BusIdle => 'E',
            Self::WeldingDetected
            | Self::InternalError
            | Self::ResidualCurrentDetected
            | Self::CommunicationTimeout
            | Self::SocketLockFailed
            | Self::CableSenseOutOfRange
            | Self::ControlPilotOutOfRange
            | Self::OvercurrentDetected
            | Self::TemperatureOutsideLimits
            | Self::ContactorOpenedError => 'F',
            Self::Other(c) => {
                let nibble = c >> 4;
                match nibble {
                    0x0A => 'A',
                    0x0B => 'B',
                    0x0C => 'C',
                    0x0D => 'D',
                    0x0E => 'E',
                    0x0F => 'F',
                    _ => '?',
                }
            }
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::WaitingForEv => "Waiting for EV (Standby)",
            Self::EvAskingForCharging => "EV connected, waiting for charge release",
            Self::EvPermissionToCharge => "EV connected, charging enabled",
            Self::Charging => "EV is charging",
            Self::ChargingReducedF16F17 => "Charging with reduced current (error F16/F17)",
            Self::ChargingReducedImbalance => "Charging with reduced current (imbalance F15)",
            Self::OutletDisabled => "Outlet disabled",
            Self::ProductionTest => "Production test",
            Self::SetupMode => "EVCC setup mode",
            Self::BusIdle => "Bus idle",
            Self::WeldingDetected => "Unintended closed contact (Welding fault)",
            Self::InternalError => "Internal controller error",
            Self::ResidualCurrentDetected => "DC residual current detected",
            Self::CommunicationTimeout => "Upstream communication timeout",
            Self::SocketLockFailed => "Socket lock failed",
            Self::CableSenseOutOfRange => "Cable sensing (CS) resistor out of range",
            Self::VentilationRequested => "State D requested by EV (external ventilation)",
            Self::ControlPilotOutOfRange => "Control pilot (CP) voltage out of range",
            Self::OvercurrentDetected => "Overcurrent detected",
            Self::TemperatureOutsideLimits => "Temperature outside operating limits",
            Self::ContactorOpenedError => "Unintended opened contact",
            Self::Other(_) => "Unknown EVCC2 status",
        }
    }
}

impl std::fmt::Display for Evcc2Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "State {} ({:#04X}): {}",
            self.as_char(),
            self.raw_code(),
            self.description()
        )
    }
}

/// Firmware and hardware version info from Register 0x0001 (2 words).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Evcc2Firmware {
    pub device_id: u8,
    pub pcba_version: String,
    pub firmware_revision: String,
    pub current_sensor_present: bool,
    pub rdc_md_enabled: bool,
    pub welding_detection_enabled: bool,
    pub outlet_enabled: bool,
}

impl Evcc2Firmware {
    pub fn decode(payload: &[u8]) -> Result<Self, Error> {
        if payload.len() < 4 {
            return Err(Error::WordsCountError);
        }
        // Byte 0: register referring to (0x01)
        // Byte 1: Bits 7..6 PCBA, Bits 5..0 Device ID
        let b1 = payload[1];
        let pcba_bits = (b1 >> 6) & 0x03;
        let pcba_version = match pcba_bits {
            0 => "141215".to_string(),
            1 => "160307".to_string(),
            2 => "170725".to_string(),
            _ => format!("unknown ({pcba_bits})"),
        };
        let device_id = b1 & 0x3F;

        // Byte 2: Major (bits 7..4) and Minor (bits 3..0)
        let b2 = payload[2];
        let major = (b2 >> 4) & 0x0F;
        let minor = b2 & 0x0F;
        let firmware_revision = format!("{major}.{minor}");

        // Byte 3: config flags
        let b3 = payload[3];
        let current_sensor_present = (b3 & 0x20) != 0;
        let rdc_md_enabled = (b3 & 0x10) != 0;
        let welding_detection_enabled = (b3 & 0x04) != 0;
        let outlet_enabled = (b3 & 0x03) == 0x02;

        Ok(Self {
            device_id,
            pcba_version,
            firmware_revision,
            current_sensor_present,
            rdc_md_enabled,
            welding_detection_enabled,
            outlet_enabled,
        })
    }
}

impl std::fmt::Display for Evcc2Firmware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Device ID: {}", self.device_id)?;
        writeln!(f, "Firmware Revision: V{}", self.firmware_revision)?;
        writeln!(f, "PCBA Version: {}", self.pcba_version)?;
        writeln!(f, "Internal Current Sensor: {}", if self.current_sensor_present { "Present" } else { "Not installed" })?;
        writeln!(f, "RDC-MD Sensor: {}", if self.rdc_md_enabled { "Enabled" } else { "Disabled" })?;
        writeln!(f, "Contactor Welding Monitor: {}", if self.welding_detection_enabled { "Enabled" } else { "Disabled" })?;
        write!(f, "Outlet State: {}", if self.outlet_enabled { "Enabled" } else { "Disabled" })
    }
}

/// Complete measurement and status set from Register 0x002E (5 words / 10 bytes).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Evcc2Measurements {
    pub status: Evcc2Status,
    pub ev_connected: bool,
    pub duty_cycle_reduced: bool,
    pub en1_closed: bool,
    pub en2_closed: bool,
    pub duty_cycle_permille: u16,
    pub configured_current_amperes: f32,
    pub current_l1_amperes: f32,
    pub current_l2_amperes: f32,
    pub current_l3_amperes: f32,
    pub total_power_watts: f32,
}

impl Evcc2Measurements {
    pub fn decode(payload: &[u8]) -> Result<Self, Error> {
        if payload.len() < 10 {
            return Err(Error::WordsCountError);
        }
        // Byte 0: 0x2E
        // Byte 1: State code
        let status = Evcc2Status::from_raw(payload[1]);

        // Byte 2:
        // bit 7: U_CP >= 10V (EV connected)
        // bit 6: Duty cycle reduced
        // bit 5: EN2 closed
        // bit 4: EN1 closed
        // bits 3..0: upper 4 bits of duty cycle
        let b2 = payload[2];
        let ev_connected = (b2 & 0x80) != 0;
        let duty_cycle_reduced = (b2 & 0x40) != 0;
        let en2_closed = (b2 & 0x20) != 0;
        let en1_closed = (b2 & 0x10) != 0;

        let duty_high = (b2 & 0x0F) as u16;
        let duty_low = payload[3] as u16;
        let duty_cycle_permille = (duty_high << 8) | duty_low;

        // Configured current limit in Amperes: duty * 0.06
        let configured_current_amperes = if duty_cycle_permille == evcc2_registers::AMPS_DISABLED || duty_cycle_permille == 0 {
            0.0
        } else {
            (duty_cycle_permille as f32 * 0.06 * 10.0).round() / 10.0
        };

        // Currents in 0.1A
        let raw_l1 = u16::from_be_bytes([payload[4], payload[5]]);
        let raw_l2 = u16::from_be_bytes([payload[6], payload[7]]);
        let raw_l3 = u16::from_be_bytes([payload[8], payload[9]]);

        let l1 = if raw_l1 == 0x03E8 || raw_l1 == 1 { 0.0 } else { raw_l1 as f32 / 10.0 };
        let l2 = if raw_l2 == 0x03E8 || raw_l2 == 1 { 0.0 } else { raw_l2 as f32 / 10.0 };
        let l3 = if raw_l3 == 0x03E8 || raw_l3 == 1 { 0.0 } else { raw_l3 as f32 / 10.0 };

        // Power in Watts at 230V per phase
        let total_power_watts = (l1 + l2 + l3) * 230.0;

        Ok(Self {
            status,
            ev_connected,
            duty_cycle_reduced,
            en1_closed,
            en2_closed,
            duty_cycle_permille,
            configured_current_amperes,
            current_l1_amperes: l1,
            current_l2_amperes: l2,
            current_l3_amperes: l3,
            total_power_watts,
        })
    }
}

impl std::fmt::Display for Evcc2Measurements {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Status: {}", self.status)?;
        writeln!(f, "EV Connected: {}", if self.ev_connected { "Yes" } else { "No" })?;
        writeln!(f, "Configured Current Limit: {:.1}A (Duty Cycle: {:.1}%)", self.configured_current_amperes, self.duty_cycle_permille as f32 / 10.0)?;
        writeln!(f, "Current L1: {:.1} A", self.current_l1_amperes)?;
        writeln!(f, "Current L2: {:.1} A", self.current_l2_amperes)?;
        writeln!(f, "Current L3: {:.1} A", self.current_l3_amperes)?;
        write!(f, "Total Active Power: {:.1} W", self.total_power_watts)
    }
}

/// Convert target Amperes (6..=80 or 0 to disable) to EVCC2 duty cycle parameter.
pub fn amperes_to_evcc2_duty_cycle(amperes: u16) -> u16 {
    if amperes == 0 {
        evcc2_registers::AMPS_DISABLED
    } else {
        ((amperes as f32 / 0.06).round() as u16).clamp(80, 1000)
    }
}
