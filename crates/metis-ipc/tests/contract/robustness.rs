//! Bounded parser properties and deterministic mutation coverage.
//!
//! The wire decoders are trust-boundary code. The generated cases keep their
//! input and allocation sizes bounded; a panic in any decoder fails the
//! property run, while successful values are checked with the round-trip
//! properties below.

use metis_core::capability::{CapabilityScope, CapabilityToken};
use metis_core::error::Result;
use metis_core::protocol::{
    CapabilityCatalogPayload, ClinicalCalcRequestPayload, ClinicalCalcResponsePayload,
    ErrorResponsePayload, FrameHeader, HEADER_SIZE, HandshakeRequestPayload,
    HandshakeResponsePayload, MessageType, PluginInvocationPayload,
    PluginInvocationResponsePayload, RemoteEventPayload, TargetCapability, TargetCapabilityPayload,
    TargetPlatform, build_frame,
};

const GENERATED_CASES: u64 = 128;
const MAX_GENERATED_BYTES: u64 = 1024;

fn token() -> CapabilityToken {
    CapabilityToken::issue(
        7,
        [0x42; 16],
        CapabilityScope::SUBMIT_CALCULATION,
        1_700_000_000,
        60,
        9,
        &[0x17; 32],
    )
}

fn consume<T>(result: Result<T>) {
    drop(std::hint::black_box(result));
}

fn next_state(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn generated_bytes() -> impl Iterator<Item = Vec<u8>> {
    (0..GENERATED_CASES).map(|seed| {
        let mut state = seed.wrapping_add(1);
        let length = usize::try_from(next_state(&mut state) % (MAX_GENERATED_BYTES + 1))
            .expect("invariant: generated length fits usize");
        (0..length)
            .map(|_| {
                u8::try_from(next_state(&mut state) & u64::from(u8::MAX))
                    .expect("invariant: masked generator output fits u8")
            })
            .collect()
    })
}

#[test]
fn bounded_wire_decoders_are_total_for_generated_bytes() {
    for bytes in generated_bytes() {
        consume(HandshakeRequestPayload::decode(&bytes));
        consume(HandshakeResponsePayload::decode(&bytes));
        consume(ClinicalCalcRequestPayload::decode(&bytes));
        consume(ClinicalCalcResponsePayload::decode(&bytes));
        consume(ErrorResponsePayload::decode(&bytes));
        consume(PluginInvocationPayload::decode(&bytes));
        consume(PluginInvocationResponsePayload::decode(&bytes));
        consume(CapabilityCatalogPayload::decode(&bytes));
        consume(TargetCapabilityPayload::decode(&bytes));
        if let Ok(event) = RemoteEventPayload::decode(&bytes) {
            consume(event.decode_as::<ClinicalCalcResponsePayload>());
        }
        if let Ok(header) = <[u8; HEADER_SIZE]>::try_from(bytes.as_slice()) {
            consume(FrameHeader::decode(&header));
        }
    }
}

#[test]
fn error_and_event_round_trips_preserve_bounded_unicode() {
    let messages = [
        "",
        "bounded rejection",
        "患者-α",
        "emoji-🧪",
        "combining-e\u{301}",
    ];
    let bodies = [Vec::new(), vec![0, 1, 2], vec![0xff; 128]];
    for message in messages {
        let error = ErrorResponsePayload {
            error_code: 0x4001,
            message: message.to_owned(),
        };
        let encoded = error.encode().expect("bounded error payload");
        assert_eq!(ErrorResponsePayload::decode(&encoded), Ok(error));
        for body in &bodies {
            let event =
                RemoteEventPayload::new(3, "test.value", body).expect("bounded event envelope");
            let encoded = event.encode().expect("bounded event payload");
            assert_eq!(RemoteEventPayload::decode(&encoded), Ok(event));
        }
    }
}

#[test]
fn clinical_round_trip_preserves_ieee_bits() {
    let values = [
        0_u64,
        1_u64 << 63,
        1.0_f64.to_bits(),
        (-1.0_f64).to_bits(),
        f64::NAN.to_bits(),
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
    ];
    for weight in values {
        for concentration in values {
            for dose in values {
                let request = ClinicalCalcRequestPayload {
                    token: token(),
                    patient_id: "患者-α".to_owned(),
                    weight_kg: f64::from_bits(weight),
                    concentration_mg_ml: f64::from_bits(concentration),
                    target_dose_mcg_kg_min: f64::from_bits(dose),
                };
                let encoded = request.encode().expect("bounded clinical request");
                let decoded = ClinicalCalcRequestPayload::decode(&encoded)
                    .expect("encoded clinical request decodes");
                assert_eq!(decoded.token, request.token);
                assert_eq!(decoded.patient_id, request.patient_id);
                assert_eq!(decoded.weight_kg.to_bits(), weight);
                assert_eq!(decoded.concentration_mg_ml.to_bits(), concentration);
                assert_eq!(decoded.target_dose_mcg_kg_min.to_bits(), dose);
            }
        }
    }
}

#[test]
fn deterministic_mutations_remain_parser_safe() {
    let clinical = ClinicalCalcRequestPayload {
        token: token(),
        patient_id: "patient-α".to_owned(),
        weight_kg: 72.5,
        concentration_mg_ml: 4.0,
        target_dose_mcg_kg_min: 0.5,
    };
    let response = ClinicalCalcResponsePayload {
        audit_sequence_id: 8,
        rate_ml_hr: 1.25,
        drug_rate_mg_hr: 5.0,
        is_pediatric: false,
        result_signature: [0x5a; 32],
    };
    let plugin = PluginInvocationPayload::new(token(), "viewer", "open", [1, 4, 9])
        .expect("bounded plugin invocation");
    let event =
        RemoteEventPayload::new(2, "clinical.result", response.encode()).expect("bounded event");
    let payloads = [
        HandshakeRequestPayload {
            client_version: 0x0100,
            client_process_id: 7,
            principal_id: [0x42; 16],
        }
        .encode(),
        HandshakeResponsePayload {
            server_version: 0x0100,
            initial_token: token(),
        }
        .encode(),
        clinical.encode().expect("clinical request"),
        response.encode(),
        ErrorResponsePayload {
            error_code: 0x3001,
            message: "bounded rejection".to_owned(),
        }
        .encode()
        .expect("error response"),
        plugin.encode().expect("plugin invocation"),
        PluginInvocationResponsePayload::new([2, 5, 10])
            .expect("plugin response")
            .encode()
            .expect("plugin response encoding"),
        CapabilityCatalogPayload::new([MessageType::HeartbeatReq])
            .expect("catalog")
            .encode()
            .expect("catalog encoding"),
        TargetCapabilityPayload::new(TargetPlatform::Windows, [TargetCapability::NativeProcess])
            .expect("target descriptor")
            .encode()
            .expect("target encoding"),
        event.encode().expect("event encoding"),
    ];

    for payload in payloads {
        for length in 0..=payload.len() {
            let candidate = &payload[..length];
            consume(HandshakeRequestPayload::decode(candidate));
            consume(HandshakeResponsePayload::decode(candidate));
            consume(ClinicalCalcRequestPayload::decode(candidate));
            consume(ClinicalCalcResponsePayload::decode(candidate));
            consume(ErrorResponsePayload::decode(candidate));
            consume(PluginInvocationPayload::decode(candidate));
            consume(PluginInvocationResponsePayload::decode(candidate));
            consume(CapabilityCatalogPayload::decode(candidate));
            consume(TargetCapabilityPayload::decode(candidate));
            if let Ok(event) = RemoteEventPayload::decode(candidate) {
                consume(event.decode_as::<ClinicalCalcResponsePayload>());
            }
        }
        for (index, mask) in
            (0..payload.len()).flat_map(|index| [1_u8, 0x80].map(move |mask| (index, mask)))
        {
            let mut candidate = payload.clone();
            candidate[index] ^= mask;
            consume(HandshakeRequestPayload::decode(&candidate));
            consume(HandshakeResponsePayload::decode(&candidate));
            consume(ClinicalCalcRequestPayload::decode(&candidate));
            consume(ClinicalCalcResponsePayload::decode(&candidate));
            consume(ErrorResponsePayload::decode(&candidate));
            consume(PluginInvocationPayload::decode(&candidate));
            consume(PluginInvocationResponsePayload::decode(&candidate));
            consume(CapabilityCatalogPayload::decode(&candidate));
            consume(TargetCapabilityPayload::decode(&candidate));
            if let Ok(event) = RemoteEventPayload::decode(&candidate) {
                consume(event.decode_as::<ClinicalCalcResponsePayload>());
            }
        }
    }

    let frame = build_frame(MessageType::HeartbeatReq, 11, b"bounded").expect("frame");
    for (index, mask) in
        (0..frame.len()).flat_map(|index| [1_u8, 0x80].map(move |mask| (index, mask)))
    {
        let mut candidate = frame.clone();
        candidate[index] ^= mask;
        if let Ok(header) = <[u8; HEADER_SIZE]>::try_from(&candidate[..HEADER_SIZE]) {
            consume(FrameHeader::decode(&header));
        }
    }
}

#[test]
fn oversized_length_fields_fail_with_bounded_errors() {
    let plugin = PluginInvocationPayload::new(token(), "viewer", "open", [1, 2, 3])
        .expect("plugin invocation")
        .encode()
        .expect("plugin encoding");
    let mut plugin_length = plugin;
    plugin_length[84..86].copy_from_slice(&u16::MAX.to_be_bytes());
    assert!(PluginInvocationPayload::decode(&plugin_length).is_err());

    let response = PluginInvocationResponsePayload::new([1, 2, 3])
        .expect("plugin response")
        .encode()
        .expect("response encoding");
    let mut response_length = response;
    response_length[..4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(PluginInvocationResponsePayload::decode(&response_length).is_err());

    let error = ErrorResponsePayload {
        error_code: 0x4001,
        message: "bounded".to_owned(),
    }
    .encode()
    .expect("error encoding");
    let mut error_length = error;
    error_length[2..4].copy_from_slice(&u16::MAX.to_be_bytes());
    assert!(ErrorResponsePayload::decode(&error_length).is_err());
}
