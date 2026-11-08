use abl_evcc_lib::{protocol as proto, tokio_sync_safe_client::SafeClient, ModbusParam};
use anyhow::{Context, Result};
use clap::Parser;
use flexi_logger::{Logger, LoggerHandle};
use log::*;
use std::{ops::Deref, panic, time::Duration};

mod commandline;
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
        commandline::Commands::Daemon { poll_iterval, mode } => match mode {
            commandline::DaemonOutput::Console => loop {
                let values = client
                    .read_all(delay)
                    .with_context(|| "Cannot read all values")?;
                if no_json {
                    println!("{values}");
                } else {
                    println!("{}", serde_json::to_string_pretty(&values)?);
                }
                std::thread::sleep(*delay.max(poll_iterval));
            },
            commandline::DaemonOutput::Mqtt { config_file } => {
                mqtt::run_mqtt_daemon(client, delay, poll_iterval, config_file, no_json)?;
            }
        },
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
        commandline::Commands::ReadAllSettings => {
            let settings = client
                .read_all_settings(delay)
                .with_context(|| "Cannot read all settings")?;
            if no_json {
                println!("{settings}");
            } else {
                println!("{}", serde_json::to_string_pretty(&settings)?);
            }
        }
        commandline::Commands::ReadCurrent { resolution } => {
            if resolution.to_lowercase() == "short" {
                let cur = client
                    .read_current_amps()
                    .with_context(|| "Cannot read current amps")?;
                if no_json {
                    println!("{cur}");
                } else {
                    println!("{}", serde_json::to_string_pretty(&cur)?);
                }
            } else {
                let cur = client
                    .read_current_full()
                    .with_context(|| "Cannot read current full")?;
                if no_json {
                    println!("{cur}");
                } else {
                    println!("{}", serde_json::to_string_pretty(&cur)?);
                }
            }
        }
        commandline::Commands::ReadDeviceInfo => {
            let info = client
                .read_device_info()
                .with_context(|| "Cannot read device info")?;
            if no_json {
                println!("{info}");
            } else {
                println!("{}", serde_json::to_string_pretty(&info)?);
            }
        }
        commandline::Commands::ReadModbusSettings => {
            let settings = client
                .read_modbus_settings()
                .with_context(|| "Cannot read modbus settings")?;
            if no_json {
                println!("{settings}");
            } else {
                println!("{}", serde_json::to_string_pretty(&settings)?);
            }
        }
        commandline::Commands::ReadSystemFlags => {
            let flags = client
                .read_system_flags()
                .with_context(|| "Cannot read system flags")?;
            if no_json {
                println!("{flags:#?}");
            } else {
                println!("{}", serde_json::to_string_pretty(&flags)?);
            }
        }
        commandline::Commands::SetCurrent { amperes } => {
            let dc_val = (amperes * 100.0 / 6.0).round() as u16;
            let duty_cycle_x10 = dc_val.clamp(80, 1000);
            let icmax = proto::SetIcmax::new(duty_cycle_x10)
                .with_context(|| "Cannot construct SetIcmax")?;
            client
                .set_current(icmax)
                .with_context(|| "Cannot set charging current")?;
            println!(
                "Charging current successfully set to: {}A (Duty Cycle: {:.1}%)",
                amperes,
                duty_cycle_x10 as f32 / 10.0
            );
        }
        commandline::Commands::SetState { action } => {
            let proto_action = match action {
                commandline::StateAction::Reset => proto::ModifyStateAction::Reset,
                commandline::StateAction::JumpToA1 => proto::ModifyStateAction::JumpToA1,
                commandline::StateAction::JumpToE0 => proto::ModifyStateAction::JumpToE0,
                commandline::StateAction::JumpToE2 => proto::ModifyStateAction::JumpToE2,
                commandline::StateAction::JumpToF1 => proto::ModifyStateAction::JumpToF1,
                commandline::StateAction::EnableLoadImbalance20A => {
                    proto::ModifyStateAction::LoadImbalanceDetection20AEnabled
                }
            };
            client
                .set_state(proto_action)
                .with_context(|| "Cannot set state")?;
            println!("Outlet state successfully modified to: {:?}", action);
        }
        commandline::Commands::SetDeviceId {
            new_device_id,
            timeout,
            power_cycle,
        } => {
            let ut = match timeout {
                commandline::TimeoutOption::NoChange => proto::UpstreamTimeoutOption::NoChange,
                commandline::TimeoutOption::Disabled => proto::UpstreamTimeoutOption::Disabled,
                commandline::TimeoutOption::Enabled => proto::UpstreamTimeoutOption::Enabled,
            };
            let pc = match power_cycle {
                commandline::PowerCycleOption::NoChange => proto::PowerCycleB1B2Option::NoChange,
                commandline::PowerCycleOption::EnterB2IfConnected => {
                    proto::PowerCycleB1B2Option::EnterB2IfConnected
                }
                commandline::PowerCycleOption::StopInB1IfConnected => {
                    proto::PowerCycleB1B2Option::StopInB1IfConnected
                }
            };
            let set_cmd = proto::SetDeviceId::new(*new_device_id, ut, pc)
                .with_context(|| "Cannot create SetDeviceId")?;
            client
                .set_device_id(set_cmd)
                .with_context(|| "Cannot set device ID")?;
            println!("Device ID successfully changed to: {}", new_device_id);
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
            let dc_val = (amperes * 100.0 / 6.0).round() as u16;
            let duty_cycle_x10 = dc_val.clamp(80, 1000);
            let icmax = proto::SetIcmax::new(duty_cycle_x10)?;
            let mut payload = Vec::new();
            payload.extend_from_slice(&proto::SetIcmax::ADDRESS.to_be_bytes());
            payload.extend_from_slice(&1u16.to_be_bytes());
            payload.push(2);
            payload.extend_from_slice(&icmax.encode_to_word().to_be_bytes());
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
