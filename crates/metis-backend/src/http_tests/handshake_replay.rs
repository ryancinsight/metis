//! Real TCP journeys for retry-safe session establishment.
use super::*;

#[test]
fn identical_handshake_replays_bytes_without_issuing_a_new_grant() {
    let _trace = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    );
    let _span = tracing::info_span!("handshake_replay").entered();
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server");
    let address = server.local_addr().expect("address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            let mut records = Vec::new();
            for _ in 0..5 {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                records.push((
                    application.session_count(),
                    application
                        .session
                        .as_ref()
                        .map(|session| (session.next_sequence, session.expires_at)),
                ));
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>(records)
        })
        .expect("real service");
    let body = handshake(PRINCIPAL);
    let first = runtime
        .block_on(request(address, &body, SESSION_ROUTE, ORIGIN))
        .expect("first");
    assert_eq!(status(&first), 200);
    let second = runtime
        .block_on(request(address, &body, SESSION_ROUTE, ORIGIN))
        .expect("retry");
    assert_eq!(status(&second), 200);
    assert_eq!(response_body(&second), response_body(&first));
    let mut different = HandshakeRequestPayload::decode(&body).expect("request");
    different.client_process_id += 1;
    let changed = runtime
        .block_on(request(address, &different.encode(), SESSION_ROUTE, ORIGIN))
        .expect("changed process");
    assert_eq!(status(&changed), 409);
    different.client_process_id -= 1;
    different.client_version += 1;
    let changed = runtime
        .block_on(request(address, &different.encode(), SESSION_ROUTE, ORIGIN))
        .expect("changed version");
    assert_eq!(status(&changed), 409);
    let wrong_origin = runtime
        .block_on(request(
            address,
            &body,
            SESSION_ROUTE,
            "http://127.0.0.1:8081",
        ))
        .expect("origin denial");
    assert_eq!(status(&wrong_origin), 403);
    let records = task.join().expect("join").expect("task").expect("records");
    assert_eq!(records.len(), 5);
    assert_eq!(records[0].0, 1);
    assert_eq!(records[0].1.expect("session").0, 2);
    assert!(records.iter().all(|record| record == &records[0]));
}
