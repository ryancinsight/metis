# 0057 — Native external link authority

Status: Accepted

Date: 2026-10-13

Change class: [minor]

Target: Metis 0.4.0, moirai-webview 0.2.0.

## Decision and ownership

The browser application manifest may declare `external_links`, an array of
exact HTTPS destinations. Omission grants no destinations. Metis validates the
array with Moirai's `ExternalLinks` contract before build or output replacement,
then serializes it into the signed bundle under Moirai's public metadata key.
The provider validates that metadata again before constructing the WebView.
The executable's bundle metadata supplies authority; frontend code cannot
expand the array. There is no application-specific destination in the provider.

Moirai owns WebKit navigation admission and the operating-system browser handoff.
Metis owns the manifest, bundle inventory and starter destinations. Native
macOS, iOS and simulator iOS share one policy; AppKit and UIKit provide their
respective handoff mechanisms. Browser deployments retain ordinary anchor
behavior. No JavaScript listener or external-page WebView is added.

The existing `ScopedOpener` capability broker retains its own origin and
`OPEN_EXTERNAL` witness contract. This bundle policy is a separate native
shell authority for declared hyperlinks; it does not manufacture broker grants
or change command routing.

## Admission contract

Let `A` be the validated exact destination set. An external handoff requires all
of: destination membership in `A`, a packaged `metis://app/` main source frame,
WebKit's `LinkActivated` navigation kind, a new-window target and no download.
Existing packaged-frame navigation remains admitted. Existing-frame external
navigation, unlisted destinations, non-main sources and other action kinds
remain denied. The navigation delegate admits an eligible popup, and the UI
delegate rechecks it before opening exactly one system-browser request. The UI
delegate returns no WebView, retaining the packaged application document.

Destinations are compared byte for byte, including path, query, fragment, port
and spelling. Entries must be absolute HTTPS URIs without credentials, invalid
percent escapes or invalid URI bytes. Moirai supplies the authoritative bounds:
64 input entries, at most 2048 bytes per entry. Duplicates consume the input
count budget. XML escaping preserves the original destination after plist
decoding. The starter explicitly declares the GitHub repository and the
WebAssembly website; the anchor bytes equal those declarations.

`LinkActivated` classifies a navigation; it does not establish a trusted user
gesture. WebKit's automatic JavaScript popup setting is disabled, but this
decision does not treat that classification as a proof against every synthetic
activation. A future trusted-gesture requirement needs a supported public
provider contract and its own evidence. CSP, broker grants and OS isolation
remain separate boundaries.

## Alternatives and failure modes

Allowing arbitrary HTTPS origins or loading remote pages in the application
would expand authority beyond the declared links and replace the packaged
document. Hardcoding logo URLs in Moirai would reverse provider ownership.
JavaScript click forwarding would duplicate platform policy and enlarge the
authored script surface. These alternatives are rejected.

An invalid manifest rejects before output changes. Missing metadata grants
nothing; malformed metadata rejects host startup. A refused system-browser
handoff emits the provider's error context and leaves the app document intact.
An iOS completion reports the system's asynchronous success or refusal; build
success alone does not establish installed-device handoff.

## Verification plan

Manifest tests inspect default and exact declared values, invalid schemes,
credentials, URI bytes and the provider bounds. Plist tests inspect all Apple
targets, empty declarations and query/fragment escaping. A page-structure check
compares the real anchor destinations with the starter manifest.
Provider decision tests cover the complete source/action/target/download
matrix. Native acceptance clicks each logo, observes the actual system-browser
destination, retains the packaged page and verifies an input-sensitive Rust
greeting afterward. Apple ABI builds, canonical Git locks and protected gates
remain delivery checks. Physical iPhone execution requires the owner's device
and signing identity; unavailable hardware is recorded as unverified.

## Local evidence

The installed macOS bundle opens the exact Metis repository URL in Safari
and retains `Hello, Anaë — 東京! You've been greeted from Rust!`. The
WebAssembly logo opens its declared page. A real WebKit fixture executes two
onload popup attempts and rejects clicked unlisted, existing-frame and subframe
requests with zero external handoffs. Its actual logo control yields one
successful OS handoff and retains the Unicode result. Download=true denial
remains covered by the portable matrix; a cross-origin HTML download attribute
can be ignored by WebKit and does not prove that native action flag.

The provider's 20 real resource/policy/Foundation tests pass. The consumer's
707 Rust tests and 396 script tests pass before concurrent default-branch
reconciliation. Ten library surfaces pass all 196 applicable minor-class
compatibility checks each. Four comparisons use compiler-produced rustdoc
JSON from each canonical locked graph because the automatic fresh-resolution
helper cannot resolve the upstream consus-raster 0.1 requirement at its new
0.2 default branch. No dependency requirement or check is waived.
