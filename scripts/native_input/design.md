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

## Physical-key contract

Given virtual keys `V` and one actual foreground GUI recipient layout `H`, map
every distinct `v` once with `MapVirtualKeyExW(v, MAPVK_VK_TO_VSC_EX, H)`.
Admit only a nonzero low scan byte and prefix zero or E0. A zero mapping, zero
scan, E1 or an unknown prefix fails before any chord input. The admitted
immutable stroke supplies both press and release: `wVk=0`, `wScan=scan`,
`flags=SCANCODE | (E0 ? EXTENDEDKEY : 0) | (up ? KEYUP : 0)`.
No layout recapture occurs between the two halves or members of one chord.
The typed recipient policy distinguishes application input from system
activation. Application input requires the real foreground queue's focus HWND
and uses its thread HKL. The system activation Alt tap explicitly selects the
foreground queue's own HKL: it must operate before focus exists. Neither policy
falls back to the other or to the injector layout. Missing required native
identities reject. These reads are non-atomic and cannot grant foreground or
delivery. A held-stroke context owns one down and one release attempt, retains
the original body failure and adds any release error as context. Nested chord
owners unwind every admitted held stroke without retries or remapping.

Rust commands admit browser physical `code`. VK-only SendInput records with
zero scans do not satisfy that boundary: Chromium's Windows input owner
documents their absent DOM code. The correction belongs in the native input
owner, while the application keeps its existing physical-code policy and
Unicode committed-text route. All callers move to mapped strokes; the old
virtual-key sender and the IME helper's zero-scan keybd_event path are removed.
Mapping a logical virtual key through an HKL does not establish a fixed browser
physical code on every layout. The real fixture and full form establish their
executed host/layout only; arbitrary-layout shortcut parity remains unverified.

Portable tests inspect exact scan/flag fields and reject zero/E1/unknown-prefix
records. A visible real Windows EDIT subclass observes actual SendInput
WM_KEYDOWN/UP and WM_SYSKEYDOWN/UP scan, extended, transition and modifier bits,
including a letter, Alt+Shift+D, SDK-mapped navigation, E0 right-Control,
control-enter and the real
WebView2 focus primer. A single five-second message-pump deadline covers the
fixture. The complete unchanged sixty-second application journey must still
verify browser delivery, commands, calculations and closure.
The Windows key table distinguishes dedicated RightArrow E04D from keypad
Right 004D. Logical VK_RIGHT does not identify that cluster: its delivered
scan/prefix must match the actual layout mapping, rather than a forced E0
expectation. Side-distinguishing VK_RCONTROL supplies the separate E0 oracle:
scan E01D, reverse mapping to VK_RCONTROL, legacy message VK_CONTROL, and actual
control state set on down and clear on up. Both Right forms retain portable
record tests; the native test preserves its Right case and adds right-Control.
Failures retain the actual HKL, full SDK mappings, strokes and native messages.
Shortcut evidence retains its own bounded before/after native focus/modifier
reads and admitted strokes across later UIA polling. They are observations,
not delivery acknowledgements. Existing query order, predicates and budgets
remain authoritative; timeout causes require actual native execution.

Sources: [scan record](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput),
[layout mapping](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mapvirtualkeyexw),
[recipient layout](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getkeyboardlayout),
[physical key table](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input),
[Chromium input owner](https://chromium.googlesource.com/chromium/src/+/main/ui/base/test/ui_controls_internal_win.cc).

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
