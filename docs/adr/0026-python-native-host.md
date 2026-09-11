# ADR 0026: Python native host facade

Status: Accepted

Date: 2026-09-11

Driver: [METIS-PYTHON-005](../../backlog.md#METIS-PYTHON-005).

## Context

The PyO3 package currently exposes a Rust-owned virtual framebuffer and input
queue. A wxPython-like consumer also needs a window lifecycle and native event
surface, while the AGENTS contract requires Python to remain a thin conversion
layer: Rust owns state, rendering, event pumping and authority.

`metis-platform` already owns the thread-affine Windows `NativeSurface` and its
bounded `NativeApplication` contract. The Python object must therefore hold a
thread-safe command handle to a Rust host thread, rather than storing a native
window or running a Python callback loop. DICOM parsing, viewer state and
medical display remain RITK responsibilities.

## Decision

Add a `NativeApplication` PyO3 class that sends typed bounded commands to a
Rust-owned native host thread. It exposes validated window construction,
frame presentation, finite event waits, close/reopen generations and typed
event dictionaries. The Windows implementation uses the existing
`metis_platform::native::NativeSurface`; other targets construct no native
window and return `ERR_UNSUPPORTED_PLATFORM_EVENT`.

The facade contains no Python callbacks, second event loop, filesystem,
network, process or DICOM authority. The host thread is the sole owner of the
thread-affine surface. The public stub and installed-wheel tests are updated in
the same change.

## Acceptance

On Windows, the extracted wheel creates a visible bounded window, presents a
known RGBA frame, returns resize/keyboard/text/close events, rejects stale
generation tokens, and closes then reopens with a new generation. A full event
batch is value-semantic and preserves provider fields, including composition,
pointer, wheel, resize and DPI data. On non-Windows, construction returns the
typed unsupported-platform error without attempting a provider call. Every
exposed class remains `Send + Sync` and the module retains `gil_used = false`.

## Verification

The focused gate is `python scripts/python_binding.py` with the built wheel,
`cargo nextest -p metis-platform -p metis-python` where supported,
warning-denied Clippy, doctests and the existing native host capture. The
Windows visible capture is tied to the source revision; `cargo-semver-checks`
reviews the added Python surface. Hosted free-threaded wheel evidence remains
owned by `METIS-PYTHON-004`. The implementation increment built the release
`cp39-abi3` wheel and passed 21 value-semantic tests; visible-window and
non-Windows provider evidence remain open under V05.
