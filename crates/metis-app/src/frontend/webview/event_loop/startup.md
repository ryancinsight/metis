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

The observed form also supplies `late_input_after_key: 68` (VK_D), anchoring
independent suffix retention at its existing Dark shortcut. The provider uses
the same copied callback records, with no extra receiver or native read. It
retains every later key, focus, move and read error. Only that private target,
`moirai_pal::windows::webview::late_input`, gains context-free INFO formatting;
startup alone retains DEBUG admission. An actual formatter regression shows
that `parent: None` does not suppress the ordinary formatter's current span.
Target routing removes those fields without changing ordinary log context.

The late stream retains at most 65,536 compact JSON bytes in 256 records;
initial and progress checkpoints spend that same quota. Its two terminal
bodies each occupy at most 534 bytes. The actual formatter's 138-byte envelope
and twofold compact-JSON string escape bound give
`2 * 65,536 + 4 * 534 + 138 * (256 + 2) = 168,812` bytes. Together with the
existing startup bound, diagnostic output is at most 596,342 bytes, below
the unchanged 1 MiB limit. These are diagnostic-stream bounds, not a bound on
all ordinary application output. Shared queue loss, callback/read/cleanup
errors, quota exhaustion, an absent real anchor or missing closure invalidate
a complete absence claim. A retained key remains positive receipt evidence.

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
acceptance. That adopted base consumes the protected revision; the current diagnostic and
focus-correction candidates are recorded below with every unrelated identity unchanged.

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

The initial input replay selects Moirai PR622 source `bd88d2c56f9d`; both
canonical locks now pin candidate `d1658acc9b3c`. This is an explicitly recorded diagnostic candidate, not an
adoption of a protected provider default. The same private selection observes
actual controller GotFocus/LostFocus/MoveFocusRequested and eligible
AcceleratorKeyPressed callbacks. Fixed callback storage retains native arguments
and raw focus/key state; the owner pump serializes records after callback return.
The observer changes neither Handled nor focus/input. Native top-level focus events have
their own Metis trace event. A controller key observation is receipt evidence,
not proof of renderer default action, UIA value change or Rust submission.
Real Ctrl+A/Escape and cleanup pass in the provider's Windows run 38743789587;
the unchanged Metis form journey must establish its own consumption boundary.

Early document/mount/bridge acquisition can fail before a coordinator exists;
the raw Rust entry point can then render a document error but cannot promise a
correlated native failure. Loader rejection or a WASM trap can precede Rust
entirely. Neither absence of a Failed message nor successful document navigation
identifies which case occurs. These diagnostic boundaries require actual
Windows observations before a startup correction is selected.

## Current native replay and admission candidate

Source5e5c4e2/run38792889172 binds to synthetic mergef602c98b/treeeb82dea2.
It passes502 Windows scripts in37.299s (501 pass, one Apple-only skip) and
produces four exact form requests/native replies. The successful rate/drug
pairs are0.48/0.96,0.6/2.4 and0.09/0.72, with audit values2,3 and5.
The intervening zero-concentration request returns0x3002. No independent
runtime ledger4 value is retained. The final UIA fields retain患者-🦀-three,
96,8,0.125 and the exact final result. Rust submit therefore consumes the
requested field values on this execution.

Light/high-contrast selection and Alt+Shift+D complete. Alt+Shift+S returns
from physical injection with owned native recipient and released modifiers,
but later UIA queries retain Theme: dark until the60-second worker expires.
The shared diagnostic quota exhausts before submit4, so absence of late
controller/DOM records does not prove absence of keyboard delivery. Source
inspection finds both physical codes dispatched through the same Rust
accelerator catalogue; no missing System branch is established.

Provider2fcef45d corrects a separate, real ready-event admission regression:
baseline44184f40/run38795557575 returns a resize after issuing another DOM
request. Positive selected waits now check their original deadline and ready
public events after preparation, before SDK entry. Deferred commands retain
their request identity and parameters. Both consumer locks advance only15/11
provider SHA records from aa532, with every other byte exact. No Metis Rust,
input, protocol, assertion, observation or timeout changes accompany this pin.
Corrected provider native execution and both complete forms remain gates.

Local validation of the2fcef pin passes strict workspace diagnostics,777 active
Rust tests, canonical preparation and strict Windows app cross-compilation.
Nextest reports one leaky process capture and two direct environment-helper
entries skipped; this does not establish native Windows behavior. The first
sandboxed attempt refuses28 loopback listeners; an identical replay with
loopback access passes. All original per-test60-second limits remain.

The admission-only Metis replay ad328f61/run38799513267 again completes all
four submissions and exact native results, but Alt+Shift+S leaves UIA Dark and
the worker exceeds60s. Its502 Windows scripts pass55.690s. Actualmergefb14678
has tree6e33d4f5, identical to ad328;1003 Git blobs and837 normalized run-source
digests bind. Module identityaba3e58f is the original prepared inventory hash;
the separately retained native-preparation WASM hash is8c14b070. Query165 still
reports Dark; query166 is sent without a final response. The controller quota
exhausts before submit4, so no late-shortcut receipt follows.

Provider2fcef/run38798617700 supplies a different, complete failed-scope
boundary: controller keys for prior values use one owned child, then the final
LostFocus leaves another owned native recipient. All4+18 failed input records
admit there, but the unexhausted controller collector observes no later keys.
UIA and timely live DOM retain patient-one after patient-two. The same browser
process/thread/layout therefore does not establish the controller endpoint.

The correction declares native-parent entry for actual WebView scenarios. After
one original UIA field selection, one native SetFocus(parent) invokes the
existing WM_SETFOCUS/MoveFocus(PROGRAMMATIC) route. Native focus must delegate
to an owned non-root descendant; the same selected field must remain focused
before the original physical records. The original selections and Unicode remain
observed. No activation ALT tap, retry, sleep, child-class guess or new deadline
enters this policy; native EDIT/RichEdit callers retain their previous policy.
Two real Win32 cases distinguish delegation from ownership and reject a parent
without a delegate. Provider6343610d applies the same entry contract to its
original16/64 matrix, with16 actual entries/12 non-first entries checked.
Both full Metis journeys, the complete Windows scripts and packaging remain
required on this exact candidate.

The6343610d consumer graph passes strict workspace Clippy,777 active Rust tests
in16.949s, canonical module preparation and strict Windows app cross-compilation.
The committed browser-tool producer runs before the separate script suite;
all504 local cases complete in15.140s with498 passes and six Windows-only skips.
All1003 tracked file hashes remain stable through that run. All515 Rust files,
original PowerShell UIA/selection actors, physical key/Unicode delivery and time
limits remain byte-identical to ad328. Syntax-tree comparison preserves every
original journey/assertion operation, adding only the declared native entries.

Provider 6343610d/run38803044085 passes 69 actual Windows cases: 50 policy,
nine preceding installed cases, eight original replacement modes, one separate
probe and the Rust bridge. Each replacement run checks 16 entries/12 non-first
entries and the original 16 replacements/64 field checks. Its separate probe
never triggers a DOM snapshot; exhausted controller observations cannot prove
a complete key history.

The actual consumer add41427/run38804568395 binds merge c1dadecf/tree91d14f7d
to 1,003 raw Git blobs and 837 normalized run-source digests. All 504 Windows
scripts complete in 40.426s with the existing Apple-only skip. The selected form
completes all four exact submissions and the Dark command, then stalls on
System at 60s. Query 240 retains Dark and query 241 is sent without a final reply.
Its native recipient is owned and modifiers release; the exhausted controller
quota cannot establish late renderer receipt. Source retains a strong Rust
coordinator in thread-local PAGE, so JavaScript handle collection does not
explain that lifetime. Only the observed form selects native suffix retention
from actual VK_D down onward. It preserves every later native key, focus, move
and read-error observation; independent quota, loss and closure metadata prevent
an incomplete suffix from asserting key absence. Ordinary form and all original
inputs, assertions and time limits remain. Both complete form journeys and
later native/package execution remain required.
The new diagnostic pin changes only 15/11 provider hashes from 6343610d in
the two locks. All other lock bytes remain exact. The Metis production Rust
delta is the private target's formatter routing; page controllers, protocols,
physical input and every time limit remain unchanged from add41427.
The diagnostic consumer passes strict workspace/Windows-app Clippy, 778 active
Rust tests in 9.242s, canonical preparation and 504 local script cases in
15.629s (498 pass, six Windows-only skips). A fresh external Git consumer
resolves and compiles the exact provider for MSVC without local path/patch
overrides. Its registry/default-branch publication is not established.

## Selected Rust command observations

Run38812113624 on561189 completes four exact submissions and504 Windows script
cases in46.278s. Native suffix329–337 includes S down/up335/336 at scan31,
Alt+Shift held, Ctrl clear and Handled=false, with no observed loss/error.
Its missing close record forbids a complete-history claim. Last complete UIA
response153 reports Dark; its receipt time is absent, and pending154 scans the
full tree. Neither that value nor its last getter marker proves S reached the
Rust listener or identifies a blocked theme getter.

Only the exact existing selected-form environment adds the private document
marker. Ordinary markup remains byte-identical and permission pages stay off.
The Rust coordinator reads that marker once at mount. Its optional producer
arms on the existing resolved Dark command, retaining actual metadata/decision
and return. Later keys additionally record entry before the existing Ready and
single metadata read. No new per-key DOM getter, listener, timer, input, focus,
permission or authored JavaScript supplies observations. A successful return
describes the original Result only. It does not assert actual post-mutation
DOM values, UIA refresh or rendered pixels.

Private records use their own versioned closed codec and exact identity fields;
the public PageEvent and command codecs retain their existing behavior. The
native owner applies the original source check first, then the selected channel,
current Ready form, identity and sequence checks. Malformed private input and
quota exhaustion produce a finite native rejection record without mutating
PageSession, a request or a deadline. All other messages use the original
functional decoder. Semantic JSON ordering/whitespace/escapes are accepted;
original received JSON bytes are retained and charged independently of producer
bytes. Environment selection remains an exact opaque transport contract.

One shared budget reserves its final record and413 bytes inside128 records and
32,768 total encoded bytes. ClosingIntent precedes the unchanged functional
CloseRequested and claims intent only; no diagnostic reply follows into
Stopping. External stop or missing terminal leaves observation completeness
unknown. Original Stopped, native destruction and cleanup remain acceptance.
The selected context-free INFO envelope is131 bytes including LF. Valid JSON
permits raw C0 only as tab/LF/CR whitespace; those, quote and backslash expand
at most twice when logged as a JSON string. The new physical bound is
`2 * 32,768 + 131 * 128 = 82,304` bytes. All three diagnostic streams total
678,646 bytes, below the unchanged1 MiB cap. Native rejection metadata spends
the same reserved space and has a separately derived finite bound.

The413-byte reserve bounds a locally encoded terminal. A received terminal with
equivalent JSON formatting may spend any remaining bytes within the unchanged
32,768-byte aggregate cap; producer and receiver byte counts stay independent.
The actual414-byte whitespace case fails before this correction and passes
afterward in both the shared codec/budget and native receiver tests. Portable
verification passes808 Rust tests, all504 local script cases in14.759s (six
Windows-only skips), strict host/MSVC diagnostics, formatting, actual release
WASM compilation and canonical preparation. Real selected/ordinary form
completion and subsequent native/package acceptance remain Windows gates.

Actual run38821192294/sourcef1523a4 produces nine valid contiguous command
records/2,945 raw bytes. D resolves Dark and returns Ok; actual Alt and Shift
callbacks enter Ready, resolve Ignored and return Ok. Native S335/336 is again
positive with the expected scan/modifiers and Handled=false. The retained
document prefix contains no S or terminal; missing closure leaves completeness
unknown. The504-case Windows suite completes43.301s (503 pass, Apple-only skip)
and four exact submissions pass,
but the form worker reaches its original60s limit. Last response193's Dark
value has no retained receipt time; pending194's node24 has no retained identity.
This does not establish browser interception, stale UIA or a blocked getter.
Provider a0467cfe controls retain the same missing S both with and without the
startup collector, alongside matching browser PIDs and non-normal SDK exit
kind1. All six fail shutdown before final assertions; the collector is not
necessary for either reproduced failure. No production shortcut policy or
Metis dependency changes follow from those negative controls.
The subsequent14cb3fdf/run38835227528 global-false control verifies the setting
before/after navigation and on actual S callbacks, then closes browser/profile
normally. It still retains D/P/eight keys without S and fails the mandatory S
assertion; later Closed-owner/stroke assertions and comparisons do not execute.
This separates that shutdown outcome from observed S delivery. Neither setting
experiment establishes a full-form correction or a precise interception cause.

Source checks preserve the closed page protocol, actual Ready admission and all
existing deadlines. The actual Windows run must retain the navigation records,
pre-Ready observation and exact native producer bytes. It must then satisfy the
existing computed form, permission, remount, destruction and package acceptance;
diagnostic output alone does not close native acceptance.

The diagnostic consumer run 38745314161 completes the first exact Unicode
calculation, then reports controller LostFocus before patient two while UIA
still claims focus and retains the previous value. No later eligible key
callbacks appear. Its 502 scripts execute in 34.004s (501 pass, Apple-only skip),
separating this functional defect from the earlier script budget failure.
Provider baseline run 38748681812 reproduces a missing parent-focus route:
SetFocus(parent) leaves its HWND as the keyboard recipient. The selected
correction synchronously calls controller MoveFocus(PROGRAMMATIC) outside any
window-state borrow; Weak window ownership and clear-before-Close preserve
reentrancy and lifecycle. The provider regression and both unchanged full
consumer journeys remain required before attributing complete form success.

Metis source b7ad7405 / run 38750658426 refutes full-form closure from that
parent-entry correction: the second dose remains 0.25 after requested 0.5.
The native controller ends with Lost/Got/Lost, while native renderer ownership
and UIA focused=true persist; no later eligible key callbacks or second submit
appear. Scripts complete in 44.777s (501 pass, one Apple-only skip); the form
still reaches its original 60-second deadline. Selection ordering remains
(1,1,0,-1); physical input admits requested UTF-16 [48,46,53],
which does not prove renderer consumption. The actor
records STA. Moirai's native MTA isolation exercises actual field focus/input
and all-field preservation, but differs from this actor's selection/native
reads and WM_NULL interleaving. Neither its compilation nor a future component
pass establishes this complete form journey or the renderer's consumption.

Provider source 23561b59 / Windows run 38763941253 now completes the isolated
ordinary mode's sixteen exact replacements and sixty-four preservation checks
in 5.683s. Selected mode completes the first reference, observes weight focus,
and admits all requested Ctrl+A and Unicode records for 64, but the last
changed ValuePattern remains 60. Its fixture log quota ends after query 99;
the final active native call is unavailable. The same controller emits no
later eligible keys after Lost/Got/Lost. Selected DOM observation completes
8,672 chains (at least 52,032 CDP calls); this establishes additional work,
not input-loss or starvation causality. Private-parent experiment 27cd682d
also passes ordinary 16/64 but still fails selected delivery after 6 edits;
it is withdrawn. Four-mode source 4e7b1968 passes ordinary and controller-only,
while startup-only and full selected retain prior values after actual focus
and admitted input. Controller callbacks are unnecessary to reproduce. The later domain/single-snapshot controls pass16/64. Coalescing source9cf4d47a /
run38779895376 bounds recurring work but still fails active startup-only final
dose0.125 (UIA0.5), after15/60 checks. Full selected16/64 exhausts its shared
observer quota and cannot establish active observation for all edits.
The one-shot provider9aca/run38785314523 captures live concentration2 after
requested4, matching returned UIA2 with admitted input, owned focus and active
sampler. The renderer responds to CDP while the requested value is absent.
This point excludes stale UIA alone; it does not establish earlier transient
assignment or identify the input-routing cause. Sourceaa532/run38788877397
still fails original startup-only concentration0/UIA4 after10/40. Its separate
controller recorder passes16/64 without triggering a snapshot, so no joint
failed-edit receipt follows from that run.
Both locks now selectaa532 plus the compatible Gaia/Eunomia/Hermes/Consus
allocator cohort; no Metis input, protocol or acceptance implementation changes.
Strict Clippy,777 local Rust cases and canonical preparation pass. The native
Windows full forms and complete script/package suite remain required. None of
these component results establishes equivalence with the PowerShell STA actor.

The [native input evidence](startup/evidence.md) retains the source-bound
acceptance history and current controller-to-Rust delivery boundary.
