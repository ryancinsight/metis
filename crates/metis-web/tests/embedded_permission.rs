//! Denial reconciliation follows `WebIDL` domains, not arbitrary rejection.

use metis_web::embedded::{BrowserPermissionResult, CapabilityObservation, PageCapability};

fn trace() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .with_test_writer()
            .finish(),
    )
}

#[test]
fn rejection_domains_preserve_the_distinction_between_denial_and_device_failure() {
    let _trace = trace();
    let _span = tracing::info_span!("assert", contract = "WebIDL rejection domains").entered();
    for kind in PageCapability::ALL {
        for name in [
            None,
            Some("NotAllowedError"),
            Some("NotFoundError"),
            Some("NotReadableError"),
            Some("AbortError"),
            Some("PermissionDeniedError"),
        ] {
            for code in [None, Some(0), Some(1), Some(2), Some(3), Some(u32::MAX)] {
                // Geolocation §10: PERMISSION_DENIED = 1. Media Capture §10.1:
                // Permission Failure rejects with DOMException NotAllowedError.
                let expected = match kind {
                    PageCapability::Geolocation => code == Some(1),
                    PageCapability::Camera | PageCapability::Microphone => {
                        name == Some("NotAllowedError")
                    }
                    PageCapability::Notifications => false,
                };
                for host_denial in [None, Some(false), Some(true)] {
                    let observation = CapabilityObservation {
                        kind,
                        host_denial,
                        browser: BrowserPermissionResult::Rejected {
                            name: name.map(str::to_owned),
                            code,
                            message: format!("Observed {kind:?} {name:?} {code:?}"),
                        },
                    };
                    assert_eq!(observation.permits_denial_reconciliation(), expected);
                    assert_eq!(
                        observation.confirms_host_denial(),
                        expected && host_denial.is_some()
                    );
                }
            }
        }
    }
}

#[test]
fn notification_values_require_their_domain_and_host_observation_is_independent() {
    let _trace = trace();
    let _span = tracing::info_span!("assert", contract = "ungranted notification result").entered();
    for browser in [
        BrowserPermissionResult::Denied,
        BrowserPermissionResult::Default,
        BrowserPermissionResult::Granted,
        BrowserPermissionResult::Unavailable {
            message: "Notification interface is absent".into(),
        },
    ] {
        for kind in PageCapability::ALL {
            let expected = kind == PageCapability::Notifications
                && matches!(
                    browser,
                    BrowserPermissionResult::Denied | BrowserPermissionResult::Default
                );
            for host_denial in [None, Some(false), Some(true)] {
                let observation = CapabilityObservation {
                    kind,
                    browser: browser.clone(),
                    host_denial,
                };
                assert_eq!(observation.permits_denial_reconciliation(), expected);
                assert_eq!(
                    observation.confirms_host_denial(),
                    expected && host_denial.is_some()
                );
            }
        }
    }
}
