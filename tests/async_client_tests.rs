#[cfg(all(test, feature = "tokio-async"))]
mod async_client_tests {
    use abl_evcc_lib::{protocol::Evcc2Status, tokio_async::AsyncAsciiClient};
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

        let client = AsyncAsciiClient::connect_tcp(
            &format!("127.0.0.1:{}", port),
            1,
            Duration::from_secs(1),
            Duration::from_millis(10),
        )
        .await
        .unwrap();

        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("AsyncAsciiClient"));
    }

    #[tokio::test]
    async fn test_async_client_from_tcp_and_setters() {
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

        let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .unwrap();
        let mut client = AsyncAsciiClient::from_tcp(
            stream,
            1,
            Duration::from_millis(500),
            Duration::from_millis(20),
        );

        assert_eq!(client.slave_id(), 1);
        assert_eq!(client.delay(), Duration::from_millis(20));
        assert_eq!(client.timeout(), Duration::from_millis(500));

        // Test setters
        client.set_slave_id(2);
        assert_eq!(client.slave_id(), 2);
        client.set_delay(Duration::from_millis(50));
        assert_eq!(client.delay(), Duration::from_millis(50));
        client.set_timeout(Duration::from_secs(1));
        assert_eq!(client.timeout(), Duration::from_secs(1));

        let status = client.read_status().await.unwrap();
        assert_eq!(status, Evcc2Status::WaitingForEv);
    }

    #[tokio::test]
    async fn test_async_ascii_client_tcp() {
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
            socket
                .write_all(b">01030A2EA1110B03E803E803E846\r\n")
                .unwrap();
            socket.flush().unwrap();
        });

        let mut client = AsyncAsciiClient::connect_tcp(
            &format!("127.0.0.1:{}", port),
            1,
            Duration::from_secs(2),
            Duration::from_millis(10),
        )
        .await
        .unwrap();

        let status = client.read_status().await.unwrap();
        assert_eq!(status, Evcc2Status::WaitingForEv);

        let meas = client.read_measurements().await.unwrap();
        assert_eq!(meas.status, Evcc2Status::WaitingForEv);
        assert!((meas.configured_current_amperes - 16.0).abs() < 0.1);
    }
}
