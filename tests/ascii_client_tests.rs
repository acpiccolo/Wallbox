#[cfg(all(test, feature = "tokio-sync"))]
mod ascii_client_mock_tests {
    use abl_evcc_lib::{protocol::Evcc2Status, tokio_sync::AsciiClient};
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    #[test]
    fn test_ascii_client_debug() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let _server = thread::spawn(move || {
            let _ = listener.accept();
        });

        let client = AsciiClient::connect_tcp(
            &format!("127.0.0.1:{}", port),
            1,
            Duration::from_secs(1),
            Duration::from_millis(10),
        )
        .unwrap();

        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("AsciiClient"));
    }

    #[test]
    fn test_ascii_client_from_tcp_and_setters() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let _server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut line = String::new();
            let _ = reader.read_line(&mut line);
            let _ = socket.write_all(b">02030200A158\r\n");
            let _ = socket.flush();
        });

        let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_millis(500)))
            .unwrap();

        let mut client = AsciiClient::from_tcp(stream, 1, Duration::from_millis(20)).unwrap();

        assert_eq!(client.slave_id(), 1);
        assert_eq!(client.delay(), Duration::from_millis(20));
        // Automatically detected from stream
        assert_eq!(client.timeout(), Duration::from_millis(500));

        // Test setters
        client.set_slave_id(2);
        assert_eq!(client.slave_id(), 2);
        client.set_delay(Duration::from_millis(50));
        assert_eq!(client.delay(), Duration::from_millis(50));
        client.set_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(client.timeout(), Duration::from_secs(1));

        let status = client.read_status().unwrap();
        assert_eq!(status, Evcc2Status::WaitingForEv);
    }

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
            socket
                .write_all(b">01030A2EA1110B03E803E803E846\r\n")
                .unwrap();
            socket.flush().unwrap();
        });

        let mut client = AsciiClient::connect_tcp(
            &format!("127.0.0.1:{}", port),
            1,
            Duration::from_secs(2),
            Duration::from_millis(10),
        )
        .unwrap();

        let status = client.read_status().unwrap();
        assert_eq!(status, Evcc2Status::WaitingForEv);
        assert_eq!(status.as_char(), 'A');

        let meas = client.read_measurements().unwrap();
        assert_eq!(meas.status, Evcc2Status::WaitingForEv);
        assert_eq!(meas.duty_cycle_permille, 267);
        assert!((meas.configured_current_amperes - 16.0).abs() < 0.1);
        assert_eq!(meas.current_l1_amperes, 0.0);
    }

    #[test]
    fn test_mock_tcp_scan_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        // Server ignores ID 1, but responds on ID 2
        thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());

            // 1. Request for ID 1 firmware fails (server responds with empty or ignores)
            // But let us reply to ID 2
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 {
                if line.contains(":020300010002") {
                    // Response for ID 2, FC 03, 4 bytes payload
                    // Bytes: 02, 03, 04, 01, 01, 11, 16 -> sum = 0x32 -> neg = 0xCE
                    let _ = socket.write_all(b">02030401011116CE\r\n");
                    let _ = socket.flush();
                    break;
                }
                line.clear();
            }
        });

        let mut client = AsciiClient::connect_tcp(
            &format!("127.0.0.1:{}", port),
            1,
            Duration::from_millis(200),
            Duration::from_millis(10),
        )
        .unwrap();

        // Switch to slave ID 2 and query
        client.set_slave_id(2);
        let fw = client.read_firmware().unwrap();
        assert_eq!(fw.firmware_revision(), "1.1");
    }
}
