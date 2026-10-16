//! Executable contracts for the real shared embedded-page codec and lifecycle.

use metis_core::embedded_assets::Digest;
use metis_core::protocol::ClinicalCalcResponsePayload;
use metis_frontend::FormInputs;
use metis_web::embedded::{
    PageCommand, PageEvent, PageIdentity, PageRole, PageSession, PageStage, ResourceCounts,
    SequenceId, decode_page_command, decode_page_event, encode_page_command, encode_page_event,
};

const BUDGET: usize = 64 * 1024;

fn identity(mount: u64) -> PageIdentity {
    PageIdentity::new(
        PageRole::Form,
        SequenceId::new(mount).expect("positive mount"),
        Digest::hash(b"analytical embedded module identity"),
    )
}

fn trace() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .with_test_writer()
            .finish(),
    )
}

#[test]
fn full_integer_domain_stays_decimal_text_and_rates_keep_their_bits() {
    let _trace = trace();
    let identity = tracing::info_span!("arrange").in_scope(|| identity(1));
    for audit in [
        0,
        1,
        (1 << 53) - 1,
        1 << 53,
        (1 << 53) + 1,
        u64::MAX - 1,
        u64::MAX,
    ] {
        let expected = PageCommand::Result {
            identity,
            request: SequenceId::new(u64::MAX).expect("maximum request"),
            response: ClinicalCalcResponsePayload {
                audit_sequence_id: audit,
                rate_ml_hr: 1.0,
                drug_rate_mg_hr: 1.875,
                is_pediatric: false,
                result_signature: Digest::hash(b"published SHA-256 codec byte vector").into_bytes(),
            },
        };
        let (json, actual) = tracing::info_span!("act").in_scope(|| {
            let json = encode_page_command(&expected, BUDGET).expect("bounded result");
            let actual = decode_page_command(&json, BUDGET).expect("exact result");
            (json, actual)
        });
        tracing::info_span!("assert").in_scope(|| {
            assert_eq!(actual, expected);
            assert!(json.contains(&format!("\"audit_sequence_id\":\"{audit}\"")));
            assert!(json.contains(&format!("\"request_id\":\"{}\"", u64::MAX)));
            let PageCommand::Result { response, .. } = actual else {
                panic!("decoded another command");
            };
            assert_eq!(response.rate_ml_hr.to_bits(), 1.0_f64.to_bits());
            assert_eq!(response.drug_rate_mg_hr.to_bits(), 1.875_f64.to_bits());
        });
    }
}

#[test]
fn real_submission_round_trip_preserves_unicode_and_each_numeric_input() {
    let _trace = trace();
    for (patient, weight, concentration, dose) in [
        ("PT-α-患者", 62.5, 1.875, 0.5),
        ("PT-β-患者", 31.25, 3.75, 0.25),
    ] {
        let expected = tracing::info_span!("arrange").in_scope(|| PageEvent::Submit {
            identity: identity((1 << 53) + 1),
            request: SequenceId::new((1 << 53) + 3).expect("positive request"),
            inputs: FormInputs::new(patient, weight, concentration, dose),
        });
        let actual = tracing::info_span!("act").in_scope(|| {
            let json = encode_page_event(&expected, BUDGET).expect("bounded submission");
            decode_page_event(&json, BUDGET).expect("exact submission")
        });
        tracing::info_span!("assert").in_scope(|| assert_eq!(actual, expected));
    }
}

#[test]
fn closed_schema_rejects_ambiguous_identifiers_versions_and_extra_fields() {
    let _trace = trace();
    let json = tracing::info_span!("arrange").in_scope(|| {
        encode_page_event(
            &PageEvent::Ready {
                identity: identity(1),
            },
            BUDGET,
        )
        .expect("valid readiness")
    });
    for replacement in ["0", "01", "+1", " 1", "1 ", "18446744073709551616"] {
        let malformed = json.replace(
            "\"mount_id\":\"1\"",
            &format!("\"mount_id\":\"{replacement}\""),
        );
        let error = tracing::info_span!("act").in_scope(|| {
            decode_page_event(&malformed, BUDGET).expect_err("noncanonical identifier")
        });
        tracing::info_span!("assert").in_scope(|| {
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
            assert!(!error.to_string().is_empty());
        });
    }
    for malformed in [
        json.replace("\"mount_id\":\"1\"", "\"mount_id\":1"),
        json.replace("\"schema\":1", "\"schema\":2"),
        json.replace("\"role\":\"form\"", "\"role\":\"remote\""),
        json.replacen('{', "{\"undeclared\":true,", 1),
    ] {
        let error = decode_page_event(&malformed, BUDGET).expect_err("closed schema");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
}

#[test]
fn byte_budget_is_inclusive_for_both_encoding_and_decoding() {
    let _trace = trace();
    let event = tracing::info_span!("arrange").in_scope(|| PageEvent::Ready {
        identity: identity(1),
    });
    let json = encode_page_event(&event, BUDGET).expect("readiness");
    tracing::info_span!("act").in_scope(|| {
        assert_eq!(
            encode_page_event(&event, json.len()).expect("exact limit"),
            json
        );
        assert_eq!(
            decode_page_event(&json, json.len()).expect("exact limit"),
            event
        );
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(
            encode_page_event(&event, json.len() - 1)
                .expect_err("over limit")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(
            decode_page_event(&json, json.len() - 1)
                .expect_err("over limit")
                .kind(),
            std::io::ErrorKind::InvalidData
        );
    });
}

#[test]
fn lifecycle_accepts_only_current_ready_mount_and_matching_pending_request() {
    let _trace = trace();
    let first = tracing::info_span!("arrange").in_scope(|| identity(7));
    let mut session = PageSession::new(first);
    let request = SequenceId::new(11).expect("request");
    assert_eq!(session.stage(), PageStage::Loading);
    assert_eq!(
        session
            .begin_request(first, request)
            .expect_err("not ready")
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    tracing::info_span!("act").in_scope(|| session.ready(first).expect("matching ready"));
    assert_eq!(session.stage(), PageStage::Ready);
    assert_eq!(
        session.ready(first).expect_err("duplicate ready").kind(),
        std::io::ErrorKind::InvalidData
    );
    session
        .begin_request(first, request)
        .expect("current request");
    assert_eq!(
        session
            .begin_request(first, SequenceId::new(12).expect("next"))
            .expect_err("pending")
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(
        session
            .complete_request(first, SequenceId::new(12).expect("wrong"))
            .expect_err("mismatch")
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    session
        .complete_request(first, request)
        .expect("matching result");
    assert_eq!(
        session
            .begin_request(first, request)
            .expect_err("duplicate sequence")
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    session.begin_stop(first).expect("stop");
    let counts = ResourceCounts {
        bridge_listeners: 1,
        ..ResourceCounts::default()
    };
    session
        .stopped(first, counts)
        .expect("actual page released application owners");
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(session.stage(), PageStage::Stopped);
        assert_eq!(
            session.remount(first).expect_err("reused mount").kind(),
            std::io::ErrorKind::InvalidData
        );
        session.remount(identity(8)).expect("monotonic remount");
        assert_eq!(session.stage(), PageStage::Loading);
        assert_eq!(
            session.ready(first).expect_err("stale ready").kind(),
            std::io::ErrorKind::InvalidData
        );
        session.ready(identity(8)).expect("new ready");
        assert_eq!(session.stage(), PageStage::Ready);
        session.begin_stop(identity(8)).expect("second stop");
        session
            .stopped(identity(8), counts)
            .expect("second acknowledgement");
        assert_eq!(session.stage(), PageStage::Stopped);
        session
            .observe_destroyed()
            .expect("actual native destruction");
        assert_eq!(session.stage(), PageStage::Closed);
    });
}

#[test]
fn stop_counts_accept_only_released_owners_and_actual_dispatcher_cardinality() {
    let _trace = trace();
    let base = tracing::info_span!("arrange").in_scope(|| ResourceCounts {
        bridge_listeners: 1,
        ..ResourceCounts::default()
    });
    for dispatchers in [0, 1, 2, 3, u32::MAX] {
        for guards in [0, 1, 2, 3, 4, 5, u32::MAX] {
            let counts = ResourceCounts {
                geolocation_dispatchers: dispatchers,
                unsettled_guards: guards,
                ..base
            };
            let observed = tracing::info_span!("act", dispatchers, guards)
                .in_scope(|| counts.application_released());
            tracing::info_span!("assert", dispatchers, guards).in_scope(|| {
                assert_eq!(observed, matches!(dispatchers, 0 | 2) && guards <= 4);
            });
        }
    }
    for live in [
        ResourceCounts {
            dom_listeners: 1,
            ..base
        },
        ResourceCounts {
            application_tasks: 1,
            ..base
        },
        ResourceCounts {
            bridge_listeners: 0,
            ..base
        },
        ResourceCounts {
            bridge_listeners: 2,
            ..base
        },
        ResourceCounts {
            retained_bridge_callbacks: 1,
            ..base
        },
        ResourceCounts {
            active_requests: 1,
            ..base
        },
        ResourceCounts {
            geolocation_watches: 1,
            ..base
        },
        ResourceCounts {
            media_leases: 1,
            ..base
        },
        ResourceCounts {
            media_tracks: 1,
            ..base
        },
    ] {
        assert!(
            !live.application_released(),
            "live owner rejects acknowledgement: {live:?}"
        );
    }
}
