use super::*;

#[test]
fn wire_failures_are_audited_and_clean_eof_is_not() {
    let (mut service, _) = service();
    let (mut peer, transport) = MemoryTransport::pair();
    peer.send_frame(b"MET").expect("partial header");
    let mut server = IpcServer::new(transport);
    assert_eq!(
        server.step(&mut service).expect_err("truncation").code,
        ErrorCode::FrameTruncated
    );
    let event = service.ledger().records().back().expect("audit");
    assert_eq!(event.event, AuditEvent::Failure(FailureContext::Receive));
    assert_eq!(event.outcome, Some(ErrorCode::FrameTruncated));
    assert_eq!(event.timestamp_millis, 1_700_000_000_000);
    drop(peer);
    assert_eq!(server.step(&mut service), Ok(false));
    assert_eq!(service.ledger().records().len(), 1);
    assert_eq!(service.ledger().verify_chain(), Ok(()));
}

#[test]
fn application_rejections_and_server_replay_have_distinct_single_records() {
    let (mut service, _) = service();
    let (mut peer, transport) = MemoryTransport::pair();
    let mut server = IpcServer::new(transport);
    peer.send_message(MessageType::HandshakeReq, 1, b"bad")
        .expect("malformed payload");
    assert_eq!(server.step(&mut service), Ok(true));
    let (response, bytes) = peer.recv_message().expect("response");
    assert_eq!(response.msg_type, MessageType::ErrorResp);
    assert_eq!(
        ErrorResponsePayload::decode(&bytes)
            .expect("payload")
            .error_code,
        ErrorCode::MalformedPayload as u16
    );
    assert_eq!(service.ledger().records().len(), 1);
    let identity = RequestIdentity {
        message_type: MessageType::HandshakeReq,
        sequence: 1,
    };
    assert_eq!(
        service.ledger().records().back().expect("audit").event,
        AuditEvent::Processed(identity)
    );
    peer.send_message(MessageType::HandshakeReq, 1, b"bad")
        .expect("replay");
    assert_eq!(
        server.step(&mut service).expect_err("replay").code,
        ErrorCode::ReplayDetected
    );
    assert_eq!(service.ledger().records().len(), 2);
    let event = service.ledger().records().back().expect("audit");
    assert_eq!(
        event.event,
        AuditEvent::Failure(FailureContext::Request(identity))
    );
    assert_eq!(event.outcome, Some(ErrorCode::ReplayDetected));
    assert_eq!(service.ledger().verify_chain(), Ok(()));
}

#[test]
fn undelivered_response_records_processing_then_delivery_failure() {
    let (mut service, _) = service();
    let (mut peer, transport) = MemoryTransport::pair();
    peer.send_message(MessageType::HeartbeatReq, 1, b"")
        .expect("request");
    drop(peer);
    let mut server = IpcServer::new(transport);
    assert_eq!(
        server.step(&mut service).expect_err("write closed").code,
        ErrorCode::ConnectionClosed
    );
    let identity = RequestIdentity {
        message_type: MessageType::HeartbeatReq,
        sequence: 1,
    };
    assert_eq!(
        service
            .ledger()
            .records()
            .iter()
            .map(|record| (record.event, record.outcome))
            .collect::<Vec<_>>(),
        vec![
            (AuditEvent::Processed(identity), None),
            (
                AuditEvent::Failure(FailureContext::Response(identity)),
                Some(ErrorCode::ConnectionClosed)
            )
        ]
    );
    assert_eq!(service.ledger().verify_chain(), Ok(()));
}
