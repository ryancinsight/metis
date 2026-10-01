use super::*;
use crate::service::ClockReading;
use metis_core::protocol::HandshakeResponsePayload;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn rejected_peer_does_not_terminate_the_origin_bound_service() {
    let cases: &[(&[u8], ErrorCode)] = &[
        (b"", ErrorCode::FrameTruncated),
        (
            b"POST /v1/session HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\n\r\nno",
            ErrorCode::FrameTruncated,
        ),
        (
            b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n",
            ErrorCode::MalformedPayload,
        ),
    ];
    for &(bytes, expected_error) in cases {
        let runtime = moirai_executor::global();
        let server = runtime
            .block_on(HttpServer::bind("127.0.0.1:0", config()))
            .expect("server bind");
        let address = server.local_addr().expect("server address");
        let task = runtime
            .spawn_async(async move {
                let mut failures = Vec::new();
                serve_browser_http(server, application(), 3, |error| failures.push(error.code))
                    .await?;
                Ok::<_, MetisError>(failures)
            })
            .expect("server task spawn");
        runtime
            .block_on(async {
                let mut peer = TcpStream::connect(&address.to_string()).await?;
                peer.write_all(bytes).await
            })
            .expect("rejected peer write and disconnect");
        let denied = runtime
            .block_on(request_method(
                address,
                "GET",
                &[],
                "/health",
                "http://untrusted.invalid",
            ))
            .expect("unauthorized response");
        assert_eq!(status(&denied), 403);
        let response = runtime
            .block_on(health_request(address))
            .expect("health response");
        assert_eq!(status(&response), 200);
        assert_eq!(response_body(&response), b"metis-http-ready\n");
        let failures = task
            .join()
            .expect("server join")
            .expect("server task")
            .expect("finite server budget");
        assert_eq!(failures, vec![expected_error]);
    }
}

#[test]
fn nine_forged_principals_do_not_occupy_launcher_session() {
    const FORGED_PRINCIPALS: usize = 9;
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            let mut statuses = Vec::with_capacity(FORGED_PRINCIPALS + 1);
            for _ in 0..=FORGED_PRINCIPALS {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                statuses.push(response.status());
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>((statuses, application.session_count()))
        })
        .expect("server task spawn");

    for index in 0..FORGED_PRINCIPALS {
        let principal = [u8::try_from(index + 1).expect("principal index"); 16];
        let response = runtime
            .block_on(request(
                address,
                &handshake(principal),
                SESSION_ROUTE,
                ORIGIN,
            ))
            .expect("session response");
        assert_eq!(status(&response), 403);
        let error = ErrorResponsePayload::decode(response_body(&response)).expect("typed error");
        assert_eq!(error.error_code, ErrorCode::InvalidPrincipal as u16);
    }
    let response = runtime
        .block_on(request(
            address,
            &handshake(PRINCIPAL),
            SESSION_ROUTE,
            ORIGIN,
        ))
        .expect("trusted session response");
    assert_eq!(status(&response), 200);
    let (statuses, count) = task
        .join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
    assert_eq!(
        statuses,
        vec![403; FORGED_PRINCIPALS]
            .into_iter()
            .chain([200])
            .collect::<Vec<_>>()
    );
    assert_eq!(count, 1);
}

/// Deterministic clock whose shared monotonic axis a test moves explicitly.
///
/// Clones observe the same axis, as [`BrowserHttpService::with_clock`]
/// requires. UTC advances with the monotonic reading so the backward-clock
/// check of the backend stays satisfied.
#[derive(Clone)]
struct TestClock(Arc<AtomicU64>);

impl TestClock {
    const UNIX_BASE: Duration = Duration::from_secs(1_700_000_000);

    fn at(monotonic: Duration) -> Self {
        let clock = Self(Arc::new(AtomicU64::new(0)));
        clock.set(monotonic);
        clock
    }

    fn set(&self, monotonic: Duration) {
        let nanos = u64::try_from(monotonic.as_nanos()).expect("test monotonic fits u64 nanos");
        self.0.store(nanos, Ordering::SeqCst);
    }
}

impl Clock for TestClock {
    fn now(&self) -> metis_core::error::Result<ClockReading> {
        let monotonic = Duration::from_nanos(self.0.load(Ordering::SeqCst));
        Ok(ClockReading {
            unix_time: Self::UNIX_BASE + monotonic,
            monotonic,
        })
    }
}

/// Per-request record of one scripted handshake run against a live server.
struct Sequence {
    statuses: Vec<u16>,
    session_counts: Vec<usize>,
    expiries: Vec<Option<Duration>>,
}

/// Sends one handshake per entry of `principals` to one service driven by a
/// [`TestClock`] starting at `start`, calling `before` with the request index
/// ahead of each so a test can move the clock.
fn run_handshakes(
    principals: Vec<[u8; 16]>,
    start: Duration,
    before: impl Fn(usize, &TestClock) + Send + 'static,
) -> Sequence {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let count = principals.len();
    let task = runtime
        .spawn_async(async move {
            let clock = TestClock::at(start);
            let mut application = BrowserHttpService::with_clock(
                [7; 32],
                SafetyEnvelope::default(),
                policy(),
                HostSessionId::new(PRINCIPAL).expect("test principal"),
                clock.clone(),
            );
            let mut sequence = Sequence {
                statuses: Vec::with_capacity(count),
                session_counts: Vec::with_capacity(count),
                expiries: Vec::with_capacity(count),
            };
            for index in 0..count {
                before(index, &clock);
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                sequence.statuses.push(response.status());
                connection.write_response(response).await?;
                sequence.session_counts.push(application.session_count());
                sequence.expiries.push(
                    application
                        .session
                        .as_ref()
                        .map(|session| session.expires_at),
                );
            }
            Ok::<_, io::Error>(sequence)
        })
        .expect("server task spawn");
    for principal in principals {
        runtime
            .block_on(request(
                address,
                &handshake(principal),
                SESSION_ROUTE,
                ORIGIN,
            ))
            .expect("session response");
    }
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response")
}

#[test]
fn session_expires_one_lifetime_after_its_handshake() {
    let opened = Duration::from_secs(5);
    let sequence = run_handshakes(vec![PRINCIPAL], opened, |_, _| {});
    assert_eq!(sequence.statuses, [200]);
    assert_eq!(sequence.expiries, [Some(opened + SESSION_LIFETIME)]);
}

#[test]
fn live_session_conflicts_until_its_expiry_then_is_replaced() {
    let opened = Duration::from_secs(5);
    let expiry = opened + SESSION_LIFETIME;
    let sequence = run_handshakes(
        vec![PRINCIPAL; 3],
        opened,
        move |index, clock| match index {
            1 => clock.set(
                expiry
                    .checked_sub(Duration::from_nanos(1))
                    .expect("invariant: expiry exceeds one nanosecond"),
            ),
            2 => clock.set(expiry),
            _ => {}
        },
    );
    // Contract: a session is live strictly before `expires_at` and expired at
    // exactly `expires_at`, so the handshake at that instant replaces it.
    assert_eq!(sequence.statuses, [200, 409, 200]);
    assert_eq!(sequence.session_counts, [1, 1, 1]);
    assert_eq!(
        sequence.expiries,
        [Some(expiry), Some(expiry), Some(expiry + SESSION_LIFETIME)]
    );
}

#[test]
fn forged_principal_neither_evicts_a_session_nor_sees_a_conflict() {
    let forged = [0x01; 16];
    let opened = Duration::from_secs(5);
    let sequence = run_handshakes(
        vec![PRINCIPAL, forged, forged],
        opened,
        move |index, clock| {
            if index == 2 {
                clock.set(opened + SESSION_LIFETIME);
            }
        },
    );
    assert_eq!(sequence.statuses, [200, 403, 403]);
    assert_eq!(sequence.session_counts, [1, 1, 1]);
    assert_eq!(sequence.expiries[0], sequence.expiries[2]);
}

/// Serves `requests` in order against `application` and returns each raw
/// client response.
fn exchange(
    application: BrowserHttpService,
    requests: Vec<(&'static str, Vec<u8>)>,
) -> Vec<Vec<u8>> {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let count = requests.len();
    let task = runtime
        .spawn_async(async move {
            let mut application = application;
            for _ in 0..count {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>(())
        })
        .expect("server task spawn");
    let responses = requests
        .into_iter()
        .map(|(route, body)| {
            runtime
                .block_on(request(address, &body, route, ORIGIN))
                .expect("exchange response")
        })
        .collect();
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
    responses
}

fn fragment_invocation(token: CapabilityToken) -> Vec<u8> {
    let action =
        FragmentAction::new(1, "status.describe", "metis-events", "session").expect("action");
    PluginInvocationPayload::new(
        token,
        "ui",
        "action",
        action.encode().expect("action bytes"),
    )
    .expect("invocation")
    .encode()
    .expect("invocation bytes")
}

#[test]
fn session_binds_to_the_supplied_principal_not_a_constant() {
    let supplied = [0x5a; 16];
    assert_ne!(supplied, PRINCIPAL);
    let application = BrowserHttpService::new(
        [7; 32],
        SafetyEnvelope::default(),
        policy(),
        HostSessionId::new(supplied).expect("supplied principal"),
    );
    let responses = exchange(
        application,
        vec![
            (SESSION_ROUTE, handshake(PRINCIPAL)),
            (SESSION_ROUTE, handshake(supplied)),
        ],
    );
    let [constant, bound] = responses.as_slice() else {
        panic!("two responses");
    };
    assert_eq!(status(constant), 403);
    let error = ErrorResponsePayload::decode(response_body(constant)).expect("typed error");
    assert_eq!(error.error_code, ErrorCode::InvalidPrincipal as u16);
    assert_eq!(status(bound), 200);
    let token = HandshakeResponsePayload::decode(response_body(bound))
        .expect("typed handshake")
        .initial_token;
    assert_eq!(token.principal_id, supplied);
}

#[test]
fn fragment_with_foreign_principal_is_unauthenticated_despite_a_live_session() {
    let foreign = [0x5a; 16];
    assert_ne!(foreign, PRINCIPAL);
    let forged = CapabilityToken::issue(
        1,
        foreign,
        CapabilityScope::UI_RENDER,
        1,
        3_600,
        1,
        &[9; 32],
    );
    let responses = exchange(
        application(),
        vec![
            (SESSION_ROUTE, handshake(PRINCIPAL)),
            (FRAGMENT_ROUTE, fragment_invocation(forged)),
        ],
    );
    let [session, fragment] = responses.as_slice() else {
        panic!("two responses");
    };
    assert_eq!(status(session), 200);
    assert_eq!(status(fragment), 401);
    let error = ErrorResponsePayload::decode(response_body(fragment)).expect("typed error");
    assert_eq!(error.error_code, ErrorCode::MissingCapability as u16);
}

#[test]
fn response_byte_bound_is_enforced_before_write() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind(
            "127.0.0.1:0",
            ServerConfig::new(8, 4096, 16, MAX_PAYLOAD_SIZE, 32, Duration::from_secs(2))
                .expect("response bound"),
        ))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(serve_browser_http(server, application(), 1, |error| {
            panic!("internal response failure must remain terminal: {error}");
        }))
        .expect("server task spawn");
    let client_result = runtime.block_on(async move {
        let mut client = TcpStream::connect(&address.to_string()).await?;
        client
            .write_all(
                format!(
                    "GET {HEALTH_ROUTE} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {ORIGIN}\r\n\r\n"
                )
                .as_bytes(),
            )
            .await?;
        client.flush().await?;
        read_to_close(&mut client).await
    });
    assert_eq!(
        client_result.expect("bounded response connection"),
        Vec::<u8>::new()
    );
    let error = task
        .join()
        .expect("server task join")
        .expect("server task result")
        .expect_err("response over the configured bound must fail");
    assert_eq!(error.code, ErrorCode::MalformedPayload);
}
