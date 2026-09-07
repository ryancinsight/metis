//! IPC contract regression tests.
#[path = "contract/client.rs"]
mod client;
#[path = "contract/payload.rs"]
mod payload;
use metis_core::capability::{CapabilityScope, CapabilityToken};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    ClinicalCalcResponsePayload, FrameHeader, HandshakeResponsePayload, MessageType,
    PROTOCOL_VERSION, RemoteEventPayload, build_frame,
};
use metis_ipc::client::HandshakeError;
use metis_ipc::server::{FailureContext, RequestIdentity};
use metis_ipc::{
    FaultConfig, FaultInjectingTransport, IpcClient, IpcHandler, IpcServer, IpcTransport,
    MemoryTransport, read_frame,
};
use std::io::{Error, ErrorKind, Read};
use std::time::Duration;

#[test]
fn every_partial_frame_reports_truncation() {
    let wire = build_frame(MessageType::HeartbeatReq, 1, b"heart beat").expect("bounded frame");
    for length in 0..wire.len() {
        let error = read_frame(&mut &wire[..length]).expect_err("partial frame");
        assert_eq!(
            error.code,
            if length == 0 {
                ErrorCode::ConnectionClosed
            } else {
                ErrorCode::FrameTruncated
            }
        );
    }
    let (header, payload) = read_frame(&mut wire.as_slice()).expect("complete frame");
    assert_eq!(header.sequence_id, 1);
    assert_eq!(payload, b"heart beat");
}

#[test]
fn fragmented_reads_preserve_frame_bytes() {
    struct Fragmented<'a>(&'a [u8]);
    impl Read for Fragmented<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let count = buf.len().min(1);
            self.0.read(&mut buf[..count])
        }
    }
    let wire = build_frame(MessageType::HeartbeatReq, 7, b"1234").expect("frame");
    let (header, payload) = read_frame(&mut Fragmented(&wire)).expect("fragmented frame");
    assert_eq!(header.sequence_id, 7);
    assert_eq!(payload, b"1234");
}

#[test]
fn read_failures_keep_transport_classification() {
    struct Failing(ErrorKind);
    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(Error::from(self.0))
        }
    }
    for (kind, expected) in [
        (ErrorKind::TimedOut, ErrorCode::Timeout),
        (ErrorKind::BrokenPipe, ErrorCode::TransportBroken),
    ] {
        assert_eq!(
            read_frame(&mut Failing(kind))
                .expect_err("I/O failure")
                .code,
            expected
        );
    }
}

#[test]
fn wire_faults_reach_the_receiver() {
    for (corrupt_crc, truncate_payload, expected) in [
        (true, false, ErrorCode::ChecksumMismatch),
        (false, true, ErrorCode::FrameTruncated),
    ] {
        let (sender, mut receiver) = MemoryTransport::pair();
        let mut sender = FaultInjectingTransport::new(
            sender,
            FaultConfig {
                corrupt_crc,
                truncate_payload,
                drop_connection: false,
            },
        );
        sender
            .send_message(MessageType::HeartbeatReq, 1, b"payload")
            .expect("write injected wire");
        assert_eq!(
            receiver
                .recv_message()
                .expect_err("receiver detects fault")
                .code,
            expected
        );
    }
}

#[test]
fn connection_drop_disconnects_the_peer() {
    let (sender, mut receiver) = MemoryTransport::pair();
    let mut sender = FaultInjectingTransport::new(
        sender,
        FaultConfig {
            corrupt_crc: false,
            truncate_payload: false,
            drop_connection: true,
        },
    );
    assert_eq!(
        sender
            .send_message(MessageType::HeartbeatReq, 1, b"")
            .expect_err("injected close")
            .code,
        ErrorCode::ConnectionClosed
    );
    assert_eq!(
        receiver
            .recv_message()
            .expect_err("peer observes close")
            .code,
        ErrorCode::ConnectionClosed
    );
}

#[test]
fn queue_capacity_and_receive_deadline_are_enforced() {
    let (mut sender, mut receiver) = MemoryTransport::pair_with_timeout(Duration::ZERO);
    assert_eq!(
        receiver
            .recv_message()
            .expect_err("empty queue deadline")
            .code,
        ErrorCode::Timeout
    );
    for sequence in 1..=16 {
        sender
            .send_message(MessageType::HeartbeatReq, sequence, b"x")
            .expect("queue capacity");
    }
    assert_eq!(
        sender
            .send_message(MessageType::HeartbeatReq, 17, b"x")
            .expect_err("full queue")
            .code,
        ErrorCode::QueueFull
    );
    for sequence in 1..=16 {
        let (header, payload) = receiver.recv_message().expect("queued frame");
        assert_eq!(header.sequence_id, sequence);
        assert_eq!(payload, b"x");
    }
    sender
        .send_message(MessageType::HeartbeatReq, 17, b"y")
        .expect("capacity recovered");
    assert_eq!(receiver.recv_message().expect("recovered queue").1, b"y");
}

#[test]
fn unrelated_responses_are_rejected() {
    for (kind, sequence, expected) in [
        (MessageType::HeartbeatResp, 2, ErrorCode::SequenceMismatch),
        (
            MessageType::HandshakeResp,
            1,
            ErrorCode::UnexpectedMessageType,
        ),
    ] {
        let (client, mut peer) = MemoryTransport::pair();
        peer.send_message(kind, sequence, b"response")
            .expect("peer response");
        let mut client = IpcClient::new(client);
        assert_eq!(
            client
                .send_and_recv(MessageType::HeartbeatReq, b"request")
                .expect_err("uncorrelated response")
                .code,
            expected
        );
        let (header, payload) = peer.recv_message().expect("client request");
        assert_eq!(header.sequence_id, 1);
        assert_eq!(payload, b"request");
    }
}

#[test]
fn handshake_validates_version_and_principal_before_installing_token() {
    for (version, principal, expected) in [
        (PROTOCOL_VERSION + 1, [1; 16], ErrorCode::VersionMismatch),
        (PROTOCOL_VERSION, [2; 16], ErrorCode::InvalidPrincipal),
    ] {
        let (client, mut peer) = MemoryTransport::pair();
        let token = CapabilityToken::issue(
            7,
            principal,
            CapabilityScope::UI_RENDER,
            10,
            20,
            30,
            b"test key",
        );
        let response = HandshakeResponsePayload {
            server_version: version,
            initial_token: token,
        };
        peer.send_message(MessageType::HandshakeResp, 1, &response.encode())
            .expect("handshake response");
        let mut client = IpcClient::new(client);
        let HandshakeError::Local(error) =
            client.handshake(5, [1; 16]).expect_err("invalid session")
        else {
            panic!("session consistency failure must remain local");
        };
        assert_eq!(error.code, expected);
        assert_eq!(client.active_token(), None);
    }
}

#[derive(Default)]
struct Heartbeat {
    count: usize,
    failures: Vec<(FailureContext, ErrorCode)>,
    event: Option<RemoteEventPayload>,
}
impl IpcHandler for Heartbeat {
    fn handle_request(
        &mut self,
        _: &FrameHeader,
        payload: &[u8],
    ) -> Result<(MessageType, Vec<u8>)> {
        self.count += 1;
        Ok((MessageType::HeartbeatResp, payload.to_vec()))
    }
    fn handle_failure(&mut self, context: FailureContext, error: ErrorCode) -> Result<()> {
        self.failures.push((context, error));
        Ok(())
    }
    fn take_event(&mut self) -> Option<RemoteEventPayload> {
        self.event.take()
    }
}

#[test]
fn sync_server_delivers_handler_event_after_its_correlated_response() {
    let (transport, server_transport) = MemoryTransport::pair();
    let mut client = IpcClient::new(transport);
    let mut server = IpcServer::new(server_transport);
    let response = ClinicalCalcResponsePayload {
        audit_sequence_id: 9,
        rate_ml_hr: 1.25,
        drug_rate_mg_hr: 2.5,
        is_pediatric: false,
        result_signature: [4; 32],
    };
    let mut handler = Heartbeat {
        count: 0,
        failures: Vec::new(),
        event: Some(RemoteEventPayload::from_event(9, &response).expect("typed event envelope")),
    };
    let server_thread = std::thread::spawn(move || server.step(&mut handler));
    assert_eq!(
        client.send_and_recv(MessageType::HeartbeatReq, b"echo"),
        Ok((MessageType::HeartbeatResp, b"echo".to_vec()))
    );
    assert_eq!(server_thread.join().expect("server thread"), Ok(true));
    let event = client.recv_event().expect("unsolicited event");
    assert_eq!(event.name(), "clinical.result");
    assert_eq!(event.event_id().get(), response.audit_sequence_id);
    assert_eq!(
        event.decode_as::<ClinicalCalcResponsePayload>(),
        Ok(response)
    );
}

#[test]
fn replay_and_decreasing_sequences_never_reach_handler() {
    let (mut peer, server) = MemoryTransport::pair();
    let mut server = IpcServer::new(server);
    let mut handler = Heartbeat::default();
    for (sequence, accepted) in [(0, false), (5, true), (5, false), (4, false), (6, true)] {
        peer.send_message(MessageType::HeartbeatReq, sequence, b"echo")
            .expect("request");
        if accepted {
            assert_eq!(server.step(&mut handler), Ok(true));
            let (header, payload) = peer.recv_message().expect("response");
            assert_eq!(header.sequence_id, sequence);
            assert_eq!(payload, b"echo");
        } else {
            assert_eq!(
                server.step(&mut handler).expect_err("replay").code,
                ErrorCode::ReplayDetected
            );
        }
    }
    assert_eq!(handler.count, 2);
    assert_eq!(
        handler.failures,
        [0, 5, 4].map(|sequence| (
            FailureContext::Request(RequestIdentity {
                message_type: MessageType::HeartbeatReq,
                sequence
            }),
            ErrorCode::ReplayDetected
        ))
    );
}

#[test]
fn handler_failure_still_consumes_request_sequence() {
    struct Rejecting {
        count: usize,
        failures: Vec<(FailureContext, ErrorCode)>,
    }
    impl IpcHandler for Rejecting {
        fn handle_request(&mut self, _: &FrameHeader, _: &[u8]) -> Result<(MessageType, Vec<u8>)> {
            self.count += 1;
            Err(MetisError::protocol(
                ErrorCode::MalformedPayload,
                "Rejected application input",
            ))
        }
        fn handle_failure(&mut self, context: FailureContext, error: ErrorCode) -> Result<()> {
            self.failures.push((context, error));
            Ok(())
        }
    }
    let (mut peer, server) = MemoryTransport::pair();
    let mut server = IpcServer::new(server);
    let mut handler = Rejecting {
        count: 0,
        failures: Vec::new(),
    };
    for expected in [ErrorCode::MalformedPayload, ErrorCode::ReplayDetected] {
        peer.send_message(MessageType::HeartbeatReq, 1, b"request")
            .expect("request");
        assert_eq!(
            server.step(&mut handler).expect_err("rejected").code,
            expected
        );
    }
    assert_eq!(handler.count, 1);
    let identity = RequestIdentity {
        message_type: MessageType::HeartbeatReq,
        sequence: 1,
    };
    assert_eq!(
        handler.failures,
        vec![
            (
                FailureContext::Handler(identity),
                ErrorCode::MalformedPayload
            ),
            (FailureContext::Request(identity), ErrorCode::ReplayDetected)
        ]
    );
}

#[test]
fn server_only_treats_clean_eof_as_successful_termination() {
    let (peer, endpoint) = MemoryTransport::pair();
    drop(peer);
    let mut server = IpcServer::new(endpoint);
    let mut handler = Heartbeat::default();
    assert_eq!(server.step(&mut handler), Ok(false));
    assert_eq!(handler.failures, Vec::new());
    let (mut peer, endpoint) = MemoryTransport::pair();
    peer.send_frame(b"MET").expect("partial header");
    let mut server = IpcServer::new(endpoint);
    assert_eq!(
        server.step(&mut handler).expect_err("truncated frame").code,
        ErrorCode::FrameTruncated
    );
    assert_eq!(
        handler.failures,
        vec![(FailureContext::Receive, ErrorCode::FrameTruncated)]
    );
}

#[test]
fn response_failures_preserve_request_identity() {
    for request_type in [MessageType::HeartbeatReq, MessageType::HandshakeReq] {
        let (mut peer, endpoint) = MemoryTransport::pair();
        peer.send_message(request_type, 9, b"input")
            .expect("request");
        drop(peer);
        let mut server = IpcServer::new(endpoint);
        let mut handler = Heartbeat::default();
        let expected = if request_type == MessageType::HeartbeatReq {
            ErrorCode::ConnectionClosed
        } else {
            ErrorCode::UnexpectedMessageType
        };
        assert_eq!(
            server
                .step(&mut handler)
                .expect_err("response failure")
                .code,
            expected
        );
        assert_eq!(handler.count, 1);
        assert_eq!(
            handler.failures,
            vec![(
                FailureContext::Response(RequestIdentity {
                    message_type: request_type,
                    sequence: 9
                }),
                expected
            )]
        );
    }
}

#[test]
fn wrong_request_direction_is_reported_without_dispatch() {
    let (mut peer, endpoint) = MemoryTransport::pair();
    peer.send_message(MessageType::HeartbeatResp, 1, b"")
        .expect("frame");
    let mut server = IpcServer::new(endpoint);
    let mut handler = Heartbeat::default();
    assert_eq!(
        server.step(&mut handler).expect_err("wrong direction").code,
        ErrorCode::UnexpectedMessageType
    );
    assert_eq!(handler.count, 0);
    assert_eq!(
        handler.failures,
        vec![(
            FailureContext::Request(RequestIdentity {
                message_type: MessageType::HeartbeatResp,
                sequence: 1
            }),
            ErrorCode::UnexpectedMessageType
        )]
    );
}

#[test]
fn failing_failure_sink_stops_server_and_retains_attempt_context() {
    struct UnavailableAudit(Vec<(FailureContext, ErrorCode)>);
    impl IpcHandler for UnavailableAudit {
        fn handle_request(&mut self, _: &FrameHeader, _: &[u8]) -> Result<(MessageType, Vec<u8>)> {
            panic!("invalid frame must not reach handler")
        }
        fn handle_failure(&mut self, context: FailureContext, error: ErrorCode) -> Result<()> {
            self.0.push((context, error));
            Err(MetisError::transport(
                ErrorCode::IoError,
                "Audit sink unavailable",
            ))
        }
    }
    let (mut peer, endpoint) = MemoryTransport::pair();
    peer.send_frame(b"MET").expect("truncation");
    let mut server = IpcServer::new(endpoint);
    let mut handler = UnavailableAudit(Vec::new());
    assert_eq!(
        server.step(&mut handler).expect_err("sink failed").code,
        ErrorCode::IoError
    );
    assert_eq!(
        handler.0,
        vec![(FailureContext::Receive, ErrorCode::FrameTruncated)]
    );
}

#[test]
fn memory_message_boundaries_reject_empty_and_trailing_frames() {
    let (mut peer, mut endpoint) = MemoryTransport::pair();
    peer.send_frame(b"").expect("empty wire");
    assert_eq!(
        endpoint.recv_message().expect_err("empty frame").code,
        ErrorCode::FrameTruncated
    );
    let mut wire = build_frame(MessageType::HeartbeatReq, 1, b"").expect("frame");
    wire.push(0);
    peer.send_frame(&wire).expect("trailing byte");
    assert_eq!(
        endpoint
            .recv_message()
            .expect_err("trailing frame bytes")
            .code,
        ErrorCode::MalformedPayload
    );
}
