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
    }

    Ok(())
}
