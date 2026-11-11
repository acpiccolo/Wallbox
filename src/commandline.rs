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
    if val == 0 || (1..=247).contains(&val) {
        Ok(val)
    } else {
        Err(format!("Address must be in range 0..=247, got {val}"))
    }
}

pub fn parse_current(s: &str) -> Result<u16, String> {
    let val = s.parse::<u16>().map_err(|e| format!("{e}"))?;
    if val == 0 || (6..=80).contains(&val) {
        Ok(val)
    } else {
        Err(format!(
            "Current must be 0 (stop) or between 6A and 80A, got {val}"
        ))
    }
}

fn default_device_name() -> String {
    if cfg!(target_os = "windows") {
        "COM1".to_string()
    } else {
        "/dev/ttyUSB0".to_string()
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ParityAndStopBit {
    #[value(
        name = "ep1b",
        help = "even parity, one stop bit (default for ABL eMH1 8E1)"
    )]
    #[default]
    EvenParityOneStopBit,
    #[value(name = "np1b", help = "no parity, one stop bit")]
    NoParityOneStopBit,
    #[value(name = "op1b", help = "odd parity, one stop bit")]
    OddParityOneStopBit,
    #[value(name = "np2b", help = "no parity, two stop bits")]
    NoParityTwoStopBits,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum Connection {
    /// Connect via local or virtual serial device (Modbus ASCII over RS485)
    Serial {
        /// Serial device path (e.g. /dev/ttyUSB0, COM1)
        #[arg(short, long, default_value_t = default_device_name())]
        device: String,

        /// Baud rate (default: 38400 for ABL eMH1)
        #[arg(long, default_value = "38400")]
        baud_rate: u32,

        /// Modbus slave address (1 to 247, or 0 for broadcast)
        #[arg(long, default_value = "1", value_parser = parse_address)]
        address: u8,

        /// Parity and stop bits (default: ep1b for 8E1)
        #[arg(long, default_value = "ep1b")]
        parity_and_stop_bit: ParityAndStopBit,

        #[command(subcommand)]
        command: Commands,
    },
    /// Connect via direct TCP to transparent serial server (e.g. Elfin EW11 at 192.168.3.128:8899)
    Tcp {
        /// TCP server address (e.g. 192.168.3.128:8899)
        address: String,

        /// Modbus slave address (1 to 247, or 0 for broadcast)
        #[arg(short, long, default_value = "1", value_parser = parse_address)]
        slave: u8,

        #[command(subcommand)]
        command: Commands,
    },
    /// Scan serial RS485 bus for connected ABL eMH1 wallboxes across baud rates and addresses
    SerialScan {
        /// Serial device path (e.g. /dev/ttyUSB0, COM1)
        #[arg(short, long, default_value_t = default_device_name())]
        device: String,

        /// Maximum Modbus slave address to scan (default: 31, max: 247)
        #[arg(long, default_value = "31")]
        max_address: u8,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
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
    /// Daemon mode to read all values continuously
    Daemon {
        /// Interval for repeated polling of values
        #[arg(value_parser = parse_duration, short, long, default_value = "2sec")]
        poll_iterval: Duration,

        #[command(subcommand)]
        mode: DaemonOutput,
    },
    /// Read all status, measurements, currents, and firmware info
    ReadAll,
    /// Read vehicle charging status (Register 0x0004 or 0x002E)
    ReadStatus,
    /// Read configured charging current limit / duty cycle (Register 0x0014)
    ReadCurrent,
    /// Set maximum charging current in Amperes per phase (0 to halt/disable, 6 to 80)
    SetCurrent {
        /// Current in Amperes per phase (0 to disable/stop, or 6 to 80)
        #[arg(value_parser = parse_current)]
        amperes: u16,

        /// Skip confirmation when setting 0A (stopping charging)
        #[arg(short, long)]
        force: bool,
    },
    /// Set maximum charging power in Watts total for 3 phases (converts to Amperes per phase at 3x230V)
    SetPower {
        /// Total 3-phase charging power in Watts (e.g. 11000 for 11kW = 16A, 0 to stop)
        watts: u32,

        /// Skip confirmation when setting 0W (stopping charging)
        #[arg(short, long)]
        force: bool,
    },
    /// Read 3-phase currents and active power (Register 0x002E)
    ReadCurrents,
    /// Read firmware revision and hardware configuration (Register 0x0001)
    ReadFirmware,
    /// Read wallbox system flags and diagnostic indicators (Register 0x0006)
    ReadSystemFlags,
    /// Read wallbox Modbus communication settings (Register 0x0003)
    ReadModbusSettings,
    /// Enable charging permission (sets charging current)
    Enable {
        /// Charging current in Amperes per phase (default: 16A)
        #[arg(default_value = "16", value_parser = parse_current)]
        amperes: u16,

        /// Optional total 3-phase charging power in Watts (overrides amperes if specified)
        #[arg(long)]
        power: Option<u32>,
    },
    /// Disable / revoke charging permission (halts charging)
    Disable {
        /// Skip confirmation when disabling charging
        #[arg(short, long)]
        force: bool,
    },
    /// Wake up sleeping vehicle via state E0 -> A1 Control Pilot sequence
    WakeUp,
    /// Reset the wallbox charge controller (Register 0x0005, code 0x5A5A)
    Reset {
        /// Skip confirmation when resetting the charge controller
        #[arg(short, long)]
        force: bool,
    },
    /// Scan the bus for connected ABL eMH1 wallbox (checks default 1, broadcast 0, then range)
    Scan {
        /// Maximum Modbus slave address to scan (default: 31, max: 247)
        #[arg(long, default_value = "31")]
        max_address: u8,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
}

const fn about_text() -> &'static str {
    "ABL eMH1 / eMH2 (EVCC2/3) wallbox Modbus ASCII tool"
}

#[derive(Parser, Debug)]
#[command(version, about = about_text(), long_about = None)]
pub struct Args {
    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,

    /// Output to stdout not in JSON format
    #[arg(long, default_value = "false")]
    pub no_json: bool,

    /// Connection type (ascii or tcp)
    #[command(subcommand)]
    pub connection: Connection,

    /// Modbus operations timeout
    #[arg(value_parser = parse_duration, long, default_value = "3000ms")]
    pub timeout: Duration,

    /// Delay between retry operations
    #[arg(value_parser = parse_duration, long, default_value = "100ms")]
    pub delay: Duration,
}
