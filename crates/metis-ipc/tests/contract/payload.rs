//! Protocol payload boundaries and adversarial byte representations.
use metis_core::capability::{CapabilityScope, CapabilityToken};
use metis_core::error::ErrorCode;
use metis_core::protocol::{
    ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, ErrorResponsePayload,
    HandshakeRequestPayload, HandshakeResponsePayload, MAX_PAYLOAD_SIZE, MessageType,
    PROTOCOL_VERSION, PluginInvocationPayload, PluginInvocationResponsePayload, RemoteEventPayload,
    build_frame,
};

fn request() -> ClinicalCalcRequestPayload {
    ClinicalCalcRequestPayload {
        token: CapabilityToken::issue(
            1,
            [2; 16],
            CapabilityScope::SUBMIT_CALCULATION,
            100,
            200,
            3,
            b"fixture signing key",
        ),
        patient_id: "patient-α".to_owned(),
        weight_kg: 70.0,
        concentration_mg_ml: 1.25,
        target_dose_mcg_kg_min: 2.5,
    }
}

#[test]
fn payloads_roundtrip_all_semantic_fields() {
    let request = request();
    assert_eq!(
        ClinicalCalcRequestPayload::decode(&request.encode().expect("request encoding"))
            .expect("request decoding"),
        request
    );
    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: [8; 16],
    };
    assert_eq!(
        HandshakeRequestPayload::decode(&handshake.encode()).expect("handshake"),
        handshake
    );
    let response = HandshakeResponsePayload {
        server_version: PROTOCOL_VERSION,
        initial_token: request.token,
    };
    assert_eq!(
        HandshakeResponsePayload::decode(&response.encode()).expect("response"),
        response
    );
    let clinical = ClinicalCalcResponsePayload {
        audit_sequence_id: 19,
        rate_ml_hr: 1.25,
        drug_rate_mg_hr: 2.5,
        is_pediatric: true,
        result_signature: [4; 32],
    };
    assert_eq!(
        ClinicalCalcResponsePayload::decode(&clinical.encode()).expect("clinical"),
        clinical
    );
    let error = ErrorResponsePayload {
        error_code: 0x3001,
        message: "invalid weight".to_owned(),
    };
    assert_eq!(
        ErrorResponsePayload::decode(&error.encode().expect("error encoding"))
            .expect("error decoding"),
        error
    );
}

#[test]
fn clinical_response_codec_roundtrips_through_remote_event_envelope() {
    let response = ClinicalCalcResponsePayload {
        audit_sequence_id: 19,
        rate_ml_hr: 1.25,
        drug_rate_mg_hr: 2.5,
        is_pediatric: true,
        result_signature: [4; 32],
    };
    let event = RemoteEventPayload::from_event(response.audit_sequence_id, &response)
        .expect("clinical event envelope");
    assert_eq!(event.name(), "clinical.result");
    assert_eq!(
        event.decode_as::<ClinicalCalcResponsePayload>(),
        Ok(response)
    );
}

#[test]
fn plugin_invocation_and_response_roundtrip_preserve_opaque_bytes() {
    let invocation = PluginInvocationPayload::new(
        request().token.clone(),
        "viewer",
        "open.series",
        [0, 1, 0xfe, 0xff],
    )
    .expect("plugin invocation");
    let encoded = invocation.encode().expect("invocation encoding");
    assert_eq!(PluginInvocationPayload::decode(&encoded), Ok(invocation));

    let response = PluginInvocationResponsePayload::new([9, 0, 8, 0, 7]).expect("response");
    let encoded = response.encode().expect("response encoding");
    assert_eq!(
        PluginInvocationResponsePayload::decode(&encoded),
        Ok(response)
    );
}

#[test]
fn plugin_payloads_reject_invalid_names_truncation_and_trailing_bytes() {
    let invocation =
        PluginInvocationPayload::new(request().token.clone(), "viewer", "open", [1, 2, 3])
            .expect("plugin invocation");
    assert_eq!(
        PluginInvocationPayload::new(request().token.clone(), "Viewer", "open", [])
            .expect_err("uppercase plugin name")
            .code,
        ErrorCode::MalformedPayload
    );
    let wire = invocation.encode().expect("invocation encoding");
    for length in 0..wire.len() {
        assert_eq!(
            PluginInvocationPayload::decode(&wire[..length])
                .expect_err("truncated invocation")
                .code,
            ErrorCode::MalformedPayload
        );
    }
    let mut trailing = wire;
    trailing.push(0);
    assert_eq!(
        PluginInvocationPayload::decode(&trailing)
            .expect_err("trailing invocation")
            .code,
        ErrorCode::MalformedPayload
    );

    let response = PluginInvocationResponsePayload::new([1, 2, 3]).expect("response");
    let wire = response.encode().expect("response encoding");
    for length in 0..wire.len() {
        assert_eq!(
            PluginInvocationResponsePayload::decode(&wire[..length])
                .expect_err("truncated response")
                .code,
            ErrorCode::MalformedPayload
        );
    }
    let mut trailing = wire;
    trailing.push(0);
    assert_eq!(
        PluginInvocationResponsePayload::decode(&trailing)
            .expect_err("trailing response")
            .code,
        ErrorCode::MalformedPayload
    );
}

#[test]
fn plugin_response_respects_the_frame_resource_bound() {
    let response = PluginInvocationResponsePayload::new(vec![0; MAX_PAYLOAD_SIZE - 4])
        .expect("maximum response body");
    assert_eq!(
        response.encode().expect("maximum response encoding").len(),
        MAX_PAYLOAD_SIZE
    );
    assert_eq!(
        PluginInvocationResponsePayload::new(vec![0; MAX_PAYLOAD_SIZE - 3])
            .expect_err("oversized response body")
            .code,
        ErrorCode::PayloadTooLarge
    );
}

#[test]
fn clinical_request_rejects_every_truncation_and_trailing_bytes() {
    let wire = request().encode().expect("request encoding");
    for length in 0..wire.len() {
        assert_eq!(
            ClinicalCalcRequestPayload::decode(&wire[..length])
                .expect_err("truncated")
                .code,
            ErrorCode::MalformedPayload
        );
    }
    let mut trailing = wire;
    trailing.push(0);
    assert_eq!(
        ClinicalCalcRequestPayload::decode(&trailing)
            .expect_err("trailing")
            .code,
        ErrorCode::MalformedPayload
    );
}

#[test]
fn fixed_payloads_reject_noncanonical_lengths() {
    let request = request();
    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 3,
        principal_id: [1; 16],
    };
    let response = HandshakeResponsePayload {
        server_version: PROTOCOL_VERSION,
        initial_token: request.token,
    };
    let clinical = ClinicalCalcResponsePayload {
        audit_sequence_id: 1,
        rate_ml_hr: 1.0,
        drug_rate_mg_hr: 1.0,
        is_pediatric: false,
        result_signature: [2; 32],
    };
    let mut wire = handshake.encode();
    for length in 0..wire.len() {
        assert_eq!(
            HandshakeRequestPayload::decode(&wire[..length])
                .expect_err("truncated")
                .code,
            ErrorCode::MalformedPayload
        );
    }
    wire.push(0);
    assert_eq!(
        HandshakeRequestPayload::decode(&wire)
            .expect_err("trailing")
            .code,
        ErrorCode::MalformedPayload
    );
    let mut wire = response.encode();
    for length in 0..wire.len() {
        assert_eq!(
            HandshakeResponsePayload::decode(&wire[..length])
                .expect_err("truncated")
                .code,
            ErrorCode::MalformedPayload
        );
    }
    wire.push(0);
    assert_eq!(
        HandshakeResponsePayload::decode(&wire)
            .expect_err("trailing")
            .code,
        ErrorCode::MalformedPayload
    );
    let mut wire = clinical.encode();
    for length in 0..wire.len() {
        assert_eq!(
            ClinicalCalcResponsePayload::decode(&wire[..length])
                .expect_err("truncated")
                .code,
            ErrorCode::MalformedPayload
        );
    }
    wire.push(0);
    assert_eq!(
        ClinicalCalcResponsePayload::decode(&wire)
            .expect_err("trailing")
            .code,
        ErrorCode::MalformedPayload
    );
}

#[test]
fn invalid_utf8_and_boolean_values_are_not_normalized() {
    let mut wire = request().encode().expect("request encoding");
    *wire.last_mut().expect("patient reference") = 0xff;
    assert_eq!(
        ClinicalCalcRequestPayload::decode(&wire)
            .expect_err("UTF-8")
            .code,
        ErrorCode::MalformedPayload
    );
    assert_eq!(
        ErrorResponsePayload::decode(&[0, 1, 0, 1, 0xff])
            .expect_err("UTF-8")
            .code,
        ErrorCode::MalformedPayload
    );
    let mut response = vec![0; 57];
    for boolean in 2..=255 {
        response[24] = boolean;
        assert_eq!(
            ClinicalCalcResponsePayload::decode(&response)
                .expect_err("noncanonical boolean")
                .code,
            ErrorCode::MalformedPayload
        );
    }
}

#[test]
fn variable_encodings_enforce_frame_capacity_without_narrowing() {
    let mut request = request();
    request.patient_id = "x".repeat(MAX_PAYLOAD_SIZE - 110);
    let wire = request.encode().expect("maximum patient reference");
    assert_eq!(wire.len(), MAX_PAYLOAD_SIZE);
    assert_eq!(
        ClinicalCalcRequestPayload::decode(&wire).expect("maximum request"),
        request
    );
    request.patient_id.push('x');
    assert_eq!(
        request.encode().expect_err("oversized reference").code,
        ErrorCode::PayloadTooLarge
    );
    let mut error = ErrorResponsePayload {
        error_code: 1,
        message: "x".repeat(MAX_PAYLOAD_SIZE - 4),
    };
    let wire = error.encode().expect("maximum diagnostic");
    assert_eq!(wire.len(), MAX_PAYLOAD_SIZE);
    assert_eq!(
        ErrorResponsePayload::decode(&wire).expect("maximum diagnostic"),
        error
    );
    error.message.push('x');
    assert_eq!(
        error.encode().expect_err("oversized diagnostic").code,
        ErrorCode::PayloadTooLarge
    );
    assert_eq!(
        build_frame(MessageType::HeartbeatReq, 1, &vec![0; MAX_PAYLOAD_SIZE + 1])
            .expect_err("oversized frame")
            .code,
        ErrorCode::PayloadTooLarge
    );
}

#[test]
fn arbitrary_short_payloads_fail_without_panics() {
    // Exhaust every single-byte value at every truncated payload length. This
    // is bounded malformed-input coverage, not a claim of exhaustive fuzzing.
    for length in 0..110 {
        for byte in 0..=255 {
            let wire = vec![byte; length];
            assert_eq!(
                ClinicalCalcRequestPayload::decode(&wire)
                    .expect_err("short clinical request")
                    .code,
                ErrorCode::MalformedPayload
            );
        }
    }
}
