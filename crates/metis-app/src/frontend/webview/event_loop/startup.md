# Native startup evidence

Status: Accepted; diagnostic contract for ADR 0059, Metis 0.5.0.

The current Windows run completes actual Rust module preparation, native
compilation and archive discovery, then observes no page message before the
unchanged readiness deadline. This does not identify a module, browser bridge
or Rust initialization failure. Successful document navigation does not imply
that its JavaScript module or Rust controller starts.

## Observation and authority

The native owner records the actual navigation URI, policy result and completion
success supplied by the existing PAL event surface. Failed navigation retains
its URI in the error. These observations neither advance `PageSession` nor
manufacture a Ready event. Only the existing exact-identity `PageEvent::Ready`
can advance Loading to Ready.

During the initial required-event wait, the independent observer samples the
desktop rectangle of a window in the owned process tree. It retains the latest
changed BMP, physical origin/extent, digest and observer-error context. It reads
no UIA or DOM text and sends no input or focus operation. Focus and occlusion
remain unknown. A readable image may aid diagnosis; it does not identify a
loader failure versus Rust initialization failure, establish readiness, replace
the native wait failure or skip a journey. The fixture deadline stays unchanged.

The verification owner retains the actual producer directory and byte identities
for its native module. Browser-build assets cannot substitute for those bytes.
Core's native inventory admission remains authoritative; a Python evidence
receipt does not decode or independently admit that inventory.

## Private Rust observation selection

The protected Moirai 0.9.1 provider owns this private fixture mechanism. Its
[installed Windows verification](https://github.com/ryancinsight/Moirai/actions/runs/38526145575)
establishes callback/DOM/shutdown behavior, not Metis controller readiness.

The form verification child selects `MOIRAI_WEBVIEW_STARTUP_OBSERVATION` with
its exact configured entry URI and the selectors `#host-status` and `#result`.
Every other scenario removes inherited selection. This private fixture input
grants neither application input nor page-protocol authority. The Rust logging
owner enables DEBUG only for `moirai_pal::windows::webview::startup`; ordinary
INFO records retain full span context. Without both an explicit selection and
the dedicated target, the provider installs no diagnostic observers.

The selector crosses both process boundaries: verifier to backend, then backend
to the contained frontend. For inherited environment `E`, the Runtime policy
projects exactly the nine existing OS-location keys and this one private key;
Isolated projects the empty set. Values remain opaque and byte-preserved at
this transport boundary. The provider validates their closed schema, URI and
budgets; malformed or empty selection must reach that rejection rather than
silently disabling observation. Asset inventories and unrelated application
values remain excluded. An absent selector remains absent.

Review of the first consumer increment finds that only the verifier-to-backend
hop is covered: the supervisor clears the selector at the frontend boundary.
That revision therefore runs the ordinary form without the intended native
diagnostics. The contained Rust regression first reproduces the dropped value,
then passes all five native-value dimensions under both environment policies
after the one-key correction. Windows execution of those vectors and actual
observation activation remain required. Diagnostic activation does not establish
that the unselected form starts.

The provider owns native resource MIME/status, content/navigation/process
events, actual CDP exception/log events and one sequential read-only DOM chain.
Runtime/Log activation and navigation share the original navigation deadline.
An asynchronous DOM completion holds only Weak state. Navigation invalidates
node identities; close cancels admission and removes every receiver before
controller close. No script evaluation or document mutation supplies evidence.

The diagnostic formatter omits time and span context. Its exact physical JSON
envelope, including LF, occupies 133 bytes. Compact provider JSON contains no
raw control bytes, so its second JSON-string layer expands at most twofold.
With aggregate encoded bytes `B = 65,536`, event count `N = 256` and two
separately bounded terminal records, the output bound is
`6 * B + 133 * (N + 2) = 427,530` bytes, below the unchanged 1 MiB trace limit.
This bounds Rust-retained copies and emitted observations; Chromium allocations
before a callback remain outside that guarantee. Exhaustion and incomplete
observations remain explicit failures of evidence completeness.

The consumer's real formatter tests verify the physical envelope, Unicode and
control-character round trips, the compact-JSON escape bound and target
selection. Real child-process tests verify scenario selection and inherited
environment removal. These portable checks do not establish native callback
execution or identify the current Windows startup cause.

## Failure and verification

Early document/mount/bridge acquisition can fail before a coordinator exists;
the raw Rust entry point can then render a document error but cannot promise a
correlated native failure. Loader rejection or a WASM trap can precede Rust
entirely. Neither absence of a Failed message nor successful document navigation
identifies which case occurs. These diagnostic boundaries require actual
Windows observations before a startup correction is selected.

Source checks preserve the closed page protocol, actual Ready admission and all
existing deadlines. The actual Windows run must retain the navigation records,
pre-Ready observation and exact native producer bytes. It must then satisfy the
existing computed form, permission, remount, destruction and package acceptance;
diagnostic output alone does not close native acceptance.
