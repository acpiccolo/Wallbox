#[cfg(test)]
mod tests {
    use abl_evcc_lib::protocol::*;

    #[test]
    fn test_lrc_calculation() {
        // Read 2 registers at 0x0001 on device 1:
        // Raw bytes: [0x01, 0x03, 0x00, 0x01, 0x00, 0x02] -> Sum = 0x07 -> wrapping_neg = 0xF9
        let bytes1 = [0x01, 0x03, 0x00, 0x01, 0x00, 0x02];
        assert_eq!(calculate_lrc(&bytes1), 0xF9);

        // Read 5 registers at 0x002E on device 1:
        // Raw bytes: [0x01, 0x03, 0x00, 0x2E, 0x00, 0x05] -> Sum = 0x37 -> wrapping_neg = 0xC9
        let bytes2 = [0x01, 0x03, 0x00, 0x2E, 0x00, 0x05];
        assert_eq!(calculate_lrc(&bytes2), 0xC9);
    }

    #[test]
    fn test_ascii_frame_encode_decode() {
        let payload = [0x00, 0x01, 0x00, 0x02];
        let encoded = encode_ascii_frame(0x01, 0x03, &payload, ':');
        assert_eq!(encoded, ":010300010002F9\r\n");

        let (dev_id, fc, decoded_payload) = decode_ascii_frame(&encoded).unwrap();
        assert_eq!(dev_id, 0x01);
        assert_eq!(fc, 0x03);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn test_evcc2_trace_payload_firmware() {
        // From user trace:
        // Request: :010300010002F9\r\n
        // Reply:   >010304014141363F\r\n
        let req = ":010300010002F9\r\n";
        let (dev_id, fc, req_payload) = decode_ascii_frame(req).unwrap();
        assert_eq!(dev_id, 1);
        assert_eq!(fc, 0x03);
        assert_eq!(req_payload, vec![0x00, 0x01, 0x00, 0x02]);

        let reply = ">010304014141363F\r\n";
        let (dev_id, fc, resp_payload) = decode_ascii_frame(reply).unwrap();
        assert_eq!(dev_id, 1);
        assert_eq!(fc, 0x03);
        assert_eq!(resp_payload[0], 4); // byte count

        let data = &resp_payload[1..];
        let fw = Evcc2Firmware::decode(data).unwrap();
        assert_eq!(fw.device_id, 1);
        assert_eq!(fw.pcba_version, "160307");
        assert_eq!(fw.firmware_revision, "4.1");
        assert!(fw.current_sensor_present);
        assert!(fw.rdc_md_enabled);
        assert!(fw.welding_detection_enabled);
        assert!(fw.outlet_enabled);
    }

    #[test]
    fn test_evcc2_trace_payload_measurements() {
        // From user trace:
        // Request: :0103002E0005C9\r\n
        // Reply:   >01030A2EA1110B03E803E803E846\r\n
        let req = ":0103002E0005C9\r\n";
        let (dev_id, fc, req_payload) = decode_ascii_frame(req).unwrap();
        assert_eq!(dev_id, 1);
        assert_eq!(fc, 0x03);
        assert_eq!(req_payload, vec![0x00, 0x2E, 0x00, 0x05]);

        let reply = ">01030A2EA1110B03E803E803E846\r\n";
        let (dev_id, fc, resp_payload) = decode_ascii_frame(reply).unwrap();
        assert_eq!(dev_id, 1);
        assert_eq!(fc, 0x03);
        assert_eq!(resp_payload[0], 10); // byte count

        let data = &resp_payload[1..];
        let meas = Evcc2Measurements::decode(data).unwrap();
        assert_eq!(meas.status, Evcc2Status::WaitingForEv);
        assert_eq!(meas.status.as_char(), 'A');
        assert_eq!(meas.status.raw_code(), 0xA1);
        assert!(!meas.ev_connected);
        assert!(!meas.duty_cycle_reduced);
        assert!(meas.en1_closed);
        assert!(!meas.en2_closed);
        assert_eq!(meas.duty_cycle_permille, 267);
        assert!((meas.configured_current_amperes - 16.0).abs() < 0.1);
        assert_eq!(meas.current_l1_amperes, 0.0);
        assert_eq!(meas.current_l2_amperes, 0.0);
        assert_eq!(meas.current_l3_amperes, 0.0);
        assert_eq!(meas.total_power_watts, 0.0);
    }

    #[test]
    fn test_evcc2_status_variants() {
        assert_eq!(Evcc2Status::from_raw(0xA1), Evcc2Status::WaitingForEv);
        assert_eq!(Evcc2Status::from_raw(0xB1), Evcc2Status::EvAskingForCharging);
        assert_eq!(Evcc2Status::from_raw(0xB2), Evcc2Status::EvPermissionToCharge);
        assert_eq!(Evcc2Status::from_raw(0xC2), Evcc2Status::Charging);
        assert_eq!(Evcc2Status::from_raw(0xE0), Evcc2Status::OutletDisabled);
        assert_eq!(Evcc2Status::from_raw(0xF1), Evcc2Status::WeldingDetected);

        assert_eq!(Evcc2Status::from_raw(0xA1).as_char(), 'A');
        assert_eq!(Evcc2Status::from_raw(0xB1).as_char(), 'B');
        assert_eq!(Evcc2Status::from_raw(0xB2).as_char(), 'B');
        assert_eq!(Evcc2Status::from_raw(0xC2).as_char(), 'C');
        assert_eq!(Evcc2Status::from_raw(0xE0).as_char(), 'E');
        assert_eq!(Evcc2Status::from_raw(0xF4).as_char(), 'F');
    }

    #[test]
    fn test_amperes_to_evcc2_duty_cycle() {
        assert_eq!(amperes_to_evcc2_duty_cycle(0), 1000);
        assert_eq!(amperes_to_evcc2_duty_cycle(6), 100);
        assert_eq!(amperes_to_evcc2_duty_cycle(16), 267);
        assert_eq!(amperes_to_evcc2_duty_cycle(32), 533);
    }

    #[test]
    fn test_evcc2_modbus_settings_decode() {
        // Register 0x0003: 0327 -> Reg: 0x03, byte 1: 0x27
        // Bits 7..6: 00 (1 stop bit)
        // Bits 5..4: 10 (Even parity)
        // Bits 3..0: 0111 (38400 Bd)
        let settings = Evcc2ModbusSettings::decode(&[0x03, 0x27]).unwrap();
        assert_eq!(settings.baud_rate, 38400);
        assert_eq!(settings.parity, "Even");
        assert_eq!(settings.stop_bits, 1);
    }

    #[test]
    fn test_evcc2_system_flags_decode() {
        // Register 0x0006: 0x06 (reg), 0x04 (pointer), 0x40, 0x10 (raw_flags: 0x4010)
        // bit 14 set: b1_b2_disabled_after_power_cycle
        // bit 4 set: b1_b2_disabled
        let flags = Evcc2SystemFlags::decode(&[0x06, 0x04, 0x40, 0x10]).unwrap();
        assert_eq!(flags.state_pointer, 4);
        assert_eq!(flags.raw_flags, 0x4010);
        assert!(flags.b1_b2_disabled);
        assert!(flags.b1_b2_disabled_after_power_cycle);
        assert!(!flags.upstream_comm_lost);
        assert!(!flags.overtemperature_detected);
    }
}
