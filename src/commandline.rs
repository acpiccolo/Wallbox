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

pub fn parse_current(s: &str) -> Result<f32, String> {
    let val = s.parse::<f32>().map_err(|e| format!("{e}"))?;
    if (6.0..=80.0).contains(&val) {
        Ok(val)
    } else {
        Err(format!("Current must be between 6.0A and 80.0A, got {val}"))
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

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateAction {
    #[value(name = "reset", help = "Reset EVCC controller")]
    Reset,
    #[value(name = "a1", help = "Jump to state A1 (Waiting for EV)")]
    JumpToA1,
    #[value(name = "e0", help = "Jump to state E0 (Outlet disabled)")]
    JumpToE0,
    #[value(name = "e2", help = "Jump to state E2 (EVCC setup mode)")]
    JumpToE2,
    #[value(name = "f1", help = "Jump to state F1 (Trigger RCD trip test)")]
    JumpToF1,
    #[value(name = "lid20a", help = "Enable 20A Load Imbalance Detection")]
    EnableLoadImbalance20A,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeoutOption {
    NoChange,
    Disabled,
    Enabled,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerCycleOption {
    NoChange,
    EnterB2IfConnected,
    StopInB1IfConnected,
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
        config_file: String,
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

    /// Read all measurement values (currents, state, voltages)
    ReadAll,

    /// Read all configuration settings
    ReadAllSettings,

    /// Read EV current and status values (Register 0x002E)
    ReadCurrent {
        /// Resolution: 'full' (0.1A) or 'short' (1A)
        #[arg(long, default_value = "full")]
        resolution: String,
    },

    /// Read device ID, firmware revision, hardware and config (Register 0x0001)
    ReadDeviceInfo,

    /// Read Modbus baud rate, parity, and stop bit settings (Register 0x0003)
    ReadModbusSettings,

    /// Read system and diagnostic flags (Register 0x0006)
    ReadSystemFlags,

    /// Set maximum charging current Icmax in Amperes (Register 0x0014)
    SetCurrent {
        /// Current in Amperes (6.0 - 80.0)
        #[arg(value_parser = parse_current)]
        amperes: f32,
    },

    /// Modify state of outlet (Register 0x0005)
    SetState {
        /// Action to perform
        #[arg(value_enum)]
        action: StateAction,
    },

    /// Set Device ID and configure power-cycle / timeout behavior (Register 0x002C)
    SetDeviceId {
        /// New Device ID to assign (1..=16)
        #[arg(value_parser = parse_address)]
        new_device_id: u8,
        /// Upstream timeout behavior
        #[arg(long, value_enum, default_value = "no-change")]
        timeout: TimeoutOption,
        /// Behavior after power cycle when EV connected
        #[arg(long, value_enum, default_value = "no-change")]
        power_cycle: PowerCycleOption,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum FrameCommands {
    /// Encode a Modbus ASCII read frame
    EncodeRead {
        device_id: u8,
        start_register: String,
        qty: u16,
    },
    /// Encode a Modbus ASCII write-current frame
    EncodeWriteCurrent {
        device_id: u8,
        amperes: f32,
    },
    /// Decode a Modbus ASCII frame string
    Decode {
        frame: String,
    },
}

const fn about_text() -> &'static str {
    "ABL EVCC2/3 wallbox command line tool"
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
