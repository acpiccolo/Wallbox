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

/// Command codes for the native ABL SURSUM serial ASCII protocol documented in docs/EVCC.pdf.
///
/// In this protocol, commands are transmitted as ASCII strings over RS485 with the syntax:
/// `!n cc [xxxx]\r\n` where:
/// - `n` is the module address (1..8, or 0 for broadcast)
/// - `cc` is the 2-digit command code
/// - `xxxx` is an optional 4-character parameter
///
/// Replies are returned with the syntax:
/// `>n cc [xxxx]\r\n`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EvccCommand {
    /// **Command 00: Module Reset**
    ///
    /// Request: `!n 00 1111`
    /// Reply: none (controller restarts)
    /// Precondition: Available only in state A or in MANUAL mode.
    Reset,

    /// **Command 01: Query Firmware Revision**
    ///
    /// Request: `!n 01`
    /// Reply: `>n 01 Vx.y` (e.g. `>1 01 V2.3`)
    /// Precondition: Available in all states (A and MANUAL).
    GetFirmware,

    /// **Command 02: Query Charge Controller State / Status**
    ///
    /// Request: `!n 02`
    /// Reply: `>n 02 xxxx` where `xxxx` represents the current state machine position:
    /// - `0000`: State A (Standby / No EV connected)
    /// - `0004`: State B2 (EV connected, charge enabled, waiting for EV)
    /// - `0005`: State C (Charging active)
    /// - `0006`: State D (Charging active with external ventilation)
    /// - `0009`: State B' (Charging stopped by vehicle)
    /// - `0013`: State B1 (EV connected, charging halted / not yet released)
    /// - `0017`: State A' (CP 0V / vehicle wake-up state)
    /// - `0033`: CS error (Cable resistance reading out of range)
    /// - `0035`: EV diode error (Vehicle diode defective)
    /// - `0037`: Lock error (Type 2 socket locking actuator failed)
    /// - `0039`: Ventilation error
    /// - `0255`: MANUAL state (automatic state machine disabled)
    GetStatus,

    /// **Command 03: Switch to MANUAL Mode**
    ///
    /// Disables the automatic IEC 61851-1 state machine to allow direct hardware diagnosis.
    /// Request: `!n 03`
    /// Reply: `>n 03`
    /// Precondition: Available only when in State A.
    SwitchToManual,

    /// **Command 04: Query Relay / Output Status**
    ///
    /// Returns the bitmask sum of currently energized outputs:
    /// - `0001`: Output X1 on (LED)
    /// - `0002`: Output X2 on (LED)
    /// - `0004`: Relay K1 on (Main contactor / power supply to vehicle)
    /// - `0008`: Relay K2 on (Ventilation relay)
    /// - `0016`: Relay K3 on (Socket locking actuator)
    /// - `0032`: CP +12V active
    /// - `0064`: CP -12V active
    /// Request: `!n 04`
    /// Reply: `>n 04 xxxx`
    GetOutputStatus,

    /// **Command 05: Set Output On**
    ///
    /// Manually energizes selected outputs specified by bitmask parameter `xxxx` (see Cmd 04).
    /// Request: `!n 05 xxxx`
    /// Reply: `>n 05`
    /// Precondition: Available only in MANUAL mode.
    SetOutput,

    /// **Command 06: Clear Output Off**
    ///
    /// Manually de-energizes selected outputs specified by bitmask parameter `xxxx` (see Cmd 04).
    /// Request: `!n 06 xxxx`
    /// Reply: `>n 06`
    /// Precondition: Available only in MANUAL mode.
    ClearOutput,

    /// **Command 07: Query Positive Control Pilot (CP) Voltage**
    ///
    /// Reads the ADC voltage reading of positive CP signal:
    /// Calculation: $U_{CP} = 0.017 \times \text{xxxx}\text{ [V]}$
    /// Request: `!n 07`
    /// Reply: `>n 07 xxxx`
    GetAdcPosUcp,

    /// **Command 08: Query Negative Control Pilot (CP) Voltage**
    ///
    /// Reads the ADC voltage reading of negative CP signal:
    /// Calculation: $U_{CP} = 0.017 \times \text{xxxx}\text{ [V]}$
    /// Request: `!n 08`
    /// Reply: `>n 08 xxxx`
    GetAdcNegUcp,

    /// **Command 09: Query Cable Sensing (CS) ADC Value**
    ///
    /// Reads the Cable Detection (Proximity / PP) ADC raw value (range 0..1023).
    /// Request: `!n 09`
    /// Reply: `>n 09 xxxx`
    GetAdcUcs,

    /// **Command 10: Query Digital Inputs**
    ///
    /// Reads the state of hardware inputs E1 (Enable switch) and E2 (Lock feedback switch):
    /// - `0000`: E1 and E2 open
    /// - `0001`: E1 closed, E2 open
    /// - `0002`: E2 closed, E1 open
    /// - `0003`: Both E1 and E2 closed
    /// Request: `!n 10`
    /// Reply: `>n 10 xxxx`
    GetInputs,

    /// **Command 11: Query Present PWM Duty Cycle / Charging Current**
    ///
    /// Returns the currently signalled PWM duty cycle in 0.1% steps:
    /// Calculation: $I = \text{PWM} \times 0.6\,\text{A}$ (or $(\text{PWM} - 64) \times 2.5\,\text{A}$ for $>51\,\text{A}$).
    /// Request: `!n 11`
    /// Reply: `>n 11 xxxx`
    GetPresentPwm,

    /// **Command 12: Set PWM Duty Cycle / Charging Current**
    ///
    /// Sets the maximum current signalled to the connected electric vehicle:
    /// - Preset steps: `0000`=6A, `0001`=10A, `0002`=13A, `0003`=16A, `0004`=20A, `0005`=30A, `0006`=32A, `0007`=63A, `0008`=70A, `0009`=80A
    /// - `0010`: Revert to factory/module default charging current
    /// - `0080`..`0970`: Direct PWM duty cycle in 0.1% steps (8.0% .. 97.0%)
    /// - `0999`: Charging prohibited (halt charging, disconnect power)
    /// Request: `!n 12 xxxx`
    /// Reply: `>n 12`
    SetPwm,

    /// **Command 13: Enable PWM Generator**
    ///
    /// Manually turns on the PWM oscillator on the Control Pilot line.
    /// Request: `!n 13`
    /// Reply: `>n 13`
    /// Precondition: Available only in MANUAL mode.
    EnablePwm,

    /// **Command 14: Disable PWM Generator**
    ///
    /// Manually turns off the PWM oscillator on the Control Pilot line.
    /// Request: `!n 14`
    /// Reply: `>n 14`
    /// Precondition: Available only in MANUAL mode.
    DisablePwm,

    /// **Command 15: Set Default Charging Current (Default PWM)**
    ///
    /// Persists the module's non-volatile default charging current limit:
    /// Parameters `0000`..`0009` corresponding to 6A..80A.
    /// Request: `!n 15 xxxx`
    /// Reply: `>n 15`
    /// Precondition: Available only in MANUAL mode.
    SetDefaultPwm,

    /// **Command 16: Configure External Ventilation Marker**
    ///
    /// Informs the controller whether external room ventilation is connected:
    /// - `1110`: Ventilation connected
    /// - `1111`: No ventilation connected
    /// Request: `!n 16 111x`
    /// Reply: `>n 16`
    /// Precondition: Available only in MANUAL mode.
    SetVentilationMarker,

    /// **Command 17: Query Ventilation Marker**
    ///
    /// Reads ventilation configuration: `0000` = connected, `0001` = not connected.
    /// Request: `!n 17`
    /// Reply: `>n 17 000x`
    GetVentilationMarker,

    /// **Command 22: Configure Module RS485 Address**
    ///
    /// Reassigns the controller's bus address to address `1` through `8`:
    /// Parameter: `1111`=Address 1, `1112`=Address 2, ... `1118`=Address 8.
    /// Request: `!n 22 111x`
    /// Reply: `>n 22`
    /// Precondition: Available only in MANUAL mode.
    SetModuleAddress,

    /// **Command 23: Query Module RS485 Address**
    ///
    /// Returns the controller's configured bus address (`0001` .. `0008`, or `0000`).
    /// Request: `!n 23`
    /// Reply: `>n 23 000x`
    GetModuleAddress,

    /// **Command 24: Factory Reset**
    ///
    /// Resets all controller parameters to factory defaults and restarts.
    /// Request: `!n 24 1111`
    /// Reply: `>n 01 Vx.y` (firmware revision banner)
    /// Precondition: Available only in MANUAL mode.
    LoadFactorySettingsAndReset,

    /// **Command 25: Terminate Charging Session / Exit MANUAL Mode**
    ///
    /// Leaves MANUAL mode and returns the charge controller to normal automatic operation.
    /// Request: `!n 25`
    /// Reply: `>n 25`
    StopChargingOrLeaveManual,

    /// **Command 26: Query Default PWM Duty Cycle**
    ///
    /// Reads the stored default PWM duty cycle in 0.1% steps:
    /// Calculation: $I_{DEFAULT} = \text{PWM} \times 0.6\,\text{A}$
    /// Request: `!n 26`
    /// Reply: `>n 26 xxxx`
    GetDefaultPwm,

    /// **Command 27: Set Charging Halt (`bBreakCharge = 1`)**
    ///
    /// Sets the break charge flag. The controller halts the state machine in State B1
    /// (vehicle connected, but power contactor opened and charging paused).
    /// Request: `!n 27`
    /// Reply: `>n 27`
    SetBreakCharge,

    /// **Command 28: Clear Charging Halt (`bBreakCharge = 0`)**
    ///
    /// Clears the break charge flag and releases charging (transition B1 -> B2 -> C).
    /// Request: `!n 28`
    /// Reply: `>n 28`
    ClearBreakCharge,

    /// **Command 29: Query Charging Halt Status (`bBreakCharge`)**
    ///
    /// Reads whether charging is halted: `0000` = released/normal, `0001` = halted in B1.
    /// Request: `!n 29`
    /// Reply: `>n 29 xxxx`
    GetBreakCharge,

    /// **Command 30: Switch to State A' (Disconnect CP / Vehicle Wake-Up)**
    ///
    /// Forces Control Pilot voltage to 0V (State A'). Used to simulate cable unplugging
    /// to wake up sleeping electric vehicles.
    /// Request: `!n 30`
    /// Reply: `>n 30`
    SwitchToAPrime,

    /// **Command 31: Leave State A' (Restore CP)**
    ///
    /// Restores standard +12V Control Pilot signal (returns from State A' to State A).
    /// Request: `!n 31`
    /// Reply: `>n 31`
    LeaveAPrime,

    /// **Custom or Unknown Command Code**
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
