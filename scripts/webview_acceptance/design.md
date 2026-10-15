# Actual Windows Rust-page acceptance

The gate executes compiler-produced Metis application and native-test binaries.
The committed verification runner builds the CLI first, invokes its canonical
`prepare` command, and supplies the resulting inventory only to native Cargo
compilation. The runtime receives no preparation environment override.

Both debug and release workspace test domains compile into canonical nextest
archives under that compile-only environment. Nextest extracts each archive
into an exclusively created empty directory, and its actual list JSON selects
the installed-runtime test executable. The full regular suites then execute
from the extracted Cargo/binary metadata under the clean environment, retaining
the committed profile and every archived test domain. Owned temporary archives
are removed after verification; metadata and artifact hashes remain evidence.

Each visible application journey and native fixture has a strict 60-second
outer process-tree deadline. The existing process-tree owner admits the child
before releasing its bootstrap and collects all descendants after completion.
Application closure must still prove its own actual page stop, native destruction
and fresh-profile cleanup before a scenario succeeds. Forced outer collection
never counts as successful application teardown.

UI Automation supplies actual control values, text, geometry and focus. Win32
SendInput supplies Unicode editing, pointer clicks, Return and physical command
shortcuts. No page script evaluates commands, supplies results, grants devices,
or emulates `chrome.webview`. The UIA tree and production JSON tracing observations
must agree with the submitted values and computed native terminal response.

Three independent numeric inputs use the real supervised headless calculation
as the unchanged-model journey, the real visible Rust page as the migrated
journey, and rational infusion equations as the independent analytical oracle.
The selected concentrations are powers of two, so the final rate-to-drug
multiplication is an exact binary scaling. Exact binary64 comparison suffices;
the gate does not choose a tolerance. Protocol input and result fields must use
the declared decimal JSON-string representation before conversion and bitwise
comparison, preserving signed zero and the original encoded evidence. Display
text retains its separate actual ECMAScript number-formatting observation.
A zero concentration must produce the
real rejection and restore submission before another valid calculation.

Real Rust fixtures own the compiled package and actual Surface. They send the
same closed production command schema through native `post_json`, observe actual
page events, and apply the canonical PageSession. Repeated stop/remount and
malformed, duplicate and stale commands must produce their specified values or
errors. A correlated terminal fixture obtains a genuine Submit through OS input
before posting a response computed by the real backend.
The three-mount fixture uses one visible Surface and one actual UIA actor.
Each mount requires reset output, an enabled submission button and a closed
command menu, then genuine OS editing, the real computed terminal output and
physical Escape. The first cycle opens the menu before submission; its first
Escape closes that menu and its second requests closure. Later mounts must
restore `Commands ready` and the reset output before another submission.
The existing production CloseRequested event controls every stop/remount;
there is no acceptance acknowledgement command or document evaluator.

The interactive permission journey independently requires all four actual native denial
events and all four settled browser outcomes. Missing hardware/API, a grant,
an unobserved native kind, an unsettled request or a timeout is a failed stage.
Denial reconciliation admits geolocation rejection with native code 1, camera
and microphone rejection named `NotAllowedError`, and notification denied/default.
Device discovery or read errors such as `NotFoundError` and `NotReadableError`
cannot replace a denial. Genuine native denial plus the declared browser denial
does not require successful device acquisition; positive acquisition is a
separate acceptance domain. Failure evidence retains exact SDK events, browser
error context and the missing native callback kinds.
The gate records that failure and its partial actual trace; it does not install
virtual devices, inject permission results or skip an unavailable runtime.
The original native preview capture invocation also runs as a separate bounded
scenario and publishes its frame only after actual stop/destruction/cleanup.

Trace records bind executable, test binary, inventory, Cargo lock and source
revision digests. Each successful lifecycle requires Ready, Stopping, Stopped,
Destroyed and successful owned-package cleanup in order. Resource observations
retain admitted settlement guards and realm dispatchers as distinct owners.
Screenshots supplement computed/UIA evidence and do not prove input, permission
delivery or lifecycle acceptance. IME, multimonitor and framebuffer scopes remain
separate existing evidence domains.

## Selected and ordinary production-form execution

Startup observation is an explicit fixture input, disabled by default. Only the
form worker admits it; every worker removes inherited selection and compile-only
inventory values before constructing its application environment. The selected
and ordinary cases invoke the same production form worker and full journey,
including all three rational calculations and the invalid-input rejection.
Success requires both cases. Selected readiness cannot establish ordinary
readiness. Worker identity and observation selection must match the coordinator's
declared case before the coordinator accepts its result.
`provider_startup_observation` records that explicit provider configuration;
`startup_observation` records passive GDI evidence and cannot establish the
provider selection or lifecycle. Ordinary execution cannot reuse a selected
report because the role and exact selection metadata are admitted together.

The immutable plan preserves interactive ownership in this order: observed form,
ordinary form, permissions, remount, correlated rejection. No two focus/input
owners overlap. Permission capture and malformed-command execution share one
noninteractive wave before remount: capture uses the native preview surface;
the malformed-command fixture creates a hidden window and sends only native
bridge commands. Their exclusive package roots include their actual process
IDs, and their profile, content and evidence directories remain separate.

The three independent headless calculations share one wave, retaining every
original input, actual process, analytical comparison and output. One dispatch
owner collects every owned future and every available original failure stream, and
returns outcomes in declared order rather than completion order. Native workers
use that same dispatch owner; no parallel copy of a journey or cleanup algorithm
is introduced. A failed or incomplete case fails the complete gate.

The canonical retention owner attempts each admitted stdout, stderr and combined
write independently for every drained outcome. Model evidence keeps its original
inclusive byte bound and rejects excess before persistence. Real persistence
errors accumulate after original child failures, so a failed first destination
does not prevent later stream retention. Worker native-log closure, partial
trace reads, observer/actor/native cleanup and final JSON writes preserve an
in-flight original error, retaining secondary native context in notes. A cleanup
failure without an earlier error remains its actual failure; it cannot count
as a successful worker.

The canonical process owner attaches actual captured text and observed child
status to the original exception. Its text decoder retains its existing UTF-8
replacement/newline normalization contract. Unavailable capture after an actual
allocation/read failure remains `None`; successful empty capture remains `""`.
Retention writes each available stream and combines them only when both reads
succeed. An observed deadline creates the primary timeout before native cleanup,
control closure or capture collection; secondary failures retain native context
without replacing that timeout. The POSIX supervisor remains unreaped until
the original group termination, and Windows Job bodies retain their semantics.
Real child/native-error constructor tests verify propagation and availability;
they do not claim to induce an OS tree-termination or Job-close failure.

Each command retains 60 seconds and its existing process-tree owner retains the
separate 10-second cleanup bound. The model wave plus six native waves therefore
declare `7 * (60 + 10) = 490` seconds. The unchanged 540-second coordinator limit
leaves 50 seconds for dispatch and collection; its existing outer owner rejects
any actual overrun. Neither per-fixture work nor cleanup deadlines are reduced.
This budget accounts for the cleanup interval that a command-only sum omits.

Portable verification inspects exact case coverage, ordering, isolation and
option rejection. Real child processes verify concurrent ownership, canonical
result ordering and failure-stream collection. A separate actual Metis headless
run checks the three original computed outputs; process-orchestration fixtures
cannot substitute for those application values. Installed Windows execution
must still verify the added ordinary form and the noninteractive wave.

The adopted Moirai 0.9.2 replay passes 480/480 cases in 14.470 seconds under the
unchanged 60-second process-tree bound with no skips. All 987 physical source
members retain exact membership and bytes before/after, and 960 timing records
pair all 480 ordered cases. Twenty focused process/acceptance cases include
native file/descriptor failures, inclusive UTF-8 stream bounds, child environments,
concurrent calculations and failure identity. Three actual compiled Metis
headless transactions from the freshly built adopted app reproduce rate/drug
pairs 0.48/0.96, 0.6/2.4
and 0.09/0.72; arithmetic process fixtures do not substitute for these values.

The preceding selected [Windows run](https://github.com/ryancinsight/metis/actions/runs/38531378051)
loads all five resources with status 200 and enables CDP observations, then
records actual Rust-handled DOM text `WebView2 bridge is not an EventTarget`.
No Ready occurs in that historical consumer run. The private Moirai 0.9.2 Rust
bridge correction is protected-merged in
[PR 621](https://github.com/ryancinsight/Moirai/pull/621) and adopted by seven Metis
requirements and both locks with all unrelated package identities preserved.
The [provider Windows run](https://github.com/ryancinsight/Moirai/actions/runs/38540325048)
passes 31 policy cases and seven installed-runtime fixtures, including its full
Rust page fixture. The adopted graph passes 777 Rust tests, 480 script cases,
42 doctests, strict diagnostics, twelve browser assets and fourteen native asset
rejection cases. CLI/app/backend selected Windows compilation passes. Both Metis
form journeys remain pending; portable and provider results cannot establish
ordinary or selected Metis full-form acceptance.

The first adopted Windows replay completes all 480 script cases in 37.016s,
with four new persistence-assertion failures and the existing Apple-only skip.
Three readers use platform-default decoding for UTF-8 evidence; one compares
a raw Windows path with its escaped diagnostic representation. No native
journey, archive execution or package acceptance runs in that replay.
The corrected readers use the persisted UTF-8 contract and exact represented
paths. All twenty focused cases pass under actual US-ASCII in 1.913s; the same
480 full-suite cases pass in 17.335s without skips. Values, case ordering and
all execution bounds remain unchanged. Native Windows replay remains required.

The corrected [Windows replay](https://github.com/ryancinsight/metis/actions/runs/38545004083)
passes 479 cases with the existing Apple-only skip in 32.495s. The actual form
emits exact-identity Ready and DOM reports the connected bridge. Focus acquisition
then fails before UIA input. The requested child and foreground success alone
do not identify actual focus, class or input queues; native observations must
precede a correction. No ordinary form, later native case, archive execution or
MSI acceptance occurs. Listed or compiled archive cases do not count as execution.

The diagnostic increment retains its original criterion handles separately from a
later non-atomic read-only snapshot. At most 22 existing SetFocus attempts and
26 failure-only native reads retain raw return/error data, window ownership,
classes, GUI-thread state and child relationships before detachment. Native
LastError can be stale and grants no success. Original exceptions and ASCII JSON
notes survive complete worker stderr into the always-uploaded native artifacts.
All 27 focused and 484 full script cases pass; eleven independent AST/control
comparisons preserve activation, queue attachment, retries, exact success and
cleanup. The 777-value Rust replay reports one unattributed output leak; its
complete output is retained. Strict Clippy/Rustdoc and 42 doctests pass.

## Native keyboard ownership

The [instrumented replay](https://github.com/ryancinsight/metis/actions/runs/38549695922)
passes 483 script cases with one existing Apple-only skip in 28.076s. Exact
Rust Ready and all five resources succeed. Its selected target is a visible,
enabled `Chrome_WidgetWin_0` in the frontend thread. Actual focus is a visible,
enabled `Chrome_WidgetWin_1` in a WebView runtime process, and native `IsChild`
reports it as a descendant of both the target and the foreground Metis window.
The first SetFocus return identifies the previous top-level focus; the later
twenty returns identify the previous delegated recipient. The final criterion
and separate queue snapshot establish actual focus. Exact HWND equality rejects
this real ownership relation before any UIA input.

For selected target T, actual focus F and expected top-level H, admission is
`GetForegroundWindow() == H && T != 0 && F != 0 && (F == T || IsChild(T, F))`. Equality
is necessary because IsChild excludes the window itself; its descendant
relation is transitive. An ancestor, sibling, unrelated window or absent focus
rejects. PID, class and thread equality cannot replace native parent ownership:
the observed WebView recipient lives in another process.
[Microsoft specifies the descendant relation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-ischild).

One canonical read observes foreground and focus once per criterion before
queue detachment, retains those exact values, and applies this predicate in
both bounded retries and final admission. A mismatched foreground rejects in
the canonical owner because accessibility callers do not inspect its tuple.
The tuple still identifies the selected container and its thread's keyboard
layout; the capture workflow compares that container with asynchronously
created content. It must not substitute the delegated recipient.

The native regression creates real Win32 windows and exercises target, child,
grandchild, sibling, ancestor, unrelated and absent focus, plus mismatched
foreground. It uses real GetFocus/IsChild and destroys every owned window;
non-Windows execution cannot establish these cases. The unchanged selected
and ordinary production journeys remain the cross-process WebView/input proof.
Retries, sleep intervals, queue attachments, input methods and deadlines retain
their existing bounds. No class/PID allowlist, synthetic browser, injected
script, replacement result or input bypass supplies acceptance.

Local verification preserves all 484 original ordered cases and adds one
real-Windows hierarchy case: 485 total, 484 pass and the Windows-only case skips
on Darwin in 16.889s. All 990 physical source members retain exact bytes during
the suite; 970 timing records pair every case. Independent review binds the
native predicate/test ABI and all four changed source/design identities. All
373 supported API-graph inputs remain exact; the existing partial comparison
result retains its scope. This source review and portable pass cannot replace
execution of the native hierarchy and complete selected/ordinary journeys.

## Retained UI Automation boundary

The [focus-corrected replay](https://github.com/ryancinsight/metis/actions/runs/38552896154)
passes the real Win32 hierarchy test and 484 other Windows cases with one
existing Apple-only skip: 485 total in 32.241s. The actual Rust form submits
Unicode patient input with 64/2/0.25 and posts request 1, audit 2, rates 0.48/0.96.
Native CDP then observes that exact result in the document. The worker reaches
its unchanged 60-second deadline before its final report. No retained UIA reply,
wait boundary or primary exception identifies the remaining operation. This
cannot establish that UIA consumes the result or reenables submission, nor
distinguish a native query, nonmatching predicate, cumulative cost or cleanup.

One Python progress owner persists the current operation and request identity
before sending, and exact bounded response bytes before parsing/admission. Its
last state and last response each retain the existing inclusive 256 KiB bound.
Same-directory pending files and replacement preserve the prior complete file
when killed before publication. Native persistence failures keep their actual
context; an existing primary exception remains primary. Sequence, monotonic
time, last-response digest and predicate outcome distinguish an unreturned
request from completed nonmatching observations. No response or value truncates.

The PowerShell actor retains entered-call markers in two alternating, already
open bounded slots. Each candidate serializes and admits its byte count before
overwriting one slot; an interrupted write can invalidate that slot, while the
other retains the prior complete marker after initialization. Marker sequence,
node index, Stopwatch ticks/frequency, process/thread and actual apartment state
locate existing native calls without adding UIA queries. Flush publishes process
buffers; it does not establish power-loss durability. A marker records entry
intent, not proof that a preempted thread enters or completes the following call.
The original COM apartment, native getter order, response protocol and input
remain unchanged. Source-derived threading risks are not timeout attribution.

The Python state/response, their two pending files and the two actor slots give
six independently bounded files: at most `6 * 262144 = 1572864` bytes. This is
separate from the unchanged per-response and native JSON trace admission limits.
Primary exceptions and each cleanup wait boundary publish before cleanup, so
outer process collection cannot erase the only observed failure in memory.
Local storage/value/boundary tests pass within the complete 493-case replay:
491 pass and two Windows-only cases skip in 14.520s, with all 993 source identities
unchanged. Official PowerShell 7.6.7 executes actual marker writes, inclusive
byte bounds and Unicode/quote path round trips, and parses the complete actor
program without errors. This portable result does not establish Windows UIA or
Windows PowerShell 5.1 execution. The real Windows actor case pumps its creating
thread's native message queue while a worker queries the owned controls; it must
verify exact Unicode values, raw response bytes and completed actor cleanup.
The next complete journey must still satisfy every original numeric, Unicode,
error, theme, focus, permission, remount, close and cleanup assertion.

The first Windows actor regression returns an admitted tree but finds no exact
Unicode value; its former temporary-directory cleanup loses the raw payload.
The regression retains the original Win32 text/UTF-16-unit count, complete raw
response, progress and actor slots under `output/browser/windows-uia-regression`.
Its unchanged assertion also reports the decoded observed nodes on mismatch.
Directory collisions preserve the platform's native exception: Windows reports
PermissionError/errno 13 where POSIX reports FileExistsError/errno 17.

The retained next Windows reply includes the nested EDIT and exact Unicode in
its name, but reports only Pane/value-null for all four native controls. Original
Win32 text, response bytes and successful actor cleanup agree. The missing
standard-control projection remains distinct from the earlier form timeout.
The actor requires the real built-in client-side provider table before its first
query through the documented [provider registration API](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.clientsettings.registerclientsideproviderassembly?view=netframework-4.8.1).
A fresh assembly identity takes the loaded UIAutomationClient's version,
culture and public-key token; actual loading must return that exact full name.
Registration or identity failures propagate through the existing actor failure
boundary. No substitute provider, query retry or changed value predicate applies.
The framework may prepend its real default table during explicit registration;
its lookup stops at the first successful factory. Installed Windows execution
must verify the same EDIT/Unicode value before full form acceptance resumes.

An equivalent-runtime Windows run loads the matching provider assembly, then
the direct PowerShell registration call throws NullReferenceException before
any query. The retained message does not identify its inner framework stack.
Registration now invokes the same public static method through its typed
MethodInfo; failure retains the original exception, the stored error record for
cleanup and its complete stderr context. No alternate provider or retry follows.

The complete helper, literal paths and actor persist as one atomic UTF-16LE
source file with a BOM before launch. PowerShell receives that owned path through
`-File`; program growth no longer expands its CreateProcess command line.
This source file remains in the original browser artifact root beside the
bounded progress and response files. The canonical stream owner closes the
parent's stderr descriptor after Popen transfers its duplicated native handle
to the child. The child owns that handle until exit; a failed launch retains its
original exception and any actual parent-stream closure error. The UTF-8 request
protocol, actor apartment, native query/getter order and deadlines remain intact.

The [next Windows run](https://github.com/ryancinsight/metis/actions/runs/38565495242)
passes registration and the unchanged real Unicode EDIT/value/role regression.
The form reaches Ready, but repeated UIA snapshots report weight `60` after the
journey requests `64`. Request 103 still progresses near the unchanged deadline;
no single blocked getter or input-loss mechanism is established. The last reply
overwrites the earlier focus reply, while in-memory actions do not survive the
outer process collection. These are evidence gaps, not input failure diagnoses.

Before focus, selection and Unicode insertion, and after insertion and the sent
WM_NULL returns, the same bounded progress state now preserves the last input
transaction independently of later polling. It contains exact requested text,
UTF-16 units, matched focus-reply nodes and sequence, foreground HWND, the
foreground thread's GUI focus recipient, ownership, and native modifier states.
Each stage contains separate read-only native observations; they are non-atomic.
Modifier fields retain raw GetAsyncKeyState values and their high-bit predicates;
zero permits a released key or a desktop/access failure, so it cannot prove release.
The complete encoded state retains the inclusive 256 KiB bound and an independent
JSON snapshot before publication, so later caller mutation cannot rewrite prior
facts. Failure rejects before replacing the previous complete state. Actual child
termination verifies that the last complete input boundary remains inspectable.

The actor names the actual ValuePattern or RangeValuePattern that supplies each
value; it preserves existing pattern precedence and all native getter calls.
Input records still use genuine SendInput. Microsoft specifies insertion, not
application consumption, for [SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput).
The existing [SendMessageTimeout](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeoutw)
call waits for its sent WM_NULL, not for queued keyboard or renderer work.
No observation grants focus, supplies a value, retries delivery or extends a
deadline. The scenario does not close the retired parent stderr handle after
launch; the child and canonical stream owner retain their existing lifetimes.

The patched-lock [Windows run](https://github.com/ryancinsight/metis/actions/runs/38586243542)
passes 493 of 494 scripts with the existing Apple-only skip, all three actual
calculations, invalid concentration rejection and Light/High Contrast selection.
It reaches the unchanged deadline waiting for the first Dark shortcut. The last
text-input snapshot predates that shortcut by 34.96 seconds; it does not establish
the shortcut recipient or modifier state. Scan-code construction is nevertheless
a source-established defect: the old injected records carry zero scans while
Rust binds browser physical codes. The [native input contract](../native_input/design.md#physical-key-contract)
owns its repair and real native regression. The shortcut now retains its own
before/after focus/modifier observations and exact admitted scans, independently
of later polling. Only another complete native journey establishes command
delivery or closes this remaining application gate.

## Read-only text-selection evidence

An input replacement retains three selection observations on the uniquely
named control: after focus/before Ctrl+A, after the physical chord releases,
and after Unicode insertion and the existing sent-message flush. Each records
its actual UIA request sequence. The actor reacquires that control's TextPattern,
DocumentRange and GetSelection on every observation. No ancestor, cached range,
TextPatternRange.Select, ValuePattern.SetValue or replacement input supplies
missing evidence. Unavailable TextPattern, SupportedTextSelection.None and
an absent range remain distinct records. GetSelection is not called for None.
An absent range is never treated as a degenerate caret.

For exactly one selected range S and the same provider's document range D,
retain four signed Int32 CompareEndpoints results: S.start versus D.start,
S.end versus D.end, S.start versus S.end and D.start versus D.end. Only sign
and zero have ordering meaning; their magnitude is not a character count.
Whole-document selection requires the first two comparisons to equal zero;
a caret requires the third comparison to equal zero. These are diagnostic
interpretations, not added acceptance predicates. Multiple or absent ranges
carry their actual count without invented unique-range endpoints. ValuePattern
remains the independent value observation; no text normalization or assumption
that TextPattern text equals ValuePattern.Value bridges their representations.

The canonical replacement operation requires an explicit value representation
from every caller. WebView input controls select CONTROL: observed value equals
the requested OS text. The single-paragraph RichEdit fixture selects
RICH_EDIT_STORY: observed ValuePattern value equals the requested text followed
by TOM's undeletable final CR. Actual WM_GETTEXT still equals the requested text.
The first native replay establishes this distinction before any replacement:
seed window text has twelve UTF-16 units while its ValuePattern has thirteen.
The expected observed value and selected representation accompany the original
input in progress. The input encoder admits only the original text; the provider
marker cannot be injected as committed input. No trimming, normalization,
detected-value fallback or alternate success predicate applies to WebView.
This policy models these two control contracts, not arbitrary RichEdit
multiline/newline conversions or universal equality between native APIs.

The existing response/progress byte bound and scenario deadline cover these
non-atomic reads. Original COM errors propagate with the native-call progress
stage. No read retries the physical input, changes selection, acknowledges
renderer delivery or increases the deadline. An observed successful replay
with the additional reads does not by itself attribute an earlier failure.

The regression uses a real single-line RichEdit control whose documented Ctrl+A
behavior selects all. Two successive canonical Uia.replace calls must replace
the initialized text, then the first Unicode value with the second. Actual
UTF-16 window text, UIA ValuePattern replies, selection observations and native
EM_EXGETSEL records remain evidence. The original plain EDIT focus/value and
keyboard metadata cases remain active; no fixture handler implements SelectAll.
For a plain paragraph with N editable UTF-16 units, TOM's single final CR gives
a complete story of N+1 units. Native selection must start at zero and end at
N or N+1: every editable unit is selected, and only the final undeletable CR
can additionally belong to the selection. The fixture retains the actual
endpoint, all Unicode messages and exact final window text. This two-element
representation domain follows the selectable final CR; it is not an error
tolerance or permission to omit any editable character.

Sources: [RichEdit shortcuts](https://learn.microsoft.com/en-us/windows/win32/controls/about-rich-edit-controls),
[GetSelection](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.textpattern.getselection),
[CompareEndpoints](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.text.textpatternrange.compareendpoints),
[TOM story invariant](https://learn.microsoft.com/en-us/windows/win32/api/tom/nn-tom-itextrange).
