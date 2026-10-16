use super::*;
use metis_core::embedded_assets::Digest;
use metis_web::embedded::command_observation::{Event, Record, encode_observation};
use metis_web::embedded::{PageIdentity, SequenceId};

fn identity(role: PageRole, mount: u64) -> PageIdentity {
    PageIdentity::new(
        role,
        SequenceId::new(mount).expect("positive mount"),
        Digest::hash(b"actual command observation policy test module"),
    )
}

fn record(identity: PageIdentity, sequence: u64, event: Event) -> String {
    let invocation = (!matches!(event, Event::Selected | Event::Quota { .. }))
        .then(|| SequenceId::new(sequence).expect("positive invocation"));
    encode_observation(
        &Record::new(
            identity,
            SequenceId::new(sequence).expect("positive record"),
            invocation,
            event,
        )
        .expect("valid actual record"),
        MAX_BYTES,
    )
    .expect("encoded record")
}

fn ready(identity: PageIdentity) -> PageSession {
    let mut session = PageSession::new(identity);
    session
        .ready(identity)
        .expect("actual readiness transition");
    session
}

fn published(receipt: Receipt<'_>) -> String {
    match receipt {
        Receipt::Record(value) => value.into_owned(),
        other => panic!("expected actual publication, got {other:?}"),
    }
}

#[test]
fn raw_json_is_retained_and_charged_without_changing_the_ready_request() {
    let _trace = trace();
    let _test = tracing::info_span!("native_command_observation_raw").entered();
    let (mut receiver, session, wire) = tracing::info_span!("arrange").in_scope(|| {
        let id = identity(PageRole::Form, 7);
        let mut session = ready(id);
        session
            .begin_request(id, SequenceId::new(23).expect("request"))
            .expect("real pending request");
        let encoded = record(id, 1, Event::Selected);
        let value: serde_json::Value = serde_json::from_str(&encoded).expect("actual record");
        let formatted = format!(
            "\t{}\r\n",
            serde_json::to_string_pretty(&value).expect("equivalent JSON")
        );
        (Receiver::new(), session, formatted)
    });
    let output =
        tracing::info_span!("act").in_scope(|| published(receiver.receive(&wire, &session)));
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(output.as_bytes(), wire.as_bytes());
        assert_eq!(receiver.budget.bytes(), wire.len());
        assert_eq!(receiver.budget.records(), 1);
        assert_eq!(receiver.next, 2);
        assert!(!receiver.budget.is_terminal());
        assert_eq!(session.stage(), PageStage::Ready);
        assert_eq!(session.identity(), identity(PageRole::Form, 7));
        assert_eq!(
            session.pending().expect("unchanged pending request").get(),
            23
        );
    });
}

#[test]
fn role_stage_identity_and_duplicate_rejections_have_one_finite_native_terminal() {
    let _trace = trace();
    let _test = tracing::info_span!("native_command_observation_admission").entered();
    let id = identity(PageRole::Form, 7);
    let mut stopping = ready(id);
    stopping.begin_stop(id).expect("actual stopping transition");
    let cases = tracing::info_span!("arrange").in_scope(|| {
        [
            (PageSession::new(id), "not_current_ready_form"),
            (stopping, "not_current_ready_form"),
            (
                ready(identity(PageRole::PermissionProbe, 7)),
                "not_current_ready_form",
            ),
            (ready(identity(PageRole::Form, 8)), "identity_mismatch"),
            (
                ready(PageIdentity::new(
                    PageRole::Form,
                    SequenceId::new(7).expect("same mount"),
                    Digest::hash(b"another actual module"),
                )),
                "identity_mismatch",
            ),
        ]
    });
    for (session, expected) in cases {
        let mut receiver = Receiver::new();
        let wire = record(id, 1, Event::Selected);
        let output =
            tracing::info_span!("act").in_scope(|| published(receiver.receive(&wire, &session)));
        tracing::info_span!("assert").in_scope(|| {
            let value: serde_json::Value = serde_json::from_str(&output).expect("native terminal");
            assert_eq!(value["kind"], "native_command_observation_terminal");
            assert_eq!(value["reason"], expected);
            assert_eq!(value["failed"], true);
            assert_eq!(receiver.budget.records(), 1);
            assert_eq!(receiver.budget.bytes(), output.len());
            assert!(receiver.budget.is_terminal());
            assert!(matches!(
                receiver.receive(&wire, &session),
                Receipt::Censored
            ));
            assert_eq!(receiver.budget.records(), 1);
        });
    }
    let mut receiver = Receiver::new();
    let session = ready(id);
    let wire = record(id, 1, Event::Selected);
    published(receiver.receive(&wire, &session));
    let output = published(receiver.receive(&wire, &session));
    let value: serde_json::Value = serde_json::from_str(&output).expect("duplicate rejection");
    assert_eq!(value["reason"], "unexpected_sequence");
    assert_eq!(value["prior_records"], 1);
    assert_eq!(value["expected_sequence"], "2");
    assert_eq!(session.stage(), PageStage::Ready);
    assert_eq!(session.pending(), None);
    let mut future = Receiver::new();
    published(future.receive(&wire, &session));
    let skipped = record(
        id,
        3,
        Event::Entered {
            stage: PageStage::Ready,
        },
    );
    let rejected = published(future.receive(&skipped, &session));
    let value: serde_json::Value = serde_json::from_str(&rejected).expect("sequence gap rejection");
    assert_eq!(value["reason"], "unexpected_sequence");
    assert_eq!(value["expected_sequence"], "2");
}

#[test]
fn closing_intent_ends_only_evidence_and_keeps_functional_close_available() {
    let _trace = trace();
    let _test = tracing::info_span!("native_command_observation_closing_intent").entered();
    let (mut receiver, session, selected, closing, functional) = tracing::info_span!("arrange")
        .in_scope(|| {
            let id = identity(PageRole::Form, 7);
            (
                Receiver::new(),
                ready(id),
                record(id, 1, Event::Selected),
                {
                    let canonical = record(id, 2, Event::ClosingIntent);
                    assert!(canonical.len() < MAX_TERMINAL_BYTES);
                    format!(
                        "{}{}",
                        canonical,
                        " ".repeat(MAX_TERMINAL_BYTES + 1 - canonical.len())
                    )
                },
                metis_web::embedded::encode_page_event(
                    &metis_web::embedded::PageEvent::CloseRequested { identity: id },
                    MAX_BYTES,
                )
                .expect("original functional close message"),
            )
        });
    let (retained, bytes) = tracing::info_span!("act").in_scope(|| {
        published(receiver.receive(&selected, &session));
        let retained = published(receiver.receive(&closing, &session));
        let bytes = receiver.budget.bytes();
        assert!(matches!(
            receiver.receive(&functional, &session),
            Receipt::Functional
        ));
        assert!(matches!(
            receiver.receive(&closing, &session),
            Receipt::Censored
        ));
        (retained, bytes)
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(retained.as_bytes(), closing.as_bytes());
        assert_eq!(closing.len(), MAX_TERMINAL_BYTES + 1);
        assert!(receiver.budget.is_terminal());
        assert_eq!(receiver.budget.bytes(), bytes);
        assert_eq!(receiver.budget.records(), 2);
        assert_eq!(session.stage(), PageStage::Ready);
        assert_eq!(session.pending(), None);
    });
}

#[test]
fn malformed_private_records_retain_the_error_and_functional_json_stays_untouched() {
    let _trace = trace();
    let _test = tracing::info_span!("native_command_observation_closed_schema").entered();
    let (mut receiver, session, malformed) = tracing::info_span!("arrange").in_scope(|| {
        let id = identity(PageRole::Form, 7);
        let mut value: serde_json::Value =
            serde_json::from_str(&record(id, 1, Event::Selected)).expect("actual encoded record");
        value["unexpected"] = serde_json::json!(true);
        (Receiver::new(), ready(id), value.to_string())
    });
    let output = tracing::info_span!("act").in_scope(|| {
        let functional = metis_web::embedded::encode_page_event(
            &metis_web::embedded::PageEvent::Ready {
                identity: session.identity(),
            },
            MAX_BYTES,
        )
        .expect("actual functional protocol record");
        assert!(matches!(
            receiver.receive(&functional, &session),
            Receipt::Functional
        ));
        assert_eq!(receiver.budget.records(), 0);
        published(receiver.receive(&malformed, &session))
    });
    tracing::info_span!("assert").in_scope(|| {
        let value: serde_json::Value = serde_json::from_str(&output).expect("native terminal");
        assert_eq!(value["reason"], "invalid_record");
        assert_eq!(value["has_error"], true);
        assert!(
            receiver
                .error
                .as_ref()
                .expect("original typed error")
                .to_string()
                .contains("unexpected")
        );
        assert_eq!(session.stage(), PageStage::Ready);
        assert_eq!(session.pending(), None);
    });
}

#[test]
fn the_exact_raw_byte_boundary_preserves_space_for_native_quota_evidence() {
    let _trace = trace();
    let _test = tracing::info_span!("native_command_observation_byte_boundary").entered();
    let (mut receiver, session, first, next) = tracing::info_span!("arrange").in_scope(|| {
        let id = identity(PageRole::Form, 7);
        let encoded = record(id, 1, Event::Selected);
        let admitted = MAX_BYTES - MAX_TERMINAL_BYTES;
        assert!(encoded.len() < admitted);
        let padded = format!("{}{}", encoded, " ".repeat(admitted - encoded.len()));
        (
            Receiver::new(),
            ready(id),
            padded,
            record(
                id,
                2,
                Event::Entered {
                    stage: PageStage::Ready,
                },
            ),
        )
    });
    let (retained, terminal) = tracing::info_span!("act").in_scope(|| {
        (
            published(receiver.receive(&first, &session)),
            published(receiver.receive(&next, &session)),
        )
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(retained.as_bytes(), first.as_bytes());
        let value: serde_json::Value =
            serde_json::from_str(&terminal).expect("finite quota notice");
        assert_eq!(value["reason"], "receiver_quota_exhausted");
        assert_eq!(value["prior_bytes"], MAX_BYTES - MAX_TERMINAL_BYTES);
        assert_eq!(receiver.budget.bytes(), first.len() + terminal.len());
        assert!(receiver.budget.bytes() <= MAX_BYTES);
        assert!(receiver.budget.is_terminal());
        assert_eq!(receiver.budget.records(), 2);
        assert_eq!(session.stage(), PageStage::Ready);
    });
}
