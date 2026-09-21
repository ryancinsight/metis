use super::*;
use metis_core::capability::{CapabilityGrantSpec, CapabilityScope};
use metis_core::error::ErrorCode;
use metis_core::host::{HostOrigin, HostPolicy, HostSessionId, WindowId};
use moirai_executor::block_on;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::thread;

const KEY: &[u8] = b"metis-scoped-network-test-key";

fn capability(
    scope: CapabilityScope,
) -> Result<VerifiedHostCapability<{ CapabilityScope::NETWORK.0 }>, metis_core::error::MetisError> {
    let policy = HostPolicy::new(
        HostOrigin::parse("http://127.0.0.1:8080").expect("test origin"),
        WindowId::new(1).expect("test window"),
    );
    let session = HostSessionId::new([13; 16]).expect("test session");
    let context = policy.context_for(session);
    let token = context.issue_capability(
        CapabilityGrantSpec {
            token_id: 13,
            principal_id: session.as_bytes(),
            scope,
            issued_at_secs: 10,
            duration_secs: 100,
            nonce: 13,
        },
        KEY,
    )?;
    policy.authorize::<{ CapabilityScope::NETWORK.0 }>(&token, &context, 11, KEY)
}

fn loopback_response(body: &'static [u8]) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let address = listener.local_addr().expect("listener address");
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("request connection");
        let mut request = Vec::new();
        let mut chunk = [0u8; 512];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let count = stream.read(&mut chunk).expect("request head");
            if count == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..count]);
            assert!(request.len() <= 16 * 1024, "request headers are bounded");
        }
        assert!(request.starts_with(b"GET /health HTTP/1.1\r\n"));
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nX-Metis: loopback\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .and_then(|()| stream.write_all(body))
            .expect("response");
        stream.shutdown(Shutdown::Both).expect("close response");
    });
    (format!("http://{address}"), task)
}

#[test]
fn real_loopback_request_requires_scope_and_returns_value() {
    let (origin, task) = loopback_response(b"metis-network");
    let provider = ScopedHttpProvider::new([origin.as_str()]).expect("provider");
    let request = ScopedHttpRequest::new(
        "GET",
        format!("{origin}/health"),
        [("X-Metis", "request")],
        None,
    )
    .expect("request");
    let response = block_on(provider.request(
        &capability(CapabilityScope::NETWORK).expect("network capability"),
        request,
    ))
    .expect("loopback response");
    task.join().expect("server task");
    assert_eq!(response.status(), 200);
    assert_eq!(response.header("x-metis"), Some("loopback"));
    assert_eq!(response.body(), b"metis-network");
    assert!(!format!("{response:?}").contains("metis-network"));
}

#[test]
fn request_validation_and_origin_policy_fail_closed() {
    assert!(matches!(
        ScopedHttpProvider::new(std::iter::empty::<&str>()),
        Err(ScopedHttpError::NoOrigins)
    ));
    assert!(matches!(
        ScopedHttpProvider::new(["metis://native"]),
        Err(ScopedHttpError::InvalidOrigin)
    ));
    assert!(matches!(
        ScopedHttpRequest::new(
            "GET",
            "http://127.0.0.1:8080/#fragment",
            std::iter::empty::<(&str, &str)>(),
            None,
        ),
        Err(ScopedHttpError::InvalidUrl)
    ));
    assert!(matches!(
        ScopedHttpRequest::new(
            "GET",
            "http://127.0.0.1:8080/",
            [("Host", "attacker.test")],
            None,
        ),
        Err(ScopedHttpError::ForbiddenHeader)
    ));
    let denied = capability(CapabilityScope::UI_RENDER).expect_err("missing network scope");
    assert_eq!(denied.code, ErrorCode::InsufficientScope);
}

#[test]
fn body_and_header_bounds_are_value_checked() {
    let body = vec![0u8; MAX_SCOPED_HTTP_BODY_BYTES + 1];
    assert!(matches!(
        ScopedHttpRequest::new(
            "POST",
            "http://127.0.0.1:8080/",
            std::iter::empty::<(&str, &str)>(),
            Some(body),
        ),
        Err(ScopedHttpError::BodyTooLarge)
    ));
    let headers = (0..=MAX_SCOPED_HTTP_HEADERS)
        .map(|index| (format!("X-Metis-{index}"), String::from("value")));
    assert!(matches!(
        ScopedHttpRequest::new("GET", "http://127.0.0.1:8080/", headers, None),
        Err(ScopedHttpError::TooManyHeaders)
    ));
}
