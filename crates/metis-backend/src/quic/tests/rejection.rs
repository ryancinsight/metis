use super::*;
use metis_core::protocol::{
    ErrorResponsePayload, HandshakeRequestPayload, HandshakeResponsePayload, PROTOCOL_VERSION,
    build_frame,
};

#[test]
fn wrong_principal_and_replayed_request_retain_protocol_rejections() {
    let _trace = trace();
    let executor = executor();
    let (server, client) = endpoints(&executor);
    let (server_connection, client_connection) = connections(&executor, &server, &client);
    let connection = server_connection.clone();
    let task = executor
        .spawn_async(
            async move {
                serve_quic_session(
                    connection.accept_stream().await.expect("stream"),
                    service(PRINCIPAL, [7; 32]),
                )
                .await
            }
            .instrument(tracing::info_span!("act")),
        )
        .expect("backend");
    moirai_executor::block_on(async {
        let stream = client_connection
            .open_stream()
            .await
            .expect("client stream");
        let mut transport = QuicIpcTransport::new(stream).expect("Metis mapping");
        let mut wire = Vec::new();
        let request = HandshakeRequestPayload {
            client_process_id: 23,
            client_version: PROTOCOL_VERSION,
            principal_id: OTHER_PRINCIPAL,
        };
        build_frame(MessageType::HandshakeReq, 1, &request.encode(), &mut wire)
            .expect("wrong principal frame");
        transport.send_frame(&wire).expect("send wrong principal");
        let (header, response) = transport.recv_message().expect("principal rejection");
        assert_eq!(header.msg_type, MessageType::ErrorResp);
        assert_eq!(header.sequence_id, 1);
        assert_eq!(
            ErrorResponsePayload::decode(&response)
                .expect("error envelope")
                .error_code,
            ErrorCode::InvalidPrincipal as u16
        );
        let request = HandshakeRequestPayload {
            principal_id: PRINCIPAL,
            ..request
        };
        build_frame(MessageType::HandshakeReq, 2, &request.encode(), &mut wire)
            .expect("valid principal frame");
        transport.send_frame(&wire).expect("valid handshake");
        let (header, response) = transport.recv_message().expect("handshake response");
        assert_eq!(header.msg_type, MessageType::HandshakeResp);
        assert_eq!(
            HandshakeResponsePayload::decode(&response)
                .expect("token")
                .initial_token
                .principal_id,
            PRINCIPAL
        );
        transport.send_frame(&wire).expect("replay same sequence");
        assert_eq!(
            transport
                .recv_message()
                .expect_err("replay closes the session")
                .code,
            ErrorCode::ConnectionClosed
        );
    });
    assert_eq!(
        task.join()
            .expect("attached")
            .expect("completed")
            .expect_err("canonical replay rejection")
            .code,
        ErrorCode::ReplayDetected
    );
}

#[test]
fn service_without_trusted_context_rejects_before_reading_application_bytes() {
    let _trace = trace();
    let executor = executor();
    let (server, client) = endpoints(&executor);
    let (server_connection, client_connection) = connections(&executor, &server, &client);
    moirai_executor::block_on(async {
        let mut stream = client_connection
            .open_stream()
            .await
            .expect("client stream");
        // An empty transport message only makes the QUIC stream observable.
        stream.send_message(&[]).await.expect("open stream on wire");
        let peer = server_connection
            .accept_stream()
            .await
            .expect("server stream");
        let untrusted = BackendService::new([7; 32], SafetyEnvelope::default());
        let error = serve_quic_session(peer, untrusted)
            .await
            .expect_err("trusted context required");
        assert_eq!(error.code, ErrorCode::InvalidPrincipal);
        assert!(error.message.contains("trusted host context"));
    });
}

#[test]
fn actual_quic_frames_retain_truncation_checksum_size_and_trailing_byte_errors() {
    use metis_core::protocol::{FrameHeader, MAX_PAYLOAD_SIZE};
    let _trace = trace();
    let mut corrupt = Vec::new();
    build_frame(MessageType::HeartbeatReq, 1, &[7, 37, 99], &mut corrupt).expect("reference frame");
    *corrupt.last_mut().expect("nonempty payload") ^= 1;
    let mut trailing = Vec::new();
    build_frame(MessageType::HeartbeatReq, 1, &[], &mut trailing).expect("complete frame");
    trailing.push(0x37);
    let oversized = FrameHeader {
        msg_type: MessageType::HeartbeatReq,
        sequence_id: 1,
        payload_crc32: 0,
        payload_len: u32::try_from(MAX_PAYLOAD_SIZE + 1).expect("wire size"),
    }
    .encode()
    .to_vec();
    for (wire, expected_code) in [
        (
            vec![0x37; metis_core::protocol::HEADER_SIZE - 1],
            ErrorCode::FrameTruncated,
        ),
        (corrupt, ErrorCode::ChecksumMismatch),
        (oversized, ErrorCode::PayloadTooLarge),
        (trailing, ErrorCode::MalformedPayload),
    ] {
        let executor = executor();
        let (server, client) = endpoints(&executor);
        let (server_connection, client_connection) = connections(&executor, &server, &client);
        let connection = server_connection.clone();
        let task = executor
            .spawn_async(
                async move {
                    serve_quic_session(
                        connection.accept_stream().await.expect("stream"),
                        service(PRINCIPAL, [7; 32]),
                    )
                    .await
                }
                .instrument(tracing::info_span!("act", error = ?expected_code)),
            )
            .expect("backend");
        let mut stream =
            moirai_executor::block_on(client_connection.open_stream()).expect("client stream");
        moirai_executor::block_on(stream.send_message(&wire))
            .expect("deliver actual adversarial wire");
        let error = task
            .join()
            .expect("attached")
            .expect("completed")
            .expect_err("malformed frame rejected");
        assert_eq!(error.code, expected_code);
    }
}
