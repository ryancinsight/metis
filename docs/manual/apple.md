# macOS, iPhone/iPad and browser applications

The same `frontend` manifest and HTML/CSS/Rust-WASM assets feed a browser or an
installed Apple application. `metis-host` uses Moirai's AppKit/UIKit lifecycle
and WKWebView resource provider. Apple hosts do not start a loopback server.

Metis 0.4.0 requires moirai-webview 0.2.0. Its packaged resource responses carry
status 200 plus exact MIME/length headers, which the WASM loader requires before
it binds the application. Delivering a WASM file alone does not establish that
the application starts. Native macOS Greet is verified for normal, Unicode and
empty names against the corrected provider.
This increment adds frontend hosting and bundling; the capability and isolation
requirements in METIS-MACOS-001 and METIS-MOBILE-001 remain independently tracked.

## Local Atlas setup

Use a recursive Atlas checkout with macOS/Xcode and the native Rust 1.97.0
compiler. The compiler version is portable; application targets remain explicit.
Install the additional standard libraries and the exact locked loader version:

```sh
rustup target add --toolchain 1.97.0 wasm32-unknown-unknown aarch64-apple-ios aarch64-apple-ios-sim
cargo install wasm-bindgen-cli --version 0.2.129 --locked
```

Run the installation command outside the Atlas checkout so its broad Cargo
overlay does not add unrelated patch records to the tool's locked build.

Run from the Atlas root. The default application is `metis-starter`:

```sh
python3 scripts/atlas-metis.py serve
python3 scripts/atlas-metis.py dev --watch
python3 scripts/atlas-metis.py apple run --target macos
```

`serve` opens a local HTTP endpoint at `http://127.0.0.1:1420/`; visit it with a
browser. `dev` opens a native macOS window; `--watch` rebuilds changed frontend
sources and reopens the window after a successful compile. Failed builds retain
the running page and report the error. `apple run` builds and opens the installed
`.app` bundle. Closing the last macOS window ends its application process.

## Native logo links

The optional frontend manifest `external_links` declares the exact HTTPS links
the native shell may hand to the system browser:

```json
"external_links": [
  "https://github.com/ryancinsight/metis",
  "https://webassembly.org/"
]
```

These are the starter's two logo destinations. The signed bundle carries that
array under Moirai's metadata key. Omission produces an empty array and grants
no external links. The provider validates the array before host startup; the
CLI validates it before build or owned-output replacement. Its limits are 64
input entries and 2048 URI bytes per entry. Only absolute HTTPS links without
credentials or malformed URI bytes are accepted. Matching includes the exact
path, query, fragment, port and spelling; declaring one path grants no sibling
path or whole origin.

A native handoff requires a popup link from the packaged main page, with the
`LinkActivated` action kind and no download. The UI delegate opens the system
browser and returns no remote WebView, preserving the packaged Greet page.
Existing-frame external navigation and unlisted targets remain blocked.
`LinkActivated` describes the navigation kind; it is not proof of a trusted
gesture. Automatic JavaScript popups are disabled. The existing capability
broker, CSP and OS isolation contracts retain their separate authority.
System refusal emits a structured error; iOS reports its
asynchronous completion. [ADR 0057](../adr/0057-native-external-links.md) records
the policy, failure modes and required runtime checks. The installed macOS
bundle opens the exact Metis repository URL in Safari and retains the Unicode
name and Rust greeting. A real WebKit fixture rejects an undeclared popup,
existing-frame external navigation and a subframe popup. Its two executed
onload popup attempts produce no handoff; the real logo control produces one.
These observations do not establish trusted-event provenance for every script
context or physical iPhone browser handoff acceptance.

## macOS window controls and exit

The native **Metis Starter** menu supplies **Quit Metis Starter** (⌘Q).
The **Window** menu supplies **Close** (⌘W), **Minimize** (⌘M), **Zoom** and
full screen (⌃⌘F). These actions run in Rust's AppKit adapter, including when
the WebView name field has focus. Native close, minimize and zoom/full-screen
buttons belong to AppKit; the page does not draw replacements.

Native tests observe minimize/restore, Zoom from 800×632 to 1728×1034 and back,
full-screen transitions and process exit on ⌘Q/⌘W. The exact Unicode greeting
survives window transitions. Native traces report nonzero, enabled, unhidden
standard buttons in normal mode. Recordings show a purple capture badge over
their title-bar area; a recording alone cannot prove unobscured button glyphs.

Application, Edit and Window menus come from [Moirai ADR 0073](https://github.com/ryancinsight/Moirai/blob/main/docs/adr/0073-apple-native-window-commands.md).
The general programmable window API, OS permission denial and remaining mobile
lifecycle journeys stay in their owning Tauri capability items.

## Development and locked sources

macOS development runs from a signed bundle under `output/apple/dev-macos`.
It uses the same application metadata and shell icon as the installed build.
Failed icon validation also retains the running development app.

The Atlas runner preserves root build settings and the shared `target`, and
runs Cargo from an isolated configuration directory without dependency patches.
Metadata and compilation use `--locked`; provider sources come from the canonical
Git lock, while current Metis frontend sources remain editable. Both dependency
locks remain unchanged through real Apple bundling. The former unlocked metadata
step rewrites Git sources to local paths; the runner removes that mutation path
and deletes its temporary configuration after exit.

Choose another frontend with the runner's `--manifest` option, before the command:

```sh
python3 scripts/atlas-metis.py --manifest /absolute/path/metis.json apple build --target macos
```

Apple compilation requires the Metis source workspace. The committed lock
resolves `moirai-webview` and the native fixes from the merged Moirai provider;
one Git source identity serves its crates. Provider changes require their
published revisions and an explicit canonical lock update; the Atlas runner
retains the committed lock under
[Atlas ADR 0044](https://github.com/ryancinsight/atlas/blob/main/docs/adr/0044-committed-cargo-lock-form.md)
and [ADR 0069 in Atlas PR 530](https://github.com/ryancinsight/atlas/pull/530).
Remaining native interaction and physical-device signing are release acceptance.

## iPhone and iPad simulators

Select a simulator identity from `xcrun simctl list devices available`. Boot it
before `apple run`; the command installs and launches on the selected device:

```sh
xcrun simctl boot 6E61403F-F72F-4C07-9A43-B8F8E036E6B1
python3 scripts/atlas-metis.py apple run --target ios-simulator --device 6E61403F-F72F-4C07-9A43-B8F8E036E6B1
```

The identity above belongs to the tested local iPhone 17 simulator; use an
identity listed on your Mac. Both iPhone and iPad use the same simulator app.
Apple Silicon uses `aarch64-apple-ios-sim`; Intel uses `x86_64-apple-ios`.
The native scene owns its window, controller and WebView until disconnection.
A reconnect constructs a window from the retained packaged resource root.
UIKit controls view geometry, keyboard, rotation and application activation.

To open the browser build in simulator Safari while `serve` runs:

```sh
xcrun simctl openurl 6E61403F-F72F-4C07-9A43-B8F8E036E6B1 http://127.0.0.1:1420/
```

Physical phones cannot use the Mac's loopback address. `metis build` produces
`dist/app`, which a static HTTPS server can serve to mobile browsers. This
increment leaves the development listener on loopback; it creates no public
endpoint or LAN permission.

## Device build and bundle contract

```sh
python3 scripts/atlas-metis.py apple build --target ios
```

This creates an unsigned device bundle for `aarch64-apple-ios`. Device install
requires the owner's Apple signing identity, provisioning profile and a
connected device. `apple run --target ios` rejects before building because it
cannot supply those decisions. App Store publication remains separate.

Default bundles live under the application manifest's `output/apple/<target>`;
`--output` selects another directory. Browser output remains in `dist`. Both
macOS and simulator builds receive ad-hoc signatures without a private identity.
The compiler uses Xcode's target SDK and deployment minima of macOS 13/iOS 16.
Those releases ship WebKit 16, required by the frontend's `wasm-unsafe-eval`
CSP; browser use likewise requires an engine supporting that directive.
Rust and crate code link into the executable. `otool -L` records these dynamic
system dependencies for the verified bundles:

| Targets | Dependencies |
| --- | --- |
| All Apple targets | WebKit, JavaScriptCore, Security, CoreData, CoreText, CoreImage, CoreGraphics, CloudKit, QuartzCore, Foundation, CoreFoundation; libSystem.B, libiconv.2, libobjc.A |
| macOS | AppKit, CoreVideo |
| iOS device and simulator | UIKit, UserNotifications, CoreLocation |

The shared provider permits regular files up to 64 MiB per resource. Bundling
checks this bound before replacing an owned output and after copying assets.
The provider rejects special files before opening, validates the opened
descriptor, and bounds the read if a file grows. On Unix, nonblocking/no-follow
open flags prevent waiting on a replaced FIFO or following a replaced leaf link.
Packaged resource directories remain owner-controlled application inputs.

Each target output contains `inventory.json` with final per-file byte counts
and SHA-256 digests, recorded after signing. Every packaged browser asset has
identical bytes to `dist/app`. A rebuild replaces a previous output only if
all files still match its inventory. Missing, added, modified, linked or special
files prevent deletion. A failed partial output requires a new output directory
or an owner-reviewed cleanup; the CLI does not erase unowned data.

## macOS Dock icon

The optional frontend manifest `icon` names an ICNS file relative to `metis.json`:

```json
"icon": "icons/application.icns"
```

The starter declares its local Metis mark. macOS bundles include those exact
bytes at `Contents/Resources/application.icns` and declare `CFBundleIconFile`.
The packager accepts the standard 16/32/128/256/512 point representations at
normal and double scale, with a 1 MiB source budget. Each supplied representation
must survive native decoding. A declared missing, linked, malformed or oversized
icon rejects before any owned output changes. An omitted icon uses the OS's
default application artwork. This field currently applies to macOS; iOS icon
catalogs remain under the mobile capability work.

Regenerate the starter icon on macOS from its canonical SVG:

```sh
python3 scripts/apple_icon.py
```

Run this command from the Metis repository. It renders all ten point/scale
images from the vector and compiles them with Apple's `sips` and `iconutil`.
The generator also accepts `--source` and `--output` for application artwork.
Rebuild and relaunch the application after changing its icon. The bundle
metadata follows [Apple's icon-file contract](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html).

## Verification recorded on 2026-10-11

| Check | Result |
| --- | --- |
| macOS backend/frontend process session | Exact 0.36 mL/h, 0.72 mg/h and two audit records; cleanup succeeds |
| CLI/starter unit and documentation tests | 60 CLI, one greeting and two documentation tests pass |
| Complete macOS workspace | 678 tests pass with the repository's isolated runner; workspace lint and documentation checks pass |
| Moirai transport/resource tests | 39 transport, six real-file resource tests and one documentation test pass |
| Apple PAL, transport and resource contracts | 125 tests pass, including closed-peer forwarding, complete payload bounds and the live-stall deadline |
| Atlas configuration, ADR indexing and starter/icon structure | Three configuration, 20 ADR-index, eight page-structure and three icon tests pass |
| Browser greeting | Exact output for Ryan, Unicode and empty input at a 393 × 852 viewport |
| macOS bundle | Builds, signature/plist validation, launch and packaged WASM delivery succeed |
| iPhone 17 and iPad Air 11-inch M4 simulators, iOS 26.5 | Installation, launch and packaged WASM delivery succeed |
| iOS device bundle | Compiles; physical device is unavailable and no valid signing identity exists |
| Native development watch lifecycle | Signed-bundle identity/icon, WASM load, source reload, compiler/icon-error preservation, source restoration and child-exit cleanup pass |
| macOS application icon | Exact packaged ICNS, ten decoded resolutions, transparent/solid-color pixel oracles and inventory/signature checks pass; visible Dock observation requires Mac unlock |
| Native macOS Greet | Actual clicks return exact Rust results for Ryan, Δ Ryan and empty input after the response-status fix |
| Native mobile input, Safari-specific input, rotation and suspend/resume | Pending; the installed Xcode tree supplies the CoreSimulator backend without a Simulator UI application |

Build and launch evidence do not establish native input, native plugin parity,
OS permission denial or physical device execution. See the current
[gap register](../../gap_audit.md) and [ADR 0055](../adr/0055-apple-application-bundles.md).

The iPad simulator emits a duplicate `UIAccessibilityLoaderWebShared` class
warning from Apple's WebCore/WebKit accessibility bundles. Resource delivery
continues; native accessibility and input require the pending interactive run.

The watch verification edits real Rust/CSS sources, checks preserved asset
digests on a compiler error, and restores the original source bytes. Intentional
termination of its final native child makes the supervisor exit with the
application-exit error and leaves no retained child. This verifies process and
rebuild behavior independently of the pending native input journey.

Apple CI repeats the complete isolated workspace suite, native icon decoding
and all three bundle builds. It verifies final inventories, signatures,
deployment minima and identical packaged frontend bytes. The Windows and Apple
jobs install the loader version in the committed lock, 0.2.129. Isolated tests
give process-wide output-reader exhaustion tests ownership of their whole budget.
