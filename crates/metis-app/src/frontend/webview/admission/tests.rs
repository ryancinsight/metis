use super::*;
use metis_core::embedded_assets::Digest;
use metis_web::embedded::{BrowserPermissionResult, SequenceId};

fn traced() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .without_time()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    )
}

fn identity() -> PageIdentity {
    PageIdentity::new(
        PageRole::PermissionProbe,
        SequenceId::new(1).expect("positive mount"),
        Digest::hash(b"published module identity vector"),
    )
}

fn observations() -> [CapabilityObservation; 4] {
    // Typed wire-value vectors exercise admission, not actual browser evidence.
    PageCapability::ALL.map(|kind| CapabilityObservation {
        kind,
        host_denial: None,
        browser: match kind {
            PageCapability::Geolocation => BrowserPermissionResult::Rejected {
                name: None,
                code: Some(1),
                message: "Geolocation permission denied".into(),
            },
            PageCapability::Camera | PageCapability::Microphone => {
                BrowserPermissionResult::Rejected {
                    name: Some("NotAllowedError".into()),
                    code: None,
                    message: format!("{} permission denied", kind.as_str()),
                }
            }
            PageCapability::Notifications => BrowserPermissionResult::Denied,
        },
    })
}

fn completed_resources() -> ResourceCounts {
    ResourceCounts {
        dom_listeners: 2,
        bridge_listeners: 1,
        geolocation_dispatchers: 2,
        ..ResourceCounts::default()
    }
}

#[test]
fn capture_requires_exact_ready_and_both_native_and_browser_evidence() {
    let _trace = traced();
    let _test = tracing::info_span!("native_probe_evidence_admission").entered();
    let mut state = tracing::info_span!("arrange").in_scope(|| {
        NativePage::new(identity(), "https://metis.invalid/index.html".into())
            .expect("canonical packaged document")
    });
    let initial = observations();
    assert!(!state.probe_capture_ready());
    assert_eq!(
        state
            .probe(identity(), initial.clone(), completed_resources())
            .expect_err("unready probe")
            .kind(),
        io::ErrorKind::InvalidData
    );
    state.session.ready(identity()).expect("exact ready");
    tracing::info_span!("act").in_scope(|| {
        state
            .probe(identity(), initial.clone(), completed_resources())
            .expect("actual outcomes vector")
    });
    assert!(!state.probe_capture_ready());
    let mut reconciled = initial;
    for kind in PageCapability::ALL {
        state
            .native_denial(
                "https://metis.invalid/",
                kind,
                kind == PageCapability::Camera,
            )
            .expect("independent denial vector");
        reconciled[kind.index()].host_denial = Some(kind == PageCapability::Camera);
        assert!(!state.probe_capture_ready());
    }
    state
        .probe(identity(), reconciled, completed_resources())
        .expect("reconciled outcomes");
    tracing::info_span!("assert").in_scope(|| {
        assert!(state.probe_capture_ready());
        assert_eq!(state.denial_commands().count(), PageCapability::ALL.len());
        assert_eq!(state.session.stage(), PageStage::Ready);
    });
}

#[test]
fn grant_absence_wrong_origin_and_fabricated_denial_fail_capture_admission() {
    let _trace = traced();
    let _test = tracing::info_span!("native_probe_negative_admission").entered();
    for kind in PageCapability::ALL {
        for browser in [
            BrowserPermissionResult::Granted,
            BrowserPermissionResult::Unavailable {
                message: kind.as_str().into(),
            },
        ] {
            let mut state = NativePage::new(identity(), "https://metis.invalid/index.html".into())
                .expect("canonical packaged document");
            state.session.ready(identity()).expect("exact ready");
            let mut values = observations();
            values[kind.index()].browser = browser;
            let error = state
                .probe(identity(), values, completed_resources())
                .expect_err("non-denial evidence");
            assert!(error.to_string().contains(kind.as_str()));
            assert!(!state.probe_capture_ready());
        }
    }
    let mut state = NativePage::new(identity(), "https://metis.invalid/index.html".into())
        .expect("canonical packaged document");
    assert!(
        state
            .source("https://other.invalid/index.html")
            .expect_err("exact source")
            .to_string()
            .contains("source")
    );
    state.session.ready(identity()).expect("exact ready");
    let mut values = observations();
    values[0].host_denial = Some(false);
    assert_eq!(
        state
            .probe(identity(), values, completed_resources())
            .expect_err("no native event")
            .to_string(),
        "Page claims an unobserved or mismatched native denial"
    );
}

#[test]
fn native_evidence_flags_cannot_reclassify_hardware_or_timeout_rejections() {
    let _trace = traced();
    let _test = tracing::info_span!("native_probe_rejection_domain").entered();
    let vectors = tracing::info_span!("arrange").in_scope(|| {
        // Adversarial typed observations validate admission, not browser evidence.
        // Geolocation codes 2/3 are position-unavailable/timeout. The media
        // names identify device/track/constraint errors rather than denial.
        let mut vectors = [None, Some(2), Some(3)]
            .map(|code| {
                (
                    PageCapability::Geolocation,
                    BrowserPermissionResult::Rejected {
                        name: None,
                        code,
                        message: format!("geolocation error vector {code:?}"),
                    },
                )
            })
            .to_vec();
        for kind in [PageCapability::Camera, PageCapability::Microphone] {
            for name in [
                "NotFoundError",
                "NotReadableError",
                "AbortError",
                "OverconstrainedError",
            ] {
                vectors.push((
                    kind,
                    BrowserPermissionResult::Rejected {
                        name: Some(name.into()),
                        code: None,
                        message: format!("{} {name} error vector", kind.as_str()),
                    },
                ));
            }
        }
        vectors.push((
            PageCapability::Notifications,
            BrowserPermissionResult::Rejected {
                name: Some("NotAllowedError".into()),
                code: None,
                message: "notification rejection is not a permission value".into(),
            },
        ));
        vectors
    });
    assert_eq!(vectors.len(), 12);
    for (kind, browser) in vectors {
        let (mut state, values) =
            tracing::info_span!("arrange", kind = kind.as_str()).in_scope(|| {
                let mut state =
                    NativePage::new(identity(), "https://metis.invalid/index.html".into())
                        .expect("canonical packaged document");
                state.session.ready(identity()).expect("exact ready");
                let mut values = observations();
                for actual in PageCapability::ALL {
                    state
                        .native_denial("https://metis.invalid/", actual, false)
                        .expect("independent typed native observation vector");
                    values[actual.index()].host_denial = Some(false);
                }
                values[kind.index()].browser = browser.clone();
                (state, values)
            });
        let error = tracing::info_span!("act").in_scope(|| {
            state
                .probe(identity(), values, completed_resources())
                .expect_err("hardware or timeout cannot reconcile denial")
        });
        tracing::info_span!("assert").in_scope(|| {
            assert!(error.to_string().contains(kind.as_str()));
            assert!(error.to_string().contains(&format!("{browser:?}")));
            assert_eq!(state.session.stage(), PageStage::Ready);
            assert!(!state.probe_capture_ready());
        });
    }
}

#[test]
fn duplicate_and_changed_completion_cannot_replace_settled_evidence() {
    let _trace = traced();
    let _test = tracing::info_span!("native_probe_monotone_reconciliation").entered();
    let mut state = NativePage::new(identity(), "https://metis.invalid/index.html".into())
        .expect("canonical packaged document");
    state.session.ready(identity()).expect("exact ready");
    let values = observations();
    state
        .probe(identity(), values.clone(), completed_resources())
        .expect("first values");
    assert_eq!(
        state
            .probe(identity(), values.clone(), completed_resources())
            .expect_err("duplicate values")
            .to_string(),
        "Probe completion is duplicated"
    );
    state
        .native_denial("https://metis.invalid/", PageCapability::Geolocation, false)
        .expect("actual denial vector");
    assert_eq!(
        state
            .native_denial("https://metis.invalid/", PageCapability::Geolocation, true)
            .expect_err("duplicate native kind")
            .to_string(),
        "Native probe permission kind is duplicated"
    );
    let mut changed = values;
    if let BrowserPermissionResult::Rejected { message, .. } = &mut changed[0].browser {
        message.push_str(" changed after settlement");
    }
    assert_eq!(
        state
            .probe(identity(), changed, completed_resources())
            .expect_err("changed browser value")
            .to_string(),
        "Reconciled probe changes a settled browser outcome or native denial"
    );
    assert!(!state.probe_capture_ready());
}

#[test]
fn live_resources_and_noncanonical_dispatcher_counts_reject_probe_completion() {
    let _trace = traced();
    let _test = tracing::info_span!("native_probe_resource_admission").entered();
    let base = completed_resources();
    let variants = [
        ResourceCounts {
            application_tasks: 1,
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
            geolocation_dispatchers: 1,
            ..base
        },
        ResourceCounts {
            media_leases: 1,
            media_tracks: 2,
            ..base
        },
        ResourceCounts {
            unsettled_guards: 1,
            ..base
        },
        ResourceCounts {
            bridge_listeners: 0,
            ..base
        },
        ResourceCounts {
            retained_bridge_callbacks: 1,
            ..base
        },
    ];
    for resources in variants {
        let mut state = NativePage::new(identity(), "https://metis.invalid/index.html".into())
            .expect("canonical packaged document");
        state.session.ready(identity()).expect("exact ready");
        assert_eq!(
            state
                .probe(identity(), observations(), resources)
                .expect_err("live ownership")
                .to_string(),
            "Completed probe still owns requests, tracks or failed cleanup"
        );
        assert!(!state.probe_capture_ready());
    }
}

#[test]
fn notification_values_cannot_fabricate_another_capability_result() {
    let _trace = traced();
    let _test = tracing::info_span!("native_probe_kind_result_domain").entered();
    let vectors = tracing::info_span!("arrange")
        .in_scope(|| PageCapability::ALL.map(|kind| (kind, observations())));
    for (kind, initial) in vectors {
        for browser in [
            BrowserPermissionResult::Denied,
            BrowserPermissionResult::Default,
        ] {
            let (mut state, values) = tracing::info_span!("arrange", kind = kind.as_str())
                .in_scope(|| {
                    let mut state =
                        NativePage::new(identity(), "https://metis.invalid/index.html".into())
                            .expect("canonical packaged document");
                    state.session.ready(identity()).expect("exact ready");
                    let mut values = initial.clone();
                    for actual in PageCapability::ALL {
                        state
                            .native_denial("https://metis.invalid/", actual, false)
                            .expect("independent typed native observation vector");
                        values[actual.index()].host_denial = Some(false);
                    }
                    values[kind.index()].browser = browser;
                    (state, values)
                });
            let result = tracing::info_span!("act").in_scope(|| {
                state
                    .probe(identity(), values.clone(), completed_resources())
                    .cloned()
            });
            tracing::info_span!("assert").in_scope(|| {
                if kind == PageCapability::Notifications {
                    assert_eq!(result.expect("actual notification value domain"), values);
                    assert!(state.probe_capture_ready());
                } else {
                    assert_eq!(
                        result.expect_err("incompatible WebIDL domain").to_string(),
                        "Browser notification outcome belongs to another capability"
                    );
                    assert!(!state.probe_capture_ready());
                }
            });
        }
    }
}

#[test]
fn sdk_permission_origin_is_distinct_from_exact_document_message_source() {
    let _trace = traced();
    let _test = tracing::info_span!("native_sdk_permission_origin").entered();
    let vectors = tracing::info_span!("arrange").in_scope(|| {
        [
            ("https://metis.invalid/index.html", "https://metis.invalid/"),
            (
                "https://metis.invalid/probe/index.html",
                "https://metis.invalid/",
            ),
            (
                "https://demo.test:8443/index.html",
                "https://demo.test:8443/",
            ),
        ]
    });
    for (document, origin) in vectors {
        let mut page =
            NativePage::new(identity(), document.into()).expect("canonical configured document");
        tracing::info_span!("act").in_scope(|| {
            page.session
                .ready(identity())
                .expect("exact document mount");
            page.source(document)
                .expect("message carries full document URI");
            page.native_denial(origin, PageCapability::Camera, true)
                .expect("SDK callback carries configured origin");
        });
        tracing::info_span!("assert").in_scope(|| {
            assert_eq!(
                page.denial_commands().collect::<Vec<_>>(),
                vec![PageCommand::PermissionDenied {
                    identity: identity(),
                    kind: PageCapability::Camera,
                    user_initiated: true,
                }]
            );
            assert_eq!(
                page.source(origin)
                    .expect_err("origin is not a message document")
                    .kind(),
                io::ErrorKind::InvalidData
            );
            for hostile in [
                document.to_owned(),
                format!("{origin}other.html"),
                format!("{origin}?query=1"),
                format!("{origin}#fragment"),
                "https://other.invalid/".into(),
                "http://metis.invalid/".into(),
                "https://metis.invalid:443/".into(),
                "https://metis.invalid.evil.test/".into(),
            ] {
                assert_eq!(
                    page.permission_source(&hostile)
                        .expect_err("exact SDK origin only")
                        .to_string(),
                    "Native permission source differs from the admitted origin"
                );
            }
        });
    }
}
