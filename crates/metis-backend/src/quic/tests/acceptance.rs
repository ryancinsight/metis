use super::*;

#[test]
fn native_client_runs_real_handshake_clinical_results_capabilities_and_events() {
    let _trace = trace();
    let arrange = tracing::info_span!("arrange").entered();
    let executor = executor();
    let (server, client) = endpoints(&executor);
    let (server_connection, client_connection) = connections(&executor, &server, &client);
    let connection = server_connection.clone();
    let task = executor
        .spawn_async(
            async move {
                serve_quic_session(
                    connection.accept_stream().await.expect("session stream"),
                    service(PRINCIPAL, [7; 32]),
                )
                .await
            }
            .instrument(tracing::info_span!("act", role = "backend")),
        )
        .expect("backend task");
    let stream = moirai_executor::block_on(client_connection.open_stream()).expect("client stream");
    let mut client = IpcClient::new(QuicIpcTransport::new(stream).expect("Metis transport"));
    drop(arrange);
    let token = client
        .handshake(23, PRINCIPAL)
        .expect("real authorized handshake");
    assert_eq!(token.principal_id, PRINCIPAL);
    assert_ne!(token.signature, [0; 32]);
    let target = client
        .discover_target_capabilities()
        .expect("native capabilities");
    assert!(target.supports(TargetCapability::NativeQuic));
    for weight in [72.5, 36.25] {
        let request = calculation(token.clone(), weight)
            .encode()
            .expect("clinical request");
        let (kind, response) = client
            .send_and_recv(MessageType::ClinicalCalcReq, &request)
            .expect("clinical reply");
        assert_eq!(kind, MessageType::ClinicalCalcResp);
        let result =
            ClinicalCalcResponsePayload::decode(&response).expect("computed clinical result");
        // Dimensional oracle: (0.5 mcg/kg/min * W kg * 60 min/hr) /
        // (4 mg/ml * 1000 mcg/mg). This is the existing native-precision law.
        let expected_rate = 0.5 * weight * 60.0 / (4.0 * 1000.0);
        assert_eq!(result.rate_ml_hr.to_bits(), expected_rate.to_bits());
        assert_eq!(
            result.drug_rate_mg_hr.to_bits(),
            (expected_rate * 4.0).to_bits()
        );
        let event = client.recv_event().expect("backend clinical event");
        assert_eq!(event.name(), "clinical.result");
        assert_eq!(
            event
                .decode_as::<ClinicalCalcResponsePayload>()
                .expect("typed computed event"),
            result
        );
    }
    client.into_transport().close().expect("client session FIN");
    let _assert = tracing::info_span!("assert").entered();
    assert_eq!(
        task.join()
            .expect("backend attached")
            .expect("backend task completed"),
        Ok(())
    );
}

#[test]
fn two_sessions_share_udp_connection_with_independent_tokens_and_sequences() {
    let _trace = trace();
    let executor = executor();
    let (server, client) = endpoints(&executor);
    let (server_connection, client_connection) = connections(&executor, &server, &client);
    let connection = server_connection.clone();
    let first = executor
        .spawn_async(
            async move {
                serve_quic_session(
                    connection.accept_stream().await.expect("first stream"),
                    service(PRINCIPAL, [7; 32]),
                )
                .await
            }
            .instrument(tracing::info_span!("act", session = "first")),
        )
        .expect("first backend");
    let stream =
        moirai_executor::block_on(client_connection.open_stream()).expect("first client stream");
    let mut first_client = IpcClient::new(QuicIpcTransport::new(stream).expect("first transport"));
    let first_token = first_client
        .handshake(23, PRINCIPAL)
        .expect("first handshake");
    // The first session remains idle while a sibling executes a calculation.
    let connection = server_connection.clone();
    let second = executor
        .spawn_async(
            async move {
                serve_quic_session(
                    connection.accept_stream().await.expect("second stream"),
                    service(PRINCIPAL, [8; 32]),
                )
                .await
            }
            .instrument(tracing::info_span!("act", session = "second")),
        )
        .expect("second backend");
    let stream =
        moirai_executor::block_on(client_connection.open_stream()).expect("second client stream");
    let mut second_client =
        IpcClient::new(QuicIpcTransport::new(stream).expect("second transport"));
    let second_token = second_client
        .handshake(29, PRINCIPAL)
        .expect("second handshake");
    assert_eq!(first_token.principal_id, PRINCIPAL);
    assert_eq!(second_token.principal_id, PRINCIPAL);
    assert_ne!(first_token.signature, second_token.signature);
    let foreign_request = calculation(first_token, 72.5)
        .encode()
        .expect("foreign-session token request");
    let (kind, rejection) = second_client
        .send_and_recv(MessageType::ClinicalCalcReq, &foreign_request)
        .expect("foreign token rejection");
    assert_eq!(kind, MessageType::ErrorResp);
    assert_eq!(
        metis_core::protocol::ErrorResponsePayload::decode(&rejection)
            .expect("typed rejection")
            .error_code,
        metis_core::ErrorCode::InvalidPrincipal as u16
    );
    let request = calculation(second_token, 72.5).encode().expect("request");
    let (kind, response) = second_client
        .send_and_recv(MessageType::ClinicalCalcReq, &request)
        .expect("sibling computation");
    assert_eq!(kind, MessageType::ClinicalCalcResp);
    assert_eq!(
        ClinicalCalcResponsePayload::decode(&response)
            .expect("result")
            .rate_ml_hr
            .to_bits(),
        0.54375_f64.to_bits()
    );
    first_client.into_transport().close().expect("first FIN");
    second_client
        .recv_event()
        .expect("consume final sibling event");
    second_client.into_transport().close().expect("second FIN");
    assert_eq!(
        first
            .join()
            .expect("first attached")
            .expect("first complete"),
        Ok(())
    );
    assert_eq!(
        second
            .join()
            .expect("second attached")
            .expect("second complete"),
        Ok(())
    );
}
