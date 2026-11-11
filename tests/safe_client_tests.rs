#[cfg(all(test, feature = "safe-client-sync"))]
mod safe_client_tests {
    use abl_evcc_lib::{
        protocol::Evcc2Status,
        tokio_sync::AsciiClient,
        tokio_sync_safe_client::SafeClient,
    };
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    #[test]
    fn test_sync_safe_client() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());

            // 1. Status query
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            socket.write_all(b">01030200A159\r\n").unwrap();
            socket.flush().unwrap();

            // 2. Measurements query
            line.clear();
            reader.read_line(&mut line).unwrap();
            socket.write_all(b">01030A2EA1110B03E803E803E846\r\n").unwrap();
            socket.flush().unwrap();
        });

        let ascii = AsciiClient::connect_tcp(
            &format!("127.0.0.1:{}", port),
            1,
            Duration::from_secs(2),
            Duration::from_millis(10),
        ).unwrap();

        let mut client = SafeClient::new(ascii);

        let status = client.read_status().unwrap();
        assert_eq!(status, Evcc2Status::WaitingForEv);

        let meas = client.read_measurements().unwrap();
        assert_eq!(meas.status, Evcc2Status::WaitingForEv);
        assert!((meas.configured_current_amperes - 16.0).abs() < 0.1);
    }
}
