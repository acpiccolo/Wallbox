#[cfg(test)]
mod tests {
    use evcc_protocol::protocol::*;

    #[test]
    fn test_lrc_calculation_spec_examples() {
        // Spec Example 1: SBC -> pcba :010300010002F9 CRLF
        // Raw bytes: 0x01, 0x03, 0x00, 0x01, 0x00, 0x02. Sum = 0x07. Wrapping neg = 0xF9.
        let bytes1 = [0x01, 0x03, 0x00, 0x01, 0x00, 0x02];
        assert_eq!(calculate_lrc(&bytes1), 0xF9);

        // Spec Example 2: SBC -> pcba :0103002E0005C9
        // Raw bytes: 0x01, 0x03, 0x00, 0x2E, 0x00, 0x05. Sum = 0x37. Wrapping neg = 0xC9.
        let bytes2 = [0x01, 0x03, 0x00, 0x2E, 0x00, 0x05];
        assert_eq!(calculate_lrc(&bytes2), 0xC9);

        // Spec Example 3: SBC -> pcba :010300330003C6
        // Raw bytes: 0x01, 0x03, 0x00, 0x33, 0x00, 0x03. Sum = 0x3A. Wrapping neg = 0xC6.
        let bytes3 = [0x01, 0x03, 0x00, 0x33, 0x00, 0x03];
        assert_eq!(calculate_lrc(&bytes3), 0xC6);

        // Spec Example 4: pcba -> SBC >011000140001DA
        // Raw bytes: 0x01, 0x10, 0x00, 0x14, 0x00, 0x01. Sum = 0x26. Wrapping neg = 0xDA.
        let bytes4 = [0x01, 0x10, 0x00, 0x14, 0x00, 0x01];
        assert_eq!(calculate_lrc(&bytes4), 0xDA);
    }

    #[test]
    fn test_ascii_frame_encode_decode() {
        let payload = [0x00, 0x2E, 0x00, 0x05];
        let encoded = encode_ascii_frame(0x01, 0x03, &payload, ':');
        assert_eq!(encoded, ":0103002E0005C9\r\n");

        let (dev_id, fc, decoded_payload) = decode_ascii_frame(&encoded).unwrap();
        assert_eq!(dev_id, 0x01);
        assert_eq!(fc, 0x03);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn test_register_0x0001_device_id_firmware() {
        // Example response: >01030401011237AD V1.2
        // Value of reg 0x0001 = 0x0101, reg 0x0002 = 0x1237
        let words = [0x0101, 0x1237];
        let info = DeviceIdAndFirmwareRevision::decode_from_words(&words).unwrap();
        assert_eq!(info.referred_register, 0x01);
        assert_eq!(info.hardware, PcbaHardware::Pcba141215);
        assert_eq!(info.device_id, 0x01);
        assert_eq!(info.major_revision, 0x01);
        assert_eq!(info.minor_revision, 0x02);
        assert!(!info.cs_reading_enabled);
        assert!(!info.upstream_timeout_enabled);
        assert!(info.internal_phase_current_meter_enabled);
        assert!(info.internal_rdc_md_enabled);
        assert!(!info.socket_enabled);
        assert!(info.welding_detection_enabled);
        assert_eq!(info.contact_state, ContactState::Invalid);
    }

    #[test]
    fn test_register_0x0003_modbus_settings() {
        // Value: 0x0327 -> referred_register=0x03, stop_bits=0b00 (1 stop bit), parity=0b10 (even), baud=0b0111 (38400 Bd)
        let words = [0x0327];
        let settings = ModbusSettings::decode_from_words(&words).unwrap();
        assert_eq!(settings.referred_register, 0x03);
        assert_eq!(settings.stop_bits, StopBits::OneStopBit);
        assert_eq!(settings.parity, Parity::Even);
        assert_eq!(settings.baud_rate, ModbusBaudRate::B38400);
    }

    #[test]
    fn test_register_0x0005_modify_state() {
        let action = ModifyStateAction::Reset;
        assert_eq!(action.encode_to_word(), 0x5A5A);
        assert_eq!(ModifyStateAction::decode_from_word(0x5A5A), ModifyStateAction::Reset);
    }

    #[test]
    fn test_register_0x0014_set_icmax() {
        let icmax = SetIcmax::new(166).unwrap(); // 16.6%
        assert_eq!(icmax.encode_to_word(), 166);
        assert!(SetIcmax::new(50).is_err()); // out of range (< 80)
    }

    #[test]
    fn test_register_0x002c_set_device_id() {
        let set_dev = SetDeviceId::new(0x05, UpstreamTimeoutOption::Enabled, PowerCycleB1B2Option::EnterB2IfConnected).unwrap();
        let word = set_dev.encode_to_word();
        let decoded = SetDeviceId::decode_from_word(word).unwrap();
        assert_eq!(decoded.device_id, 0x05);
        assert_eq!(decoded.upstream_timeout, UpstreamTimeoutOption::Enabled);
        assert_eq!(decoded.power_cycle_b1_b2, PowerCycleB1B2Option::EnterB2IfConnected);
    }

    #[test]
    fn test_register_0x002e_read_current_full() {
        // Example: pcba->SBC >01030A2EC3810A0064006400001F
        // payload = 2E C3 81 0A 00 64 00 64 00 00
        // words = [0x2EC3, 0x810A, 0x0064, 0x0064, 0x0000]
        let words = [0x2EC3, 0x810A, 0x0064, 0x0064, 0x0000];
        let cur = ReadCurrentFull::decode_from_words(&words).unwrap();
        assert_eq!(cur.referred_register, 0x2E);
        assert_eq!(cur.evse_state, 0xC3); // State C2
        assert!(cur.ucp_le_10v);
        assert_eq!(cur.duty_cycle_ic_x10, 0x010A); // 266 -> 26.6%
        assert_eq!(cur.ict1_x10_decia, 100); // 10.0 A
        assert_eq!(cur.ict2_x10_decia, 100); // 10.0 A
        assert_eq!(cur.ict3_x10_decia, 0);   // 0.0 A
    }

    #[test]
    fn test_register_0x0033_read_current_amps() {
        // Example: pcba->SBC >0103063380C30A0A00EC
        // payload = 33 80 C3 0A 0A 00
        // words = [0x3380, 0xC30A, 0x0A00]
        let words = [0x3380, 0xC30A, 0x0A00];
        let cur = ReadCurrentAmps::decode_from_words(&words).unwrap();
        assert_eq!(cur.referred_register, 0x33);
        assert!(cur.ucp_le_10v);
        assert_eq!(cur.evse_state, 0xC3);
        assert_eq!(cur.ict1_amps, 10);
        assert_eq!(cur.ict2_amps, 10);
        assert_eq!(cur.ict3_amps, 0);
    }
}
