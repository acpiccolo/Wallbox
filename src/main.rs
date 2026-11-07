use abl_evcc_lib::protocol::*;
use clap::{Parser, Subcommand, ValueEnum};

/// ABL EVCC2/3 Wallbox Modbus-ASCII Protocol CLI Tool
#[derive(Parser, Debug)]
#[command(name = "abl-evcc", version, about = "ABL EVCC2/3 Wallbox Modbus-ASCII Protocol CLI Tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateActionArg {
    /// Reset EVCC controller
    Reset,
    /// Jump to state A1 (Waiting for EV)
    JumpToA1,
    /// Jump to state E0 (Outlet disabled)
    JumpToE0,
    /// Jump to state E2 (EVCC setup mode)
    JumpToE2,
    /// Jump to state F1 (Trigger RCD trip test)
    JumpToF1,
    /// Enable 20A Load Imbalance Detection
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

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Read current & status values of the EVCC outlet (Register 0x002E)
    ReadCurrent {
        /// Modbus Device ID (1..16, default: 1)
        #[arg(short, long, default_value_t = 1)]
        device_id: u8,
        /// Format: 'full' (0.1A resolution, 5 registers) or 'short' (1A resolution, 3 registers)
        #[arg(long, default_value = "full")]
        resolution: String,
    },

    /// Read device ID, firmware revision, hardware and config (Register 0x0001)
    ReadDeviceInfo {
        /// Modbus Device ID (1..16, default: 1)
        #[arg(short, long, default_value_t = 1)]
        device_id: u8,
    },

    /// Read Modbus baud rate, parity, and stop bit settings (Register 0x0003)
    ReadModbusSettings {
        /// Modbus Device ID (1..16, default: 1)
        #[arg(short, long, default_value_t = 1)]
        device_id: u8,
    },

    /// Read system and diagnostic flags (Register 0x0006)
    ReadSystemFlags {
        /// Modbus Device ID (1..16, default: 1)
        #[arg(short, long, default_value_t = 1)]
        device_id: u8,
    },

    /// Set maximum charging current / PWM duty cycle Icmax (Register 0x0014)
    SetCurrent {
        /// Modbus Device ID (1..16, default: 1)
        #[arg(short, long, default_value_t = 1)]
        device_id: u8,
        /// Charging current in Amperes (between 6.0 and 80.0 A) or duty cycle if --duty-cycle is set
        #[arg(value_name = "VALUE")]
        value: f32,
        /// Treat VALUE as duty cycle percentage directly (8.0% - 100.0%) instead of Amperes
        #[arg(long, default_value_t = false)]
        duty_cycle: bool,
    },

    /// Modify outlet state (Register 0x0005)
    SetState {
        /// Modbus Device ID (1..16, default: 1)
        #[arg(short, long, default_value_t = 1)]
        device_id: u8,
        /// Action to perform: reset, jump-to-a1, jump-to-e0, jump-to-e2, jump-to-f1, enable-load-imbalance20-a
        #[arg(value_enum)]
        action: StateActionArg,
    },

    /// Set Device ID and configure power-cycle / timeout behavior (Register 0x002C)
    SetDeviceId {
        /// Current Modbus Device ID (1..16, default: 1)
        #[arg(short, long, default_value_t = 1)]
        device_id: u8,
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

    /// Decode and inspect a raw Modbus ASCII frame string
    DecodeFrame {
        /// ASCII frame string (e.g. ":0103002E0005C9\r\n" or ">01030A2EC3...")
        frame: String,
    },

    /// Low-level raw register read frame generator
    RawRead {
        /// Modbus Device ID (1..16)
        device_id: u8,
        /// Register start address (e.g. 0x002E or 46)
        start_register: String,
        /// Quantity of 16-bit words
        qty: u16,
    },
}

fn parse_u16_hex_or_dec(arg: &str) -> Result<u16, String> {
    if arg.starts_with("0x") || arg.starts_with("0X") {
        u16::from_str_radix(&arg[2..], 16).map_err(|e| e.to_string())
    } else {
        arg.parse::<u16>().map_err(|e| e.to_string())
    }
}

fn build_read_frame(device_id: u8, address: u16, quantity: u16) -> String {
    let mut payload = Vec::with_capacity(4);
    payload.extend_from_slice(&address.to_be_bytes());
    payload.extend_from_slice(&quantity.to_be_bytes());
    encode_ascii_frame(device_id, 0x03, &payload, ':')
}

fn build_write_single_reg_frame(device_id: u8, address: u16, word: u16) -> String {
    let mut payload = Vec::with_capacity(7);
    payload.extend_from_slice(&address.to_be_bytes());
    payload.extend_from_slice(&1u16.to_be_bytes()); // quantity = 1
    payload.push(2); // byte count = 2
    payload.extend_from_slice(&word.to_be_bytes());
    encode_ascii_frame(device_id, 0x10, &payload, ':')
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::ReadCurrent {
            device_id,
            resolution,
        } => {
            let (reg, qty) = if resolution.to_lowercase() == "short" {
                (ReadCurrentAmps::ADDRESS, ReadCurrentAmps::QUANTITY)
            } else {
                (ReadCurrentFull::ADDRESS, ReadCurrentFull::QUANTITY)
            };
            let frame = build_read_frame(device_id, reg, qty);
            print!("{}", frame);
        }

        Commands::ReadDeviceInfo { device_id } => {
            let frame = build_read_frame(
                device_id,
                DeviceIdAndFirmwareRevision::ADDRESS,
                DeviceIdAndFirmwareRevision::QUANTITY,
            );
            print!("{}", frame);
        }

        Commands::ReadModbusSettings { device_id } => {
            let frame = build_read_frame(
                device_id,
                ModbusSettings::ADDRESS,
                ModbusSettings::QUANTITY,
            );
            print!("{}", frame);
        }

        Commands::ReadSystemFlags { device_id } => {
            let frame = build_read_frame(
                device_id,
                SystemFlags::ADDRESS,
                SystemFlags::QUANTITY,
            );
            print!("{}", frame);
        }

        Commands::SetCurrent {
            device_id,
            value,
            duty_cycle,
        } => {
            let duty_cycle_x10 = if duty_cycle {
                (value * 10.0).round() as u16
            } else {
                // According to IEC 61851-1:
                // For 6A <= I <= 31A: Duty cycle = I / 0.6 = I * 10 / 6
                // Duty cycle [%]*10 = (I * 10 / 0.6) = I * 100 / 6
                let dc_val = (value * 100.0 / 6.0).round() as u16;
                dc_val.clamp(80, 1000)
            };

            let icmax = match SetIcmax::new(duty_cycle_x10) {
                Ok(val) => val,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let frame = build_write_single_reg_frame(device_id, SetIcmax::ADDRESS, icmax.encode_to_word());
            print!("{}", frame);
        }

        Commands::SetState { device_id, action } => {
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
            let frame = build_write_single_reg_frame(
                device_id,
                ModifyStateAction::ADDRESS,
                proto_action.encode_to_word(),
            );
            print!("{}", frame);
        }

        Commands::SetDeviceId {
            device_id,
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
            let set_cmd = match SetDeviceId::new(new_device_id, ut, pc) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let frame = build_write_single_reg_frame(
                device_id,
                SetDeviceId::ADDRESS,
                set_cmd.encode_to_word(),
            );
            print!("{}", frame);
        }

        Commands::DecodeFrame { frame } => {
            match decode_ascii_frame(&frame) {
                Ok((dev_id, fc, payload)) => {
                    println!("Decoded Modbus ASCII Frame:");
                    println!("  Device ID: {:#04X} ({})", dev_id, dev_id);
                    println!("  Function Code: {:#04X}", fc);
                    let hex_payload: Vec<String> =
                        payload.iter().map(|b| format!("{:02X}", b)).collect();
                    println!("  Payload: {}", hex_payload.join(""));

                    // Attempt automatic response register decoding if Read response (FC 0x03)
                    if fc == 0x03 && payload.len() >= 2 {
                        let byte_count = payload[0] as usize;
                        if payload.len() >= 1 + byte_count && byte_count >= 2 {
                            let reg_data = &payload[1..1 + byte_count];
                            let words = bytes_to_words(reg_data);
                            let referred_reg = (words[0] >> 8) as u8;
                            println!("  Referred Register: {:#04X}", referred_reg);

                            match referred_reg {
                                0x01 => {
                                    if let Ok(info) = DeviceIdAndFirmwareRevision::decode_from_words(&words) {
                                        println!("  Interpreted: {}", info);
                                    }
                                }
                                0x03 => {
                                    if let Ok(settings) = ModbusSettings::decode_from_words(&words) {
                                        println!("  Interpreted: {}", settings);
                                    }
                                }
                                0x2E => {
                                    if let Ok(cur) = ReadCurrentFull::decode_from_words(&words) {
                                        println!("  Interpreted: {}", cur);
                                    }
                                }
                                0x33 => {
                                    if let Ok(cur) = ReadCurrentAmps::decode_from_words(&words) {
                                        println!("  Interpreted: {}", cur);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Frame decoding error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::RawRead {
            device_id,
            start_register,
            qty,
        } => {
            let reg = match parse_u16_hex_or_dec(&start_register) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Invalid register address: {}", e);
                    std::process::exit(1);
                }
            };
            let frame = build_read_frame(device_id, reg, qty);
            print!("{}", frame);
        }
    }
}
