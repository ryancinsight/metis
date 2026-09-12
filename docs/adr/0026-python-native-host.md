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
`cp39-abi3` wheel and passed 22 value-semantic tests, including two independent
hidden windows whose close state and event batches remain isolated. Trusted
installed-IME, visual two-window and non-Windows provider evidence remain open
under V05.

## Revision 2026-09-11

The native binding now detaches the Rust provider request for frame submission,
finite event waits, close and reopen. Borrowed Python frame bytes are converted
before detachment; event dictionaries are constructed after reattachment. The
host thread and bounded command waits therefore do not retain the interpreter
lock, while the public Python signatures and the `Send + Sync`/`gil_used =
false` contract remain unchanged.

## Revision 2026-09-12

The Windows capture utility now accepts a bounded RGBA PNG produced by an
application such as RITK. It validates the PNG header, checksums, dimensions,
decompression length and scanline filters before presenting the decoded bytes;
the source file digest is included in the capture result. This extends visual
evidence without moving DICOM parsing or viewer state into the binding.

The real-frame demonstration uses the public RITK CT/MIP PNG as the input frame
and captures it through the same visible `NativeApplication` path. The Python
tool's decoder tests cover every supported PNG filter and checksum rejection;
the manual image and its source digest are evidence of the handoff, not a DICOM
implementation in Métis.
