//! Handshake rejection semantics through encoded transport frames.
use metis_core::capability::{CapabilityScope, CapabilityToken};
use metis_core::error::ErrorCode;
use metis_core::protocol::{ErrorResponsePayload, MessageType};
use metis_ipc::client::HandshakeError;
use metis_ipc::{IpcClient, IpcTransport, MemoryTransport};

#[test]
fn correlated_handshake_rejections_preserve_known_and_unknown_codes() {
    for error_code in [ErrorCode::InvalidPrincipal as u16, 0xffff] {
        let (transport, mut peer) = MemoryTransport::pair();
        let mut client = IpcClient::new(transport);
        client.set_active_token(CapabilityToken::issue(
            1,
            [1; 16],
            CapabilityScope::UI_RENDER,
            1,
            2,
            3,
            b"test key",
        ));
        let rejection = ErrorResponsePayload {
            error_code,
            message: "Session rejected".to_owned(),
        };
        peer.send_message(
            MessageType::ErrorResp,
            1,
            &rejection.encode().expect("canonical error payload"),
        )
        .expect("encoded correlated response");
        assert_eq!(
            client.handshake(5, [1; 16]),
            Err(HandshakeError::Remote(rejection))
        );
        assert_eq!(client.active_token(), None);
        let (header, _) = peer.recv_message().expect("handshake request");
        assert_eq!(header.msg_type, MessageType::HandshakeReq);
        assert_eq!(header.sequence_id, 1);
    }
}

#[test]
fn malformed_handshake_rejections_preserve_decoder_failures() {
    let wire = ErrorResponsePayload {
        error_code: ErrorCode::InvalidPrincipal as u16,
        message: "rejected".to_owned(),
    }
    .encode()
    .expect("canonical payload");
    let mut malformed: Vec<Vec<u8>> = (0..wire.len()).map(|end| wire[..end].to_vec()).collect();
    let mut trailing = wire.clone();
    trailing.push(0);
    malformed.push(trailing);
    malformed.push(vec![0x20, 0x06, 0, 1, 0xff]);
    for payload in malformed {
        let expected = ErrorResponsePayload::decode(&payload).expect_err("invalid payload");
        let (transport, mut peer) = MemoryTransport::pair();
        peer.send_message(MessageType::ErrorResp, 1, &payload)
            .expect("valid frame with invalid payload");
        let mut client = IpcClient::new(transport);
        assert_eq!(
            client.handshake(5, [1; 16]),
            Err(HandshakeError::Local(expected))
        );
        assert_eq!(client.active_token(), None);
    }
}

#[test]
fn uncorrelated_rejection_is_not_reported_as_peer_decision() {
    let (transport, mut peer) = MemoryTransport::pair();
    let rejection = ErrorResponsePayload {
        error_code: ErrorCode::InvalidPrincipal as u16,
        message: "wrong request".to_owned(),
    };
    peer.send_message(
        MessageType::ErrorResp,
        2,
        &rejection.encode().expect("error payload"),
    )
    .expect("uncorrelated response");
    let mut client = IpcClient::new(transport);
    let HandshakeError::Local(error) = client.handshake(5, [1; 16]).expect_err("sequence") else {
        panic!("correlation must precede error decoding");
    };
    assert_eq!(error.code, ErrorCode::SequenceMismatch);
    assert_eq!(client.active_token(), None);
}
