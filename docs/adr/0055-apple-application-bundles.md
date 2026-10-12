# ADR 0055: Shared browser and installed Apple applications

- Status: Accepted
- Date: 2026-10-11
- Change class: [minor]
- Target version: 0.2.1

## Decision and contract

Build the existing manifest's frontend once as HTML/CSS/Rust-WASM. `metis build`
and `metis serve` retain browser output. `metis apple build` combines that output
with a `metis-host` executable using Moirai's Apple WebView provider. `metis dev`
supports a macOS window. `metis apple run` launches a macOS bundle or installs
and launches an iPhone/iPad simulator bundle selected by device identity.

Metis owns the compiler-produced executable, app manifest and payload inventory;
Moirai owns WebKit and AppKit/UIKit. The host does not contain build tooling.
Native macOS, device iOS and simulator iOS use Cargo's explicit target dimension,
the matching system SDK and the shared Atlas build cache.

The starter CSP uses `wasm-unsafe-eval`, whose WebKit support begins in Safari
16 ([browser compatibility data](https://github.com/mdn/browser-compat-data/blob/main/http/headers/Content-Security-Policy.json)).
Deployment minima are macOS 13 and iOS/iPadOS 16, which ship that engine.
One target contract supplies the compiler environment and bundle metadata.
An older OS with a separately updated Safari is outside this initial native
deployment contract; no CSP relaxation is required by this host.

Every produced executable comes from a successful Cargo artifact message.
Every bundle contains the identical staged browser asset bytes and the target's
Info.plist layout. A previous Apple output is replaced only when every file still matches its
post-signing inventory. Modified or unrecorded output is refused. Bundle
names and identifiers are validated manifest values. Device builds are unsigned
and require the owner's Apple provisioning identity before installation; local
macOS and simulator signing uses an ad-hoc identity without credentials.

One public Moirai resource bound defines the consumer packaging limit. Assets
must fit that bound before output replacement and after copying; an oversized
input preserves an existing owned bundle. Runtime resource delivery validates
regular-file metadata before opening and on the opened descriptor.

## Failure modes and acceptance

The workspace has three canonical executable roles: `metis-app` for the
existing application, `metis` for distribution tooling, and `metis-host` for
the Apple entry. Backend/frontend roles remain libraries. Windows distribution
inventories its one declared application executable. Workspace target and
direct-dependency contracts live in `scripts/verification/workspace.py`; the
gate and tests call that module after the internal contract split.

Missing SDK/toolchain, stale loader version, compiler failure, invalid manifest,
unexpected output contents and mismatched simulator platform fail with context.
The same Rust greeting must compute from typed input on macOS, iPhone and iPad
native hosts and in Safari. Resource traversal and symlink escape fail at the
provider boundary. Build success alone does not establish device execution.

The initial mobile host supports the browser application's DOM, touch, keyboard,
resize/rotation and system-managed suspend/resume. Native plugin parity and cold
launch persistence retain their existing explicit backlog scope. A separate
Apple frontend or opening Safari in place of an installed application does not
satisfy this decision.

## macOS application identity correction — 2026-10-11

The optional frontend manifest `icon` names a local ICNS file relative to the
manifest. For macOS, preflight validates its bounded bytes and decodes it with
Apple's icon tool before an existing output can be replaced. The bundle copies
the validated bytes into `Contents/Resources/application.icns` and names that
file in `CFBundleIconFile`. This field currently supplies macOS shell branding;
iOS icon catalogs retain their separate mobile scope.

Development launches use the same signed bundle construction and its actual
executable. AppKit therefore reads the same application identity and icon in
development and packaged launches. Supervision still owns the process; failed
compilation or packaging retains the running application. The starter ICNS is
generated from its canonical SVG at all ten Apple point/scale representations.
Missing, linked, oversized or undecodable declared icons reject the build.
Verification binds plist values, packaged bytes, decoded dimensions and pixel
values, signatures, and real supervised reload behavior. Dock observation is a
separate native UI check and requires an unlocked desktop.
