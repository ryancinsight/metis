> Windows build/run commands require the [current Rust asset preparation](../../../crates/metis-app/src/frontend/webview/manual.md) before compilation. Capture records retain their cited revisions.

### Capture a complete native application window

The same capture utility can launch any visible Windows application that uses
the Métis surface and save the complete HWND, including its title bar and
client content. The utility waits for the process to become input-idle, finds
the first visible top-level window owned by that process, captures it with
`PrintWindow`, then posts `WM_CLOSE` and waits for an
orderly exit. The process arguments are passed as separate values; no shell
string is evaluated.

```powershell
$target = (cargo metadata --format-version 1 --no-deps |
  ConvertFrom-Json).target_directory
python scripts/python_native_capture.py `
  --command (Join-Path $target "debug\metis-app.exe") `
  --argument=--metis-native-window `
  --argument=60 `
  --argument=2 `
  --argument=0.2 `
  --output output\native-host-window.bmp
```

Pass a second output and a validated client size to exercise the same
application after a real `SetWindowPos` resize. The utility derives the outer
window rectangle from its current Win32 style, reads the resulting client
rectangle back, forces a synchronous repaint, and records the per-window DPI
before and after the operation:

```powershell
python scripts/python_native_capture.py `
  --command (Join-Path $target "debug\metis-app.exe") `
  --argument=--metis-native-window `
  --argument=60 `
  --argument=2 `
  --argument=0.2 `
  --resize 1024 720 `
  --output output\native-host-window-initial.png `
  --resize-output output\native-host-window-resized.png
```

The JSON line printed by the command contains separate `initial` and
`resized` records with outer and client dimensions, image digests, `dpi`, and
`pixels_changed`. A resize capture proves the requested client geometry was
applied and that the application produced a new frame. It observes the
effective DPI; changing the operating-system display scale is a separate
physical-host journey and is not simulated by this utility.

To exercise that physical journey when the host exposes more than one monitor,
move the same window to a monitor selected by deterministic desktop-coordinate
order and capture it again:

```powershell
python scripts/python_native_capture.py `
  --command (Join-Path $target "debug\metis-app.exe") `
  --argument=--metis-native-window `
  --argument=60 `
  --argument=2 `
  --argument=0.2 `
  --move-monitor 1 `
  --output output\native-host-dpi-initial.png `
  --monitor-output output\native-host-dpi-moved.png
```

The monitor probe enumerates real attached monitors, centers the HWND in the
selected work area, lets the owner process handle its bounded `WM_DPICHANGED`
queue, then captures the resulting frame. Its JSON includes monitor bounds,
initial and moved `dpi`, the corresponding fixed-point `display_scale_milli`,
and `dpi_changed`/`pixels_changed` values. Add `--require-dpi-change` when a
machine-specific acceptance run must fail closed unless the selected monitor
has a different effective DPI. A single-monitor or equal-scale desktop is
reported as `dpi_changed: false`; it is not presented as physical transition
evidence. The two PNGs are still inspectable host captures, and no browser or
DICOM data enters this probe.

On the development Windows desktop used for this revision, the probe observed
one `3072×1728` monitor at `96` DPI. Moving the window to monitor `0` produced
identical initial/moved image digests and `dpi_changed: false`; the host
therefore supplies a real single-monitor capture but no heterogeneous-scale
transition claim. A machine with two monitors at different effective DPI can
rerun the same command with `--require-dpi-change` to turn that residual into a
fail-closed acceptance result.

This is host evidence, so the image includes operating-system chrome and can
vary with the Windows theme, scale and font rasterizer. The deterministic
frame and event trace remain the contract-level checks above. The utility is
format-neutral: applications such as RITK supply their own decoded image and
viewer state before handing pixels to Métis; DICOM parsing and medical display
semantics do not enter this repository. Resolving `target_directory` keeps the
command valid with Atlas's shared build cache and with a standalone checkout.

### Capture real Unicode input

`python_native_input_capture.py` composes the [native input domains](../../../scripts/native_input/design.md).
It launches the executable, captures its first frame, focuses its HWND and sends bounded UTF-16 units
through Win32 `SendInput` with `KEYEVENTF_UNICODE`. The optional
`--shortcut control-enter` argument then sends a real Control+Enter key
sequence through the same queue, exercising the production submission policy.
The runner flushes the target window with a bounded `WM_NULL` call, captures
the resulting frame and writes a manifest containing both image digests,
geometry, effective DPI, the input digest, shortcut name and observed
keyboard-layout handle. The input is delivered by the operating-system queue;
the runner does not post application text messages or modify the framebuffer.

```powershell
$target = (cargo metadata --format-version 1 --no-deps |
  ConvertFrom-Json).target_directory
python scripts/python_native_input_capture.py `
  --command (Join-Path $target "debug\metis-app.exe") `
  --argument=--metis-native-window `
  --argument=60 `
  --argument=2 `
  --argument=0.2 `
  --text "東京😀" `
  --shortcut control-enter `
  --initial-output output\native-unicode-before.png `
  --output output\native-unicode-after.png `
  --manifest output\native-unicode.json
```

The capture is Unicode and shortcut evidence, not installed-IME evidence. The
utility deliberately reports `ime.status = not_exercised`:
`KEYEVENTF_UNICODE` delivers committed text and cannot establish a CJK
preedit/convert/commit journey. A machine with an installed IME still requires
the separate physical keyboard journey recorded under the desktop residuals.
The manifest's `input.foreground_window_set` and `pixels_changed` values must
both be true and both images are inspected; with `--shortcut control-enter`,
the after image must also show the submitted result. Foreground activation
denial is a failed capture, and a process exit alone is not a pass.

The same runner records trusted pointer evidence with `--click-client X Y`
and `--scroll-client X Y NOTCHES`. Coordinates are 96-dpi logical client
pixels: the runner is DPI-unaware, so the manifest geometry, both captured
frames and the CLI coordinates share one coordinate space and can be
compared directly at any display scale, and the manifest's `window.dpi`
records the effective scale for consumers that need physical pixels. A pointer claim is refused before delivery when another top-level window owns
the input point or covers the capture surface, naming the covering window
class and handle, and the scroll record reports `frame_changed` alongside
the bounded settle rounds its observation ran.

Since the 2026-10-12 display-scale fix, `metis-app` launches the packaged
WebView2 form with a forced browser device scale factor
(`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` =
`--force-device-scale-factor=<dpi/96>`, probed under a per-monitor-aware
thread context), so the browser's pointer-to-layout mapping matches its
rasterization over the rendered page and the uncompensated button point
(505, 613) submits at 120 effective DPI without compensation. The factor is
fixed when the browser environment is created: a live monitor-scale
transition requires a relaunch, and physical-mouse evidence at a scaled
display remains registered in [gap_audit.md](../../../gap_audit.md) and
[backlog.md](../../../backlog.md).

Wheel delivery was measured on the Metis WebView2 form at 120 effective DPI
through single-notch steps: three consecutive notches shifted the page 125,
125 and 125 logical pixels until the page's 440-pixel scroll range clamped
the fourth step at 65. Notch-to-shift is therefore proportional per notch
and boundary-limited, and the runner's one-record-per-notch pacing (rather
than a coalesced wheel burst) is what the page receives.

### Exercise the installed Windows IME

`python_native_ime_capture.py` is the separate installed-IME journey. It
launches the real production `metis-app.exe`, captures a before frame, focuses
the HWND, activates a requested Windows keyboard layout, toggles the native IME
through Win32 and sends a bounded physical key sequence. It captures preedit,
commit, a second preedit and Escape cancellation only after the IME reports an
open context. Direct combining-mark, emoji and mixed-direction fixtures run in
fresh production hosts through `SendInput(KEYEVENTF_UNICODE)` and are recorded
separately; committed Unicode transport is not presented as IME composition.

The measured command was:

```powershell
D:\miniforge3\python.exe -u -S scripts/python_native_ime_capture.py `
  --command D:\atlas\target\debug\metis-app.exe `
  --argument=--metis-native-window `
  --argument=60 `
  --argument=2 `
  --argument=0.2 `
  --layout japanese `
  --output-dir output/native-ime-measured `
  --manifest output/native-ime-measured/trace.json `
  --source-revision 6c0d553d48dc71bed0311c7c5ce813962eca153b
```

The capture used an 800×600 client, 818×647 outer window and 120-DPI
effective scale. Combining `é`, emoji `👩‍🔬` and mixed-direction
`ABC אבג 123` each appended exactly to the UI Automation value and changed the
captured pixels: `demo` became `demoé`, `demo👩‍🔬` and
`demoABC אבג 123`, respectively. The manifest is
`output/native-ime-measured/trace.json` with SHA-256
`bc1be3276628f450206f8b94ac0da60108fcf4353adaa9136ff9b585376ee0e`; the
production binary SHA-256 is
`d163fcf2b8c5189775a4ebb1d9d9cc30820a3ac64bc8564c254095c3545fdb9`.

The Windows input-method inventory contained only `en-US` with
`0409:00000409`. The runner captured `cjk/before.png`, but the Japanese
probe reported `context: false` and `open: false` both before and after the
IME toggle:
`layout japanese did not expose an open installed IME (before={'context':
False, 'open': False, 'conversion': None, 'sentence': None}, after={'context':
False, 'open': False, 'conversion': None, 'sentence': None})`. The runner
therefore recorded `ime_journey.status = blocked` and captured no preedit,
commit or cancel state. The physical sequence is limited to 32 keys, UI
Automation reads to 5 seconds and process shutdown to 10 seconds. Install and
enable a CJK Windows IME before using this command as V03 composition evidence;
this run remains a measured host residual, not a CJK acceptance result.

The older OS-window captures below demonstrate the visible form and WebView2
shell. RITK's migrated Windows viewer session now supplies a validated frame
through the format-neutral boundary; the integration is tracked in
[RITK PR #267](https://github.com/ryancinsight/ritk/pull/267) and its
[DICOM workflow manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md).
Metis remains format-neutral: it owns the host window, events and framebuffer,
while RITK owns DICOM opening, decoding and medical display semantics.


[Windows native manual](../native.md) · [Current Rust page contract](../../../crates/metis-app/src/frontend/webview/manual.md)
