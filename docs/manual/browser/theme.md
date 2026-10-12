# Browser theme

[Browser manual](../browser.md)


## Theme and branding

The **Theme** select applies one of four bounded presentation modes:

- **System preference** sets `data-metis-theme="system"` and follows the
  browser's `prefers-color-scheme` value.
- **Light** sets `data-metis-theme="light"`.
- **Dark** sets `data-metis-theme="dark"`.
- **High contrast** sets `data-metis-theme="high-contrast"` and keeps borders
  and focus indicators legible on a black and white palette.

Rust owns the selected mode through `metis_web::Theme`; the browser host writes
the same value to the document body and `#metis-app`. `styles.css` maps those
attributes to semantic variables such as `--metis-page`, `--metis-surface`,
`--metis-text`, `--metis-accent` and `--metis-focus`. An application can keep
the selector and replace those variables in a same-origin stylesheet to apply
its own palette. Theme state changes presentation only; it does not change
backend authority, IPC messages or application data.

The page loads the starter [Métis vector mark](../../../examples/browser/assets/metis-mark.svg)
from the same-origin `assets/` directory and uses it as the favicon and header
image. A PNG alternate and the multi-resolution
[native icon](../../../examples/browser/assets/metis-mark.ico) remain in the same
directory for browsers and packaged applications that need those formats.
`python scripts/browser.py build` copies all five local assets and fails if a
declared resource is missing. Replace the SVG, PNG, ICO and the `.metis-mark`
rule with project-owned artwork for a branded application. The bundled WAV is
the bounded playback fixture used by the browser trace, and the WOFF2 file is
the bounded font-loading fixture. The browser host does not fetch an icon,
font or media resource from a remote origin.

The packaging boundary admits a strict SVG subset so an installer never stores
an unbounded document language: one fixed positive viewport, optional
`xMidYMid meet`, literal hexadecimal paints, finite numeric opacity and
self-closing path elements. XML declarations, entities, scripts, external
references, unknown attributes, malformed geometry and dimensions over 4096
pixels are rejected before the resource enters a portable or MSI payload.

To demonstrate the contract, build and serve the workbench, select each mode,
and capture the header, view-options card and focus ring at the same viewport.
The static asset suite checks the declared selector values and variable branches;
runtime captures must record the browser engine and operating-system
presentation settings because CSS media preferences are host behavior.

The 2026-09-08 CUA trace used the generated page at 1280×720 CSS pixels and
device scale 1.25. It selected **Light**, **High contrast** and **Dark** in the
native Theme control; each accessibility snapshot reported the matching option
and `View options: ... theme <mode>`, while the inspected screenshots showed the
light, black-and-white and dark palettes with the same mark and layout. A DOM
read after the dark selection reported `data-metis-theme="dark"` on both the
body and `#metis-app`, dark page/text colors and a loaded local mark. This is
explicit mode evidence for one browser engine; system media preference,
forced-colors, the V04 viewport/scale matrix and native host integration remain
separate host checks. MSI shortcut icon wiring is covered by the native package
database test and the distribution workflow.

The **Pointer capture** card demonstrates the browser pointer lifecycle that a
drag interaction needs. Press or drag **Pointer capture surface**. On
`pointerdown`, Rust reads the browser `PointerMetadata` snapshot through
Moirai, captures the identifier on the surface and verifies that the browser
retained it. The status includes the pointer device, viewport coordinates,
changed and held buttons, modifier keys and primary-pointer state. Captured
`pointermove` events update the same metadata record while dragging. On
`pointerup` or `pointercancel`, Rust releases the same identifier and updates
the status to the released metadata record for the trace's first pointer.
The capture handle is owned by the mounted listener set, so **Stop host**
drops the callbacks with the rest of the browser application. The surface
accepts at most two distinct identifiers at a time, captures each through
Moirai and reports a typed browser-host error if a duplicate, third pointer or
provider capture/release operation is rejected.

The 2026-09-07 pointer trace used the generated browser build at a 1280×720
CSS-pixel viewport and device scale 1.25. Its accessibility tree exposed the
surface as a named group, and the full-page screenshot showed the pointer card,
the semantic status and the unchanged backend-result panel. The provider
revision and observed status are recorded in
[browser pointer-capture evidence](../../VERIFICATION.md#browser-pointer-capture-evidence--2026-09-07).
The metadata trace and provider revision are recorded in
[browser pointer-metadata evidence](../../VERIFICATION.md#browser-pointer-metadata-evidence--2026-09-07).
The pointer trace used provider revision
`a3c86cd183a18edc35db30f1d35e79fe80092df4`; the current consumer lock uses
the co-evolution revision `9e045f73be49b9ae6272045dd71705dcf14eccf8`.

The **Wheel** status below the pointer status demonstrates the browser scroll
boundary. Scroll the named **Pointer capture surface**. Rust reads Moirai's
`WheelMetadata` record and renders the horizontal, vertical and depth deltas,
their pixel/line/page unit, viewport coordinates and modifier keys. The
listener prevents the browser default action after the event kind is
validated; the gesture section below applies the bounded pan and zoom policy.

The 2026-09-07 wheel trace used the generated browser build at 1280×720 CSS
pixels and device scale 1.25. An in-app browser scroll action produced
`Wheel: delta (0.00, -129.60, 0.00) pixel at (386, 580), modifiers none`; a
horizontal action produced `delta (426.40, 0.00, 0.00)` at the same target.
The trace is automation-generated browser input; the CUA surface does not
expose the hardware `isTrusted` flag, so this evidence does not claim a
physical-wheel or cross-engine result. The provider revision is
`f634b3a802ec0355da22f111ed01067d2435c5cb`; full command output and the
rendered screenshot are recorded in
[browser wheel metadata evidence](../../VERIFICATION.md#browser-wheel-metadata-evidence--2026-09-07).

The pointer surface also exercises the Rust-owned gesture policy. Drag from
one point to another to pan the content; ordinary wheel input pans by its
normalized CSS-pixel delta; hold Control while scrolling to zoom. With two
captured pointers, move either contact to pan by the pair's centroid and zoom
by its distance ratio. The policy accepts at most two distinct pointers,
rejects a third, clamps pan to ±1024 CSS pixels and zoom to 50–300%,
normalizes line and page units to 16 and 640 CSS pixels, and rejects non-finite
deltas without changing state. `gesture-status` reports the action, pan and
zoom, while the content's CSS transform provides the visible result. A
zero-distance pair waits for a valid baseline before changing zoom. Touch
pointers use the same bounded policy.

The 2026-09-07 gesture trace used the generated build at the same 1280×720
CSS-pixel viewport and device scale 1.25. An upward scroll rendered
`Gesture: wheel pan; pan (0.0, -129.6) CSS px; zoom 100%`; a rightward scroll
then rendered `pan (426.4, -129.6) CSS px`; dragging from `(300, 590)` to
`(420, 620)` rendered `Gesture: release; pan (546.4, -99.6) CSS px; zoom 100%`.
The live screenshot showed the translated content and the semantic status.
The trace is automation-generated; CUA cannot expose hardware trust, native
touch, IME or another browser engine. Native policy tests cover line/page
normalization, bounded zoom and non-finite rejection; the live trace does not
claim physical-input or cross-engine parity.

The two-pointer policy is covered by the native Rust suite: the second pointer
establishes a centroid and finite-distance baseline, movement updates pan and
zoom from that baseline, a zero-distance pair waits for valid separation, and
a third pointer is rejected. The generated page instruction and semantic
pointer card are visible in the [pinch gesture evidence](../../VERIFICATION.md#browser-pinch-gesture-evidence--2026-09-08).
The Codex browser cannot inject trusted physical touch, so a live screenshot
does not claim hardware multi-touch behavior.

The **File drop** card demonstrates the browser file and metadata workflow.
Drag one or more files onto **File drop**. Rust prevents the browser's default
navigation, asks Moirai for a bounded `DropFiles` capture, and renders the file
count and the first three display names with byte sizes. The zone exposes
`dragenter`, `dragover`, `dragleave` and `drop` state through the semantic
`drop-status` region and its `data-drop-state` attribute. A rejected metadata
record leaves the zone in the typed rejected state and reports the provider
error without retaining the batch.

For an accepted drop, the browser host reads every selected file asynchronously
through Moirai's owned browser `File` handles. Each file is limited to 64 MiB
and the batch to 256 MiB; a 64 KiB continuation buffer keeps each `FileReader`
turn bounded while the provider still enforces its 1 MiB maximum read. The
consumer reports `reading` and then `complete` or `failed` through
`drop-byte-status` and `data-byte-state`. The handoff performs no format
classification. A completed batch is available to a trusted WASM consumer
through `metis_web::take_file_drop`, which transfers ownership of the batch.
Call `FileDropBatch::into_files` and
`FileDropPayload::into_parts` when the consumer must retain the entries; moving
the tuple preserves each byte allocation. A later drop replaces an unconsumed
batch, and stop/remount drops it. No browser name is turned into a filesystem
path. A RITK adapter receives the named bytes and performs any format-specific
scan, decode and study opening; this slice closes the bounded zero-copy byte
handoff, not a format-specific workflow.

The provider caps one drop at 512 files, 4096 UTF-8 bytes per name and 256 bytes
per media type; the consumer keeps the batch below its separate 256 MiB byte
budget. The CUA browser surface cannot synthesize a trusted
operating-system file drop, attach a local file to a synthetic event or expose
`isTrusted`, so a manual trace must record the browser engine and whether the
drop came from a physical file operation. Native policy tests cover bounds,
typed rejection and payload size budgets; the WASM gate compiles the real
provider-backed read path.

The 2026-09-08 CUA trace opened the generated build at a 1280×720 CSS-pixel
viewport with device scale 1.25. The accessibility tree exposed **File drop**,
both status regions and the named **File drop zone** group; the
screenshot showed the drop card between the pointer and backend-result cards
with its ready state and focus outline. The browser engine version was
unavailable, and no trusted local file was attached, so the trace does not
claim a successful live byte read or format-specific decode. The provider-backed
full batch path is established by the native policy suite and the strict WASM
build; the RITK consumer smoke is now recorded in the
[RITK DICOM workflow](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md#inspect-the-browser-canvas-visual-smoke).
The ownership-consuming API is covered by a pointer-identity test, so moving
a completed batch does not copy its file contents.

The **Text and composition** card exercises the browser's native editing
surface while keeping application state in Rust. Focus **Clinical note**, type
ordinary text, select a range with the pointer or keyboard, and inspect
`text-status`, `selection-status` and the `data-selection-*` attributes. The
seeded value includes an accented character, an em dash, CJK text and mixed
symbols so the displayed selection offsets make the browser's UTF-16
coordinate convention visible. The `text-preview` line is regenerated from
the bounded Rust value after every input event.

For an IME-capable environment, switch to a CJK input method and type into
**Clinical note**. `compositionstart` and `compositionupdate` show the bounded
preedit and locale, `compositionend` reports the commit transition, and
`compositioncancel` clears the preedit without inventing a committed value.
The browser `InputEvent` snapshot also reports its operation name, optional
data and composing marker. The listener set is dropped with **Stop host**, so
remounting starts with the seeded value and a fresh selection.

The 2026-09-08 CUA trace opened the generated page at
`http://127.0.0.1:8095/?cache=text-clean-20260908` in a 1280×720 CSS-pixel
viewport at device scale 1.25. The accessibility tree and inspected screenshot
showed the seeded value and a `16`-unit caret. Browser `typeText` input appended
` typedX`; the rendered value became `Résumé — 東京 / 影像 typedX`, the text
status reported `Text: input insertText applied; data X`, and the selection
status reported a `23`-unit forward caret. This is ordinary browser input
evidence; CUA does not expose a trusted operating-system IME or `isTrusted`.

The 2026-09-17 CUA trace reopened the same page at
`http://127.0.0.1:8095/?cache=text-navigation-20260917` in the same
`1280×720` CSS-pixel viewport at device scale `1.25`. Pressing **End** and then
**ArrowLeft** in **Clinical note** left the seeded value unchanged, moved the
caret from UTF-16 offset `16` to `15`, rendered
`Text: keyboard ArrowLeft moved selection`, and set
`data-text-state="navigated"`. The screenshot showed the focused textarea,
the visible navigation status and the updated selection line. This is a local
Chromium observation of the HTML5/CSS surface; it does not claim bidi layout,
line metrics or trusted IME behavior.

The same input listener keeps the browser's `InputEvent.inputType` as bounded
metadata and classifies it as edit, delete, paste, cut, undo, redo, composition
or other. The mapping includes quotation paste, line or word deletion, drag
deletion and transpose insertion from the W3C [Input Events] vocabulary. The
browser performs its native keyboard clipboard or history action before the
event reaches Metis; the resulting value and UTF-16 selection are validated by
the Rust policy and the status line reports the semantic operation together
with the raw operation name. For example, native undo reports
`Text: input undo (historyUndo) applied; data none`, while ordinary typing keeps
the existing `Text: input insertText applied; data ...` status. This is operation
classification and value validation; it is separate from the explicit browser
clipboard controls below.

The text card also contains **Read browser clipboard** and **Copy note to
clipboard**. A click starts Moirai's bounded browser promise inside the click
callback, preserving the browser's transient user-activation rule. Read text is
limited to 1 MiB, validated by `TextState`, written into the note and reported
with the accepted UTF-8 byte count; writes report the bounded byte count after
the browser confirms the promise. `clipboard-status` exposes `ready`,
`unavailable`, `reading`, `writing`, `read`, `written` and `failed` states.
Secure-context, permission and activation failures remain visible as failed
states. This demonstrates the real HTML5/CSS control path, but it does not
claim trusted OS clipboard contents, permission grants, assistive-technology
behavior or native IME delivery. Browser keyboard clipboard/history behavior
and the explicit provider are both host contracts; RITK remains the owner of
DICOM parsing and presentation.

[Input Events]: https://w3c.github.io/input-events/

The policy caps the value at 1 MiB, event metadata at 128 UTF-8 bytes and the
locale at 64 bytes. UTF-16 offsets are transport coordinates; the Rust policy
rejects an offset inside a surrogate pair or Unicode extended grapheme cluster
before it changes state. After the browser performs its default action, a
`keyup` listener records `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`,
`Home`, `End`, `PageUp` and `PageDown` selection snapshots through the same
policy. The text status reports the navigation key and the textarea exposes
`data-text-state="navigated"` with the accepted UTF-16 selection. Other keys
retain the browser's native editing behavior. This workflow does not claim
bidi shaping, line metrics, fallback-chain metrics, trusted clipboard contents or
permissions, assistive-technology behavior or native IME delivery; the
operation classifier only records the browser event and resulting value.
CUA can show the real HTML textarea, statuses and focus ring, but it cannot
provide a trusted OS IME event or expose `isTrusted`; record the browser engine
and input method when collecting host evidence.

[Input Events]: https://w3c.github.io/input-events/
