/// Represents errors that can occur in the ABL EVCC library.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// Error indicating an invalid LRC checksum in the received frame.
    #[error("LRC checksum error: expected {expected:#04X}, calculated {calculated:#04X}")]
    InvalidLrc { expected: u8, calculated: u8 },

    /// Error indicating an invalid or malformed Modbus ASCII frame format.
    #[error("Invalid Modbus ASCII frame format")]
    InvalidFrame,

    /// Error decoding hex characters from ASCII string.
    #[error("Failed to decode hex string")]
    HexDecodeError,

    /// An exception response was returned by the slave device.
    #[error("Modbus exception response: function code {function_code:#04X}, exception code {exception_code:#04X}")]
    ExceptionResponse {
        function_code: u8,
        exception_code: u8,
    },

    /// The provided Modbus address is outside permissible range (1..=247, or 0 for broadcast).
    #[error("The address value {0} is outside the permissible range (0, 1..=247)")]
    AddressOutOfRange(u8),

    /// The provided baud rate is not supported.
    #[error("Baud rate must be any value of 9600, 19200, 38400, 57600.")]
    InvalidBaudRate,

    /// The provided Device ID is outside permissible range (1..=16).
    #[error("Device ID {0} is out of permissible range (1..=16)")]
    DeviceIdOutOfRange(u8),

    /// The provided charging current is outside permissible range (0 or 6..=80A).
    #[error("Charging current {0}A is out of permissible range (0 or 6..=80A)")]
    CurrentOutOfRange(u16),

    /// An invalid register value was encountered during parsing.
    #[error("Invalid register value encountered")]
    InvalidValue,

    /// Incorrect number of bytes or words received for register decoding.
    #[error("Words count error for register decoding")]
    WordsCountError,

    /// Error indicating that a reply frame does not match the requested slave ID.
    #[error("Mismatched slave ID: expected {expected}, got {received}")]
    SlaveIdMismatch { expected: u8, received: u8 },

    /// Error indicating that a reply frame does not match the requested function code.
    #[error("Mismatched function code: expected {expected:#04X}, got {received:#04X}")]
    FunctionCodeMismatch { expected: u8, received: u8 },

    /// The response payload received was empty.
    #[error("Empty response payload received")]
    EmptyPayload,

    /// Incomplete payload received from slave device.
    #[error("Incomplete data payload: expected {expected} bytes, got {received}")]
    IncompletePayload { expected: usize, received: usize },
}
