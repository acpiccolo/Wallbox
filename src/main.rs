use abl_evcc_lib::{protocol as proto, tokio_sync::AsciiClient};
use anyhow::{bail, Context, Result};
use clap::Parser;
use flexi_logger::{Logger, LoggerHandle};
use log::*;
use std::{io::Write, ops::Deref, panic, time::Duration};

mod commandline;
mod daemon;
mod mqtt;

/// Calculates the minimum serial inter-frame delay based on baud rate (minimum 3.5 character times).
/// Reference: <https://minimalmodbus.readthedocs.io/en/stable/serialcommunication.html#timing-of-the-serial-communications>
pub fn minimum_serial_delay(baud_rate: u32) -> Duration {
    // Timing of serial communications (minimum 3.5 character times)
    let rate = baud_rate as f64;
    let bit_time = Duration::from_secs_f64(1.0 / rate);
    let char_time = bit_time * 11;
    let result = Duration::from_millis((char_time.as_secs_f64() * 3.5 * 1_000.0) as u64);
    let min_duration = Duration::from_micros(1_750);
    if result < min_duration {
        min_duration
    } else {
        result
    }
}

pub fn check_serial_delay(delay: Duration, baud_rate: u32) -> Duration {
    let min_delay = minimum_serial_delay(baud_rate);
    if delay < min_delay {
        warn!(
            "Your serial delay of {delay:?} is below the minimum delay of {min_delay:?}, fallback to minimum"
        );
        return min_delay;
    }
    delay
}

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
                println!("Charging current successfully set to: {}A per phase (Total 3-phase power: {}W)", amperes, (*amperes as u32) * 3 * 230);
            }
        }
        commandline::Commands::SetPower { watts, force } => {
            let amps = proto::power_watts_to_amperes(*watts);
            if amps == 0 && !force {
                eprintln!(
                    "Warning: Setting charging power to 0W will immediately halt active charging."
                );
                eprintln!("Pass --force to skip confirmation. Continue? [y/N]");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    eprintln!("Operation cancelled.");
                    return Ok(());
                }
            }
            client
                .set_current(amps)
                .with_context(|| format!("Cannot set charging power to {watts}W ({amps}A)"))?;
            if amps == 0 {
                println!("Charging power disabled / stopped (0W / 0A).");
            } else {
                println!(
                    "Charging power {}W (3-phase) successfully converted and set to: {}A per phase (effective power: {}W).",
                    watts, amps, (amps as u32) * 3 * 230
                );
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
                eprintln!(
                    "Pass --force to skip confirmation. Are you sure you want to proceed? [y/N]"
                );
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
        commandline::Commands::Enable { amperes, power } => {
            let amps = if let Some(p) = power {
                let converted = proto::power_watts_to_amperes(*p);
                println!(
                    "Converted power {}W (3-phase) to {}A per phase.",
                    p, converted
                );
                converted
            } else {
                *amperes
            };
            client
                .enable(Some(amps))
                .with_context(|| format!("Cannot enable charging at {amps}A"))?;
            println!("Charging successfully enabled at {}A per phase.", amps);
        }
        commandline::Commands::Disable { force } => {
            if !force {
                eprintln!(
                    "Warning: Disabling charging will cut power and stop the charging session!"
                );
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
        commandline::Commands::Scan { max_address, force } => {
            handle_tcp_scan(client, *max_address, *force, delay)?;
        }
    }
    Ok(())
}

fn confirm_only_one_module_connected(force: bool) -> Result<bool> {
    if force {
        return Ok(true);
    }
    println!(
        "WARNING: This scan operation should only be performed if a SINGLE ABL eMH1 wallbox\n\
         is connected to the Modbus RS485 bus segment.\n\
         Multiple devices on the same bus without known IDs may lead to communication collisions."
    );
    print!("Do you want to continue with this understanding? [y/N]: ");
    std::io::stdout()
        .flush()
        .context("Failed to flush stdout")?;
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .context("Failed to read user confirmation")?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}

fn scan_device_address(client: &mut AsciiClient, slave_id: u8) -> Result<()> {
    client.set_slave_id(slave_id);
    let firmware = client.read_firmware()?;
    let status = client.read_status().ok();
    println!("SUCCESS!");
    println!("  Device found with Modbus Slave ID: {slave_id}");
    if let Some(st) = status {
        println!("  Vehicle / Charger Status: {st:?} ({})", st.as_char());
    }
    println!("  Firmware Revision: {}", firmware.firmware_revision());
    println!("  PCBA Version: {}", firmware.pcba_version);
    Ok(())
}

fn parity_and_stop_bits_to_serial(
    p_and_s: proto::ParityAndStopBit,
) -> (tokio_serial::Parity, tokio_serial::StopBits) {
    match p_and_s {
        proto::ParityAndStopBit::EvenParityOneStopBit => {
            (tokio_serial::Parity::Even, tokio_serial::StopBits::One)
        }
        proto::ParityAndStopBit::NoParityOneStopBit => {
            (tokio_serial::Parity::None, tokio_serial::StopBits::One)
        }
        proto::ParityAndStopBit::OddParityOneStopBit => {
            (tokio_serial::Parity::Odd, tokio_serial::StopBits::One)
        }
        proto::ParityAndStopBit::NoParityTwoStopBits => {
            (tokio_serial::Parity::None, tokio_serial::StopBits::Two)
        }
    }
}

fn handle_serial_scan(
    device: &str,
    max_address: u8,
    force: bool,
    timeout: Duration,
    delay: Duration,
) -> Result<()> {
    if !confirm_only_one_module_connected(force)? {
        println!("Serial scan aborted by user.");
        return Ok(());
    }

    println!("Starting serial bus scan on {device} (max address: {max_address})...");

    let default_baud = proto::BaudRate::default();
    let default_parity = proto::ParityAndStopBit::default();
    let (default_p, default_s) = parity_and_stop_bits_to_serial(default_parity);

    // Step 1: Test default connection first (38400 8E1, Slave ID 1, then Broadcast 0, then 2..=max_address)
    print!("Checking default connection: {default_baud} Bd, {default_parity}, Slave ID 1 ... ");
    std::io::stdout()
        .flush()
        .context("Failed to flush stdout")?;

    let default_delay = check_serial_delay(delay, default_baud.as_u32());
    if let Ok(mut client) = AsciiClient::connect_serial(
        device,
        default_baud.as_u32(),
        default_p,
        default_s,
        1,
        timeout,
        default_delay,
    ) {
        if scan_device_address(&mut client, 1).is_ok() {
            println!("  Communication established at default settings: {default_baud} Bd, {default_parity}");
            return Ok(());
        }
        println!("no response.");

        // Check broadcast address 0 on default connection
        print!("Checking broadcast address 0 at {default_baud} Bd, {default_parity} ... ");
        std::io::stdout()
            .flush()
            .context("Failed to flush stdout")?;
        std::thread::sleep(default_delay);
        if scan_device_address(&mut client, 0).is_ok() {
            println!("  Communication established at {default_baud} Bd, {default_parity} (via broadcast address 0)");
            return Ok(());
        }
        println!("no response.");

        // Check addresses 2..=max_address on default connection
        for addr in 2..=max_address {
            print!("Checking {default_baud} Bd, {default_parity}, Slave ID {addr} ... ");
            std::io::stdout()
                .flush()
                .context("Failed to flush stdout")?;
            std::thread::sleep(default_delay);
            if scan_device_address(&mut client, addr).is_ok() {
                println!("  Communication established at default settings: {default_baud} Bd, {default_parity}");
                return Ok(());
            }
            println!("no response.");
        }
    } else {
        println!("failed to open serial port.");
    }

    // Step 2: Loop through all other BaudRate and ParityAndStopBit combinations (skipping the default one)
    for baud in proto::BaudRate::ALL {
        for parity in proto::ParityAndStopBit::ALL {
            // Skip the default configuration since it was already comprehensively tested in Step 1
            if baud == default_baud && parity == default_parity {
                continue;
            }

            let (p, s) = parity_and_stop_bits_to_serial(parity);
            let scan_delay = check_serial_delay(delay, baud.as_u32());
            println!("Testing configuration: {baud} Bd, {parity} ...");

            let mut client = match AsciiClient::connect_serial(
                device,
                baud.as_u32(),
                p,
                s,
                1,
                timeout,
                scan_delay,
            ) {
                Ok(c) => c,
                Err(e) => {
                    debug!("Failed to configure serial port for {baud} Bd {parity}: {e}");
                    continue;
                }
            };

            // Check ID 1, then Broadcast 0
            for &addr in &[1, 0] {
                print!("  Checking {baud} Bd, {parity}, Slave ID {addr} ... ");
                std::io::stdout()
                    .flush()
                    .context("Failed to flush stdout")?;
                std::thread::sleep(scan_delay);
                if scan_device_address(&mut client, addr).is_ok() {
                    println!("  Communication established at {baud} Bd, {parity}");
                    return Ok(());
                }
                println!("no response.");
            }

            // Check addresses 2..=max_address
            for addr in 2..=max_address {
                print!("  Checking {baud} Bd, {parity}, Slave ID {addr} ... ");
                std::io::stdout()
                    .flush()
                    .context("Failed to flush stdout")?;
                std::thread::sleep(scan_delay);
                if scan_device_address(&mut client, addr).is_ok() {
                    println!("  Communication established at {baud} Bd, {parity}");
                    return Ok(());
                }
                println!("no response.");
            }
        }
    }

    bail!("No ABL eMH1 wallbox detected on {device} across tested baud rates and addresses 0..={max_address}.");
}

fn handle_tcp_scan(
    client: &mut AsciiClient,
    max_address: u8,
    force: bool,
    delay: Duration,
) -> Result<()> {
    if !confirm_only_one_module_connected(force)? {
        println!("TCP scan aborted by user.");
        return Ok(());
    }

    println!("Starting TCP slave ID scan (max address: {max_address})...");

    // 1. Check default ID 1
    print!("Checking default: Slave ID 1 ... ");
    std::io::stdout()
        .flush()
        .context("Failed to flush stdout")?;
    if scan_device_address(client, 1).is_ok() {
        return Ok(());
    }
    println!("no response.");

    // 2. Check broadcast address 0
    print!("Checking broadcast address 0 ... ");
    std::io::stdout()
        .flush()
        .context("Failed to flush stdout")?;
    std::thread::sleep(delay);
    if scan_device_address(client, 0).is_ok() {
        return Ok(());
    }
    println!("no response.");

    // 3. Scan addresses 2..=max_address
    for addr in 2..=max_address {
        print!("Checking Slave ID {addr} ... ");
        std::io::stdout()
            .flush()
            .context("Failed to flush stdout")?;
        std::thread::sleep(delay);
        if scan_device_address(client, addr).is_ok() {
            return Ok(());
        }
        println!("no response.");
    }

    bail!("No ABL eMH1 wallbox detected across addresses 0..={max_address}.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_delay() {
        assert_eq!(minimum_serial_delay(1200).as_millis(), 32);
        assert_eq!(minimum_serial_delay(2400).as_millis(), 16);
        assert_eq!(minimum_serial_delay(4800).as_millis(), 8);
        assert_eq!(minimum_serial_delay(9600).as_millis(), 4);
        assert_eq!(minimum_serial_delay(19200).as_millis(), 2);
        assert_eq!(minimum_serial_delay(38400), Duration::from_micros(1_750));
        assert_eq!(minimum_serial_delay(57600), Duration::from_micros(1_750));

        // Fallback test
        let low_delay = Duration::from_millis(1);
        assert_eq!(
            check_serial_delay(low_delay, 9600),
            Duration::from_millis(4)
        );

        let good_delay = Duration::from_millis(50);
        assert_eq!(
            check_serial_delay(good_delay, 38400),
            Duration::from_millis(50)
        );
    }
}

fn main() -> Result<()> {
    let args = commandline::Args::parse();
    let _log_handle = logging_init(args.verbose.log_level_filter());
    let mut delay = args.delay;

    match &args.connection {
        commandline::Connection::Serial {
            device,
            baud_rate,
            address,
            parity_and_stop_bit,
            command,
        } => {
            trace!(
                "Open serial {device} address {address} baud rate {baud_rate} parity and stop bits {parity_and_stop_bit:?}"
            );
            delay = check_serial_delay(delay, *baud_rate);

            if let commandline::Commands::Scan { max_address, force } = command {
                return handle_serial_scan(device, *max_address, *force, args.timeout, delay);
            }

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
                delay,
            )?;
            handle_commands(&mut client, command, delay, args.no_json)?;
        }
        commandline::Connection::Tcp {
            address,
            slave,
            command,
        } => {
            trace!("Open TCP {address} slave {slave}");
            let mut client = AsciiClient::connect_tcp(address, *slave, args.timeout, delay)?;
            handle_commands(&mut client, command, delay, args.no_json)?;
        }
        commandline::Connection::SerialScan {
            device,
            max_address,
            force,
        } => {
            return handle_serial_scan(device, *max_address, *force, args.timeout, delay);
        }
    }

    Ok(())
}
