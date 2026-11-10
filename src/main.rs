use abl_evcc_lib::{protocol as proto, tokio_sync_safe_client::SafeClient, ModbusParam};
use anyhow::{Context, Result};
use clap::Parser;
use flexi_logger::{Logger, LoggerHandle};
use log::*;
use std::{ops::Deref, panic, time::Duration};

mod commandline;
mod daemon;
mod mqtt;

fn logging_init(loglevel: LevelFilter) -> LoggerHandle {
    let log_handle = Logger::try_with_env_or_str(loglevel.as_str())
        .expect("Cannot init logging")
        .start()
        .expect("Cannot start logging");

    panic::set_hook(Box::new(|panic_info| {
        let (filename, line, column) = panic_info
            .location()
            .map(|loc| (loc.file(), loc.line(), loc.column()))
            .unwrap_or(("<unknown>", 0, 0));
        let cause = panic_info
            .payload()
            .downcast_ref::<String>()
            .map(String::deref);
        let cause = cause.unwrap_or_else(|| {
            panic_info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .unwrap_or("<cause unknown>")
        });

        error!(
            "Thread '{}' panicked at {}:{}:{}: {}",
            std::thread::current().name().unwrap_or("<unknown>"),
            filename,
            line,
            column,
            cause
        );
    }));
    log_handle
}

fn minimum_rtu_delay(baud_rate: u32) -> Duration {
    let bit_time = Duration::from_secs_f64(1.0 / (baud_rate as f64));
    let char_time = bit_time * 11;
    let result = Duration::from_millis((char_time.as_secs_f64() * 3.5 * 1_000.0) as u64);
    let min_duration = Duration::from_micros(1_750);
    if result < min_duration {
        min_duration
    } else {
        result
    }
}

fn check_rtu_delay(delay: Duration, baud_rate: u32) -> Duration {
    let min_rtu_delay = minimum_rtu_delay(baud_rate);
    if delay < min_rtu_delay {
        warn!(
            "Your RTU delay of {delay:?} is below the minimum delay of {min_rtu_delay:?}, fallback to minimum"
        );
        return min_rtu_delay;
    }
    delay
}

fn handle_commands(
    client: &mut SafeClient,
    command: &commandline::Commands,
    delay: &Duration,
    no_json: bool,
) -> Result<()> {
    match command {
        commandline::Commands::Daemon { poll_iterval, mode } => {
            let interval = *delay.max(poll_iterval);
            daemon::run(client, mode.clone(), interval, no_json)?;
        }
        commandline::Commands::ReadAll => {
            let values = client
                .read_all(delay)
                .with_context(|| "Cannot read all values")?;
            if no_json {
                println!("{values}");
            } else {
                println!("{}", serde_json::to_string_pretty(&values)?);
            }
        }
        commandline::Commands::ReadStatus => {
            let status = client
                .read_status()
                .with_context(|| "Cannot read vehicle charging status")?;
            if no_json {
                println!("{status}");
            } else {
                println!("{}", serde_json::to_string_pretty(&status)?);
            }
        }
        commandline::Commands::ReadCurrent => {
            let current = client
                .read_current()
                .with_context(|| "Cannot read charging current")?;
            if no_json {
                println!("{current}");
            } else {
                println!("{}", serde_json::to_string_pretty(&current)?);
            }
        }
        commandline::Commands::ReadPower => {
            let power = client
                .read_power_or_current()
                .with_context(|| "Cannot read power or current")?;
            if no_json {
                println!("{power}");
            } else {
                println!("{}", serde_json::to_string_pretty(&power)?);
            }
        }
        commandline::Commands::SetCurrent { amperes } => {
            let current = proto::ChargingCurrent::new(*amperes)
                .with_context(|| format!("Invalid charging current value: {amperes}A"))?;
            client
                .set_current(current)
                .with_context(|| format!("Cannot set charging current to {amperes}A"))?;
            println!("Charging current successfully set to: {current}");
        }
    }
    Ok(())
}

fn handle_frame_commands(cmd: &commandline::FrameCommands) -> Result<()> {
    match cmd {
        commandline::FrameCommands::EncodeRead {
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
            let frame = proto::encode_ascii_frame(*device_id, 0x03, &payload, ':');
            print!("{frame}");
        }
        commandline::FrameCommands::EncodeWriteCurrent {
            device_id,
            amperes,
        } => {
            let current = proto::ChargingCurrent::new(*amperes)?;
            let mut payload = Vec::new();
            payload.extend_from_slice(&proto::ChargingCurrent::ADDRESS.to_be_bytes());
            payload.extend_from_slice(&1u16.to_be_bytes());
            payload.push(2);
            payload.extend_from_slice(&current.encode_to_word().to_be_bytes());
            let frame = proto::encode_ascii_frame(*device_id, 0x10, &payload, ':');
            print!("{frame}");
        }
        commandline::FrameCommands::Decode { frame } => {
            let (dev_id, fc, payload) = proto::decode_ascii_frame(frame)?;
            println!("Decoded Frame:");
            println!("  Device ID: {:#04X} ({})", dev_id, dev_id);
            println!("  Function Code: {:#04X}", fc);
            let hex_payload: Vec<String> = payload.iter().map(|b| format!("{:02X}", b)).collect();
            println!("  Payload: {}", hex_payload.join(""));
        }
        commandline::FrameCommands::EncodeEvccRequest {
            address,
            command_code,
            parameter,
        } => {
            let cmd = proto::EvccCommand::from_code(*command_code);
            let param_opt = if parameter.is_empty() {
                None
            } else {
                Some(parameter.as_str())
            };
            let line = proto::encode_evcc_request(*address, cmd, param_opt);
            print!("{line}");
        }
        commandline::FrameCommands::DecodeEvccReply { line } => {
            let (addr, cmd_code, param) = proto::parse_evcc_reply(line)?;
            let cmd = proto::EvccCommand::from_code(cmd_code);
            println!("Decoded EVCC Serial Reply:");
            println!("  Module Address: {}", addr);
            println!("  Command: {:02} ({})", cmd_code, cmd.name());
            if let Some(p) = param {
                println!("  Parameter: {}", p);
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = commandline::Args::parse();
    let mut delay = args.delay;
    let _log_handle = logging_init(args.verbose.log_level_filter());

    match &args.connection {
        commandline::Connection::Tcp {
            address,
            slave,
            command,
        } => {
            let socket_addr = address
                .parse()
                .with_context(|| format!("Cannot parse address {address}"))?;
            trace!("Open TCP address {socket_addr}");
            let mut ctx = tokio_modbus::client::sync::tcp::connect_slave(
                socket_addr,
                tokio_modbus::Slave(*slave),
            )
            .with_context(|| format!("Cannot open {socket_addr:?}"))?;
            ctx.set_timeout(args.timeout);
            let mut client = SafeClient::new(ctx);
            handle_commands(&mut client, command, &delay, args.no_json)?;
        }
        commandline::Connection::Rtu {
            device,
            baud_rate,
            address,
            parity_and_stop_bit,
            command,
        } => {
            trace!("Open RTU {device} address {address} baud rate {baud_rate}");
            delay = check_rtu_delay(delay, *baud_rate);
            let (parity, stop_bits) = match parity_and_stop_bit {
                commandline::ParityAndStopBit::EvenParityOneStopBit => {
                    (tokio_serial::Parity::Even, tokio_serial::StopBits::One)
                }
                commandline::ParityAndStopBit::NoParityOneStopBit => {
                    (tokio_serial::Parity::None, tokio_serial::StopBits::One)
                }
                commandline::ParityAndStopBit::OddParityOneStopBit => {
                    (tokio_serial::Parity::Odd, tokio_serial::StopBits::One)
                }
                commandline::ParityAndStopBit::NoParityTwoStopBits => {
                    (tokio_serial::Parity::None, tokio_serial::StopBits::Two)
                }
            };
            let builder = abl_evcc_lib::tokio_common::serial_port_builder(
                device, *baud_rate, parity, stop_bits,
            );
            let mut ctx = tokio_modbus::client::sync::rtu::connect_slave(
                &builder,
                tokio_modbus::Slave(*address),
            )
            .with_context(|| format!("Cannot open device {device} baud rate {baud_rate}"))?;
            ctx.set_timeout(args.timeout);
            let mut client = SafeClient::new(ctx);
            handle_commands(&mut client, command, &delay, args.no_json)?;
        }
        commandline::Connection::Frame { command } => {
            handle_frame_commands(command)?;
        }
        commandline::Connection::Ascii {
            device,
            tcp,
            baud_rate,
            address,
            parity_and_stop_bit,
            command,
        } => {
            handle_ascii_commands(
                device.as_deref(),
                tcp.as_deref(),
                *baud_rate,
                *address,
                parity_and_stop_bit,
                command,
                delay,
                args.timeout,
                args.no_json,
            )?;
        }
        commandline::Connection::Serial {
            device,
            tcp,
            baud_rate,
            address,
            command,
        } => {
            handle_serial_commands(device.as_deref(), tcp.as_deref(), *baud_rate, *address, command, args.timeout)?;
        }
    }

    Ok(())
}

fn handle_ascii_commands(
    device: Option<&str>,
    tcp_addr: Option<&str>,
    baud_rate: u32,
    address: u8,
    parity_and_stop_bit: &commandline::ParityAndStopBit,
    command: &commandline::AsciiCommands,
    delay: Duration,
    timeout: Duration,
    no_json: bool,
) -> Result<()> {
    let (parity, stop_bits) = match parity_and_stop_bit {
        commandline::ParityAndStopBit::EvenParityOneStopBit => {
            (tokio_serial::Parity::Even, tokio_serial::StopBits::One)
        }
        commandline::ParityAndStopBit::NoParityOneStopBit => {
            (tokio_serial::Parity::None, tokio_serial::StopBits::One)
        }
        commandline::ParityAndStopBit::OddParityOneStopBit => {
            (tokio_serial::Parity::Odd, tokio_serial::StopBits::One)
        }
        commandline::ParityAndStopBit::NoParityTwoStopBits => {
            (tokio_serial::Parity::None, tokio_serial::StopBits::Two)
        }
    };

    let mut client = match (device, tcp_addr) {
        (Some(_), Some(_)) => anyhow::bail!("Cannot specify both --device and --tcp. Choose one transport."),
        (None, None) => {
            let default_dev = if cfg!(target_os = "windows") { "COM1" } else { "/dev/ttyUSB0" };
            abl_evcc_lib::ascii_client::AsciiClient::connect_serial(
                default_dev, baud_rate, parity, stop_bits, address, timeout, delay,
            )?
        }
        (Some(dev), None) => {
            abl_evcc_lib::ascii_client::AsciiClient::connect_serial(
                dev, baud_rate, parity, stop_bits, address, timeout, delay,
            )?
        }
        (None, Some(tcp)) => {
            abl_evcc_lib::ascii_client::AsciiClient::connect_tcp(
                tcp, address, timeout, delay,
            )?
        }
    };

    match command {
        commandline::AsciiCommands::Daemon { poll_iterval, mode } => {
            let interval = delay.max(*poll_iterval);
            daemon::run_ascii(&mut client, mode.clone(), interval, no_json)?;
        }
        commandline::AsciiCommands::ReadAll => {
            let fw = client.read_firmware().ok();
            let meas = client.read_measurements()
                .with_context(|| "Cannot read measurements and status from wallbox")?;
            if no_json {
                if let Some(f) = &fw {
                    println!("--- Controller Info ---");
                    println!("{f}");
                    println!("\n--- Measurements & Status ---");
                }
                println!("{meas}");
            } else {
                #[derive(serde::Serialize)]
                struct FullReport {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    firmware: Option<proto::Evcc2Firmware>,
                    measurements: proto::Evcc2Measurements,
                }
                let report = FullReport {
                    firmware: fw,
                    measurements: meas,
                };
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }
        commandline::AsciiCommands::ReadStatus => {
            let status = client.read_status()
                .with_context(|| "Cannot read charging status")?;
            if no_json {
                println!("{status}");
            } else {
                println!("{}", serde_json::to_string_pretty(&status)?);
            }
        }
        commandline::AsciiCommands::ReadCurrent => {
            let (amperes, duty) = client.read_configured_current()
                .with_context(|| "Cannot read configured current")?;
            if no_json {
                println!("Configured Charging Current: {:.1}A (Duty Cycle: {:.1}%)", amperes, duty as f32 / 10.0);
            } else {
                let json = serde_json::json!({
                    "current_amperes": amperes,
                    "duty_cycle_permille": duty,
                    "duty_cycle_percent": duty as f32 / 10.0,
                    "enabled": amperes >= 6.0
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
        }
        commandline::AsciiCommands::SetCurrent { amperes } => {
            client.set_current(*amperes)
                .with_context(|| format!("Cannot set charging current to {amperes}A"))?;
            if *amperes == 0 {
                println!("Charging current disabled / stopped (0A).");
            } else {
                println!("Charging current successfully set to: {}A", amperes);
            }
        }
        commandline::AsciiCommands::ReadCurrents => {
            let meas = client.read_measurements()
                .with_context(|| "Cannot read phase currents")?;
            if no_json {
                println!("Current L1: {:.1} A", meas.current_l1_amperes);
                println!("Current L2: {:.1} A", meas.current_l2_amperes);
                println!("Current L3: {:.1} A", meas.current_l3_amperes);
                println!("Total Active Power: {:.1} W", meas.total_power_watts);
            } else {
                let json = serde_json::json!({
                    "current_l1": meas.current_l1_amperes,
                    "current_l2": meas.current_l2_amperes,
                    "current_l3": meas.current_l3_amperes,
                    "power_watts": meas.total_power_watts
                });
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
        }
        commandline::AsciiCommands::ReadFirmware => {
            let fw = client.read_firmware()
                .with_context(|| "Cannot read firmware information")?;
            if no_json {
                println!("{fw}");
            } else {
                println!("{}", serde_json::to_string_pretty(&fw)?);
            }
        }
        commandline::AsciiCommands::Enable { amperes } => {
            client.enable(Some(*amperes))
                .with_context(|| format!("Cannot enable charging at {amperes}A"))?;
            println!("Charging successfully enabled at {}A.", amperes);
        }
        commandline::AsciiCommands::Disable => {
            client.disable()
                .with_context(|| "Cannot disable charging")?;
            println!("Charging successfully disabled / halted.");
        }
        commandline::AsciiCommands::WakeUp => {
            println!("Sending wake-up sequence to vehicle (cycling state E0 -> A1)...");
            client.wake_up()
                .with_context(|| "Failed to execute wake-up sequence")?;
            println!("Wake-up sequence completed.");
        }
    }

    Ok(())
}

fn handle_serial_commands(
    device: Option<&str>,
    tcp_addr: Option<&str>,
    baud_rate: u32,
    address: u8,
    command: &commandline::EvccSerialCommands,
    timeout: Duration,
) -> Result<()> {
    use std::io::{BufRead, BufReader, Write};

    let (cmd, param_str) = match command {
        commandline::EvccSerialCommands::Raw {
            command_code,
            parameter,
        } => (proto::EvccCommand::from_code(*command_code), parameter.clone()),
        commandline::EvccSerialCommands::GetFirmware => (proto::EvccCommand::GetFirmware, String::new()),
        commandline::EvccSerialCommands::GetStatus => (proto::EvccCommand::GetStatus, String::new()),
        commandline::EvccSerialCommands::GetInputs => (proto::EvccCommand::GetInputs, String::new()),
        commandline::EvccSerialCommands::GetPresentPwm => (proto::EvccCommand::GetPresentPwm, String::new()),
        commandline::EvccSerialCommands::SetPwm { parameter } => (proto::EvccCommand::SetPwm, parameter.clone()),
        commandline::EvccSerialCommands::SetCurrent { amperes } => {
            let param = if *amperes == 0 {
                "0999".to_string()
            } else {
                match *amperes {
                    6 => "0000".to_string(),
                    10 => "0001".to_string(),
                    13 => "0002".to_string(),
                    16 => "0003".to_string(),
                    20 => "0004".to_string(),
                    30 => "0005".to_string(),
                    32 => "0006".to_string(),
                    63 => "0007".to_string(),
                    70 => "0008".to_string(),
                    80 => "0009".to_string(),
                    other => {
                        let pwm = if other <= 51 {
                            (other as f32 / 0.06).round() as u16
                        } else {
                            ((other as f32 / 2.5 + 64.0) * 10.0).round() as u16
                        };
                        format!("{:04}", pwm.clamp(80, 970))
                    }
                }
            };
            (proto::EvccCommand::SetPwm, param)
        }
        commandline::EvccSerialCommands::EnablePwm => (proto::EvccCommand::EnablePwm, String::new()),
        commandline::EvccSerialCommands::DisablePwm => (proto::EvccCommand::DisablePwm, String::new()),
        commandline::EvccSerialCommands::GetDefaultPwm => (proto::EvccCommand::GetDefaultPwm, String::new()),
        commandline::EvccSerialCommands::BreakCharge => (proto::EvccCommand::SetBreakCharge, String::new()),
        commandline::EvccSerialCommands::ResumeCharge => (proto::EvccCommand::ClearBreakCharge, String::new()),
        commandline::EvccSerialCommands::GetBreakCharge => (proto::EvccCommand::GetBreakCharge, String::new()),
        commandline::EvccSerialCommands::SwitchToAPrime => (proto::EvccCommand::SwitchToAPrime, String::new()),
        commandline::EvccSerialCommands::LeaveAPrime => (proto::EvccCommand::LeaveAPrime, String::new()),
        commandline::EvccSerialCommands::Reset => (proto::EvccCommand::Reset, "1111".to_string()),
    };

    let param_opt = if param_str.is_empty() {
        None
    } else {
        Some(param_str.as_str())
    };

    let req = proto::encode_evcc_request(address, cmd, param_opt);

    // Communicate over either transparent TCP socket or serial RS485 port
    let mut stream = match (device, tcp_addr) {
        (Some(_), Some(_)) => anyhow::bail!("Cannot specify both --device and --tcp. Choose one transport."),
        (None, None) => {
            let default_dev = if cfg!(target_os = "windows") { "COM1" } else { "/dev/ttyUSB0" };
            trace!("Defaulting to serial device {default_dev} at {baud_rate} 8N1");
            let port = tokio_serial::new(default_dev, baud_rate)
                .parity(tokio_serial::Parity::None)
                .stop_bits(tokio_serial::StopBits::One)
                .data_bits(tokio_serial::DataBits::Eight)
                .timeout(timeout)
                .open()
                .with_context(|| format!("Cannot open serial device {default_dev}"))?;
            Stream::Serial(port)
        }
        (Some(dev), None) => {
            trace!("Opening serial device {dev} at {baud_rate} 8N1");
            let port = tokio_serial::new(dev, baud_rate)
                .parity(tokio_serial::Parity::None)
                .stop_bits(tokio_serial::StopBits::One)
                .data_bits(tokio_serial::DataBits::Eight)
                .timeout(timeout)
                .open()
                .with_context(|| format!("Cannot open serial device {dev}"))?;
            Stream::Serial(port)
        }
        (None, Some(tcp)) => {
            trace!("Connecting to transparent TCP bridge {tcp}");
            let socket_addr: std::net::SocketAddr = tcp
                .parse()
                .with_context(|| format!("Cannot parse TCP address: {tcp}"))?;
            let tcp_stream = std::net::TcpStream::connect_timeout(&socket_addr, timeout)
                .with_context(|| format!("Cannot connect to TCP endpoint {tcp}"))?;
            tcp_stream.set_read_timeout(Some(timeout))?;
            tcp_stream.set_write_timeout(Some(timeout))?;
            Stream::Tcp(tcp_stream)
        }
    };

    trace!("Sending: {:?}", req);
    stream.write_all(req.as_bytes())
        .with_context(|| "Failed to write request to EVCC controller")?;
    stream.flush().with_context(|| "Failed to flush stream")?;

    let mut reader = BufReader::new(stream);
    let mut reply = String::new();
    reader
        .read_line(&mut reply)
        .with_context(|| "Failed to read reply from EVCC controller")?;

    trace!("Received: {:?}", reply);
    let (res_addr, res_cmd, param) = proto::parse_evcc_reply(&reply)?;
    let parsed_cmd = proto::EvccCommand::from_code(res_cmd);
    println!("Reply from EVCC module {}:", res_addr);
    println!("  Command: {:02} ({})", res_cmd, parsed_cmd.name());
    if let Some(p) = param {
        println!("  Value: {}", p);
        if res_cmd == 2 {
            // Status code explanations
            let desc = match p.trim() {
                "0000" => "State A (Standby / No EV connected)",
                "0004" => "State B2 (EV connected, charge enabled, waiting for EV)",
                "0005" => "State C (Charging active)",
                "0006" => "State D (Charging with ventilation)",
                "0009" => "State B' (Charging stopped by EV)",
                "0013" => "State B1 (EV connected, charge suspended/not enabled)",
                "0017" => "State A' (CP off / Wake-up state)",
                "0033" => "Error CS (Cable resistance out of range)",
                "0035" => "Error EV (Vehicle diode fault)",
                "0037" => "Error lock (Socket locking failed)",
                "0039" => "Error ventilation",
                "0255" => "State MANUAL",
                _ => "Unknown state code",
            };
            println!("  Interpretation: {}", desc);
        }
    }

    Ok(())
}

enum Stream {
    Serial(Box<dyn tokio_serial::SerialPort>),
    Tcp(std::net::TcpStream),
}

impl std::io::Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Serial(s) => s.read(buf),
            Self::Tcp(t) => t.read(buf),
        }
    }
}

impl std::io::Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Serial(s) => s.write(buf),
            Self::Tcp(t) => t.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Serial(s) => s.flush(),
            Self::Tcp(t) => t.flush(),
        }
    }
}
