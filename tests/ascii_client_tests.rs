#[cfg(all(test, feature = "tokio-sync"))]
mod ascii_client_mock_tests {
    use abl_evcc_lib::{
        protocol::Evcc2Status,
        tokio_sync::AsciiClient,
    };
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    #[test]
    fn test_mock_tcp_ascii_client() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        // Spawn mock EVCC2 server replying with real traces
        thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());

            // 1. Expect status request: :010300040001F7\r\n
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.contains("010300040001"));
            // Reply status 0xA1: >01030200A159\r\n
            // Bytes: 01, 03, 02, 00, A1 -> sum = 0xA7 -> neg = 0x59
            socket.write_all(b">01030200A159\r\n").unwrap();
            socket.flush().unwrap();

            // 2. Expect measurements request: :0103002E0005C9\r\n
            line.clear();
            reader.read_line(&mut line).unwrap();
            assert!(line.contains("0103002E0005"));
            socket.write_all(b">01030A2EA1110B03E803E803E846\r\n").unwrap();
            socket.flush().unwrap();
        });

        let mut client = AsciiClient::connect_tcp(
            &format!("127.0.0.1:{}", port),
            1,
            Duration::from_secs(2),
            Duration::from_millis(10),
        ).unwrap();

        let status = client.read_status().unwrap();
        assert_eq!(status, Evcc2Status::WaitingForEv);
        assert_eq!(status.as_char(), 'A');

        let meas = client.read_measurements().unwrap();
        assert_eq!(meas.status, Evcc2Status::WaitingForEv);
        assert_eq!(meas.duty_cycle_permille, 267);
        assert!((meas.configured_current_amperes - 16.0).abs() < 0.1);
        assert_eq!(meas.current_l1_amperes, 0.0);
    }
}
