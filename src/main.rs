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
            let meas = client
                .read_measurements()
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
        commandline::Commands::SetCurrent { amperes } => {
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
        commandline::Commands::Enable { amperes } => {
            client
                .enable(Some(*amperes))
                .with_context(|| format!("Cannot enable charging at {amperes}A"))?;
            println!("Charging successfully enabled at {}A.", amperes);
        }
        commandline::Commands::Disable => {
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

    let (parity, stop_bits) = match args.parity_and_stop_bit {
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

    let mut client = match (args.device.as_deref(), args.tcp.as_deref()) {
        (Some(_), Some(_)) => {
            anyhow::bail!("Cannot specify both --device and --tcp. Choose one transport.")
        }
        (None, None) => {
            let default_dev = if cfg!(target_os = "windows") {
                "COM1"
            } else {
                "/dev/ttyUSB0"
            };
            AsciiClient::connect_serial(
                default_dev,
                args.baud_rate,
                parity,
                stop_bits,
                args.address,
                args.timeout,
                args.delay,
            )?
        }
        (Some(dev), None) => AsciiClient::connect_serial(
            dev,
            args.baud_rate,
            parity,
            stop_bits,
            args.address,
            args.timeout,
            args.delay,
        )?,
        (None, Some(tcp)) => {
            AsciiClient::connect_tcp(tcp, args.address, args.timeout, args.delay)?
        }
    };

    handle_commands(&mut client, &args.command, args.delay, args.no_json)?;

    Ok(())
}
