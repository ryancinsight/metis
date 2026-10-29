# Bounded HTTP peer lifecycle

The service must admit a valid request while another accepted peer waits for
bytes. The real two-peer regression retains the production 30-second read
deadline and a five-second client deadline; the former serial loop fails it.

One future owns the listener, all peer futures and the sole authenticated
application. Each accepted peer has exactly one pending read or write. A read
completion invokes `application.respond` synchronously, then moves only the
response and connection into its write future. Application state never crosses
a peer I/O await and is never cloned or concurrently mutated. Ready requests
are serialized by observed completion, rather than blocked by accept order.

Accepted attempts never exceed the validated request budget, at most 64.
Pending peer futures cannot exceed accepted attempts. Moirai's configured
connection pool is an additional bound. Its `WouldBlock` admission result
pauses acceptance until a read failure or completed write releases a slot;
read-to-write transition retains the slot and does not re-arm admission.
This preserves smaller provider limits without spinning or retrying a request.

All futures use the existing Moirai sockets, timers and executor. The local
poll operation multiplexes this one finite service; it creates no threads or
detached tasks. Dropping the service drops pending accept/read/write futures,
including their provider reservations and sockets. Peer failures consume their
accepted attempt; fatal errors return after dropping the remaining futures.

The small completion carrier boxes a ready request only until the synchronous
application call. Response data moves directly into its write future. Memory
stays bounded by accepted peers and the unchanged header/body/response limits.
Error logs retain only the transport phase and standard I/O kind, preserving
typed error codes without logging peer payloads, tokens or raw error messages.
