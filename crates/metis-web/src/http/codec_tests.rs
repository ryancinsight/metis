use super::codec::{PRINCIPAL, accept_handshake, handshake, invocation};
use metis_core::{
    capability::{CapabilityScope, CapabilityToken},
    protocol::{
        FragmentAction, HandshakeRequestPayload, HandshakeResponsePayload, PROTOCOL_VERSION,
        PluginInvocationPayload,
    },
};

fn token(principal: [u8; 16], scope: CapabilityScope) -> CapabilityToken {
    CapabilityToken::issue(
        7,
        principal,
        scope,
        10,
        20,
        30,
        b"http browser acceptance key",
    )
}

#[test]
fn http_requests_use_the_authoritative_binary_codecs_and_unicode_bytes() {
    let _trace = super::test_support::trace();
    let _test =
        tracing::info_span!("http_requests_use_the_authoritative_binary_codecs_and_unicode_bytes")
            .entered();
    let request = tracing::info_span!("act")
        .in_scope(|| HandshakeRequestPayload::decode(&handshake()).expect("handshake"));
    let _assert = tracing::info_span!("assert").entered();
    assert_eq!(request.client_version, PROTOCOL_VERSION);
    assert_eq!(request.client_process_id, 42);
    assert_eq!(request.principal_id, PRINCIPAL);
    let token = token(PRINCIPAL, CapabilityScope::UI_RENDER);
    for (generation, value) in [
        (1, ""),
        (7, "Δ Ryan 🦀"),
        (u64::MAX, "<script>text only</script>"),
    ] {
        let bytes =
            invocation(token.clone(), generation, "metis-events", value).expect("invocation");
        let decoded = PluginInvocationPayload::decode(&bytes).expect("canonical envelope");
        assert_eq!(decoded.token(), &token);
        assert_eq!(decoded.plugin_name(), "ui");
        assert_eq!(decoded.operation_name(), "action");
        let action = FragmentAction::decode(decoded.body()).expect("action");
        assert_eq!(action.generation(), generation);
        assert_eq!(action.input(), value);
        assert_eq!(action.target(), "metis-events");
    }
}

#[test]
fn handshake_requires_the_declared_version_principal_scope_and_exact_body() {
    let _trace = super::test_support::trace();
    let _test = tracing::info_span!(
        "handshake_requires_the_declared_version_principal_scope_and_exact_body"
    )
    .entered();
    let expected =
        tracing::info_span!("arrange").in_scope(|| token(PRINCIPAL, CapabilityScope::UI_RENDER));
    let act = tracing::info_span!("act").entered();
    let encoded = HandshakeResponsePayload {
        server_version: PROTOCOL_VERSION,
        initial_token: expected.clone(),
    }
    .encode();
    drop(act);
    let _assert = tracing::info_span!("assert").entered();
    assert_eq!(
        accept_handshake(&encoded).expect("accepted token"),
        expected
    );
    for response in [
        HandshakeResponsePayload {
            server_version: PROTOCOL_VERSION + 1,
            initial_token: expected.clone(),
        },
        HandshakeResponsePayload {
            server_version: PROTOCOL_VERSION,
            initial_token: token([0x67; 16], CapabilityScope::UI_RENDER),
        },
        HandshakeResponsePayload {
            server_version: PROTOCOL_VERSION,
            initial_token: token(PRINCIPAL, CapabilityScope::READ_FILE),
        },
    ] {
        assert_eq!(
            accept_handshake(&response.encode())
                .expect_err("identity contract")
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
    let mut trailing = encoded.clone();
    trailing.push(1);
    for malformed in [&encoded[..encoded.len() - 1], trailing.as_slice()] {
        assert_eq!(
            accept_handshake(malformed)
                .expect_err("exact length")
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}

#[test]
fn host_endpoint_configuration_is_exact_and_never_grants_authority() {
    use super::codec::endpoint_origin;
    let _trace = super::test_support::trace();
    let _test =
        tracing::info_span!("host_endpoint_configuration_is_exact_and_never_grants_authority")
            .entered();
    for (url, origin) in [
        ("http://127.0.0.1:8766/health", "http://127.0.0.1:8766"),
        ("http://127.0.0.1:8767/health", "http://127.0.0.1:8767"),
        ("https://example.test/health", "https://example.test"),
    ] {
        assert_eq!(
            endpoint_origin(url).expect("host endpoint").as_str(),
            origin
        );
    }
    for url in [
        "",
        "javascript:alert(1)",
        "metis://app/health",
        "http://user@host/health",
        "http://host/path",
        "http://host/health?extra=1",
        "http://host/health#fragment",
        "http://host/../health",
    ] {
        assert!(matches!(
            endpoint_origin(url).expect_err("invalid endpoint").kind(),
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData
        ));
    }
}
