use evcc_protocol::protocol::*;
use std::env;

fn print_usage() {
    println!("EVCC2/3 Protocol Tool");
    println!("Usage:");
    println!("  evcc-protocol encode-read <device_id> <start_register> <qty>");
    println!("  evcc-protocol encode-write-icmax <device_id> <duty_cycle_x10>");
    println!("  evcc-protocol decode-frame <frame_string>");
    println!("  evcc-protocol parse-read-current <hex_payload>");
    println!();
    println!("Examples:");
    println!("  evcc-protocol encode-read 1 0x002E 5");
    println!("  evcc-protocol encode-write-icmax 1 166");
    println!("  evcc-protocol decode-frame \":0103002E0005C9\\r\\n\"");
    println!("  evcc-protocol parse-read-current \"2EC3810A006400640000\"");
}

fn parse_u16_arg(arg: &str) -> Result<u16, String> {
    if arg.starts_with("0x") || arg.starts_with("0X") {
        u16::from_str_radix(&arg[2..], 16).map_err(|e| e.to_string())
    } else {
        arg.parse::<u16>().map_err(|e| e.to_string())
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "encode-read" => {
            if args.len() < 5 {
                eprintln!("Error: Missing arguments for encode-read");
                print_usage();
                return;
            }
            let dev_id: u8 = args[2].parse().expect("Invalid device_id");
            let reg = parse_u16_arg(&args[3]).expect("Invalid register address");
            let qty = parse_u16_arg(&args[4]).expect("Invalid quantity");

            let mut payload = Vec::new();
            payload.extend_from_slice(&reg.to_be_bytes());
            payload.extend_from_slice(&qty.to_be_bytes());

            let frame = encode_ascii_frame(dev_id, 0x03, &payload, ':');
            print!("{}", frame);
        }
        "encode-write-icmax" => {
            if args.len() < 4 {
                eprintln!("Error: Missing arguments for encode-write-icmax");
                print_usage();
                return;
            }
            let dev_id: u8 = args[2].parse().expect("Invalid device_id");
            let duty_x10: u16 = args[3].parse().expect("Invalid duty cycle x10 value");

            let icmax = match SetIcmax::new(duty_x10) {
                Ok(ic) => ic,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    return;
                }
            };

            let reg = SetIcmax::ADDRESS;
            let qty = SetIcmax::QUANTITY;
            let byte_count = (qty * 2) as u8;

            let mut payload = Vec::new();
            payload.extend_from_slice(&reg.to_be_bytes());
            payload.extend_from_slice(&qty.to_be_bytes());
            payload.push(byte_count);
            payload.extend_from_slice(&icmax.encode_to_word().to_be_bytes());

            let frame = encode_ascii_frame(dev_id, 0x10, &payload, ':');
            print!("{}", frame);
        }
        "decode-frame" => {
            if args.len() < 3 {
                eprintln!("Error: Missing frame argument");
                print_usage();
                return;
            }
            let frame_str = &args[2];
            match decode_ascii_frame(frame_str) {
                Ok((dev_id, fc, payload)) => {
                    println!("Decoded Frame:");
                    println!("  Device ID: {:#04X} ({})", dev_id, dev_id);
                    println!("  Function Code: {:#04X}", fc);
                    let hex_payload: Vec<String> = payload.iter().map(|b| format!("{:02X}", b)).collect();
                    println!("  Payload: {}", hex_payload.join(""));
                }
                Err(e) => eprintln!("Frame decoding failed: {}", e),
            }
        }
        "parse-read-current" => {
            if args.len() < 3 {
                eprintln!("Error: Missing hex payload argument");
                print_usage();
                return;
            }
            let hex_str = &args[2];
            let mut bytes = Vec::new();
            for i in (0..hex_str.len()).step_by(2) {
                if i + 2 <= hex_str.len() {
                    if let Ok(b) = u8::from_str_radix(&hex_str[i..i + 2], 16) {
                        bytes.push(b);
                    }
                }
            }
            let words = bytes_to_words(&bytes);
            match ReadCurrentFull::decode_from_words(&words) {
                Ok(cur) => println!("ReadCurrentFull: {}", cur),
                Err(e) => eprintln!("Decoding error: {}", e),
            }
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
        }
    }
}
