use super::*;
use metis_core::capability::{CapabilityScope, CapabilityToken};
use metis_core::host::{HostOrigin, WindowId};
use metis_core::protocol::{
    FragmentAction, FragmentPatchSet, HandshakeRequestPayload, PROTOCOL_VERSION,
};
use moirai_async::io::AsyncWriteExt;
use moirai_async::net::TcpStream;
use moirai_core::TaskSpawner;
use moirai_core::executor::ExecutorControl;
use moirai_http::{HttpServer, ServerConfig};
use std::time::Duration;

const ORIGIN: &str = "http://127.0.0.1:8080";
const PRINCIPAL: [u8; 16] = [0x66; 16];

fn policy() -> HostPolicy {
    HostPolicy::new(
        HostOrigin::parse(ORIGIN).expect("test origin"),
        WindowId::new(1).expect("test window"),
    )
}

fn application() -> BrowserHttpService {
    BrowserHttpService::new([7; 32], SafetyEnvelope::default(), policy())
}

fn config() -> ServerConfig {
    ServerConfig::new(
        8,
        4096,
        16,
        MAX_PAYLOAD_SIZE,
        131_072,
        Duration::from_secs(2),
    )
    .expect("test limits")
}

async fn read_to_close(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut output = Vec::with_capacity(256);
    let mut chunk = [0_u8; 512];
    loop {
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Ok(output);
        }
        output.extend_from_slice(
            chunk
                .get(..count)
                .ok_or_else(|| io::Error::other("test read exceeded its buffer"))?,
        );
        if output.len() > 131_072 {
            return Err(io::Error::other("test response exceeded its bound"));
        }
    }
}

fn response_body(response: &[u8]) -> &[u8] {
    let marker = b"\r\n\r\n";
    let position = response
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("response header");
    response
        .get(position + marker.len()..)
        .expect("response body")
}

fn status(response: &[u8]) -> u16 {
    let line_end = response
        .iter()
        .position(|byte| *byte == b'\r')
        .expect("status line");
    let line = std::str::from_utf8(response.get(..line_end).expect("status line bytes"))
        .expect("status line utf8");
    line.split_whitespace()
        .nth(1)
        .expect("status code")
        .parse()
        .expect("status number")
}

fn header<'a>(response: &'a [u8], name: &str) -> Option<&'a str> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")?;
    let head = std::str::from_utf8(response.get(..header_end)?).ok()?;
    head.lines().skip(1).find_map(|line| {
        let (header_name, value) = line.split_once(':')?;
        header_name
            .eq_ignore_ascii_case(name)
            .then_some(value.trim())
    })
}

async fn request(
    address: std::net::SocketAddr,
    body: &[u8],
    route: &str,
    origin: &str,
) -> io::Result<Vec<u8>> {
    request_method(address, "POST", body, route, origin).await
}

async fn request_method(
    address: std::net::SocketAddr,
    method: &str,
    body: &[u8],
    route: &str,
    origin: &str,
) -> io::Result<Vec<u8>> {
    let mut client = TcpStream::connect(&address.to_string()).await?;
    let request = format!(
        "{method} {route} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {origin}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    client.write_all(request.as_bytes()).await?;
    client.write_all(body).await?;
    client.flush().await?;
    read_to_close(&mut client).await
}

fn handshake(principal_id: [u8; 16]) -> Vec<u8> {
    HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id,
    }
    .encode()
}

async fn health_request(address: std::net::SocketAddr) -> io::Result<Vec<u8>> {
    let mut client = TcpStream::connect(&address.to_string()).await?;
    client
        .write_all(
            format!("GET {HEALTH_ROUTE} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {ORIGIN}\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    client.flush().await?;
    read_to_close(&mut client).await
}

async fn preflight_request(address: std::net::SocketAddr) -> io::Result<Vec<u8>> {
    let mut client = TcpStream::connect(&address.to_string()).await?;
    client
        .write_all(
            format!(
                "OPTIONS {SESSION_ROUTE} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {ORIGIN}\r\nAccess-Control-Request-Method: POST\r\nAccess-Control-Request-Headers: content-type\r\n\r\n"
            )
            .as_bytes(),
        )
        .await?;
    client.flush().await?;
    read_to_close(&mut client).await
}

#[test]
fn loopback_handshake_and_fragment_response_are_typed() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            for _ in 0..2 {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>(application.session_count())
        })
        .expect("server task spawn");

    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode();
    let response = runtime
        .block_on(request(address, &handshake, SESSION_ROUTE, ORIGIN))
        .expect("handshake response");
    assert_eq!(status(&response), 200);
    let token = HandshakeResponsePayload::decode(response_body(&response))
        .expect("typed handshake")
        .initial_token;
    let action =
        FragmentAction::new(7, "status.describe", "metis-events", "session").expect("action");
    let invocation = PluginInvocationPayload::new(
        token,
        "ui",
        "action",
        action.encode().expect("action bytes"),
    )
    .expect("invocation")
    .encode()
    .expect("invocation bytes");
    let response = runtime
        .block_on(request(address, &invocation, FRAGMENT_ROUTE, ORIGIN))
        .expect("fragment response");
    assert_eq!(status(&response), 200);
    let patches = FragmentPatchSet::decode(response_body(&response)).expect("typed patches");
    assert_eq!(patches.generation(), 7);
    assert_eq!(patches.patches().len(), 1);
    assert_eq!(
        patches.patches()[0].text(),
        Some("Fragment action status.describe accepted: session")
    );
    assert_eq!(
        task.join()
            .expect("server task join")
            .expect("server task result")
            .expect("server response"),
        1
    );
}

#[test]
fn origin_and_route_policy_fail_with_typed_errors() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            for _ in 0..2 {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>(())
        })
        .expect("server task spawn");
    let response = runtime
        .block_on(request(address, &[], SESSION_ROUTE, "http://evil.test"))
        .expect("denial response");
    assert_eq!(status(&response), 403);
    let error = ErrorResponsePayload::decode(response_body(&response)).expect("typed denial");
    assert_eq!(error.error_code, ErrorCode::NavigationDenied as u16);
    let response = runtime
        .block_on(request(address, &[], "/v1/unknown", ORIGIN))
        .expect("route response");
    assert_eq!(status(&response), 404);
    let error = ErrorResponsePayload::decode(response_body(&response)).expect("typed route error");
    assert_eq!(error.error_code, ErrorCode::MalformedPayload as u16);
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
}

#[test]
fn method_and_missing_session_policy_fail_with_typed_errors() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            for _ in 0..2 {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>(())
        })
        .expect("server task spawn");

    let action =
        FragmentAction::new(1, "status.describe", "metis-events", "missing").expect("action");
    let token = CapabilityToken::issue(
        1,
        [0x77; 16],
        CapabilityScope::UI_RENDER,
        1,
        3_600,
        1,
        &[9; 32],
    );
    let invocation = PluginInvocationPayload::new(
        token,
        "ui",
        "action",
        action.encode().expect("action bytes"),
    )
    .expect("invocation")
    .encode()
    .expect("invocation bytes");
    let response = runtime
        .block_on(request(address, &invocation, FRAGMENT_ROUTE, ORIGIN))
        .expect("missing-session response");
    assert_eq!(status(&response), 401);
    let error = ErrorResponsePayload::decode(response_body(&response)).expect("typed error");
    assert_eq!(error.error_code, ErrorCode::MissingCapability as u16);

    let response = runtime
        .block_on(request_method(address, "GET", &[], SESSION_ROUTE, ORIGIN))
        .expect("method response");
    assert_eq!(status(&response), 405);
    let error = ErrorResponsePayload::decode(response_body(&response)).expect("typed error");
    assert_eq!(error.error_code, ErrorCode::UnexpectedMessageType as u16);
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
}

#[test]
fn preflight_returns_the_exact_origin_policy() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            let connection = server.accept().await?;
            let (request, connection) = connection.read_request().await?;
            let response = application.respond(&request)?;
            connection.write_response(response).await
        })
        .expect("server task spawn");
    let response = runtime
        .block_on(preflight_request(address))
        .expect("preflight response");
    assert_eq!(status(&response), 204);
    assert_eq!(response_body(&response), b"");
    assert_eq!(
        header(&response, "Access-Control-Allow-Origin"),
        Some(ORIGIN)
    );
    assert_eq!(header(&response, "Vary"), Some("Origin"));
    assert_eq!(
        header(&response, "Access-Control-Allow-Methods"),
        Some("GET, POST, OPTIONS")
    );
    assert_eq!(
        header(&response, "Access-Control-Allow-Headers"),
        Some("content-type")
    );
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
}

#[test]
fn malformed_and_oversized_requests_are_rejected_before_dispatch() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind(
            "127.0.0.1:0",
            ServerConfig::new(8, 4096, 16, 8, 256, Duration::from_secs(2)).expect("bounds"),
        ))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let connection = server.accept().await?;
            connection.read_request().await.map(|_| ())
        })
        .expect("server task spawn");
    runtime
        .block_on(async move {
            let mut client = TcpStream::connect(&address.to_string()).await?;
            client
                .write_all(b"POST /v1/session HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: ")
                .await?;
            client.write_all(ORIGIN.as_bytes()).await?;
            client
                .write_all(b"\r\nContent-Length: 9\r\n\r\n123456789")
                .await
        })
        .expect("oversized request write");
    let result = task
        .join()
        .expect("oversized task join")
        .expect("oversized task result");
    let error = result.expect_err("oversized body must be rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn idle_peer_hits_deadline_and_teardown_is_finite() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind(
            "127.0.0.1:0",
            ServerConfig::new(8, 4096, 16, 64, 256, Duration::from_millis(50)).expect("deadline"),
        ))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let connection = server.accept().await?;
            connection.read_request().await.map(|_| ())
        })
        .expect("server task spawn");
    let client = runtime
        .block_on(TcpStream::connect(&address.to_string()))
        .expect("idle peer connect");
    let result = task
        .join()
        .expect("deadline task join")
        .expect("deadline task result");
    let error = result.expect_err("idle peer must hit deadline");
    drop(client);
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn disconnected_peer_returns_a_typed_read_failure() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let connection = server.accept().await?;
            connection.read_request().await.map(|_| ())
        })
        .expect("server task spawn");
    runtime
        .block_on(async move {
            let mut client = TcpStream::connect(&address.to_string()).await?;
            client
                .write_all(
                    b"POST /v1/session HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 4\r\n\r\nno",
                )
                .await
        })
        .expect("partial request write");
    let result = task
        .join()
        .expect("disconnect task join")
        .expect("disconnect task result");
    let error = result.expect_err("disconnected peer must fail the request");
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
fn public_server_budget_closes_after_a_health_probe() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(serve_browser_http(server, application(), 1))
        .expect("server task spawn");
    let response = runtime
        .block_on(health_request(address))
        .expect("health response");
    assert_eq!(status(&response), 200);
    assert_eq!(response_body(&response), b"metis-http-ready\n");
    task.join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
}

#[test]
fn session_capacity_is_bounded_and_reports_queue_full() {
    let runtime = moirai_executor::global();
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config()))
        .expect("server bind");
    let address = server.local_addr().expect("server address");
    let task = runtime
        .spawn_async(async move {
            let mut application = application();
            let mut statuses = Vec::with_capacity(MAX_HTTP_SESSIONS + 1);
            for _ in 0..=MAX_HTTP_SESSIONS {
                let connection = server.accept().await?;
                let (request, connection) = connection.read_request().await?;
                let response = application.respond(&request)?;
                statuses.push(response.status());
                connection.write_response(response).await?;
            }
            Ok::<_, io::Error>((statuses, application.session_count()))
        })
        .expect("server task spawn");

    for index in 0..=MAX_HTTP_SESSIONS {
        let principal = [u8::try_from(index + 1).expect("principal index"); 16];
        let response = runtime
            .block_on(request(
                address,
                &handshake(principal),
                SESSION_ROUTE,
                ORIGIN,
            ))
            .expect("session response");
        let expected_status = if index < MAX_HTTP_SESSIONS { 200 } else { 503 };
        assert_eq!(status(&response), expected_status);
        if index == MAX_HTTP_SESSIONS {
            let error =
                ErrorResponsePayload::decode(response_body(&response)).expect("typed error");
            assert_eq!(error.error_code, ErrorCode::QueueFull as u16);
        }
    }
    let (statuses, count) = task
        .join()
        .expect("server task join")
        .expect("server task result")
        .expect("server response");
    assert_eq!(
        statuses,
        vec![200; MAX_HTTP_SESSIONS]
            .into_iter()
            .chain([503])
            .collect::<Vec<_>>()
    );
    assert_eq!(count, MAX_HTTP_SESSIONS);
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
        .spawn_async(async move {
            let connection = server.accept().await?;
            let (request, connection) = connection.read_request().await?;
            let response = application().respond(&request)?;
            connection.write_response(response).await
        })
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
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}
