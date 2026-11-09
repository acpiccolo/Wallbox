#[cfg(test)]
mod tests {
    use abl_evcc_lib::protocol::*;

    #[test]
    fn test_lrc_calculation() {
        // Read 1 register at 0x0100 (256) on device 1:
        // Raw bytes: [0x01, 0x03, 0x01, 0x00, 0x00, 0x01] -> Sum = 0x06 -> wrapping_neg = 0xFA
        let bytes1 = [0x01, 0x03, 0x01, 0x00, 0x00, 0x01];
        assert_eq!(calculate_lrc(&bytes1), 0xFA);

        // Read 1 register at 0x0101 (257) on device 1:
        // Raw bytes: [0x01, 0x03, 0x01, 0x01, 0x00, 0x01] -> Sum = 0x07 -> wrapping_neg = 0xF9
        let bytes2 = [0x01, 0x03, 0x01, 0x01, 0x00, 0x01];
        assert_eq!(calculate_lrc(&bytes2), 0xF9);
    }

    #[test]
    fn test_ascii_frame_encode_decode() {
        let payload = [0x01, 0x00, 0x00, 0x01];
        let encoded = encode_ascii_frame(0x01, 0x03, &payload, ':');
        assert_eq!(encoded, ":010301000001FA\r\n");

        let (dev_id, fc, decoded_payload) = decode_ascii_frame(&encoded).unwrap();
        assert_eq!(dev_id, 0x01);
        assert_eq!(fc, 0x03);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn test_register_256_vehicle_status_ascii() {
        // 65: 'A'
        let status_a = ChargeStatus::decode_from_words(&[65]).unwrap();
        assert_eq!(status_a.0, VehicleStatus::ReadyA);
        assert_eq!(status_a.0.as_char(), 'A');
        assert_eq!(status_a.0.raw_code(), 65);

        // 66: 'B'
        let status_b = ChargeStatus::decode_from_words(&[66]).unwrap();
        assert_eq!(status_b.0, VehicleStatus::ConnectedB);
        assert_eq!(status_b.0.as_char(), 'B');
        assert_eq!(status_b.0.raw_code(), 66);

        // 67: 'C'
        let status_c = ChargeStatus::decode_from_words(&[67]).unwrap();
        assert_eq!(status_c.0, VehicleStatus::ChargingC);
        assert_eq!(status_c.0.as_char(), 'C');

        // Upper-byte encoded ASCII 'B' (0x4200)
        let status_b_upper = ChargeStatus::decode_from_words(&[0x4200]).unwrap();
        assert_eq!(status_b_upper.0, VehicleStatus::ConnectedB);

        // Other status
        let status_other = ChargeStatus::decode_from_words(&[999]).unwrap();
        assert_eq!(status_other.0, VehicleStatus::Other(999));
    }

    #[test]
    fn test_register_257_charging_current() {
        // 16 Amperes
        let cur16 = ChargingCurrent::new(16).unwrap();
        assert_eq!(cur16.encode_to_word(), 16);
        assert!(cur16.is_enabled());

        // 0 Amperes (Stop / Disabled)
        let cur0 = ChargingCurrent::new(0).unwrap();
        assert_eq!(cur0.encode_to_word(), 0);
        assert!(!cur0.is_enabled());

        // Out of range check (e.g. 5A is below minimum 6A)
        assert!(ChargingCurrent::new(5).is_err());
        assert!(ChargingCurrent::new(81).is_err());

        // Decode from words
        let decoded = ChargingCurrent::decode_from_words(&[32]).unwrap();
        assert_eq!(decoded.0, 32);
        assert!(decoded.is_enabled());
    }

    #[test]
    fn test_register_258_power_or_current() {
        let val = PowerOrCurrent::decode_from_words(&[3680]).unwrap();
        assert_eq!(val.raw(), 3680);
        assert_eq!(val.to_string(), "Wert: 3680");
    }
}
