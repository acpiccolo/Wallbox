use abl_evcc_lib::{protocol::*, tokio_sync_safe_client::SafeClient};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::time::Duration;

mod mqtt;

fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| format!("Invalid duration: {}", e))
}

/// ABL EVCC2/3 Wallbox Modbus Protocol CLI Tool
#[derive(Parser, Debug)]
#[command(name = "abl-evcc", version, about = "ABL EVCC2/3 Wallbox Modbus Protocol CLI Tool", long_about = None)]
pub struct Cli {
    /// Timeout for Modbus operations
    #[arg(long, default_value = "1000ms", value_parser = parse_duration)]
    pub timeout: Duration,

    #[command(subcommand)]
    pub connection: Connection,
}

fn default_serial_device() -> String {
    if cfg!(target_os = "windows") {
        "COM1".to_string()
    } else {
        "/dev/ttyUSB0".to_string()
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialParity {
    None,
    Even,
    Odd,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialStopBits {
    One,
    Two,
}

#[derive(Subcommand, Debug)]
pub enum Connection {
    /// Connect via Serial Modbus RTU / RS485
    Rtu {
        /// Serial device port
        #[arg(short, long, default_value_t = default_serial_device())]
        device: String,

        /// Baud rate (default: 38400 for ABL EVCC)
        #[arg(short, long, default_value_t = 38400)]
        baud_rate: u32,

        /// Parity (default: even for ABL EVCC)
        #[arg(long, value_enum, default_value_t = SerialParity::Even)]
        parity: SerialParity,

        /// Stop bits (default: one for ABL EVCC)
        #[arg(long, value_enum, default_value_t = SerialStopBits::One)]
        stop_bits: SerialStopBits,

        /// Modbus slave address (1..=16)
        #[arg(short, long, default_value_t = 1)]
        address: u8,

        #[command(subcommand)]
        command: DeviceCommands,
    },

    /// Connect via Modbus TCP (e.g. RS485-to-Ethernet gateway)
    Tcp {
        /// TCP host:port address (e.g. 192.168.1.100:502)
        #[arg(short, long)]
        address: String,

        /// Modbus slave address (1..=16)
        #[arg(short, long, default_value_t = 1)]
        slave: u8,

        #[command(subcommand)]
        command: DeviceCommands,
    },

    /// Offline frame utilities (generate or decode ASCII frames without connecting)
    Frame {
        #[command(subcommand)]
        command: FrameCommands,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateActionArg {
    Reset,
    JumpToA1,
    JumpToE0,
    JumpToE2,
    JumpToF1,
    EnableLoadImbalance20A,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeoutArg {
    NoChange,
    Disabled,
    Enabled,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerCycleB1B2Arg {
    NoChange,
    EnterB2IfConnected,
    StopInB1IfConnected,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum DaemonOutput {
    /// Continuously read and print values to stdout (console)
    Console,
    /// Continuously read and publish values to an MQTT Broker
    Mqtt {
        /// The configuration file for the MQTT broker
        #[arg(long, default_value = mqtt::MqttConfig::DEFAULT_CONFIG_FILE)]
        config_file: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum DeviceCommands {
    /// Continuous polling daemon mode (Console or MQTT)
    Daemon {
        /// Polling interval (e.g. 2s, 500ms)
        #[arg(short, long, default_value = "2s", value_parser = parse_duration)]
        poll_interval: Duration,

        /// Output mode (Console or Mqtt)
        #[command(subcommand)]
        mode: DaemonOutput,

        /// Disable JSON formatted output
        #[arg(long, default_value_t = false)]
        no_json: bool,
    },

    /// Read current & status values of the EVCC outlet
    ReadCurrent {
        /// Format: 'full' (0.1A resolution) or 'short' (1A resolution)
        #[arg(long, default_value = "full")]
        resolution: String,
    },

    /// Read device ID, firmware revision, hardware and config
    ReadDeviceInfo,

    /// Read Modbus baud rate, parity, and stop bit settings
    ReadModbusSettings,

    /// Read system and diagnostic flags
    ReadSystemFlags,

    /// Set maximum charging current (Icmax)
    SetCurrent {
        /// Charging current in Amperes (6.0 - 80.0 A) or duty cycle if --duty-cycle is set
        #[arg(value_name = "VALUE")]
        value: f32,
        /// Treat VALUE as duty cycle percentage directly (8.0% - 100.0%) instead of Amperes
        #[arg(long, default_value_t = false)]
        duty_cycle: bool,
    },

    /// Modify outlet state
    SetState {
        /// Action to perform: reset, jump-to-a1, jump-to-e0, jump-to-e2, jump-to-f1, enable-load-imbalance20-a
        #[arg(value_enum)]
        action: StateActionArg,
    },

    /// Set Device ID and configure power-cycle / timeout behavior
    SetDeviceId {
        /// New Device ID to assign (1..16)
        #[arg(value_name = "NEW_DEVICE_ID")]
        new_device_id: u8,
        /// Upstream timeout behavior
        #[arg(long, value_enum, default_value_t = TimeoutArg::NoChange)]
        timeout: TimeoutArg,
        /// Behavior after power cycle when EV connected
        #[arg(long, value_enum, default_value_t = PowerCycleB1B2Arg::NoChange)]
        power_cycle: PowerCycleB1B2Arg,
    },
}

#[derive(Subcommand, Debug)]
pub enum FrameCommands {
    /// Generate an ASCII read-frame
    EncodeRead {
        device_id: u8,
        start_register: String,
        qty: u16,
    },
    /// Generate an ASCII write-icmax frame
    EncodeWriteCurrent {
        device_id: u8,
        current_amps: f32,
    },
    /// Decode and inspect a raw Modbus ASCII frame string
    Decode {
        frame: String,
    },
}

fn execute_device_command(client: &mut SafeClient, command: &DeviceCommands) -> Result<()> {
    match command {
        DeviceCommands::Daemon {
            poll_interval,
            mode,
            no_json,
        } => match mode {
            DaemonOutput::Console => {
                println!("Daemon started. Polling every {:?}", poll_interval);
                loop {
                    let cur = client
                        .read_current_full()
                        .context("Failed reading current")?;
                    if *no_json {
                        println!("{}", cur);
                    } else {
                        println!("{}", serde_json::to_string_pretty(&cur)?);
                    }
                    std::thread::sleep(*poll_interval);
                }
            }
            DaemonOutput::Mqtt { config_file } => {
                mqtt::run_mqtt_daemon(client, poll_interval, config_file, *no_json)?;
            }
        },
        DeviceCommands::ReadCurrent { resolution } => {
            if resolution.to_lowercase() == "short" {
                let cur = client.read_current_amps().context("Failed reading short current")?;
                println!("{}", cur);
            } else {
                let cur = client.read_current_full().context("Failed reading full current")?;
                println!("{}", cur);
            }
        }
        DeviceCommands::ReadDeviceInfo => {
            let info = client.read_device_info().context("Failed reading device info")?;
            println!("{}", info);
        }
        DeviceCommands::ReadModbusSettings => {
            let settings = client.read_modbus_settings().context("Failed reading modbus settings")?;
            println!("{}", settings);
        }
        DeviceCommands::ReadSystemFlags => {
            let flags = client.read_system_flags().context("Failed reading system flags")?;
            println!("{:#?}", flags);
        }
        DeviceCommands::SetCurrent { value, duty_cycle } => {
            let duty_cycle_x10 = if *duty_cycle {
                (value * 10.0).round() as u16
            } else {
                let dc_val = (value * 100.0 / 6.0).round() as u16;
                dc_val.clamp(80, 1000)
            };
            let icmax = SetIcmax::new(duty_cycle_x10).context("Invalid current / duty cycle")?;
            client.set_current(icmax).context("Failed setting current")?;
            println!("Current successfully set (duty cycle: {:.1}%)", duty_cycle_x10 as f32 / 10.0);
        }
        DeviceCommands::SetState { action } => {
            let proto_action = match action {
                StateActionArg::Reset => ModifyStateAction::Reset,
                StateActionArg::JumpToA1 => ModifyStateAction::JumpToA1,
                StateActionArg::JumpToE0 => ModifyStateAction::JumpToE0,
                StateActionArg::JumpToE2 => ModifyStateAction::JumpToE2,
                StateActionArg::JumpToF1 => ModifyStateAction::JumpToF1,
                StateActionArg::EnableLoadImbalance20A => {
                    ModifyStateAction::LoadImbalanceDetection20AEnabled
                }
            };
            client.set_state(proto_action).context("Failed setting state")?;
            println!("State modified successfully to: {:?}", action);
        }
        DeviceCommands::SetDeviceId {
            new_device_id,
            timeout,
            power_cycle,
        } => {
            let ut = match timeout {
                TimeoutArg::NoChange => UpstreamTimeoutOption::NoChange,
                TimeoutArg::Disabled => UpstreamTimeoutOption::Disabled,
                TimeoutArg::Enabled => UpstreamTimeoutOption::Enabled,
            };
            let pc = match power_cycle {
                PowerCycleB1B2Arg::NoChange => PowerCycleB1B2Option::NoChange,
                PowerCycleB1B2Arg::EnterB2IfConnected => PowerCycleB1B2Option::EnterB2IfConnected,
                PowerCycleB1B2Arg::StopInB1IfConnected => PowerCycleB1B2Option::StopInB1IfConnected,
            };
            let set_cmd = SetDeviceId::new(*new_device_id, ut, pc).context("Invalid Device ID parameter")?;
            client.set_device_id(set_cmd).context("Failed setting device ID")?;
            println!("Device ID successfully changed to: {}", new_device_id);
        }
    }
    Ok(())
}

fn execute_frame_command(command: &FrameCommands) -> Result<()> {
    match command {
        FrameCommands::EncodeRead {
            device_id,
            start_register,
            qty,
        } => {
            let reg = if start_register.starts_with("0x") || start_register.starts_with("0X") {
                u16::from_str_radix(&start_register[2..], 16)?
            } else {
                start_register.parse::<u16>()?
            };
            let mut payload = Vec::new();
            payload.extend_from_slice(&reg.to_be_bytes());
            payload.extend_from_slice(&qty.to_be_bytes());
            let frame = encode_ascii_frame(*device_id, 0x03, &payload, ':');
            print!("{}", frame);
        }
        FrameCommands::EncodeWriteCurrent {
            device_id,
            current_amps,
        } => {
            let dc_val = (current_amps * 100.0 / 6.0).round() as u16;
            let duty_cycle_x10 = dc_val.clamp(80, 1000);
            let icmax = SetIcmax::new(duty_cycle_x10).context("Invalid current")?;
            let mut payload = Vec::new();
            payload.extend_from_slice(&SetIcmax::ADDRESS.to_be_bytes());
            payload.extend_from_slice(&1u16.to_be_bytes());
            payload.push(2);
            payload.extend_from_slice(&icmax.encode_to_word().to_be_bytes());
            let frame = encode_ascii_frame(*device_id, 0x10, &payload, ':');
            print!("{}", frame);
        }
        FrameCommands::Decode { frame } => {
            let (dev_id, fc, payload) = decode_ascii_frame(frame).context("Failed decoding ASCII frame")?;
            println!("Decoded Frame:");
            println!("  Device ID: {:#04X} ({})", dev_id, dev_id);
            println!("  Function Code: {:#04X}", fc);
            let hex_payload: Vec<String> = payload.iter().map(|b| format!("{:02X}", b)).collect();
            println!("  Payload: {}", hex_payload.join(""));
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let timeout: Duration = cli.timeout;

    match cli.connection {
        Connection::Rtu {
            device,
            baud_rate,
            parity,
            stop_bits,
            address,
            command,
        } => {
            let parity = match parity {
                SerialParity::None => tokio_serial::Parity::None,
                SerialParity::Even => tokio_serial::Parity::Even,
                SerialParity::Odd => tokio_serial::Parity::Odd,
            };
            let stop_bits = match stop_bits {
                SerialStopBits::One => tokio_serial::StopBits::One,
                SerialStopBits::Two => tokio_serial::StopBits::Two,
            };
            let builder = abl_evcc_lib::tokio_common::serial_port_builder(
                &device, baud_rate, parity, stop_bits,
            );
            let mut ctx = tokio_modbus::client::sync::rtu::connect_slave(&builder, tokio_modbus::Slave(address))
                .with_context(|| format!("Cannot open serial device {} baud rate {}", device, baud_rate))?;
            ctx.set_timeout(timeout);
            let mut client = SafeClient::new(ctx);
            execute_device_command(&mut client, &command)?;
        }
        Connection::Tcp {
            address,
            slave,
            command,
        } => {
            let socket_addr = address
                .parse()
                .with_context(|| format!("Cannot parse TCP socket address {}", address))?;
            let mut ctx = tokio_modbus::client::sync::tcp::connect_slave(socket_addr, tokio_modbus::Slave(slave))
                .with_context(|| format!("Cannot open TCP connection to {}", socket_addr))?;
            ctx.set_timeout(timeout);
            let mut client = SafeClient::new(ctx);
            execute_device_command(&mut client, &command)?;
        }
        Connection::Frame { command } => {
            execute_frame_command(&command)?;
        }
    }

    Ok(())
}
