use super::*;

#[test]
fn rejected_peer_does_not_terminate_the_origin_bound_service() {
    let cases: &[(&[u8], ErrorCode)] = &[
        (b"", ErrorCode::FrameTruncated),
        (
            b"POST /v1/session HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\n\r\nno",
            ErrorCode::FrameTruncated,
        ),
        (
            b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n",
            ErrorCode::MalformedPayload,
        ),
    ];
    for &(bytes, expected_error) in cases {
        let runtime = moirai_executor::global();
        let server = runtime
            .block_on(HttpServer::bind("127.0.0.1:0", config()))
            .expect("server bind");
        let address = server.local_addr().expect("server address");
        let task = runtime
            .spawn_async(async move {
                let mut failures = Vec::new();
                serve_browser_http(server, application(), 3, |error| failures.push(error.code))
                    .await?;
                Ok::<_, MetisError>(failures)
            })
            .expect("server task spawn");
        runtime
            .block_on(async {
                let mut peer = TcpStream::connect(&address.to_string()).await?;
                peer.write_all(bytes).await
            })
            .expect("rejected peer write and disconnect");
        let denied = runtime
            .block_on(request_method(
                address,
                "GET",
                &[],
                "/health",
                "http://untrusted.invalid",
            ))
            .expect("unauthorized response");
        assert_eq!(status(&denied), 403);
        let response = runtime
            .block_on(health_request(address))
            .expect("health response");
        assert_eq!(status(&response), 200);
        assert_eq!(response_body(&response), b"metis-http-ready\n");
        let failures = task
            .join()
            .expect("server join")
            .expect("server task")
            .expect("finite server budget");
        assert_eq!(failures, vec![expected_error]);
    }
}

#[test]
fn session_capacity_is_bounded_and_reports_queue_full() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            let mut statuses = Vec::with_capacity(MAX_HTTP_SESSIONS + 1);
            for _ in 0..=MAX_HTTP_SESSIONS {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                statuses.push(response.status());
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>((statuses, application.session_count()))
        })
        .expect("server task spawn");

    for index in 0..=MAX_HTTP_SESSIONS {
        let principal = [u8::try_from(index + 1).expect("principal index"); 16];
        let response = runtime
            .block_on(request(
                address,
                &handshake(principal),
                SESSION_ROUTE,
                ORIGIN,
            ))
            .expect("session response");
        let expected_status = if index < MAX_HTTP_SESSIONS { 200 } else { 503 };
        assert_eq!(status(&response), expected_status);
        if index == MAX_HTTP_SESSIONS {
            let error =
                ErrorResponsePayload::decode(response_body(&response)).expect("typed error");
            assert_eq!(error.error_code, ErrorCode::QueueFull as u16);
        }
    }
    let (statuses, count) = task
        .join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
    assert_eq!(
        statuses,
        vec![200; MAX_HTTP_SESSIONS]
            .into_iter()
            .chain([503])
            .collect::<Vec<_>>()
    );
    assert_eq!(count, MAX_HTTP_SESSIONS);
}

#[test]
fn response_byte_bound_is_enforced_before_write() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind(
            "127.0.0.1:0",
            ServerConfig::new(8, 4096, 16, MAX_PAYLOAD_SIZE, 32, Duration::from_secs(2))
                .expect("response bound"),
        ))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(serve_browser_http(server, application(), 1, |error| {
            panic!("internal response failure must remain terminal: {error}");
        }))
        .expect("server task spawn");
    let client_result = runtime.block_on(async move {
        let mut client = TcpStream::connect(&address.to_string()).await?;
        client
            .write_all(
                format!(
                    "GET {HEALTH_ROUTE} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {ORIGIN}\r\n\r\n"
                )
                .as_bytes(),
            )
            .await?;
        client.flush().await?;
        read_to_close(&mut client).await
    });
    assert_eq!(
        client_result.expect("bounded response connection"),
        Vec::<u8>::new()
    );
    let error = task
        .join()
        .expect("server task join")
        .expect("server task result")
        .expect_err("response over the configured bound must fail");
    assert_eq!(error.code, ErrorCode::MalformedPayload);
}
