//! This module defines the core data structures, protocol framing, and traits for the EVCC1 Modbus protocol.
//!
//! Registers (decimal):
//! - 256 (0x0100): Charging status (Vehicle status) - ASCII values, e.g. 65 ('A' = Standby/Ready), 66 ('B' = Connected), 67 ('C' = Charging)
//! - 257 (0x0101): Maximum charging current (Setpoint/Status) - Read/Write holding register in Amperes (e.g. 16 = 16A, 0 = Disabled)
//! - 258 (0x0102): Active charging power / current (Read Holding)

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

/// 16-bit word stored in a Modbus register.
pub type Word = u16;

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

/// A trait for defining Modbus parameters and registers.
pub trait ModbusParam: Sized {
    /// The Modbus register address.
    const ADDRESS: u16;
    /// Quantity of Modbus 16-bit words.
    const QUANTITY: u16;
    /// Associated protocol return / value type.
    type ProtocolType;
}

/// Convert a slice of 16-bit words into big-endian byte vector.
pub fn words_to_bytes(words: &[Word]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_be_bytes()).collect()
}

/// Convert a byte slice into a vector of 16-bit words (big-endian).
pub fn bytes_to_words(bytes: &[u8]) -> Vec<Word> {
    bytes
        .chunks(2)
        .map(|chunk| {
            if chunk.len() == 2 {
                u16::from_be_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], 0])
            }
        })
        .collect()
}

// -----------------------------------------------------------------------------
// Register 256 (0x0100): Ladestatus (Fahrzeug-Status) (Read Holding, 1 word)
// -----------------------------------------------------------------------------

/// Vehicle charging status according to IEC 61851-1 / EVCC1.
/// Represented as numeric ASCII code (e.g. 65 for 'A', 66 for 'B', etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VehicleStatus {
    /// Standby / Ready (No vehicle connected) - ASCII 65 ('A')
    Ready,
    /// Vehicle connected, waiting for charge enable - ASCII 66 ('B')
    Connected,
    /// Charging in progress - ASCII 67 ('C')
    Charging,
    /// Charging with external ventilation - ASCII 68 ('D')
    Ventilation,
    /// No power / disconnected / disabled - ASCII 69 ('E')
    Disabled,
    /// Fault / Error condition - ASCII 70 ('F')
    Fault,
    /// Other status value
    Other(u16),
}

impl VehicleStatus {
    pub fn from_raw(val: u16) -> Self {
        // Handle both high byte or low byte encoding if device returns ASCII in lower or upper byte
        let ascii_code = if (val & 0xFF) >= 0x20 && (val & 0xFF) <= 0x7E {
            val & 0xFF
        } else if (val >> 8) >= 0x20 && (val >> 8) <= 0x7E {
            val >> 8
        } else {
            val
        };

        match ascii_code {
            65 => Self::Ready,
            66 => Self::Connected,
            67 => Self::Charging,
            68 => Self::Ventilation,
            69 => Self::Disabled,
            70 => Self::Fault,
            other => Self::Other(other),
        }
    }

    pub fn as_char(&self) -> char {
        match self {
            Self::Ready => 'A',
            Self::Connected => 'B',
            Self::Charging => 'C',
            Self::Ventilation => 'D',
            Self::Disabled => 'E',
            Self::Fault => 'F',
            Self::Other(val) => {
                if *val >= 32 && *val <= 126 {
                    (*val as u8) as char
                } else {
                    '?'
                }
            }
        }
    }

    pub fn raw_code(&self) -> u16 {
        match self {
            Self::Ready => 65,
            Self::Connected => 66,
            Self::Charging => 67,
            Self::Ventilation => 68,
            Self::Disabled => 69,
            Self::Fault => 70,
            Self::Other(val) => *val,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Ready => "Standby / Ready (no vehicle connected)",
            Self::Connected => "Vehicle connected (waiting for charge enable)",
            Self::Charging => "Charging active (vehicle is charging)",
            Self::Ventilation => "Charging with external ventilation",
            Self::Disabled => "Charge point disabled / switched off",
            Self::Fault => "Fault / error condition",
            Self::Other(_) => "Unknown status",
        }
    }
}

impl std::fmt::Display for VehicleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Status {}: {} (ASCII {})",
            self.as_char(),
            self.description(),
            self.raw_code()
        )
    }
}

/// Modbus holding register 256: Charging status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChargeStatus(pub VehicleStatus);

impl ModbusParam for ChargeStatus {
    const ADDRESS: u16 = 256; // 0x0100
    const QUANTITY: u16 = 1;
    type ProtocolType = Self;
}

impl ChargeStatus {
    pub fn decode_from_words(words: &[Word]) -> Result<Self, Error> {
        if words.is_empty() {
            return Err(Error::WordsCountError);
        }
        Ok(Self(VehicleStatus::from_raw(words[0])))
    }
}

impl std::fmt::Display for ChargeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// -----------------------------------------------------------------------------
// Register 257 (0x0101): Maximum charging current (Read / Write Holding, 1 word)
// -----------------------------------------------------------------------------

/// Modbus holding register 257: Maximum charging current in Amperes (e.g. 16 for 16A, 0 for stop)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChargingCurrent(pub u16);

impl ModbusParam for ChargingCurrent {
    const ADDRESS: u16 = 257; // 0x0101
    const QUANTITY: u16 = 1;
    type ProtocolType = Self;
}

impl ChargingCurrent {
    pub fn new(amperes: u16) -> Result<Self, Error> {
        if amperes == 0 || (6..=80).contains(&amperes) {
            Ok(Self(amperes))
        } else {
            Err(Error::CurrentOutOfRange(amperes))
        }
    }

    pub fn encode_to_word(&self) -> Word {
        self.0
    }

    pub fn decode_from_words(words: &[Word]) -> Result<Self, Error> {
        if words.is_empty() {
            return Err(Error::WordsCountError);
        }
        Ok(Self(words[0]))
    }

    pub fn is_enabled(&self) -> bool {
        self.0 >= 6
    }
}

impl std::fmt::Display for ChargingCurrent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == 0 {
            write!(f, "0A (Disabled / Charge permission revoked)")
        } else {
            write!(f, "{}A", self.0)
        }
    }
}

// -----------------------------------------------------------------------------
// Register 258 (0x0102): Active charging power or current (Read Holding, 1 word)
// -----------------------------------------------------------------------------

/// Modbus holding register 258: Active charging power or current
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PowerOrCurrent(pub u16);

impl ModbusParam for PowerOrCurrent {
    const ADDRESS: u16 = 258; // 0x0102
    const QUANTITY: u16 = 1;
    type ProtocolType = Self;
}

impl PowerOrCurrent {
    pub fn decode_from_words(words: &[Word]) -> Result<Self, Error> {
        if words.is_empty() {
            return Err(Error::WordsCountError);
        }
        Ok(Self(words[0]))
    }

    pub fn raw(&self) -> u16 {
        self.0
    }
}

impl std::fmt::Display for PowerOrCurrent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Value: {}", self.0)
    }
}

// =============================================================================
// ABL SURSUM EVCC Native Serial ASCII Protocol (as documented in docs/EVCC.pdf)
// =============================================================================

/// Command codes for the serial ASCII protocol documented in docs/EVCC.pdf
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EvccCommand {
    /// 00: Reset (Parameter: 1111)
    Reset,
    /// 01: Get firmware revision (Reply: Vx.y)
    GetFirmware,
    /// 02: Get state (0000=A, 0004=B2, 0005=C, 0006=D, 0009=B', 0013=B1, 0017=A', 0033=CS error, etc.)
    GetStatus,
    /// 03: Switch to MANUAL mode
    SwitchToManual,
    /// 04: Get output status (sum of active outputs)
    GetOutputStatus,
    /// 05: Set output (Parameter: xxxx)
    SetOutput,
    /// 06: Clear output (Parameter: xxxx)
    ClearOutput,
    /// 07: Get ADC value positive U_CP
    GetAdcPosUcp,
    /// 08: Get ADC value negative U_CP
    GetAdcNegUcp,
    /// 09: Get ADC value U_CS
    GetAdcUcs,
    /// 10: Get digital input status (E1, E2)
    GetInputs,
    /// 11: Get present PWM duty cycle (in 0.1% steps)
    GetPresentPwm,
    /// 12: Set PWM duty cycle / charging current
    SetPwm,
    /// 13: Enable PWM
    EnablePwm,
    /// 14: Disable PWM
    DisablePwm,
    /// 15: Set default PWM duty cycle
    SetDefaultPwm,
    /// 16: Set ventilation marker (1110 = connected, 1111 = no ventilation)
    SetVentilationMarker,
    /// 17: Get ventilation marker
    GetVentilationMarker,
    /// 22: Set module address (111x)
    SetModuleAddress,
    /// 23: Get module address (0..8)
    GetModuleAddress,
    /// 24: Load factory settings and reset (1111)
    LoadFactorySettingsAndReset,
    /// 25: Stop charging or leave MANUAL mode
    StopChargingOrLeaveManual,
    /// 26: Get default PWM duty cycle
    GetDefaultPwm,
    /// 27: Set bBreakCharge
    SetBreakCharge,
    /// 28: Clear bBreakCharge
    ClearBreakCharge,
    /// 29: Get bBreakCharge
    GetBreakCharge,
    /// 30: Switch to state A'
    SwitchToAPrime,
    /// 31: Leave state A' (Switch to state A)
    LeaveAPrime,
    /// Other command code
    Other(u8),
}

impl EvccCommand {
    pub fn code(&self) -> u8 {
        match self {
            Self::Reset => 0,
            Self::GetFirmware => 1,
            Self::GetStatus => 2,
            Self::SwitchToManual => 3,
            Self::GetOutputStatus => 4,
            Self::SetOutput => 5,
            Self::ClearOutput => 6,
            Self::GetAdcPosUcp => 7,
            Self::GetAdcNegUcp => 8,
            Self::GetAdcUcs => 9,
            Self::GetInputs => 10,
            Self::GetPresentPwm => 11,
            Self::SetPwm => 12,
            Self::EnablePwm => 13,
            Self::DisablePwm => 14,
            Self::SetDefaultPwm => 15,
            Self::SetVentilationMarker => 16,
            Self::GetVentilationMarker => 17,
            Self::SetModuleAddress => 22,
            Self::GetModuleAddress => 23,
            Self::LoadFactorySettingsAndReset => 24,
            Self::StopChargingOrLeaveManual => 25,
            Self::GetDefaultPwm => 26,
            Self::SetBreakCharge => 27,
            Self::ClearBreakCharge => 28,
            Self::GetBreakCharge => 29,
            Self::SwitchToAPrime => 30,
            Self::LeaveAPrime => 31,
            Self::Other(c) => *c,
        }
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Reset,
            1 => Self::GetFirmware,
            2 => Self::GetStatus,
            3 => Self::SwitchToManual,
            4 => Self::GetOutputStatus,
            5 => Self::SetOutput,
            6 => Self::ClearOutput,
            7 => Self::GetAdcPosUcp,
            8 => Self::GetAdcNegUcp,
            9 => Self::GetAdcUcs,
            10 => Self::GetInputs,
            11 => Self::GetPresentPwm,
            12 => Self::SetPwm,
            13 => Self::EnablePwm,
            14 => Self::DisablePwm,
            15 => Self::SetDefaultPwm,
            16 => Self::SetVentilationMarker,
            17 => Self::GetVentilationMarker,
            22 => Self::SetModuleAddress,
            23 => Self::GetModuleAddress,
            24 => Self::LoadFactorySettingsAndReset,
            25 => Self::StopChargingOrLeaveManual,
            26 => Self::GetDefaultPwm,
            27 => Self::SetBreakCharge,
            28 => Self::ClearBreakCharge,
            29 => Self::GetBreakCharge,
            30 => Self::SwitchToAPrime,
            31 => Self::LeaveAPrime,
            c => Self::Other(c),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Reset => "Reset",
            Self::GetFirmware => "Get firmware",
            Self::GetStatus => "Get status",
            Self::SwitchToManual => "Switch to MANUAL",
            Self::GetOutputStatus => "Status output",
            Self::SetOutput => "Set output",
            Self::ClearOutput => "Clear output",
            Self::GetAdcPosUcp => "Get ADC value pos. U_CP",
            Self::GetAdcNegUcp => "Get ADC value neg. U_CP",
            Self::GetAdcUcs => "Get ADC value U_CS",
            Self::GetInputs => "Get input",
            Self::GetPresentPwm => "Get present PWM",
            Self::SetPwm => "Set PWM",
            Self::EnablePwm => "PWM enabled",
            Self::DisablePwm => "PWM disabled",
            Self::SetDefaultPwm => "Set default PWM",
            Self::SetVentilationMarker => "Set marker ventilation",
            Self::GetVentilationMarker => "Get marker ventilation",
            Self::SetModuleAddress => "Set address of module",
            Self::GetModuleAddress => "Get address of module",
            Self::LoadFactorySettingsAndReset => "Load factory settings and reset",
            Self::StopChargingOrLeaveManual => "Stop charging or leave MANUAL",
            Self::GetDefaultPwm => "Get default PWM",
            Self::SetBreakCharge => "Set bBreakCharge",
            Self::ClearBreakCharge => "Clear bBreakCharge",
            Self::GetBreakCharge => "Get bBreakCharge",
            Self::SwitchToAPrime => "Switch to A'",
            Self::LeaveAPrime => "Leave A'",
            Self::Other(_) => "Custom command",
        }
    }
}

/// Encode an EVCC serial request line: `!n cc [parameter]\r\n`
pub fn encode_evcc_request(address: u8, cmd: EvccCommand, param: Option<&str>) -> String {
    match param {
        Some(p) if !p.trim().is_empty() => format!("!{} {:02} {}\r\n", address, cmd.code(), p.trim()),
        _ => format!("!{} {:02}\r\n", address, cmd.code()),
    }
}

/// Parse an EVCC serial reply line: `>n cc[ parameter]\r\n`
pub fn parse_evcc_reply(line: &str) -> Result<(u8, u8, Option<String>), Error> {
    let trimmed = line.trim();
    if !trimmed.starts_with('>') {
        return Err(Error::InvalidFrame);
    }
    let content = &trimmed[1..];
    let parts: Vec<&str> = content.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(Error::InvalidFrame);
    }

    let addr = parts[0].parse::<u8>().map_err(|_| Error::InvalidValue)?;
    let cmd = parts[1].parse::<u8>().map_err(|_| Error::InvalidValue)?;
    let param = if parts.len() > 2 {
        Some(parts[2..].join(" "))
    } else {
        None
    };

    Ok((addr, cmd, param))
}
