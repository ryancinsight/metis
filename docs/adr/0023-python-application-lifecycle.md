# ADR 0023: Python application lifecycle

Status: Accepted

Date: 2026-09-09

Driver: [METIS-PYTHON-003](../../backlog.md#METIS-PYTHON-003).

## Context

The PyO3 package exposes validated values and bounded image composition, but a
Python host still needs one Rust-owned application state machine. The same
surface must be usable by desktop, browser and test hosts without making Python
own event routing, framebuffer memory or lifecycle ordering. The existing
`metis-platform::PlatformSurface` already supplies a bounded framebuffer and
FIFO `PlatformEvent` queue.

## Decision

Expose `Application` from `metis._metis`. It stores one `PlatformSurface` behind
a Rust `Mutex` and exposes generation-token operations for clearing, reading a
row-major RGBA frame, enqueueing input and polling events. Construction starts
generation zero. `close` removes the surface; `reopen` allocates a new bounded
surface and increments the generation. Every surface operation validates the
token and rejects closed or stale access with the existing typed Metis error
mapping.

The queue remains bounded by `metis-platform::MAX_EVENTS`; no callback is
stored, no Python object crosses into Rust state, and no second event loop is
created. Python receives a cold-boundary byte copy of the frame and a small
dictionary for each event. The mutex uses PyO3's interpreter-aware lock path,
and state guards drop before Python objects are constructed. Native windows
remain provider-owned, while RITK continues to own DICOM parsing and
medical-display semantics.

## Alternatives

An application-local Python queue would duplicate FIFO and capacity rules and
would allow stale lifecycle state. A callback registry would make execution
re-enter Python and complicate free-threaded safety. A second event loop would
duplicate host scheduling. A native-window class in the stable binding would
make platform-specific availability appear portable. The selected virtual
surface keeps one state owner and leaves host-specific shells at their seams.

## Verification and limits

The built-wheel pytest suite checks exact frame bytes, FIFO event order, queue
rejection, close invalidation, generation-safe reopen and concurrent reads.
This proves the virtual software lifecycle contract. It does not prove native
window integration, browser scheduling or OS sandboxing; those remain provider
tests. Free-threaded Python execution also requires a `cp3XXt` wheel and
interpreter matrix before it can be claimed as release evidence.

## Revision 2026-09-09

The exposed classes now pass a compile-time `Send + Sync` audit, and the module
declares `gil_used = false`. The binding tests add concurrent mutation of one
`Application` and a runtime assertion that a free-threaded interpreter remains
free-threaded after import. The release caller publishes separate CPython 3.9
`abi3` wheels, version-specific `cp314-cp314t` and `cp315-cp315t` wheels, and Python 3.15
`abi3t` wheels. Metis exposes `abi3t` as a package feature forwarding PyO3
`abi3t-py315`; the default feature forwards `abi3-py39`. Atlas runs the same
installed-wheel value tests for each artifact family. The `abi3t` job excludes
musllinux until a compatible Python 3.15t image exists. No registry token,
signing key or private key is used.
