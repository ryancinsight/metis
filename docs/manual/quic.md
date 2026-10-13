# Native remote sessions over QUIC

Metis 0.5.0 adds the optional `quic` feature to `metis-ipc` and `metis-backend`.
It consumes Moirai 0.9.0 and `moirai-quic` 0.1.0 from the exact published provider
revision recorded in Cargo.toml and Cargo.lock. Canonical native verification
uses that Git source without development path patches. Default-branch promotion
and Windows/Linux execution remain release gates.

One TLS-authenticated UDP connection supports bounded independent bidirectional
streams. Each stream carries one ordered Metis session, with its own handler,
trusted host context, fresh session MAC key, token, replay sequence and audit.
Commands and events use reliable streams. TLS ALPN is `metis-ipc/1`; one Moirai
message contains exactly one bounded Metis frame. Early data and unreliable
datagrams are disabled. Existing private pipes and browser transports remain
appropriate for those hosts; this feature supplies native remote sessions.

## Runnable authenticated session

The `quic_session` example uses real UDP, mutual TLS, the production backend,
capability handshake, target descriptor, clinical calculation and audited event.
Supply your certificate chains, matching keys and CA roots. The server also
requires the exact authorized client certificate and one fresh 32-byte binary
session MAC key for this single service lifetime. No trust store is installed.
Certificate equality authorizes the client; its SHA-256-derived principal is
only a session label. Native QUIC builds require ring's C/assembly build tools;
ring is linked statically, while OS socket/CRT libraries remain system libraries.

The commands below use a standalone checkout and its canonical Git lock. In
Atlas, a Cargo working directory outside the development path overlay preserves
that lock; keep the shared target directory and non-patch build settings.

Run the server in one terminal, replacing the paths with the actual files:

```sh
cargo run --locked -p metis-backend --features quic --example quic_session -- \
  server 127.0.0.1:4433 localhost server-chain.pem server-key.pem \
  client-ca.pem authorized-client.pem session-mac.key
```

Run the client in another terminal. The server certificate must cover
`localhost`, and its trust root must appear in `server-ca.pem`:

```sh
cargo run --locked -p metis-backend --features quic --example quic_session -- \
  client 127.0.0.1:4433 localhost client-chain.pem client-key.pem \
  server-ca.pem 72.5 4.0 0.5
```

The demonstration inputs produce the existing backend law
`(dose * weight * 60) / (concentration * 1000)`: 0.54375 ml/hr and 2.175 mg/hr.
The client verifies that the audited event contains the same computed result.
It then consumes peer FIN; the server awaits transport acknowledgement before
both endpoints close and drain. The example supports IPv4 and IPv6 addresses.

## Application integration

Construct a `QuicConfig` with `metis_ipc::quic_config`, explicit stream/connection
limits, a queued-handshake budget and finite operation/idle deadlines. Bind a
`moirai_quic::QuicEndpoint` using your existing Moirai spawner. Verify client
identity against application policy before constructing each independent
`BackendService::with_trusted_context`; provide a fresh MAC key per service.
Pass an admitted stream and that service to `serve_quic_session`.

Native blocking hosts use `IpcClient<QuicIpcTransport>`. Native servers await
backpressure through `AsyncIpcServer<T: NativeSessionTransport>`, shared with
the existing WebSocket path. `IpcClient::into_transport().close()` completes a
consumed session; unexpected trailing messages return a protocol error. Reads
and writes that fail, time out or are cancelled invalidate partial stream
framing, while sibling streams retain their state.

Perpetual loss can discard every retransmission of the same byte interval.
The contract returns complete bytes or a finite error; loss rate alone supplies
no delivery bound. Transport selection remains explicit when UDP is blocked.
HTTP/3/WebTransport needs a browser application mapping and is not supplied by
this native binary session protocol.

See [ADR 0058](../adr/0058-native-quic-sessions.md) and
[Moirai ADR 0075](https://github.com/ryancinsight/Moirai/blob/7c54b34c6d2b89ef69ebd0a058683f1fca01584e/docs/adr/0075-quic-multiplexed-sessions.md).
