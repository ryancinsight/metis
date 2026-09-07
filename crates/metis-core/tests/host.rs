//! Host boundary and capability-binding contract tests.

use metis_core::capability::{CapabilityGrantSpec, CapabilityScope, CapabilityToken};
use metis_core::error::ErrorCode;
use metis_core::{HostContext, HostOrigin, HostPolicy, HostSessionId, WindowId};

const KEY: [u8; 32] = [0x5A; 32];

fn make_context(origin: &str, window: u64, principal: [u8; 16]) -> HostContext {
    HostContext::new(
        HostOrigin::try_from(origin).expect("test origin"),
        WindowId::new(window).expect("test window"),
        HostSessionId::new(principal).expect("test session"),
    )
}

#[test]
fn origin_parser_canonicalizes_allowed_network_origins() {
    assert_eq!(
        HostOrigin::try_from("HTTPS://LOCALHOST:8443")
            .expect("origin")
            .as_str(),
        "https://localhost:8443"
    );
    assert_eq!(
        HostOrigin::try_from("tauri://LOCALHOST")
            .expect("origin")
            .as_str(),
        "tauri://localhost"
    );
}

#[test]
fn origin_parser_rejects_opaque_and_injection_forms() {
    for value in [
        "",
        "null",
        "file:///tmp/app",
        "data:text/html,app",
        "javascript://host",
        "http://user@host",
        "http://host/path",
        "http://host:0",
        "http://*",
        "http://[::1",
        "http://host:bad",
    ] {
        assert_eq!(
            HostOrigin::try_from(value)
                .expect_err("invalid origin")
                .code,
            ErrorCode::InvalidOrigin,
            "value: {value}"
        );
    }
}

#[test]
fn origin_parser_accepts_and_canonicalizes_strict_ipv6() {
    assert_eq!(
        HostOrigin::try_from("HTTP://[2001:DB8:0:0:0:0:0:1]:443")
            .expect("valid IPv6 origin")
            .as_str(),
        "http://[2001:db8::1]:443"
    );
    for value in [
        "http://[::::]",
        "http://[1:2:3:4:5:6:7:8:9]",
        "http://[gggg::1]",
        "http://[::1]extra",
    ] {
        assert_eq!(
            HostOrigin::try_from(value)
                .expect_err("malformed IPv6 origin")
                .code,
            ErrorCode::InvalidOrigin,
            "value: {value}"
        );
    }
}

#[test]
fn policy_authorizes_exact_context_and_scope() {
    let principal = [0x11; 16];
    let origin = HostOrigin::try_from("http://localhost:8765").expect("origin");
    let window = WindowId::new(7).expect("window");
    let policy = HostPolicy::new(origin.clone(), window);
    let context = HostContext::new(
        origin,
        window,
        HostSessionId::new(principal).expect("session"),
    );
    let token = context
        .issue_capability(
            CapabilityGrantSpec {
                token_id: 41,
                principal_id: principal,
                scope: CapabilityScope::SUBMIT_CALCULATION,
                issued_at_secs: 100,
                duration_secs: 60,
                nonce: 42,
            },
            &KEY,
        )
        .expect("issued bound token");
    let grant = policy
        .authorize::<{ CapabilityScope::SUBMIT_CALCULATION.0 }>(&token, &context, 101, &KEY)
        .expect("authorized host context");
    assert_eq!(grant.token_id(), 41);
    assert_eq!(grant.context(), &context);
}

#[test]
fn policy_rejects_origin_window_and_session_substitution() {
    let principal = [0x22; 16];
    let policy = HostPolicy::new(
        HostOrigin::try_from("https://app.example").expect("origin"),
        WindowId::new(3).expect("window"),
    );
    let expected_context = make_context("https://app.example", 3, principal);
    let token = expected_context
        .issue_capability(
            CapabilityGrantSpec {
                token_id: 1,
                principal_id: principal,
                scope: CapabilityScope::SUBMIT_CALCULATION,
                issued_at_secs: 10,
                duration_secs: 60,
                nonce: 1,
            },
            &KEY,
        )
        .expect("issued bound token");
    for (candidate, code) in [
        (
            make_context("https://evil.example", 3, principal),
            ErrorCode::NavigationDenied,
        ),
        (
            make_context("https://app.example", 4, principal),
            ErrorCode::InvalidWindow,
        ),
        (
            make_context("https://app.example", 3, [0x33; 16]),
            ErrorCode::InvalidPrincipal,
        ),
    ] {
        assert_eq!(
            policy
                .authorize::<{ CapabilityScope::SUBMIT_CALCULATION.0 }>(
                    &token, &candidate, 11, &KEY
                )
                .expect_err("substitution must be denied")
                .code,
            code
        );
    }
}

#[test]
fn host_binding_rejects_unbound_and_retargeted_signatures() {
    let principal = [0x44; 16];
    let context = make_context("https://app.example", 3, principal);
    let plain = CapabilityToken::issue(
        9,
        principal,
        CapabilityScope::SUBMIT_CALCULATION,
        10,
        60,
        1,
        &KEY,
    );
    assert_eq!(
        plain
            .verify_for_host(CapabilityScope::SUBMIT_CALCULATION, 11, &KEY, &context)
            .expect_err("unbound token")
            .code,
        ErrorCode::InvalidCapabilitySignature
    );

    let bound = context
        .issue_capability(
            CapabilityGrantSpec {
                token_id: 10,
                principal_id: principal,
                scope: CapabilityScope::SUBMIT_CALCULATION,
                issued_at_secs: 10,
                duration_secs: 60,
                nonce: 2,
            },
            &KEY,
        )
        .expect("bound token");
    let other_context = make_context("https://other.example", 3, principal);
    assert_eq!(
        bound
            .verify_for_host(
                CapabilityScope::SUBMIT_CALCULATION,
                11,
                &KEY,
                &other_context,
            )
            .expect_err("retargeted token")
            .code,
        ErrorCode::InvalidCapabilitySignature
    );
}

#[test]
fn policy_emits_strict_csp_without_inline_or_wildcard_sources() {
    let policy = HostPolicy::native();
    let csp = policy.content_security_policy();
    assert!(csp.contains("default-src 'self'"));
    assert!(csp.contains("script-src 'self' 'wasm-unsafe-eval'"));
    assert!(csp.contains("connect-src 'self'"));
    assert!(!csp.contains("unsafe-inline"));
    assert!(!csp.contains('*'));
}
