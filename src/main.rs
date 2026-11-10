use abl_evcc_lib::{ascii_client::AsciiClient, protocol as proto};
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

fn handle_commands(
    client: &mut AsciiClient,
    command: &commandline::Commands,
    delay: Duration,
    no_json: bool,
) -> Result<()> {
    match command {
        commandline::Commands::Daemon { poll_iterval, mode } => {
            let interval = delay.max(*poll_iterval);
            daemon::run(client, mode.clone(), interval, no_json)?;
        }
        commandline::Commands::ReadAll => {
            let fw = client.read_firmware().ok();
            let flags = client.read_system_flags().ok();
            let meas = client
                .read_measurements()
                .with_context(|| "Cannot read measurements and status from wallbox")?;
            if no_json {
                if let Some(f) = &fw {
                    println!("--- Controller Info ---");
                    println!("{f}\n");
                }
                if let Some(fl) = &flags {
                    println!("--- System Flags ---");
                    println!("{fl}\n");
                }
                println!("--- Measurements & Status ---");
                println!("{meas}");
            } else {
                #[derive(serde::Serialize)]
                struct FullReport {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    firmware: Option<proto::Evcc2Firmware>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    system_flags: Option<proto::Evcc2SystemFlags>,
                    measurements: proto::Evcc2Measurements,
                }
                let report = FullReport {
                    firmware: fw,
                    system_flags: flags,
                    measurements: meas,
                };
                println!("{}", serde_json::to_string_pretty(&report)?);
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
            let (amperes, duty) = client
                .read_configured_current()
                .with_context(|| "Cannot read configured current")?;
            if no_json {
                println!(
                    "Configured Charging Current: {:.1}A (Duty Cycle: {:.1}%)",
                    amperes,
                    duty as f32 / 10.0
                );
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
        commandline::Commands::SetCurrent { amperes, force } => {
            if *amperes == 0 && !force {
                eprintln!("Warning: Setting charging current to 0A will immediately halt active charging.");
                eprintln!("Pass --force to skip confirmation. Continue? [y/N]");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    eprintln!("Operation cancelled.");
                    return Ok(());
                }
            }
            client
                .set_current(*amperes)
                .with_context(|| format!("Cannot set charging current to {amperes}A"))?;
            if *amperes == 0 {
                println!("Charging current disabled / stopped (0A).");
            } else {
                println!("Charging current successfully set to: {}A", amperes);
            }
        }
        commandline::Commands::ReadCurrents => {
            let meas = client
                .read_measurements()
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
        commandline::Commands::ReadFirmware => {
            let fw = client
                .read_firmware()
                .with_context(|| "Cannot read firmware information")?;
            if no_json {
                println!("{fw}");
            } else {
                println!("{}", serde_json::to_string_pretty(&fw)?);
            }
        }
        commandline::Commands::ReadSystemFlags => {
            let flags = client
                .read_system_flags()
                .with_context(|| "Cannot read system flags")?;
            if no_json {
                println!("{flags}");
            } else {
                println!("{}", serde_json::to_string_pretty(&flags)?);
            }
        }
        commandline::Commands::ReadModbusSettings => {
            let settings = client
                .read_modbus_settings()
                .with_context(|| "Cannot read Modbus settings")?;
            if no_json {
                println!("{settings}");
            } else {
                println!("{}", serde_json::to_string_pretty(&settings)?);
            }
        }
        commandline::Commands::Reset { force } => {
            if !force {
                eprintln!("CRITICAL WARNING: Resetting the wallbox charge controller may interrupt active charging sessions!");
                eprintln!("Pass --force to skip confirmation. Are you sure you want to proceed? [y/N]");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    eprintln!("Operation cancelled.");
                    return Ok(());
                }
            }
            println!("Sending reset command to wallbox charge controller...");
            client
                .reset()
                .with_context(|| "Failed to reset charge controller")?;
            println!("Reset command acknowledged.");
        }
        commandline::Commands::Enable { amperes } => {
            client
                .enable(Some(*amperes))
                .with_context(|| format!("Cannot enable charging at {amperes}A"))?;
            println!("Charging successfully enabled at {}A.", amperes);
        }
        commandline::Commands::Disable { force } => {
            if !force {
                eprintln!("Warning: Disabling charging will cut power and stop the charging session!");
                eprintln!("Pass --force to skip confirmation. Continue? [y/N]");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    eprintln!("Operation cancelled.");
                    return Ok(());
                }
            }
            client
                .disable()
                .with_context(|| "Cannot disable charging")?;
            println!("Charging successfully disabled / halted.");
        }
        commandline::Commands::WakeUp => {
            println!("Sending wake-up sequence to vehicle (cycling state E0 -> A1)...");
            client
                .wake_up()
                .with_context(|| "Failed to execute wake-up sequence")?;
            println!("Wake-up sequence completed.");
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = commandline::Args::parse();
    let _log_handle = logging_init(args.verbose.log_level_filter());

    match &args.connection {
        commandline::Connection::Serial {
            device,
            baud_rate,
            address,
            parity_and_stop_bit,
            command,
        } => {
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

            let mut client = AsciiClient::connect_serial(
                device,
                *baud_rate,
                parity,
                stop_bits,
                *address,
                args.timeout,
                args.delay,
            )?;
            handle_commands(&mut client, command, args.delay, args.no_json)?;
        }
        commandline::Connection::Tcp {
            address,
            slave,
            command,
        } => {
            let mut client =
                AsciiClient::connect_tcp(address, *slave, args.timeout, args.delay)?;
            handle_commands(&mut client, command, args.delay, args.no_json)?;
        }
    }

    Ok(())
}
