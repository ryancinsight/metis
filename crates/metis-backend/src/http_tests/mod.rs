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

mod cases;
