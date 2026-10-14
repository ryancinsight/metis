# Native startup evidence

Status: Accepted; diagnostic contract for ADR 0059, Metis 0.5.0.

The earlier Windows run completes Rust module preparation, native compilation
and archive discovery, then receives no page message before the unchanged
readiness deadline. That result alone does not identify a module or bridge
failure. The subsequent [observed run](https://github.com/ryancinsight/metis/actions/runs/38531378051)
loads all five resources with correct MIME types and records the Rust-handled
DOM error `WebView2 bridge is not an EventTarget`. The observer supplies the
rejected operation without evaluating page code or manufacturing Ready.

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

The private mechanism introduced in protected Moirai 0.9.1 remains in 0.9.2. Its
[installed Windows verification](https://github.com/ryancinsight/Moirai/actions/runs/38526145575)
establishes callback/DOM/shutdown behavior, not Metis controller readiness.

Only the explicitly observed form worker selects
`MOIRAI_WEBVIEW_STARTUP_OBSERVATION` with its configured entry URI and selectors
`#host-status` and `#result`. The complete ordinary form and every other scenario
remove inherited selection. This private fixture input
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
after the one-key correction. The observed consumer run above activates the
actual diagnostic, then stops before archive execution on the identified bridge
error. Windows execution of the environment vectors and both complete form
journeys remain required. Diagnostic activation does not establish ordinary
startup.

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
environment removal. These portable checks establish their own transport
contracts; the separate native DOM evidence above identifies the bridge cast.

Moirai 0.9.2 replaces that nominal DOM cast with checked native callable methods
and preserves the original receiver/callback through removal. Protected merge
`2b189f0a8aaf4a45aab1f2085c2ec748e483b8b3` owns the adopted implementation.
Its [installed Rust fixture](https://github.com/ryancinsight/Moirai/actions/runs/38540325048/job/115773188924)
passes Unicode/string identities, computed replies, inclusive 65,536-byte bounds,
65,537-byte rejection, close/remount, actual Destroyed and profile cleanup.
This provider result does not establish Metis Ready/Submit/Stop or permission
acceptance. Both standalone locks consume that protected revision while every
unrelated dependency identity remains unchanged.

The adopted [Metis Windows replay](https://github.com/ryancinsight/metis/actions/runs/38549695922)
loads all five resources with status 200 and observes the exact form Ready and
module identity. DOM reports `Host bridge connected; backend authority remains
outside the page.` The subsequent OS focus criterion fails before the UIA actor
or form input begins. Its requested container and top share the frontend thread;
actual focus belongs to a visible, enabled descendant in a WebView runtime
process. Both native child relationships and all five GUI queues confirm that
recipient. Exact HWND equality rejects valid delegated focus.
Ordinary form, later native cases, archive execution and MSI remain unexecuted.

The independent native input helper records the actual criterion
handles and its bounded actual SetFocus samples, followed by read-only class,
owner and GUI-queue observations before detachment. Raw LastError may be stale;
the snapshot is non-atomic. ASCII JSON exception notes reach complete retained
worker stderr. The corrected canonical predicate requires actual foreground
equality and nonzero focus on the selected container or its native descendant.
It rejects sibling, ancestor, unrelated and absent focus, including callers
that ignore the returned tuple. Selected-container identity and thread layout,
queue attachments and retry/deadline bounds remain unchanged. Real Win32
hierarchy regression and both complete Metis journeys require Windows replay.

The [focus-corrected replay](https://github.com/ryancinsight/metis/actions/runs/38552896154)
passes the native hierarchy regression and submits the first Unicode input.
Request 1 produces audit 2, rates 0.48/0.96, and that exact document result.
The worker then reaches 60 seconds without retaining UIA progress or its
primary exception before cleanup. Native DOM success cannot establish that
UIA reads the result, submission reenables or the remaining journey runs.
The [acceptance progress contract](../../../../../../scripts/webview_acceptance/design.md#retained-ui-automation-boundary)
retains bounded actual replies and entered-call markers independently of final
cleanup. Query order, COM model, predicates, input and deadlines stay unchanged;
the timeout remains unattributed until those native records exist.

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
