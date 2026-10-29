//! Real ordered peer admission under the production 30-second read deadline.

use super::*;
use std::io::{Read, Write};
use std::net::TcpStream as BlockingTcpStream;
use std::time::Instant;

const CLIENT_BUDGET: Duration = Duration::from_secs(5);

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "five-second client deadline"))
}

fn timed_handshake(address: std::net::SocketAddr) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + CLIENT_BUDGET;
    let mut client = BlockingTcpStream::connect_timeout(&address, remaining(deadline)?)?;
    let body = handshake(PRINCIPAL);
    let header = format!(
        "POST {SESSION_ROUTE} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {ORIGIN}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    client.set_write_timeout(Some(remaining(deadline)?))?;
    client.write_all(header.as_bytes())?;
    client.set_write_timeout(Some(remaining(deadline)?))?;
    client.write_all(&body)?;
    let mut response = Vec::new();
    let mut buffer = [0_u8; 512];
    loop {
        client.set_read_timeout(Some(remaining(deadline)?))?;
        let count = client.read(&mut buffer)?;
        if count == 0 {
            return Ok(response);
        }
        response.extend_from_slice(&buffer[..count]);
        if response.len() > 128 * 1024 {
            return Err(io::Error::other("response exceeds production byte budget"));
        }
    }
}

#[test]
fn accepted_idle_peer_does_not_block_a_typed_handshake() {
    let runtime = moirai_executor::global();
    let config = ServerConfig::new(
        2,
        16 * 1024,
        64,
        MAX_PAYLOAD_SIZE,
        128 * 1024,
        Duration::from_secs(30),
    )
    .expect("production request bounds with two finite peer slots");
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config))
        .expect("server bind");
    let address = server.local_addr().expect("ephemeral loopback address");
    let idle = BlockingTcpStream::connect_timeout(&address, CLIENT_BUDGET)
        .expect("first idle peer connects");
    // Observe an actual accepted socket before opening the second peer.
    let first = runtime
        .block_on(server.accept())
        .expect("idle peer accepted");
    let service = runtime
        .spawn_async(async move {
            let mut failures = Vec::new();
            let result = moirai_async::timer::timeout(
                Duration::from_secs(40),
                server::serve_accepted(server, first, application(), 2, None, |error| {
                    failures.push(error.code);
                }),
            )
            .await;
            (result, failures)
        })
        .expect("finite service task");
    let client = std::thread::spawn(move || timed_handshake(address));
    let response = client.join().expect("bounded client join");
    // Cleanup precedes assertions on both the red and green paths. The idle
    // socket stays open for the whole second request's five-second budget.
    drop(idle);
    let (service_result, failures) = service
        .join()
        .expect("owned service handle")
        .expect("finite service completion");
    service_result
        .expect("service teardown within 40 seconds")
        .expect("service transport result");
    let response = response.expect("typed handshake while the idle peer remains open");
    assert_eq!(status(&response), 200);
    let handshake = HandshakeResponsePayload::decode(response_body(&response))
        .expect("typed handshake response");
    assert_eq!(handshake.server_version, PROTOCOL_VERSION);
    assert_eq!(handshake.initial_token.principal_id, PRINCIPAL);
    assert_eq!(failures.len(), 1, "only the closed idle peer fails");
}
