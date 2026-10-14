# Native startup evidence

Status: Accepted; diagnostic contract for ADR 0059, Metis 0.5.0.

The current Windows run completes actual Rust module preparation, native
compilation and archive discovery, then observes no page message before the
unchanged readiness deadline. This does not identify a module, browser bridge
or Rust initialization failure. Successful document navigation does not imply
that its JavaScript module or Rust controller starts.

## Observation and authority

The native owner records the actual navigation URI, policy result and completion
success supplied by the existing PAL event surface. Failed navigation retains
its URI in the error. These observations neither advance `PageSession` nor
manufacture a Ready event. Only the existing exact-identity `PageEvent::Ready`
can advance Loading to Ready.

Before that event, the independent acceptance observer may read the actual
owned window's accessible status and rendered image. It sends no input before
Ready and uses the existing fixture deadline. Observer failures remain evidence;
they cannot replace the native failure, establish readiness or skip a journey.
The observer distinguishes an unchanged loader document from an actual Rust
error rendered before the coordinator can publish a correlated failure.

The verification owner retains the actual producer directory and byte identities
for its native module. Browser-build assets cannot substitute for those bytes.
Core's native inventory admission remains authoritative; a Python evidence
receipt does not decode or independently admit that inventory.

## Failure and verification

Early document/mount/bridge acquisition can fail before a coordinator exists;
the raw Rust entry point can then render a document error but cannot promise a
correlated native failure. Loader rejection or a WASM trap can precede Rust
entirely. Neither absence of a Failed message nor successful document navigation
identifies which case occurs. These diagnostic boundaries require actual
Windows observations before a startup correction is selected.

Source checks preserve the closed page protocol, actual Ready admission and all
existing deadlines. The actual Windows run must retain the navigation records,
pre-Ready observation and exact native producer bytes. It must then satisfy the
existing computed form, permission, remount, destruction and package acceptance;
diagnostic output alone does not close native acceptance.
