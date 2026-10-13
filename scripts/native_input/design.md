# Native input verification boundary

Status: Accepted

Date: 2026-10-13

Change class: [patch]

## Contract and ownership

The command `python_native_input_capture.py` retains its arguments and manifest
schema. The entry script calls the canonical CLI. The CLI owns parsing, the
workflow owns process/capture sequencing, frames owns readiness observations,
and Windows leaf modules own ABI records, keyboard delivery, geometry, focus,
pointer visibility/delivery and queue flushing. No leaf imports the CLI or
workflow. Every previous internal importer moves to its owning leaf; the old
entry module exposes no compatibility helpers.

The evidence domain computes manifest values from observed workflow inputs;
it owns the original Win32 labels, encoded lengths/hash and IME boundary
statement. The workflow supplies observations and retains process sequencing.

Windows has an LLP64 data model: LONG and DWORD stay 32 bits, WORD stays 16 bits,
and ULONG_PTR has the process pointer width. Host C `long` is not the Windows
LONG contract on macOS. ABI records use exact signed/unsigned ctypes widths,
natural member alignment and the real INPUT union arms; no manually sized
padding substitutes for HARDWAREINPUT. POINT and RECT share that same leaf
with capture/display consumers. Win32 BOOL signatures use signed 32-bit values;
SendMessageTimeoutW returns signed pointer-sized LRESULT.

For a field of size `s` and alignment `a`, its offset is the preceding occupied
extent rounded up to a multiple of `a`. The aggregate alignment is its largest
member alignment, and its final size rounds the occupied extent to that
alignment. This yields INPUT size 40 on Win64 and 28 on Win32, MOUSEINPUT size
32/24 and KEYBDINPUT size 24/16. POINT is 8 bytes, RECT is 16 bytes and
HARDWAREINPUT is 8 bytes on both widths. These ABI definitions do not claim
installed 32-bit application execution; the current native capture target is
Windows x64.

The absolute move record uses MOVE (0x0001), VIRTUALDESK (0x4000) and ABSOLUTE
(0x8000), whose bitwise union is 0xC001. Press/release records carry no movement.
Unicode input retains UTF-16 unit order; control-enter releases its modifier
even if enter delivery fails. Capture requires actual foreground activation
and rejects a pointer/screen claim when another window owns its sampled point.
Settle budgets, input bounds, manifest values and capture sequencing remain
unchanged through the module split.

## Verification and failures

Tests compare record sizes, all field offsets, alignment, signed coordinate
boundaries and serialized bytes with the Windows declarations. The flag test
uses the literal documented value rather than deriving its oracle from the
implementation constant. Existing Unicode, parser, focus-selection, geometry,
click/wheel and settle assertions remain active. The full script suite must
pass without platform skips added to hide ABI drift. Native Windows input
delivery remains a separate installed-host verification; portable ctypes tests
cannot establish it.

Compiler symbol tables verify that imported module owners and called helpers
remain global through extraction; assigning those same names locally would
cause Python to read uninitialized local bindings before the native operation.
Evidence tests cover every combination of text, shortcut and click plus
observed scroll values. They invoke real formatting and encoding operations;
they do not substitute native delivery or claim installed Windows execution.

## Native capture domain completion

`python_native_capture.py` is also an executable entrypoint only. Its canonical
`native_capture` package owns the bounded frame/window model, PNG decoding and
PNG/BMP encoding, Windows bitmap/process ABI, window lifecycle and observation,
GDI pixel acquisition, native Python presentation, process capture workflow and
CLI. Windows and image leaves depend on the model; workflows depend on those
leaves; only the CLI composes workflows. Capture consumers import each owning
domain directly. Shared RECT remains in the Windows input ABI leaf, with no
dependency on input workflows or compatibility re-export.

The extracted CLI must launch `scripts/python_native_capture.py` for its
isolated wheel child. Using the CLI leaf's own path would change module lookup
and break the hidden `--site` role. Its derived entrypoint names the original
script; all command arguments, dimensions, budgets, decorators, runtime calls,
process cleanup and image/manifest bytes retain the previous contracts.
The process-window convenience forwarder is removed; presentation calls the
canonical discovery operation with its singleton process set.
Verification compares the parser AST and moved definitions after name
normalization, exercises existing codec/CLI tests and runs the full script
suite. Each implementation leaf stays below 500 lines; the executable entry
contains no exported legacy helper surface.

Sources: [Windows data types](https://learn.microsoft.com/en-us/windows/win32/winprog/windows-data-types),
[INPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-input),
[MOUSEINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput),
[KEYBDINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput),
[HARDWAREINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-hardwareinput),
[POINT](https://learn.microsoft.com/en-us/windows/win32/api/windef/ns-windef-point),
[RECT](https://learn.microsoft.com/en-us/windows/win32/api/windef/ns-windef-rect),
[aggregate alignment](https://learn.microsoft.com/en-us/cpp/build/x64-software-conventions),
[message signature](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeoutw).
