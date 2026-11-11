#[cfg(all(test, feature = "safe-client-async"))]
mod async_safe_client_tests {
    use abl_evcc_lib::{
        protocol::Evcc2Status,
        tokio_async_safe_client::SafeClient,
    };
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    #[tokio::test]
    async fn test_async_client_debug() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let _server = thread::spawn(move || {
            let _ = listener.accept();
        });

        let client = abl_evcc_lib::tokio_async::AsyncAsciiClient::connect_tcp(
            &format!("127.0.0.1:{}", port),
            1,
            Duration::from_millis(10),
        ).await.unwrap();

        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("AsyncAsciiClient"));
    }

    #[tokio::test]
    async fn test_async_safe_client_tcp() {
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

        let mut client = SafeClient::connect_tcp(
            &format!("127.0.0.1:{}", port),
            1,
            Duration::from_millis(10),
        ).await.unwrap();

        let status = client.read_status().await.unwrap();
        assert_eq!(status, Evcc2Status::WaitingForEv);

        let meas = client.read_measurements().await.unwrap();
        assert_eq!(meas.status, Evcc2Status::WaitingForEv);
        assert!((meas.configured_current_amperes - 16.0).abs() < 0.1);
    }
}
