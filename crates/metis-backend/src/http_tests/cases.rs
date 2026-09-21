use super::*;

#[test]
fn loopback_handshake_and_fragment_response_are_typed() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            for _ in 0..2 {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>(application.session_count())
        })
        .expect("server task spawn");

    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode();
    let response = runtime
        .block_on(request(address, &handshake, SESSION_ROUTE, ORIGIN))
        .expect("handshake response");
    assert_eq!(status(&response), 200);
    let token = HandshakeResponsePayload::decode(response_body(&response))
        .expect("typed handshake")
        .initial_token;
    let action =
        FragmentAction::new(7, "status.describe", "metis-events", "session").expect("action");
    let invocation = PluginInvocationPayload::new(
        token,
        "ui",
        "action",
        action.encode().expect("action bytes"),
    )
    .expect("invocation")
    .encode()
    .expect("invocation bytes");
    let response = runtime
        .block_on(request(address, &invocation, FRAGMENT_ROUTE, ORIGIN))
        .expect("fragment response");
    assert_eq!(status(&response), 200);
    let patches = FragmentPatchSet::decode(response_body(&response)).expect("typed patches");
    assert_eq!(patches.generation(), 7);
    assert_eq!(patches.patches().len(), 1);
    assert_eq!(
        patches.patches()[0].text(),
        Some("Fragment action status.describe accepted: session")
    );
    assert_eq!(
        task.join()
            .expect("server task join")
            .expect("server task result")
            .expect("server response"),
        1
    );
}

#[test]
fn origin_and_route_policy_fail_with_typed_errors() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            for _ in 0..2 {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>(())
        })
        .expect("server task spawn");
    let response = runtime
        .block_on(request(address, &[], SESSION_ROUTE, "http://evil.test"))
        .expect("denial response");
    assert_eq!(status(&response), 403);
    let error = ErrorResponsePayload::decode(response_body(&response)).expect("typed denial");
    assert_eq!(error.error_code, ErrorCode::NavigationDenied as u16);
    let response = runtime
        .block_on(request(address, &[], "/v1/unknown", ORIGIN))
        .expect("route response");
    assert_eq!(status(&response), 404);
    let error = ErrorResponsePayload::decode(response_body(&response)).expect("typed route error");
    assert_eq!(error.error_code, ErrorCode::MalformedPayload as u16);
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
}

#[test]
fn method_and_missing_session_policy_fail_with_typed_errors() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            for _ in 0..2 {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>(())
        })
        .expect("server task spawn");

    let action =
        FragmentAction::new(1, "status.describe", "metis-events", "missing").expect("action");
    let token = CapabilityToken::issue(
        1,
        [0x77; 16],
        CapabilityScope::UI_RENDER,
        1,
        3_600,
        1,
        &[9; 32],
    );
    let invocation = PluginInvocationPayload::new(
        token,
        "ui",
        "action",
        action.encode().expect("action bytes"),
    )
    .expect("invocation")
    .encode()
    .expect("invocation bytes");
    let response = runtime
        .block_on(request(address, &invocation, FRAGMENT_ROUTE, ORIGIN))
        .expect("missing-session response");
    assert_eq!(status(&response), 401);
    let error = ErrorResponsePayload::decode(response_body(&response)).expect("typed error");
    assert_eq!(error.error_code, ErrorCode::MissingCapability as u16);

    let response = runtime
        .block_on(request_method(address, "GET", &[], SESSION_ROUTE, ORIGIN))
        .expect("method response");
    assert_eq!(status(&response), 405);
    let error = ErrorResponsePayload::decode(response_body(&response)).expect("typed error");
    assert_eq!(error.error_code, ErrorCode::UnexpectedMessageType as u16);
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
}

#[test]
fn preflight_returns_the_exact_origin_policy() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            let connection = server.accept().await?;
            let (request, connection) = connection.read_request().await?;
            let response = application.respond(&request)?;
            connection.write_response(response).await
        })
        .expect("server task spawn");
    let response = runtime
        .block_on(preflight_request(address))
        .expect("preflight response");
    assert_eq!(status(&response), 204);
    assert_eq!(response_body(&response), b"");
    assert_eq!(
        header(&response, "Access-Control-Allow-Origin"),
        Some(ORIGIN)
    );
    assert_eq!(header(&response, "Vary"), Some("Origin"));
    assert_eq!(
        header(&response, "Access-Control-Allow-Methods"),
        Some("GET, POST, OPTIONS")
    );
    assert_eq!(
        header(&response, "Access-Control-Allow-Headers"),
        Some("content-type")
    );
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
}

#[test]
fn malformed_and_oversized_requests_are_rejected_before_dispatch() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind(
            "127.0.0.1:0",
            ServerConfig::new(8, 4096, 16, 8, 256, Duration::from_secs(2)).expect("bounds"),
        ))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut failures = Vec::new();
            serve_browser_http(server, application(), 2, |error| failures.push(error.code)).await?;
            Ok::<_, MetisError>(failures)
        })
        .expect("server task spawn");
    runtime
        .block_on(async move {
            let mut client = TcpStream::connect(&address.to_string()).await?;
            client
                .write_all(b"POST /v1/session HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: ")
                .await?;
            client.write_all(ORIGIN.as_bytes()).await?;
            client
                .write_all(b"\r\nContent-Length: 9\r\n\r\n123456789")
                .await
        })
        .expect("oversized request write");
    let response = runtime
        .block_on(health_request(address))
        .expect("health after oversized peer");
    assert_eq!(status(&response), 200);
    assert_eq!(response_body(&response), b"metis-http-ready\n");
    let result = task
        .join()
        .expect("oversized task join")
        .expect("oversized task result");
    let failures = result.expect("oversized peer must not terminate listener");
    assert_eq!(failures, vec![ErrorCode::MalformedPayload]);
}

#[test]
fn idle_peer_hits_deadline_and_teardown_is_finite() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind(
            "127.0.0.1:0",
            ServerConfig::new(8, 4096, 16, 64, 256, Duration::from_millis(50)).expect("deadline"),
        ))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let (failure_sender, failure_receiver) = std::sync::mpsc::sync_channel(2);
    let task = runtime
        .spawn_async(async move {
            let mut failures = Vec::new();
            serve_browser_http(server, application(), 2, |error| {
                failure_sender.send(error.code).expect("failure observer");
                failures.push(error.code);
            })
            .await?;
            Ok::<_, MetisError>(failures)
        })
        .expect("server task spawn");
    let client = runtime
        .block_on(TcpStream::connect(&address.to_string()))
        .expect("idle peer connect");
    let (response_sender, response_receiver) = std::sync::mpsc::sync_channel(1);
    let health_task = std::thread::spawn(move || {
        let response = moirai_executor::block_on(health_request(address));
        response_sender.send(response).expect("health observer");
    });
    // A timer on the request future could conceal a lost readiness wake by
    // polling an already-readable socket when the timer fires.
    let response = response_receiver
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_else(|error| {
            panic!(
                "health request exceeded the ordinary HTTP deadline: {error}; observed peer failures: {:?}",
                failure_receiver.try_iter().collect::<Vec<_>>()
            )
        })
        .expect("health after idle peer");
    health_task.join().expect("health task join");
    assert_eq!(status(&response), 200);
    assert_eq!(response_body(&response), b"metis-http-ready\n");
    let result = task
        .join()
        .expect("deadline task join")
        .expect("deadline task result");
    let failures = result.expect("idle peer must not terminate listener");
    drop(client);
    assert_eq!(failures, vec![ErrorCode::Timeout]);
}

#[test]
fn disconnected_peer_returns_a_typed_read_failure() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let connection = server.accept().await?;
            connection.read_request().await.map(|_| ())
        })
        .expect("server task spawn");
    runtime
        .block_on(async move {
            let mut client = TcpStream::connect(&address.to_string()).await?;
            client
                .write_all(
                    b"POST /v1/session HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 4\r\n\r\nno",
                )
                .await
        })
        .expect("partial request write");
    let result = task
        .join()
        .expect("disconnect task join")
        .expect("disconnect task result");
    let error = result.expect_err("disconnected peer must fail the request");
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
fn public_server_budget_closes_after_a_health_probe() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(serve_browser_http(server, application(), 1, |error| {
            panic!("unexpected peer failure: {error}");
        }))
        .expect("server task spawn");
    let response = runtime
        .block_on(health_request(address))
        .expect("health response");
    assert_eq!(status(&response), 200);
    assert_eq!(response_body(&response), b"metis-http-ready\n");
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
}

#[test]
fn response_delay_probe_rejects_unbounded_values() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let error = runtime
        .block_on(serve_browser_http_with_response_delay(
            server,
            application(),
            1,
            Some(MAX_HTTP_RESPONSE_DELAY + Duration::from_nanos(1)),
            |error| panic!("unexpected peer failure: {error}"),
        ))
        .expect_err("response delay above the probe bound must fail");
    assert_eq!(error.code, ErrorCode::Timeout);
}

#[path = "../http_tests_extra.rs"]
mod extra;
