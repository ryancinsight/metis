# Framework conformance revision history

Revision 2026-09-24: three comparator rows move. Shortcuts: `metis_core::input`
parses Tauri/Electron accelerator text, resolves browser `KeyboardEvent` codes
and Windows virtual keys onto one key vocabulary and refuses conflicting
bindings in a bounded `ShortcutMap`; the native window and the browser
workbench both activate the local commands through it, and menus announce the
bindings with `aria-keyshortcuts`, which the semantic tree now carries. These
are application-scoped accelerators, not OS-global hotkeys. Large lists:
`metis_ui_lang::VirtualList` supplies the windowing egui's `show_rows` and
GPUI's `uniform_list`/`list` provide — constant-time uniform extents and
logarithmic variable extents in a Fenwick tree — with no virtualized widget
built on it yet. Developer loop: `metis dev --watch`, the analogue of
`tauri dev`, now runs on every host through a polling fingerprint where the
host has no native change notification. Glyph coverage is memoized across
repaints with bit-identical output, cutting the benchmarked keystroke repaint
from 1.01 ms to 0.21 ms; matched comparator measurements remain open.

Revision 2026-09-19: `CapabilityScope::READ_FILE` and the native
`ScopedFileProvider` now form the first concrete OS-authority provider. The
provider requires a host-policy witness, anchors every read below the selected
root through Moirai's directory-handle opener, enforces a 64 MiB read bound and
rejects traversal, links, directories and oversized files. The focused
provider tests prove both successful bytes and missing-scope denial; RITK still
owns DICOM parsing and series policy.

Revision 2026-09-05: the software V01 runner now binds seven real backend states
to recorded semantics, exact raster comparisons and source/fixture provenance.
Deliberate text, geometry and color changes exercise regression detection;
browser/native host evidence remains with the host items.

Revision 2026-09-06: the user names `ritk-snap` as the concrete viewer migration
target. [METIS-MIGRATION-001](../../../backlog.md#METIS-MIGRATION-001) now includes
its egui/eframe surface and [V09](../../VERIFICATION.md#V09) DICOM opening/display
oracles. RITK retains format, geometry and medical-display ownership; this does
not replace the distinct Tauri fixture or claim a working Métis viewer.

Revision 2026-09-06: Iced 0.14.0 is added as a source-pinned comparator. Its
Elm-style state/update/view model, native runtime, WebAssembly renderer path,
async tasks and headless/E2E testing direction inform Metis's state, host and
visual contracts. The archived `iced_web` DOM runtime is not treated as current
Iced behavior, so it does not close Metis's HTML5/CSS reuse gap.

Revision 2026-09-06: the browser slice adds `metis-web`, a real HTML5/CSS host
that loads generated WASM and drives Rust-owned controls through Moirai's DOM
handles. The local trace proves editable state, typed invalid-input rejection
and explicit missing-bridge failure. It does not close the authenticated live
service, origin/session, accessibility or cross-engine rows.

Revision 2026-09-07: Moirai `16a1b88` adds cancellable browser-local tasks.
Metis request-table cancellation and `metis_stop` listener teardown now have
native and browser lifecycle evidence; authenticated live-service task wiring,
allocation measurement and cross-engine rows remain open.

Revision 2026-09-07: Metis adds [ADR 0011](../0011-host-authority-policy.md),
which defines exact origin/window/session admission, host-bound capability HMAC
associated data and the strict external-asset CSP/navigation policy. Core
substitution tests and browser asset checks close the local authority-kernel
increment; service-side Origin validation, OS permissions and live bridge
evidence remain open.

Revision 2026-09-07: Moirai PR #269 added the bounded native WebSocket service
and PR #270 merged at `be87d009cd0e877beef719b47bdcbadc45659069`. Metis now
validates the browser Origin before `101 Switching Protocols`, runs the framed
`AsyncIpcServer` under a trusted host context, and exercises the live loopback
bridge in the browser. Post-drop allocation, TLS, cross-engine and desktop
host evidence remain open.

Revision 2026-09-07: Moirai `d879779247c8cfc5870f62f99a5364cbbf2d3c58` adds the
checked-state and disabled-control DOM seams. Metis now demonstrates Rust-owned
checkbox, radio and range controls, select state and submit lifecycle gating
while menu/dialog, pointer capture, IME, accessibility technology and
native-window input remain open.

Revision 2026-09-07: Moirai `8f02b8b7de6cf6361b519bd79759d8508568fbdb` adds typed
HTML dialog open/close/state and focus seams. Metis now demonstrates modal
session details, provider-backed close and opener focus restoration in the
authenticated browser trace. Pointer capture, IME, accessibility technology and
native-window input remain open.

Revision 2026-09-07: Moirai PR #276 merged at
`5a5e4b1540eff39bc3f082c6907f0c82fa14dcc8` adds pointer-event identifiers and
owned Element capture, state and release calls. Metis now demonstrates a
Rust-owned pointer surface that captures and releases ID `1` in the browser;
drag/drop policy, wheel input interpretation, touch gestures, IME, accessibility technology
and native-window input remain open.

Revision 2026-09-08: Moirai PR #279 merged at
`630f914bcb34d4d65cc5e3db27a121163d040199` adds bounded browser file-drop
metadata. Metis renders a semantic drop zone and revalidates names and media
types before retaining at most 64 entries. File bytes, filesystem paths,
multi-touch/pinch interpretation, IME, accessibility technology, native event
production and OS pump remain open.

Revision 2026-09-11: Moirai PR #327 merged at
`fd836cd1715e1cd6f537290ce670829ba875be94` raises the shared browser metadata
bound to 512 entries so the committed 409-slice DICOM study fits one bounded
drop. The consumer-owned 256 MiB byte budget remains unchanged; DICOM parsing,
geometry and viewer state remain in RITK.

Revision 2026-09-07: Moirai PR #277 merged at
`a3c86cd183a18edc35db30f1d35e79fe80092df4` adds the copyable
`PointerMetadata` snapshot for device type, CSS-pixel coordinates, button
state, modifier keys and primary-pointer state. Metis renders the snapshot on
capture and movement. Moirai PR #278 merged at
`f634b3a802ec0355da22f111ed01067d2435c5cb` adds `WheelMetadata` for bounded
deltas, browser units, viewport coordinates and modifier state; Metis renders
vertical and horizontal scroll traces. Metis now applies a Rust-owned bounded
single-pointer drag pan, two-pointer centroid/distance pinch pan and zoom,
wheel pan and Ctrl+wheel zoom policy; drag/drop and native event production
remain open.

Revision 2026-09-08: Moirai PR #280 merged at
`0862716265d657b8069d5a47fd1e77ae26ddd006` adds bounded browser text values,
UTF-16 selection snapshots and input/composition metadata. Metis now owns a
bounded textarea policy with input, selection and composition lifecycle
listeners; grapheme segmentation, bidi/layout metrics, clipboard/undo, trusted
native IME, accessibility technology and native event production remain open.

Revision 2026-09-16: `TextState` rejects UTF-16 selection offsets inside
Unicode extended grapheme clusters using the pure-Rust UAX #29 segmenter. The
browser host still owns caret movement, bidi/layout metrics, clipboard/undo,
trusted native IME, accessibility technology and native event production.

Revision 2026-09-17: the browser text listener records the post-default
selection for the bounded navigation-key set on `keyup` and validates it with
the Rust scalar and extended-grapheme policy. Native browser caret behavior is
preserved; bidi/line metrics, clipboard/undo, trusted native IME and
accessibility technology remain host evidence.

Revision 2026-09-17: the text policy classifies the browser's bounded
`InputEvent.inputType` values as edit, delete, paste, cut, undo, redo,
composition or other. Metis retains the raw operation name and validates the
value and selection produced by the host's default action before rendering the
operation status. This records host clipboard/history behavior without adding a
second clipboard or undo implementation to Metis.

Revision 2026-09-17: the classifier covers the W3C [Input Events] edit,
deletion, quotation-paste and transpose names that can reach a text control;
unknown names remain `other` so the boundary does not guess at future browser
operations.

Revision 2026-09-18: the browser accessibility probe records a bounded native
accessibility tree through Chromium and Microsoft Edge's
`Accessibility.getFullAXTree` WebDriver endpoints, checking the admitted role
and accessible-name contract. Firefox and WebKit record an explicit protocol
unavailability result; screen-reader speech and operating-system accessibility
bridges remain host evidence.

[Input Events]: https://w3c.github.io/input-events/

Revision 2026-09-08: Metis now retains two browser pointer identifiers and
applies a bounded centroid/distance pinch policy. Duplicate and third-pointer
presses are rejected, and the remaining physical-touch, cross-engine and
native-host evidence limits stay open.

Revision 2026-09-08: Moirai PR #283 merged at `3ae43143` adds a bounded Win32
window provider and finite event wait. Metis exposes `NativeSurface` over that
provider, and the `metis-app --metis-native-window` role composes the adapter
with the production frontend and supervised private IPC, including input,
resize, DPI, focus and close transitions. At that revision a committed native
visual capture, WebView2, permissions, accessibility/IME and non-Windows
providers remained open.

Revision 2026-09-08: Moirai PR #284 merged at `7f5ddf80` makes the provider
return retained lifecycle events before waiting on the operating-system queue.
Metis advances its lock and verifies the initial resize event through the same
native adapter test; visual, permission, accessibility/IME and non-Windows
evidence remain open.

Revision 2026-09-08: Moirai PR #286 merged at `c91e2cdd` adds bounded native IME
composition phases, and PR #287 merged at `7ad8eeee` closes cancellation when a
composition message carries no string. Metis consumes preedit, commit and
cancellation through the native host; installed IME, accessibility and visual
journeys remain open.

Revision 2026-09-08: Moirai PR #289 merged at
`5c8a9e8be32ad6beac14ed263c2f11c3663b87cb` adds bounded browser file access.
Metis now captures validated `DropFiles`, reads the accepted batch through
provider-owned browser `File` handles and exposes one bounded named-byte handoff
slot. That pre-boundary revision still surfaced a format marker; PR #32 removed
the decision. Full DICOM parsing and study decoding remain RITK work; native file
grants, trusted physical-drop evidence and cross-engine evidence remain open.

Revision 2026-09-08: htmx is added as a hypermedia interaction comparator. Its
event→request→target→swap model informs a typed Rust/WASM dispatcher and
allowlisted DOM targets, while arbitrary server markup, JavaScript filters and
response-header commands remain outside Metis's admitted contract. The browser
host now delegates ordinary input and control events at the mounted root;
specialized pointer, file, text and dialog listeners retain their own typed
providers.

Revision 2026-09-09: [ADR 0022](../0022-typed-browser-actions.md) delivers the
first authenticated action/patch path. The existing session-details trigger
uses the scoped `ui` plugin and a generation-bound binary action; the browser
preflights bounded text/attribute patches against its target policy before
applying them. This is a presentation seam only. DICOM scanning, decoding,
series selection, geometry and viewer semantics remain in RITK.

Revision 2026-09-09: the matrix now reflects the delivered state and style
contract. State, command and async rows point to their completed current
surfaces; native Windows HWND support is distinguished from the unimplemented
system WebView host; and the software renderer rejects custom declarations it
cannot honor.

Revision 2026-09-09: the software display list admits validated raster image
placements. Source crops are checked against bounded image storage, destination
rectangles are clipped before iteration, and nearest-neighbor pixels use the
framebuffer's source-over alpha contract. Format decoding, orientation metadata,
browser media and accelerated vector paths remain with their owning work items.

Revision 2026-09-13: the software display list admits bounded width-aware
polylines with explicit endpoint caps and vertex joins. The renderer classifies
each visible pixel once, applies a four-radius miter bound with bevel fallback,
and retains the existing one-pixel segment primitive. Normalized affine image
placement is now available through the same raster surface; GPU vectors,
arbitrary affine vector paths, browser vector parity and device-loss recovery
remain open.

Revision 2026-09-09: the Windows native and packaged WebView2 application roles
have committed initial and successful form captures with trusted keyboard and
pointer actions. The capture manifest records the source revision, runtime,
window sizes and image hashes; the supervised runtime admits only the operating-
system path variables needed by WebView2. Native accessibility, installed IME,
OS permission probes, physical resize/DPI evidence and non-Windows hosts remain
open.

Revision 2026-09-09: Metis PR #32 removes the remaining browser-side DICOM
candidate and Part 10 marker decisions. The browser handoff reports bounded
file metadata and byte progress only; RITK owns format scanning, decoding,
geometry and medical-display semantics.

Revision 2026-09-10: Axum 0.8.9's server/router contract is added as a
server-side comparator. Its `Router`, typed `State`, extractors, middleware and
`IntoResponse` surfaces were initially compared without adding an Axum
dependency; Moirai remains the first-party transport owner. The 2026-09-11
revision below records the admitted loopback target. DICOM parsing, study
selection, geometry and medical display remain exclusively in RITK.

Revision 2026-09-11: `METIS-AXUM-001` admits
and implements a loopback-only `metis-app --metis-http-service` demonstration
over Moirai. The route, session, deadline and typed-fragment evidence closes
the local presentation boundary; public deployment, TLS and DICOM behavior
remain outside this decision and RITK remains the format owner.

Revision 2026-09-11: `metis-web` exposes a format-neutral borrowed
`CanvasFrame`/`CanvasSurface` seam over Moirai's bounded HTML5 canvas provider.
RITK's `WebCanvasPresenter` implements the seam for its existing
`PresentationFrame`; this keeps DICOM parsing, geometry and medical-display
policy in RITK while allowing the browser viewer to migrate from eframe in a
later increment. The typed consumer boundary is verified; a live browser
canvas capture and the complete three-view viewer shell remain open.

Revision 2026-09-11 (provider pin): Moirai PR #321 merged at
`ccdc878dad21d9daeedc966f1cb97364e2558593`, including the bounded canvas
provider and its stabilized worker-quiescence test. Metis's standalone lock
now resolves every direct Moirai package to that merged revision so the
borrowed canvas seam is available to the locked browser gate. This changes no
ownership boundary: DICOM parsing, geometry and medical-display policy remain
in RITK.

Revision 2026-09-11 (visual provenance): the provider pin changes the visual
fixture identity without changing any capture pixels or semantic observations.
The committed `captures.json` provenance was refreshed through
`scripts/verify.py --update-snapshots`; the exact full gate then passed through
the software visual comparison. This is a fixture provenance update, not a
relaxed image or semantic oracle.

Revision 2026-09-11 (RITK browser smoke): RITK PR #284 records a local browser
run of the packaged `ritk-snap` library through this borrowed canvas seam. The
synthetic Part 10 byte batch was accepted by the Métis host and decoded by RITK;
the RITK canvas returned a visible frame and the inspected PNG is linked from
the RITK DICOM workflow manual. This closes the local consumer byte-to-frame
smoke only. Trusted physical browser input, configured cross-engine drivers,
GPU presentation and full-window capture remain open; Métis still contains no
DICOM parser, metadata, geometry or viewer state.

Revision 2026-09-11 (RITK browser input): [RITK PR #290](https://github.com/ryancinsight/ritk/pull/290),
merged as `d6acfc3e`, routes the bounded Métis canvas pointer and wheel records
through RITK's existing presentation reducer, including orthogonal-axis routing
and pointer-cancel cleanup. The format-neutral input and frame seams stay in
Métis; RITK remains the owner of DICOM parsing, metadata, geometry and viewer
state. Physical and cross-engine browser evidence is still an acceptance gap.

Revision 2026-09-11 (format-neutral host comments): commit `8ea5935` changes
only ownership comments in the host boundary. The visual fixture provenance was
regenerated for that source revision; capture hashes and semantic records remain
unchanged, and the exact full gate passes.

Revision 2026-09-15 (RITK native consumer): RITK PR #392 merged at
`3f46a08bf` and exercised a pathless `ritk-snap --metis-native` launch through
the Windows folder picker. The selected public 94-file MRI-DIR T2 study was
scanned, decoded and rendered as three orthogonal planes by RITK; the reviewed
window capture and the 1280 × 800 framebuffer are recorded in RITK's
`dicom-metis-real-mri.json`. Cancellation and an unreadable replacement preserve
the previous framebuffer. Metis supplies the bounded selected path and window
surface; DICOM discovery, decoding, geometry and clinical presentation remain
in RITK.

Revision 2026-09-15 (package and browser consumers): RITK's locked Windows
package run `34990164849` verified the portable executable, per-user MSI,
inventory and `--help`; its local replay opened the public 410-file CT study
and the install/uninstall lifecycle completed without residual state. The
package provenance records that signing and publication were not performed.
RITK's cross-engine chooser run `34984468153` passed exact three-canvas pixel,
file-hash, rejection, cine-rate and teardown oracles on Chromium 152 and
Firefox 155. Safari 26.6.2 accepted the 94-file selection but rejected every
bounded read probe before presentation, so the WebKit/SafariDriver grant stays
an external residual owned by the browser provider boundary. These runs add
real consumer evidence without moving DICOM behavior into Metis.

Revision 2026-09-15 (V12 lifecycle fixtures): the three-run eframe and native
Métis records now bind the public 409-file CT MIP semantics, and the native
Métis record also binds the public 94-file MRI study, to capture digests,
executable revisions, process-tree samples and explicitly approximate 95%
half-widths. The application manual presents the rows together and keeps their
surface and process-boundary differences visible. No matched GPUI or Tauri
fixture, WASM used-memory or allocation count, compositor/input latency or
security probe has run; the records document fixture evidence and do not rank
frameworks.

Revision 2026-09-16 (stable browser canvas extents): Moirai merge `5cf572f7`
keeps a validated canvas bitmap at its current width and height when repeated
RGBA frames have the same extent; a changed extent still takes the bounded
resize path. Metis pins all direct Moirai packages to that revision. This
reduces redundant browser bitmap-reset work without claiming a measured WASM,
browser-heap or process-memory improvement; those V12 measurements remain
open.

Revision 2026-09-16 (content-box consumer pin): Metis's standalone lock now
resolves every direct Moirai package to merge
`c9a4431b2320adb8cd3e5a5f12d1632c18378950`, which retains the extent guard and
the bounded content-box mapping used by RITK's browser canvases. The hosted
RITK chooser matrix binds this provider to the real 94-file study: Chromium and
Firefox pass the exact pixel, file, rejection and teardown oracles, while
Safari's selected-file read remains an external WebKit authorization failure.
This revision does not move DICOM parsing or clinical presentation into Metis.

Revision 2026-09-16 (explicit browser GPU surface): Metis now pins Moirai merge
`21b66ba424ad8f50d8574d6e9714be696f807e82`, which adds an explicit asynchronous
WebGPU canvas provider. `CanvasSurface` keeps its existing raster constructors
and exposes opt-in GPU constructors over the same borrowed RGBA8 and bounded
input contracts; adapter or device setup failures remain typed unsupported
errors with no implicit fallback. A real browser GPU run, device-loss recovery,
compositor behavior and resource measurements remain open.

Revision 2026-09-18 (explicit browser GPU recovery): Moirai PR #400 merged at
`7aa9d4e27fe8d12c0129accddccbf70bbc796e08` and adds explicit provider
recreation. `CanvasSurface::recreate` retains the canvas and input listener
guards while replacing provider handles only after successful setup; raster
surfaces reject recovery and no path falls back implicitly. Real recovered GPU
pixels, compositor behavior and resource measurements remain open.

Revision 2026-09-16 (browser aggregate-memory observation): the format-neutral
workbench, canvas and saved-study runners accept an opt-in
`--browser-memory-sample` probe. A secure, cross-origin-isolated browser may
report `performance.measureUserAgentSpecificMemory()` as an explicit aggregate
estimate, with bounded timeout and unavailable/rejected records. The values
are implementation-dependent and do not close WASM allocator, native-process,
allocation or matched-framework evidence.

Revision 2026-09-16 (hosted typed fragment matrix): Metis workflow run
[35143857171](https://github.com/ryancinsight/metis/actions/runs/35143857171)
passed the real authenticated fragment service on Chromium 152, Firefox 155
and WebKit/Safari 26.6.2. Each trace records the health and one-patch
handshake, malformed/unauthorized/target/stale rejection probes, generation
reset/remount and clean session teardown. This closes the configured fragment
matrix; provider-private allocation/listener counts, TLS, operating-system
permissions, physical input and the RITK WebKit selected-file read remain
separate evidence requirements.

Revision 2026-09-18 (real eframe MRI fixture): RITK PR #477 merged at
`6e067a592fd718afbefde1f62f14ceb42eca8c4f` adds a reviewed eframe/egui
compatibility capture of the same public 94-file MRI-DIR T2 study used by the
Métis MRI fixture. The three-run record reports a 1600 × 1000 surface with a
series browser, three orthogonal planes and a GPU MIP; the current Métis row is
1280 × 800 with three planes. The V12 table now carries both rows and their
uncertainty, but the mismatch keeps them descriptive rather than comparative.
The next performance increment is a producer-owned common surface contract
before any GPUI or Tauri run is ranked, followed by separate WASM allocator,
compositor/input latency and host-permission measurements.

Revision 2026-09-18 (matched eframe orthogonal fixture): RITK PR #480 merged
at `0fd885adc6124468aa745a2f548dd685b74f0774` adds a shell-free eframe
presentation with the same axial, coronal and sagittal semantic surfaces as
the Métis MRI record. The capture reuses RITK's physical-spacing placement and
binds real saved-study pixels and process samples. Its 1600 × 1000 host extent
and process boundary remain different from Métis, so the record closes the
semantic prerequisite without establishing a framework resource ranking.

Revision 2026-09-18 (Svelte comparator): official Svelte 5 documentation is
added as a source-pinned web-component comparator. Svelte's compiler and runes
describe a JavaScript/CSS component model, while SvelteKit adds configurable
SSR, CSR and prerendering deployment modes. Custom-element output is useful for
DOM interoperability analysis, but none of these documents establishes Rust or
WASM memory, authority, native-window or installer parity. Metis therefore
keeps Rust-owned state, Moirai transport and HostPolicy as the authority path;
Svelte and SvelteKit remain comparison subjects with no dependency or runtime
added to the workspace.

Revision 2026-09-20 (historical gap plan): the RITK consumer replay used
geometry source build `b432ae69db2a0578d8f1eb176ae6fdb9233231a9`, delivered
in RITK PR #518 merge `694904718d7ec922883ab2a6b572e9aa29edab99`, with Metis
`165c4ec923e76ea7bc32b6b4fb99b4338166b3`, Moirai
`2a54e010532f76c88027fec8a468620c92fe66b3` and standalone lock
`4f4b96958a0545203775e21b3bd1152a864bd6b77847d2b638213d6b420b1084`.
The locked 94-file MRI replay remains byte-identical at 1280 × 800 and
continues to prove the format-neutral Metis handoff plus RITK-owned decoding,
geometry and clinical presentation. Historical replay hashes remain in their
original evidence records and are not current comparison inputs.

Revision 2026-09-22 (current lock refresh): RITK PR #588 merged as
`27ac7d12bc69a04f7fb12e5ba41ef8fd4fe93aa9` and refreshed the authoritative
standalone replay from source `b671ae72a20bb6d113de35314e73874f197d2a83`, Metis
`ad27d2148cc6b8fa9702cbbd4f37d9e106aa6535`, Moirai
`0e2e1bbb2d81e16dd9c694ba46a9e9710e034417` and Cargo.lock SHA-256
`c1ccbc7785cce37d4e2322002fd19b0bb4ab03fec03ab7d4154fa6cfbcf470b1`. The
saved 94-file MRI-DIR study still produces the byte-identical 1280 × 800 frame
(411,589 non-black pixels); executable SHA-256 is
`2f44ae2f9a3facc7c8659e5ac246af235021ab9a3e84ffabbff9d2d823099035`.
DICOM discovery, decoding, geometry and clinical presentation remain RITK-owned,
while Metis remains the format-neutral host.

The next dependency-ordered closure is explicit. Moirai and Metis now carry
typed capability scopes into real file and process providers with denial tests
at each boundary; network and device providers, plus OS sandboxing, remain
open. The current origin/window/session HMAC policy authenticates a caller but
is not an OS sandbox. Second, native Windows IME, accessibility and physical display-scale
journeys must produce visible host evidence, followed by the macOS and Linux
providers. Third, the performance item must supply one equal consumer fixture
before any GPUI or Tauri ranking: the same study, frame dimensions, controls,
process boundary, input trace and lifecycle budget, with WASM used/allocation,
compositor and growth measurements kept as separate metrics. Fourth, the
browser WebGPU recovery path needs a real adapter/device-loss/recovered-pixel
trace. Safari selected-file authorization and hosted free-threaded Python
wheel evidence remain host or registry-environment blockers; they cannot be
closed by changing format-neutral Metis APIs. This order keeps DICOM and voxel
aspect ownership in RITK and prevents a comparator document from becoming a
support claim.

Revision 2026-09-19 (scoped native process provider): Metis now exposes a
format-neutral `ScopedProcessProvider` behind `CapabilityScope::RUN_PROCESS`.
The host fixes an absolute executable, argument-value allowlist, containment
policy and optional bounded environment; the provider clears the environment
by default, passes arguments directly and never parses a shell string. Moirai
merge `8a8daa60cca5484822c772bc6b574acaa8f133ad` adds the explicit stderr pipe
used by this boundary. Metis drains that pipe privately, returns only its byte
count, caps stdout and terminates on a finite deadline with typed cleanup
errors. Real-child tests cover success, denial, missing scope, secret-output
redaction and deadline cleanup. This closes the first process-provider slice;
network providers, operating-system sandboxing and the remaining sidecar
lifecycle are still open under `METIS-SERVICES-001`. DICOM remains RITK-owned.


[Input Events]: https://w3c.github.io/input-events/
