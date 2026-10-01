use super::*;
use metis_core::error::ErrorCode;
use metis_core::protocol::{ErrorResponsePayload, HandshakeRequestPayload, PROTOCOL_VERSION};
use moirai_async::io::AsyncWriteExt;
use moirai_async::net::TcpStream;
use std::io;

const ORIGIN: &str = "http://127.0.0.1:8080";
const LAUNCHER_PRINCIPAL: [u8; 16] = [0x5a; 16];
const OTHER_PRINCIPAL: [u8; 16] = [0x11; 16];

async fn post_handshake(address: std::net::SocketAddr, principal: [u8; 16]) -> io::Result<Vec<u8>> {
    let body = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: principal,
    }
    .encode();
    let mut client = TcpStream::connect(&address.to_string()).await?;
    let head = format!(
        "POST /v1/session HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {ORIGIN}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    client.write_all(head.as_bytes()).await?;
    client.write_all(&body).await?;
    client.flush().await?;
    let mut output = Vec::new();
    let mut chunk = [0_u8; 512];
    loop {
        let count = client.read(&mut chunk).await?;
        if count == 0 {
            return Ok(output);
        }
        output.extend_from_slice(
            chunk
                .get(..count)
                .ok_or_else(|| io::Error::other("test read exceeded its buffer"))?,
        );
    }
}

fn status(response: &[u8]) -> u16 {
    let line_end = response
        .iter()
        .position(|byte| *byte == b'\r')
        .expect("status line");
    std::str::from_utf8(response.get(..line_end).expect("status line bytes"))
        .expect("status line utf8")
        .split_whitespace()
        .nth(1)
        .expect("status code")
        .parse()
        .expect("status number")
}

fn body(response: &[u8]) -> &[u8] {
    let position = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("response header");
    response.get(position + 4..).expect("response body")
}

#[test]
fn http_application_admits_only_the_launcher_principal() {
    let server = moirai_executor::block_on(HttpServer::bind(
        "127.0.0.1:0",
        ServerConfig::new(
            8,
            4096,
            16,
            metis_core::MAX_PAYLOAD_SIZE,
            131_072,
            Duration::from_secs(2),
        )
        .expect("test limits"),
    ))
    .expect("server bind");
    let address = server.local_addr().expect("server address");
    let origin = HostOrigin::parse(ORIGIN).expect("test origin");
    let application = http_application(&origin, LAUNCHER_PRINCIPAL).expect("application");
    let task = std::thread::spawn(move || {
        moirai_executor::block_on(serve_browser_http_with_response_delay(
            server,
            application,
            2,
            None,
            |error| panic!("unexpected peer failure: {error}"),
        ))
    });

    let other = moirai_executor::block_on(post_handshake(address, OTHER_PRINCIPAL))
        .expect("other principal response");
    assert_eq!(status(&other), 403);
    let error = ErrorResponsePayload::decode(body(&other)).expect("typed error");
    assert_eq!(error.error_code, ErrorCode::InvalidPrincipal as u16);
    let launcher = moirai_executor::block_on(post_handshake(address, LAUNCHER_PRINCIPAL))
        .expect("launcher principal response");
    assert_eq!(status(&launcher), 200);
    task.join()
        .expect("server thread join")
        .expect("finite server budget");
}
