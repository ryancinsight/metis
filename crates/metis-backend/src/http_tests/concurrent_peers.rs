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

fn write_before_deadline(
    client: &mut BlockingTcpStream,
    mut bytes: &[u8],
    deadline: Instant,
) -> io::Result<()> {
    while !bytes.is_empty() {
        client.set_write_timeout(Some(remaining(deadline)?))?;
        let count = client.write(bytes)?;
        remaining(deadline)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "handshake write ended",
            ));
        }
        bytes = bytes
            .get(count..)
            .ok_or_else(|| io::Error::other("write exceeded the request byte count"))?;
    }
    Ok(())
}

fn timed_handshake(address: std::net::SocketAddr) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + CLIENT_BUDGET;
    let client = BlockingTcpStream::connect_timeout(&address, remaining(deadline)?)?;
    handshake_until(client, deadline)
}

fn handshake_until(mut client: BlockingTcpStream, deadline: Instant) -> io::Result<Vec<u8>> {
    let body = handshake(PRINCIPAL);
    let header = format!(
        "POST {SESSION_ROUTE} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {ORIGIN}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    write_before_deadline(&mut client, header.as_bytes(), deadline)?;
    write_before_deadline(&mut client, &body, deadline)?;
    let mut response = Vec::new();
    let mut buffer = [0_u8; 512];
    loop {
        client.set_read_timeout(Some(remaining(deadline)?))?;
        let count = client.read(&mut buffer)?;
        remaining(deadline)?;
        if count == 0 {
            return Ok(response);
        }
        response.extend_from_slice(
            buffer
                .get(..count)
                .ok_or_else(|| io::Error::other("read exceeded the response buffer"))?,
        );
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

#[test]
fn single_connection_pool_rearms_after_a_response_releases_its_slot() {
    let runtime = moirai_executor::global();
    let config = ServerConfig::new(
        1,
        16 * 1024,
        64,
        MAX_PAYLOAD_SIZE,
        128 * 1024,
        Duration::from_secs(30),
    )
    .expect("single provider slot");
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config))
        .expect("server");
    let address = server.local_addr().expect("address");
    let client = BlockingTcpStream::connect_timeout(&address, CLIENT_BUDGET).expect("first client");
    let first = runtime.block_on(server.accept()).expect("first accept");
    let service = runtime
        .spawn_async(async move {
            let mut failures = Vec::new();
            let result = moirai_async::timer::timeout(
                Duration::from_secs(40),
                server::serve_accepted(
                    server,
                    first,
                    application(),
                    2,
                    Some(Duration::from_millis(10)),
                    |error| failures.push(error.code),
                ),
            )
            .await;
            (result, failures)
        })
        .expect("finite service");
    let initial =
        std::thread::spawn(move || handshake_until(client, Instant::now() + CLIENT_BUDGET));
    let repeated = std::thread::spawn(move || timed_handshake(address));
    let initial = initial.join().expect("first client join");
    let repeated = repeated.join().expect("second client join");
    let (result, failures) = service
        .join()
        .expect("service handle")
        .expect("service join");
    result
        .expect("bounded service")
        .expect("pool backpressure must not end service");
    let initial = initial.expect("first handshake");
    let repeated = repeated.expect("second handshake after slot release");
    assert_eq!(status(&initial), 200);
    assert_eq!(status(&repeated), 200);
    assert_eq!(
        response_body(&initial),
        response_body(&repeated),
        "one state owner must return the exact issued grant on replay"
    );
    assert_eq!(
        HandshakeResponsePayload::decode(response_body(&repeated))
            .expect("typed replay")
            .initial_token
            .principal_id,
        PRINCIPAL
    );
    assert!(
        failures.is_empty(),
        "backpressure is not a peer failure: {failures:?}"
    );
}

#[test]
fn cancelling_service_drops_accepted_peers_and_pending_admission() {
    let runtime = moirai_executor::global();
    let config = ServerConfig::new(
        2,
        16 * 1024,
        64,
        MAX_PAYLOAD_SIZE,
        128 * 1024,
        Duration::from_secs(30),
    )
    .expect("two bounded provider slots");
    let server = runtime
        .block_on(HttpServer::bind("127.0.0.1:0", config))
        .expect("server");
    let address = server.local_addr().expect("address");
    let mut idle = BlockingTcpStream::connect_timeout(&address, CLIENT_BUDGET).expect("idle peer");
    let first = runtime
        .block_on(server.accept())
        .expect("actual accepted peer");
    let mut failures = Vec::new();
    let result = runtime.block_on(moirai_async::timer::timeout(
        Duration::from_millis(50),
        server::serve_accepted(server, first, application(), 2, None, |error| {
            failures.push(error.code);
        }),
    ));
    idle.set_read_timeout(Some(CLIENT_BUDGET))
        .expect("bounded close observation");
    let closed = idle.read(&mut [0_u8; 1]);
    drop(idle);
    assert!(
        result.is_err(),
        "the service must still be awaiting peer I/O at cancellation"
    );
    assert_eq!(
        closed.expect("cancelled service closes the actual accepted socket"),
        0
    );
    assert!(
        failures.is_empty(),
        "owner cancellation does not fabricate a peer failure"
    );
}
