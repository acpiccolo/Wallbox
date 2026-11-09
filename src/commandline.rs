use crate::mqtt::MqttConfig;
use clap::{Parser, Subcommand, ValueEnum};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use std::time::Duration;

pub fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| format!("{e}"))
}

pub fn parse_address(s: &str) -> Result<u8, String> {
    let val = if s.starts_with("0x") || s.starts_with("0X") {
        u8::from_str_radix(&s[2..], 16).map_err(|e| format!("{e}"))?
    } else {
        s.parse::<u8>().map_err(|e| format!("{e}"))?
    };
    if (1..=16).contains(&val) {
        Ok(val)
    } else {
        Err(format!("Address must be in range 1..=16, got {val}"))
    }
}

pub fn parse_current(s: &str) -> Result<u16, String> {
    let val = s.parse::<u16>().map_err(|e| format!("{e}"))?;
    if val == 0 || (6..=80).contains(&val) {
        Ok(val)
    } else {
        Err(format!("Current must be 0 (stop) or between 6A and 80A, got {val}"))
    }
}

fn default_device_name() -> String {
    if cfg!(target_os = "windows") {
        "COM1".to_string()
    } else {
        "/dev/ttyUSB0".to_string()
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParityAndStopBit {
    #[value(name = "ep1b", help = "even parity, one stop bit (default)")]
    EvenParityOneStopBit,
    #[value(name = "np1b", help = "no parity, one stop bit")]
    NoParityOneStopBit,
    #[value(name = "op1b", help = "odd parity, one stop bit")]
    OddParityOneStopBit,
    #[value(name = "np2b", help = "no parity, two stop bits")]
    NoParityTwoStopBits,
}

impl Default for ParityAndStopBit {
    fn default() -> Self {
        Self::EvenParityOneStopBit
    }
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum Connection {
    /// Use Modbus/TCP connection
    Tcp {
        /// TCP address (e.g. 192.168.0.222:502)
        address: String,

        /// Modbus slave address from 1 to 16
        #[arg(short, long, default_value = "1", value_parser = parse_address)]
        slave: u8,

        #[command(subcommand)]
        command: Commands,
    },
    /// Use Modbus/RTU connection
    Rtu {
        /// Device
        #[arg(short, long, default_value_t = default_device_name())]
        device: String,

        /// Baud rate (default: 38400 for ABL EVCC)
        #[arg(long, default_value = "38400")]
        baud_rate: u32,

        /// RS485 address from 1 to 16
        #[arg(long, default_value = "1", value_parser = parse_address)]
        address: u8,

        /// Parity and stop bits
        #[arg(long, default_value = "ep1b")]
        parity_and_stop_bit: ParityAndStopBit,

        #[command(subcommand)]
        command: Commands,
    },
    /// Offline ASCII frame utilities
    Frame {
        #[command(subcommand)]
        command: FrameCommands,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum DaemonOutput {
    /// Continuously read and print values to the standard output (console).
    Console,
    /// Continuously read and publish values to an MQTT Broker
    Mqtt {
        /// The configuration file for the MQTT broker
        #[arg(long, default_value = MqttConfig::DEFAULT_CONFIG_FILE)]
        mqtt_config: String,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum Commands {
    /// Daemon mode to read all values
    Daemon {
        /// Interval for repeated polling of the values
        #[arg(value_parser = parse_duration, short, long, default_value = "2sec")]
        poll_iterval: Duration,

        #[command(subcommand)]
        mode: DaemonOutput,
    },

    /// Read all measurement and status values (Registers 256, 257, 258)
    ReadAll,

    /// Read vehicle charging status (Register 256 / 0x0100)
    ReadStatus,

    /// Read maximum charging current setting / status (Register 257 / 0x0101)
    ReadCurrent,

    /// Read active charging power / current (Register 258 / 0x0102)
    ReadPower,

    /// Set maximum charging current in Amperes (Register 257 / 0x0101)
    SetCurrent {
        /// Current in Amperes (0 to disable/stop, or 6 to 80)
        #[arg(value_parser = parse_current)]
        amperes: u16,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum FrameCommands {
    /// Encode a Modbus ASCII read frame
    EncodeRead {
        device_id: u8,
        #[arg(default_value = "256")]
        start_register: String,
        #[arg(default_value = "1")]
        qty: u16,
    },
    /// Encode a Modbus ASCII write-current frame (Register 257)
    EncodeWriteCurrent {
        device_id: u8,
        #[arg(value_parser = parse_current)]
        amperes: u16,
    },
    /// Decode a Modbus ASCII frame string
    Decode {
        frame: String,
    },
}

const fn about_text() -> &'static str {
    "ABL EVCC1 wallbox command line tool"
}

#[derive(Parser, Debug)]
#[command(version, about = about_text(), long_about = None)]
pub struct Args {
    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,

    /// Output to stdout not in JSON format
    #[arg(long, default_value = "false")]
    pub no_json: bool,

    // Connection type
    #[command(subcommand)]
    pub connection: Connection,

    /// Modbus Input/Output operations timeout
    #[arg(value_parser = parse_duration, long, default_value = "1000ms")]
    pub timeout: Duration,

    /// Delay between multiple modbus commands
    #[arg(value_parser = parse_duration, long, default_value = "50ms")]
    pub delay: Duration,
}
