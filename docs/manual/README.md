# Metis user manual

Metis targets Rust applications on desktop and the web, with WASM application
logic and HTML5/CSS presentation. It uses Atlas providers for execution and
rendering. A system WebView is in scope for desktop web compatibility; the
current `metis-app` executable demonstrates backend and software presentation
roles in separate processes, launched from one application executable.

This manual describes the working application surface. The current demonstration
tests separate processes through pipes; the gallery renders real backend sessions
through bounded memory transport into a software framebuffer; and the browser
workbench runs Rust/WASM controls in an HTML5/CSS document. With the documented
loopback service command, the workbench also completes an authenticated
WebSocket handshake and a real backend calculation. The Windows platform crate
exposes a Moirai-backed native pixel and event surface, and the `metis-app`
demonstration connects it to the frontend through the same private process
workflow. Operating-system permissions remain a host gap. The browser shell
enforces its strict CSP, and the service `HostPolicy` binds grants to an exact
origin, window and session. The platform crate also exposes a bounded Windows
WebView2 consumer seam, and `metis-app --metis-webview` demonstrates an
HTML5/CSS form over the same supervised private IPC. The visible installed-runtime
capture is recorded in the native-surface workflow; TLS and OS enforcement remain
separate workflows.

The [target contract](../adr/0002-web-application-contract.md) describes the
Tauri migration goal and required web support. These are implementation targets,
not features available through the build commands below.

Metis stays format-neutral: it does not parse DICOM or retain medical viewer
state. RITK owns DICOM discovery, decoding, geometry and clinical presentation;
Metis carries only the bounded host, event and framebuffer handoff.
The [application gallery](applications.md#real-dicom-application-evidence) leads
with actual public CT and MRI studies rendered through the native Métis surface,
browser canvas and eframe application. It also gives a copyable local command
for opening a saved clinical study; private pixels and identifiers remain on
the local machine.

![Actual saved CT study rendered through the Métis host](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-installer-ct.png?raw=true)

The current Windows default-shell verification is bound to the exact run in
RITK's [default-shell CT provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-default-shell-ct.json).
That run selected 409 saved DICOM files, rendered the axial, coronal and
sagittal planes, rejected an invalid study, and repeated the same 1280 × 800
PNG across three lifecycle runs. This is the RITK-owned DICOM workflow using
the Metis host; Metis does not parse DICOM or retain patient state.

The same gallery includes a real 94-file MRI-DIR T2 study rendered through the
Métis browser canvas. RITK decodes the saved files and supplies the three
orthogonal frames; the browser capture records the accepted file count, exact
byte read and non-black canvas pixels. The [reviewed Edge capture](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-edge-gallery.png?raw=true)
and [provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-edge.json)
are public evidence. Private patient studies remain local.

![Actual saved MRI-DIR T2 study rendered through the Métis browser canvas](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-edge-gallery.png?raw=true)

The gallery pairs the image with RITK provenance and a browser component-state
record. Those records identify the files read, non-black canvas pixels, mounted
listener generation, theme, pointer release and clean teardown. Synthetic form
captures remain below as protocol examples.

The Windows pathless workflow also opens a saved study through the native
folder picker. RITK scans and decodes the selected 94-file MRI-DIR T2 study,
then presents its axial, coronal and sagittal planes through the Métis surface;
the picker window capture includes the real operating-system dialog and is
reviewed in the [RITK provenance record](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-mri.json).

![Actual MRI study after native folder selection through Métis](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-picker-mri-window.png?raw=true)

The configured browser chooser run adds real three-canvas galleries for
[Chromium 152](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-cross-engine-chromium.png?raw=true)
and [Firefox 155](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-cross-engine-firefox.png?raw=true).
Both engines pass the file-hash, pixel, bounded-rejection, cine and teardown
oracles. Safari 26.6.2 accepts the selection but rejects the first bounded
read; the [cross-engine provenance](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-cross-engine.json)
keeps that external WebKit authorization residual explicit.

![Chromium 152 real MRI canvas gallery](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-cross-engine-chromium.png?raw=true)

![Firefox 155 real MRI canvas gallery](https://github.com/ryancinsight/ritk/blob/main/docs/manual/images/dicom-metis-real-browser-mri-cross-engine-firefox.png?raw=true)

All DICOM scanning, decoding, geometry and clinical presentation remain in
RITK. Metis supplies the bounded file, window and framebuffer contracts.
The browser manual also includes an `http-health.html` probe for the bounded
Moirai loopback service; it demonstrates CORS and readiness only, not DICOM.

- [Build and run](getting-started.md): requirements, the demonstration and verification.
- [Run the Windows native surface](native.md): present a framebuffer and inspect real Win32 events.
- [Build executables and installers](distribution.md): configure an application, create a portable bundle, install and remove it.
- [Use the Python binding](python.md): build and test the typed `metis-rs` PyO3 package.
- [Create a presentation](presentation.md): supported markup, styles and application state.
- [Migrate presentation styles](style-migration.md): handle strict software-style diagnostics and move full CSS to the browser path.
- [Connect a backend](backend.md): process ownership, requests and errors.
- [Application gallery](applications.md): snapshots produced by the actual examples.
- [Inspect application output](testing.md): run visual checks, interpret the current demonstration and review snapshot changes.
- [Run the browser workbench](browser.md): build the WASM host, serve the generated HTML/CSS and inspect real Rust-driven control transitions.
- [Theme and branding](browser.md#theme-and-branding): select a bounded palette, override semantic CSS variables and replace the starter mark.
- [Framework comparison](../adr/0003-framework-conformance.md): source-pinned gaps against Tauri, egui, GPUI, Iced and Axum, with htmx as the hypermedia reference.

Public repository: [ryancinsight/metis](https://github.com/ryancinsight/metis).
For individual Rust API contracts, build `cargo doc --workspace --no-deps`.
Architectural alternatives and security boundaries live in the
[decision records](../adr/README.md), outside this user manual.
