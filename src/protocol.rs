//! This module defines the core data structures, protocol framing, and traits for the EVCC2/3 Modbus ASCII protocol.
//!
//! It includes definitions for all supported Modbus registers, data types for EVCC settings, status, and measurements,
//! and helper traits/functions for encoding and decoding Modbus ASCII frames and registers.
//!
//! The documentation and structures in this module are based on the "EVCC2/3 API subset for external applications" specification.

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

    /// The provided duty cycle percentage value is out of permissible range.
    #[error("Duty cycle {0} is out of permissible range")]
    DutyCycleOutOfRange(u16),

    /// An unexpected or invalid value was received in a register field.
    #[error("Invalid register value encountered")]
    InvalidValue,

    /// The number of words provided is incorrect for the target register or structure.
    #[error("Words count error for register decoding")]
    WordsCountError,

    /// The provided baud rate value is unsupported.
    #[error("Unsupported baud rate value")]
    InvalidBaudRate,

    /// The provided parity value is unsupported.
    #[error("Unsupported parity value")]
    InvalidParity,

    /// The provided stop bit value is unsupported.
    #[error("Unsupported stop bit value")]
    InvalidStopBits,
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
// Register 0x0001: Read device-ID, firmware-revision and config (R2, 2 words)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PcbaHardware {
    Pcba141215,
    Pcba160307,
    Pcba170725,
    Reserved(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ContactState {
    NoFunction,
    MustBeClosedForTransition,
    ClosedOutletEnabled,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceIdAndFirmwareRevision {
    pub referred_register: u8,
    pub hardware: PcbaHardware,
    pub device_id: u8,
    pub major_revision: u8,
    pub minor_revision: u8,
    pub cs_reading_enabled: bool,
    pub upstream_timeout_enabled: bool,
    pub internal_phase_current_meter_enabled: bool,
    pub internal_rdc_md_enabled: bool,
    pub socket_enabled: bool,
    pub welding_detection_enabled: bool,
    pub contact_state: ContactState,
}

impl ModbusParam for DeviceIdAndFirmwareRevision {
    const ADDRESS: u16 = 0x0001;
    const QUANTITY: u16 = 2;
    type ProtocolType = Self;
}

impl DeviceIdAndFirmwareRevision {
    pub fn decode_from_words(words: &[Word]) -> Result<Self, Error> {
        if words.len() < 2 {
            return Err(Error::WordsCountError);
        }
        let raw = ((words[0] as u32) << 16) | (words[1] as u32);

        let referred_register = ((raw >> 24) & 0xFF) as u8;
        let hw_code = ((raw >> 22) & 0x03) as u8;
        let hardware = match hw_code {
            0b00 => PcbaHardware::Pcba141215,
            0b01 => PcbaHardware::Pcba160307,
            0b10 => PcbaHardware::Pcba170725,
            other => PcbaHardware::Reserved(other),
        };
        let device_id = ((raw >> 16) & 0x3F) as u8;
        let major_revision = ((raw >> 12) & 0x0F) as u8;
        let minor_revision = ((raw >> 8) & 0x0F) as u8;

        let cs_reading_enabled = (raw & (1 << 7)) != 0;
        let upstream_timeout_enabled = (raw & (1 << 6)) != 0;
        let internal_phase_current_meter_enabled = (raw & (1 << 5)) != 0;
        let internal_rdc_md_enabled = (raw & (1 << 4)) != 0;
        let socket_enabled = (raw & (1 << 3)) != 0;
        let welding_detection_enabled = (raw & (1 << 2)) != 0;

        let contact_code = (raw & 0x03) as u8;
        let contact_state = match contact_code {
            0b00 => ContactState::NoFunction,
            0b01 => ContactState::MustBeClosedForTransition,
            0b10 => ContactState::ClosedOutletEnabled,
            _ => ContactState::Invalid,
        };

        Ok(Self {
            referred_register,
            hardware,
            device_id,
            major_revision,
            minor_revision,
            cs_reading_enabled,
            upstream_timeout_enabled,
            internal_phase_current_meter_enabled,
            internal_rdc_md_enabled,
            socket_enabled,
            welding_detection_enabled,
            contact_state,
        })
    }
}

impl std::fmt::Display for DeviceIdAndFirmwareRevision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DevID: {}, FW: V{}.{}, HW: {:?}, SocketEn: {}",
            self.device_id, self.major_revision, self.minor_revision, self.hardware, self.socket_enabled
        )
    }
}

// -----------------------------------------------------------------------------
// Register 0x0003: Read MODBUS settings (R1, 1 word)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StopBits {
    OneStopBit,
    TwoStopBits,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Parity {
    NoParity,
    Odd,
    Even,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ModbusBaudRate {
    B9600,
    B19200,
    B38400,
    B57600,
    Reserved(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModbusSettings {
    pub referred_register: u8,
    pub stop_bits: StopBits,
    pub parity: Parity,
    pub baud_rate: ModbusBaudRate,
}

impl ModbusParam for ModbusSettings {
    const ADDRESS: u16 = 0x0003;
    const QUANTITY: u16 = 1;
    type ProtocolType = Self;
}

impl ModbusSettings {
    pub fn decode_from_words(words: &[Word]) -> Result<Self, Error> {
        if words.is_empty() {
            return Err(Error::WordsCountError);
        }
        let raw = words[0];
        let referred_register = ((raw >> 8) & 0xFF) as u8;

        let sb_code = (raw >> 6) & 0x03;
        let stop_bits = match sb_code {
            0b00 => StopBits::OneStopBit,
            0b11 => StopBits::TwoStopBits,
            _ => StopBits::Invalid,
        };

        let parity_code = (raw >> 4) & 0x03;
        let parity = match parity_code {
            0b00 => Parity::NoParity,
            0b01 => Parity::Odd,
            0b10 => Parity::Even,
            _ => Parity::Invalid,
        };

        let baud_code = (raw & 0x0F) as u8;
        let baud_rate = match baud_code {
            0b0101 => ModbusBaudRate::B9600,
            0b0110 => ModbusBaudRate::B19200,
            0b0111 => ModbusBaudRate::B38400,
            0b1000 => ModbusBaudRate::B57600,
            other => ModbusBaudRate::Reserved(other),
        };

        Ok(Self {
            referred_register,
            stop_bits,
            parity,
            baud_rate,
        })
    }
}

impl std::fmt::Display for ModbusSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Reg: {:#04X}, StopBits: {:?}, Parity: {:?}, Baud: {:?}",
            self.referred_register, self.stop_bits, self.parity, self.baud_rate
        )
    }
}

// -----------------------------------------------------------------------------
// Register 0x0005: Modify state (W1, 1 word)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ModifyStateAction {
    LoadImbalanceDetection20AEnabled,
    Reset,
    JumpToA1,
    JumpToE0,
    JumpToE2,
    JumpToF1,
    Custom(u16),
}

impl ModbusParam for ModifyStateAction {
    const ADDRESS: u16 = 0x0005;
    const QUANTITY: u16 = 1;
    type ProtocolType = Self;
}

impl ModifyStateAction {
    pub fn encode_to_word(&self) -> Word {
        match self {
            Self::LoadImbalanceDetection20AEnabled => 0x3838,
            Self::Reset => 0x5A5A,
            Self::JumpToA1 => 0xA1A1,
            Self::JumpToE0 => 0xE0E0,
            Self::JumpToE2 => 0xE2E2,
            Self::JumpToF1 => 0xF1F1,
            Self::Custom(val) => *val,
        }
    }

    pub fn decode_from_word(word: Word) -> Self {
        match word {
            0x3838 => Self::LoadImbalanceDetection20AEnabled,
            0x5A5A => Self::Reset,
            0xA1A1 => Self::JumpToA1,
            0xE0E0 => Self::JumpToE0,
            0xE2E2 => Self::JumpToE2,
            0xF1F1 => Self::JumpToF1,
            val => Self::Custom(val),
        }
    }
}

// -----------------------------------------------------------------------------
// Register 0x0006: Read system-flags (R2, 2 words)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CpLimits {
    Default,
    China,
    Reserved(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EvCurrentRatio {
    Le100Percent,
    Between100And105Percent,
    Between105And110Percent,
    Between110And120Percent,
    Gt120Percent,
    Reserved(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemFlags {
    pub referred_register: u8,
    pub state_machine_pointer: u8,
    pub upstream_timeout_triggers_f4: bool,
    pub transition_b1_b2_disabled_after_power_cycle: bool,
    pub cp_limits: CpLimits,
    pub upstream_comm_lost: bool,
    pub load_imbalance_detected: bool,
    pub onboard_temp_60_to_80: bool,
    pub internal_phase_meter_failed: bool,
    pub transition_e0_a1_disabled: bool,
    pub transition_b1_b2_disabled: bool,
    pub transition_a1_b1_disabled: bool,
    pub ev_current_ratio: EvCurrentRatio,
}

impl ModbusParam for SystemFlags {
    const ADDRESS: u16 = 0x0006;
    const QUANTITY: u16 = 2;
    type ProtocolType = Self;
}

impl SystemFlags {
    pub fn decode_from_words(words: &[Word]) -> Result<Self, Error> {
        if words.len() < 2 {
            return Err(Error::WordsCountError);
        }
        let raw = ((words[0] as u32) << 16) | (words[1] as u32);

        let referred_register = ((raw >> 24) & 0xFF) as u8;
        let state_machine_pointer = ((raw >> 16) & 0xFF) as u8;

        let upstream_timeout_triggers_f4 = (raw & (1 << 15)) != 0;
        let transition_b1_b2_disabled_after_power_cycle = (raw & (1 << 14)) != 0;

        let cp_code = ((raw >> 12) & 0x03) as u8;
        let cp_limits = match cp_code {
            0b00 => CpLimits::Default,
            0b10 => CpLimits::China,
            other => CpLimits::Reserved(other),
        };

        let upstream_comm_lost = (raw & (1 << 9)) != 0;
        let load_imbalance_detected = (raw & (1 << 8)) != 0;
        let onboard_temp_60_to_80 = (raw & (1 << 7)) != 0;
        let internal_phase_meter_failed = (raw & (1 << 6)) != 0;

        let transition_e0_a1_disabled = (raw & (1 << 5)) != 0;
        let transition_b1_b2_disabled = (raw & (1 << 4)) != 0;
        let transition_a1_b1_disabled = (raw & (1 << 3)) != 0;

        let ev_ratio_code = (raw & 0x07) as u8;
        let ev_current_ratio = match ev_ratio_code {
            0b000 => EvCurrentRatio::Le100Percent,
            0b001 => EvCurrentRatio::Between100And105Percent,
            0b101 => EvCurrentRatio::Between105And110Percent,
            0b011 => EvCurrentRatio::Between110And120Percent,
            0b111 => EvCurrentRatio::Gt120Percent,
            other => EvCurrentRatio::Reserved(other),
        };

        Ok(Self {
            referred_register,
            state_machine_pointer,
            upstream_timeout_triggers_f4,
            transition_b1_b2_disabled_after_power_cycle,
            cp_limits,
            upstream_comm_lost,
            load_imbalance_detected,
            onboard_temp_60_to_80,
            internal_phase_meter_failed,
            transition_e0_a1_disabled,
            transition_b1_b2_disabled,
            transition_a1_b1_disabled,
            ev_current_ratio,
        })
    }
}

// -----------------------------------------------------------------------------
// Register 0x0014: Set Icmax (W1, 1 word)
// -----------------------------------------------------------------------------

/// Duty cycle Icmax [%] * 10 (range 80..1000 representing 8.0%..100.0%).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetIcmax(pub u16);

impl ModbusParam for SetIcmax {
    const ADDRESS: u16 = 0x0014;
    const QUANTITY: u16 = 1;
    type ProtocolType = Self;
}

impl SetIcmax {
    pub fn new(duty_cycle_x10: u16) -> Result<Self, Error> {
        if (80..=1000).contains(&duty_cycle_x10) {
            Ok(Self(duty_cycle_x10))
        } else {
            Err(Error::DutyCycleOutOfRange(duty_cycle_x10))
        }
    }

    pub fn encode_to_word(&self) -> Word {
        self.0
    }

    pub fn decode_from_word(word: Word) -> Result<Self, Error> {
        Self::new(word)
    }
}

// -----------------------------------------------------------------------------
// Register 0x002C: Set device-ID, Upstream-Timeout and transition B1->B2 (W1, 1 word)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UpstreamTimeoutOption {
    NoChange = 0b00,
    Disabled = 0b01,
    Enabled = 0b10,
    NoChangeAlt = 0b11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PowerCycleB1B2Option {
    NoChange = 0b00,
    EnterB2IfConnected = 0b01,
    StopInB1IfConnected = 0b10,
    NoChangeAlt = 0b11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetDeviceId {
    pub upstream_timeout: UpstreamTimeoutOption,
    pub power_cycle_b1_b2: PowerCycleB1B2Option,
    pub device_id: u8,
}

impl ModbusParam for SetDeviceId {
    const ADDRESS: u16 = 0x002C;
    const QUANTITY: u16 = 1;
    type ProtocolType = Self;
}

impl SetDeviceId {
    pub fn new(device_id: u8, upstream_timeout: UpstreamTimeoutOption, power_cycle_b1_b2: PowerCycleB1B2Option) -> Result<Self, Error> {
        if (1..=16).contains(&device_id) {
            Ok(Self {
                upstream_timeout,
                power_cycle_b1_b2,
                device_id,
            })
        } else {
            Err(Error::DeviceIdOutOfRange(device_id))
        }
    }

    pub fn encode_to_word(&self) -> Word {
        let mut val = (self.device_id & 0xFF) as u16;
        val |= ((self.power_cycle_b1_b2 as u16) & 0x03) << 12;
        val |= ((self.upstream_timeout as u16) & 0x03) << 14;
        val
    }

    pub fn decode_from_word(word: Word) -> Result<Self, Error> {
        let device_id = (word & 0xFF) as u8;
        let ut_code = ((word >> 14) & 0x03) as u8;
        let pc_code = ((word >> 12) & 0x03) as u8;

        let upstream_timeout = match ut_code {
            0b00 => UpstreamTimeoutOption::NoChange,
            0b01 => UpstreamTimeoutOption::Disabled,
            0b10 => UpstreamTimeoutOption::Enabled,
            _ => UpstreamTimeoutOption::NoChangeAlt,
        };

        let power_cycle_b1_b2 = match pc_code {
            0b00 => PowerCycleB1B2Option::NoChange,
            0b01 => PowerCycleB1B2Option::EnterB2IfConnected,
            0b10 => PowerCycleB1B2Option::StopInB1IfConnected,
            _ => PowerCycleB1B2Option::NoChangeAlt,
        };

        Self::new(device_id, upstream_timeout, power_cycle_b1_b2)
    }
}

// -----------------------------------------------------------------------------
// Register 0x002E: Read current (full) (R5, 5 words = 80 bits)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReadCurrentFull {
    pub referred_register: u8,
    pub evse_state: u8,
    pub ucp_le_10v: bool,
    pub duty_cycle_reduced_bc: bool,
    pub en2_closed: bool,
    pub en1_closed: bool,
    pub duty_cycle_ic_x10: u16,
    pub ict1_x10_decia: u16,
    pub ict2_x10_decia: u16,
    pub ict3_x10_decia: u16,
}

impl ModbusParam for ReadCurrentFull {
    const ADDRESS: u16 = 0x002E;
    const QUANTITY: u16 = 5;
    type ProtocolType = Self;
}

impl ReadCurrentFull {
    pub fn decode_from_words(words: &[Word]) -> Result<Self, Error> {
        if words.len() < 5 {
            return Err(Error::WordsCountError);
        }

        let referred_register = ((words[0] >> 8) & 0xFF) as u8;
        let evse_state = (words[0] & 0xFF) as u8;

        let flags_duty = words[1];
        let ucp_le_10v = (flags_duty & (1 << 15)) != 0;
        let duty_cycle_reduced_bc = (flags_duty & (1 << 14)) != 0;
        let en2_closed = (flags_duty & (1 << 13)) != 0;
        let en1_closed = (flags_duty & (1 << 12)) != 0;
        let duty_cycle_ic_x10 = flags_duty & 0x0FFF;

        let ict1_x10_decia = words[2];
        let ict2_x10_decia = words[3];
        let ict3_x10_decia = words[4];

        Ok(Self {
            referred_register,
            evse_state,
            ucp_le_10v,
            duty_cycle_reduced_bc,
            en2_closed,
            en1_closed,
            duty_cycle_ic_x10,
            ict1_x10_decia,
            ict2_x10_decia,
            ict3_x10_decia,
        })
    }
}

impl std::fmt::Display for ReadCurrentFull {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "State: {:#04X}, Ic: {:.1}%, L1: {:.1}A, L2: {:.1}A, L3: {:.1}A",
            self.evse_state,
            (self.duty_cycle_ic_x10 as f32) / 10.0,
            (self.ict1_x10_decia as f32) / 10.0,
            (self.ict2_x10_decia as f32) / 10.0,
            (self.ict3_x10_decia as f32) / 10.0
        )
    }
}

// -----------------------------------------------------------------------------
// Register 0x0033: Read current amps (R3, 3 words = 48 bits)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReadCurrentAmps {
    pub referred_register: u8,
    pub ucp_le_10v: bool,
    pub evse_state: u8,
    pub ict1_amps: u8,
    pub ict2_amps: u8,
    pub ict3_amps: u8,
}

impl ModbusParam for ReadCurrentAmps {
    const ADDRESS: u16 = 0x0033;
    const QUANTITY: u16 = 3;
    type ProtocolType = Self;
}

impl ReadCurrentAmps {
    pub fn decode_from_words(words: &[Word]) -> Result<Self, Error> {
        if words.len() < 3 {
            return Err(Error::WordsCountError);
        }

        let referred_register = ((words[0] >> 8) & 0xFF) as u8;
        let ucp_le_10v = (words[0] & (1 << 7)) != 0;

        let evse_state = (words[1] >> 8) as u8;
        let ict1_amps = (words[1] & 0xFF) as u8;

        let ict2_amps = (words[2] >> 8) as u8;
        let ict3_amps = (words[2] & 0xFF) as u8;

        Ok(Self {
            referred_register,
            ucp_le_10v,
            evse_state,
            ict1_amps,
            ict2_amps,
            ict3_amps,
        })
    }
}

impl std::fmt::Display for ReadCurrentAmps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "State: {:#04X}, L1: {}A, L2: {}A, L3: {}A",
            self.evse_state, self.ict1_amps, self.ict2_amps, self.ict3_amps
        )
    }
}
