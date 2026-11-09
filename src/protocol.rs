//! This module defines the core data structures, protocol framing, and traits for the EVCC1 Modbus protocol.
//!
//! Registers (decimal):
//! - 256 (0x0100): Ladestatus (Fahrzeug-Status) - ASCII values, e.g. 65 ('A' = Standby/Ready), 66 ('B' = Connected), 67 ('C' = Charging)
//! - 257 (0x0101): Maximaler Ladestrom (Vorgabe/Status) - Read/Write holding register in Amperes (e.g. 16 = 16A, 0 = Disabled)
//! - 258 (0x0102): Aktuelle Ladeleistung / Stromstärke (Read Holding)

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
    /// ASCII 65 ('A'): Standby / Ready (No vehicle connected)
    ReadyA,
    /// ASCII 66 ('B'): Vehicle connected, waiting for charge enable
    ConnectedB,
    /// ASCII 67 ('C'): Charging in progress
    ChargingC,
    /// ASCII 68 ('D'): Charging with ventilation
    VentilationD,
    /// ASCII 69 ('E'): No power / disconnected / disabled
    DisabledE,
    /// ASCII 70 ('F'): Fault / Error
    FaultF,
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
            65 => Self::ReadyA,
            66 => Self::ConnectedB,
            67 => Self::ChargingC,
            68 => Self::VentilationD,
            69 => Self::DisabledE,
            70 => Self::FaultF,
            other => Self::Other(other),
        }
    }

    pub fn as_char(&self) -> char {
        match self {
            Self::ReadyA => 'A',
            Self::ConnectedB => 'B',
            Self::ChargingC => 'C',
            Self::VentilationD => 'D',
            Self::DisabledE => 'E',
            Self::FaultF => 'F',
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
            Self::ReadyA => 65,
            Self::ConnectedB => 66,
            Self::ChargingC => 67,
            Self::VentilationD => 68,
            Self::DisabledE => 69,
            Self::FaultF => 70,
            Self::Other(val) => *val,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::ReadyA => "Standby / Bereit (kein Fahrzeug verbunden)",
            Self::ConnectedB => "Fahrzeug verbunden (wartet auf Ladefreigabe)",
            Self::ChargingC => "Ladevorgang aktiv (Fahrzeug lädt)",
            Self::VentilationD => "Ladevorgang mit externer Belüftung",
            Self::DisabledE => "Ladepunkt deaktiviert / Ausgeschaltet",
            Self::FaultF => "Fehlerzustand (Fault / Error)",
            Self::Other(_) => "Unbekannter Status",
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

/// Modbus holding register 256: Ladestatus
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
// Register 257 (0x0101): Maximaler Ladestrom (Read / Write Holding, 1 word)
// -----------------------------------------------------------------------------

/// Modbus holding register 257: Maximaler Ladestrom in Ampere (z.B. 16 für 16A, 0 für Stopp)
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
            write!(f, "0A (Deaktiviert / Ladefreigabe entzogen)")
        } else {
            write!(f, "{}A", self.0)
        }
    }
}

// -----------------------------------------------------------------------------
// Register 258 (0x0102): Aktuelle Ladeleistung / Stromstärke (Read Holding, 1 word)
// -----------------------------------------------------------------------------

/// Modbus holding register 258: Aktuelle Ladeleistung bzw. Stromstärke
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
        write!(f, "Wert: {}", self.0)
    }
}
