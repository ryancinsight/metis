# Native Rust page startup evidence — 2026-10-14 [patch]

Owner: [native acceptance gap](../../../../../../../gap_audit.md#native-rust-page-startup-evidence--2026-10-14-patch).

[Run 38511644468](https://github.com/ryancinsight/metis/actions/runs/38511644468)
tests the preceding preparation repair in [PR 504](https://github.com/ryancinsight/metis/pull/504). Apple passes
773 tests, 42 doctests and three bundles; all five guards and fourteen fuzz
targets pass with 99,906,875 actual executions. Windows runs 460 script cases
in 21.647s: 459 pass and one existing Apple-only case skips. Browser resources,
native preparation, strict native compilation, build and archive discovery pass.
The archive's 773 tests have not executed when the native acceptance step fails.

The actual form emits no page message before its 30-second readiness deadline.
Its retained trace observes browser-resource release and browser shutdown, then
the readiness timeout; it does not observe form input, remount or permissions.
Three headless model transactions pass separately and do not validate that UI.
The generated native module bytes, page text, console/resource errors and image
are absent, so the precise startup cause remains unobserved. Static review finds
no bootstrap ABI, markup, shortcut or default-theme mismatch.

The [startup contract](../startup.md)
requires actual navigation records, read-only pre-Ready observations and original
producer-byte retention. Navigation success and diagnostic pixels cannot grant
Ready, change input ordering or extend the existing fixture deadline. Final
local diagnostics pass 773 Rust tests in 8.193s, 469 script tests in 14.051s,
42 doctests and strict checks. All 979 source files retain exact membership and
bytes. Actual canonical preparation retains all four original files with
independently rehashed receipts; fresh producer/native admission, all fourteen
mutations, twelve browser assets and selected CLI/app MSVC strict checks pass.
The diagnostic replay in [run 38515576891](https://github.com/ryancinsight/metis/actions/runs/38515576891)
stops earlier: 469 Windows script cases in 22.356s report four errors, one
failure and one existing Apple-only skip. The new producer-receipt tests cannot
match the inventory identity. Windows `DirEntry.stat()` omits native identity
fields; this owner must use actual path metadata. That failure supplies no
native startup observation. The correction reads fresh non-following path
metadata on every host and retains native errors with operation/path context.
All six filesystem cases and the full 470-case local script suite pass;
canonical preparation retains and independently rehashes all four original
files. All 979 source identities remain unchanged during that replay. Rust
implementation and test bytes retain their prior passing gate identities.
The corrected [Windows replay](https://github.com/ryancinsight/metis/actions/runs/38516491259)
passes all six receipt cases: 470 script cases run in 28.547s, with 469 passes
and one existing Apple-only skip. Its four original native files match the
receipt and acceptance inventory. All 221 module imports match the generated
glue; every referenced export exists and the raw start has signature `() -> ()`.
Actual navigation succeeds, but Ready still times out after 30 seconds. The
retained desktop image shows a white client with no readable Rust failure.
Neither those pixels nor a coherent ABI identifies the failing startup stage.
Native resource/runtime and document observations remain required before a
product correction. Apple passes 773 tests, 42 doctests and three bundles;
all fourteen fuzz targets pass with 102,728,525 executions. Installed acceptance remains
[METIS-WEBVIEW-RUST-001](../../../../../../../backlog.md#METIS-WEBVIEW-RUST-001).

The selected diagnostic increment in [Moirai PR 620](https://github.com/ryancinsight/Moirai/pull/620)
uses private Rust-native observations rather
than injected page code. Form-only child selection, a dedicated JSON formatter,
one generation-bound asynchronous DOM chain and receiver cleanup preserve the
existing protocol and deadlines. The exact formatter envelope is 133 bytes
including LF; the aggregate selected-diagnostic bound is 427,530 bytes. The
[protected provider run](https://github.com/ryancinsight/Moirai/actions/runs/38526145575)
passes 31 Windows policy/frame cases, including six new COM/state regressions,
and six installed-runtime fixtures. The selected DOM/shutdown fixture passes
in 1.747s on runtime 153.0.4234.48 and SDK 0.39.1. Metis adopts the merged
0.9.1 source in seven requirements and both locks, preserving all 317 unrelated
root identities, 240 unrelated fuzz identities and 978 other physical inputs.
Fresh API comparison passes 30 supported boundaries and 4,900 substantive
queries against 25 nonempty graphs; five empty roots, skipped queries and the
15 unsupported boundaries remain excluded. A fresh
working-tree audit scans 497 product code files and reparses all five entry
documents: the same four required initializers retain 359 UTF-8 bytes, twelve
physical lines and nine nonblank lines. All eighteen independent browser
observer owners retain exact bytes; no application-policy JavaScript is added.

The final provider-adoption replay passes 776 Rust tests in 8.050s, 471 script
tests in 13.737s, 42 doctests and strict diagnostics. All 981 physical input
identities remain unchanged through verification. Fresh preparation, independent
producer/native admission, all fourteen rejection mutations and twelve browser
assets pass. Strict Windows CLI/app compilation passes after the binary caller
propagates the concrete subscriber initialization error through its existing
error boundary. Canonical preparation retains and rehashes all four original
files. These checks do not establish actual Metis Windows startup.

Subsequent source review finds that the selected form environment reaches only
the backend. The Rust supervisor clears it before the contained frontend;
the direct Python-child test does not exercise that boundary. Consequently
[run 38528638420](https://github.com/ryancinsight/metis/actions/runs/38528638420)
executes the ordinary form without the intended native observation selection.
Its Windows suite runs 471 cases in 26.833s: 470 pass and one existing
Apple-only case skips. All four original preparation files match their retained
identities. Actual form navigation succeeds, then Ready times out with no
startup-observation records. Apple passes 776 Rust tests, 42 doctests and three
bundles. The later form/input/permission/package journeys remain unexecuted.
The correction transports exactly the private selector through Runtime while
keeping Isolated empty and all unrelated application values excluded. Real
two-hop contained-process tests must verify selection, absence, Unicode and
opaque malformed values. Native startup attribution still requires the actual
selected Windows run; diagnostic activation cannot establish an unselected
form correction.

The launch-boundary regression reproduces exact selection loss before the
one-key Runtime correction and passes all ten real-process combinations after
it. The final replay passes 777 Rust tests, 471 script tests, 42 doctests and
strict workspace diagnostics. The two ignored child-entry tests execute through
their active owner across those combinations; they are not independent runtime
acceptance. Windows execution and selected native startup observations are pending
at that revision. The preceding published revision also passes all fourteen fuzz targets
with 133,569,653 actual executions; its aggregate remains failed at native Ready.
The current audit scans 498 product code files and all five entry documents;
the four initializer bodies still contain 359 bytes of authored JavaScript.

The selector-corrected [run 38531378051](https://github.com/ryancinsight/metis/actions/runs/38531378051)
runs 471 Windows script cases in 30.123s: 470 pass and the existing Apple-only
case skips. All four prepared originals match their receipts. HTML, CSS,
bootstrap, generated glue and WASM return 200 with their declared content types;
both CDP observation domains enable. Actual selected DOM reports
`Rust page operation failed` and `WebView2 bridge is not an EventTarget`.
This is a handled Rust initialization failure at the provider bridge admission
boundary, not inferred resource or bootstrap failure. No Ready, form input,
permission/remount journey or native archive execution occurs. The owning
Moirai 0.9.2 private Rust correction is now protected-merged in
[PR 621](https://github.com/ryancinsight/Moirai/pull/621). Its
[Windows run](https://github.com/ryancinsight/Moirai/actions/runs/38540325048)
passes 31 policy cases and seven installed-runtime fixtures, including the full
Rust page fixture in 1.946s; all sixteen active checks and 446 patch compatibility
queries pass. Metis updates seven requirements and both locks to the merged
revision: fifteen root and eleven fuzz family records change; all 317 root and
240 fuzz unrelated identities remain exact. Gaia 0.5.3 and independent
moirai-webview 0.2.0 remain unchanged. The adopted graph passes 777 Rust tests,
480 script cases, 42 doctests, strict diagnostics, twelve browser assets and
fourteen native asset rejection cases. CLI/app/backend selected Windows
compilation passes; actual Metis Windows acceptance remains pending.

The acceptance source now requires two complete form runs through the same
worker/journey: selected observation and ordinary production startup. Both
retain all three calculations, invalid-input rejection, themes, commands,
shortcuts, focus and closure. Selection defaults off, rejects other roles and
removes inherited selector/compile-only inventory values. Worker role and
explicit provider configuration must match the requested case; GDI screenshot
observations remain a distinct field and cannot grant Ready.

Three headless owners share one wave; only the independent permission-capture
and hidden malformed-command cases share a native wave. All focus/input cases
remain sequential. Seven total waves retain 60s execution plus 10s cleanup each:
490s within the unchanged 540s coordinator bound. Every owned outcome drains
before propagation. Original child/body errors precede actual stream, report
and cleanup failures; later streams still retain, and oversized model evidence
rejects before persistence. Real native filesystem/descriptor failures verify
these rules without replacing application or OS input. The canonical process
owner preserves original exceptions and available captured text/child status;
an observed timeout remains primary before cleanup and capture. Unavailable
capture remains distinct from successfully read empty text. Constructor tests
use real child streams/native errors without claiming induced OS cleanup failure.

Earlier [Windows run 38552896154](https://github.com/ryancinsight/metis/actions/runs/38552896154)
runs 485 cases in 32.241s: 484 pass and one existing Apple-only case skips; the real-Win32 focus hierarchy case passes.
The implemented nonzero target-or-descendant focus/foreground-top law is verified on Windows; requested-target layout, activation, retries and deadlines remain unchanged.
All four prepared originals, five resources with status 200/declared MIME types and Ready pass. Actual Unicode `患者-🦀-one` input 64/2/0.25 submits request 1; native Result and CDP DOM show 0.48 mL/hour, 0.96 mg/hour and audit 2.
The worker then reaches 60.0879668s. No UIA raw reply/progress or primary error before cleanup survives; query, predicate, cumulative cost and cleanup remain unattributed possibilities.
The selected diagnostic patch retains bounded last Python operation/request/raw reply, two alternating fixed PowerShell native-call-stage slots and actual apartment state, plus primary error/cleanup stage before waits.
The preceding local replay completes all 493 cases in 14.520s: 491 pass and two Windows-only cases skip; all 993 source identities remain exact. Official PowerShell 7.6.7 verifies real marker storage/bounds, Unicode/quote paths and complete-program parsing. The later native actor verification below supersedes this local-only scope; input, query order, predicates, deadlines and COM model stay unchanged. No timeout cause or fix is established.
Diagnostic run 38559577365 completes 493 Windows cases in 27.423s: 489 pass, one assertion fails, two error and one existing Apple-only case skips. Windows returns native PermissionError/errno 13 for two directory collisions; the new actor returns an admitted tree without the expected Unicode value. Its temporary response is deleted before artifact retention, so the mismatch remains unattributed. Original native text and complete actor files now retain under the existing browser artifact root; application preparation and journeys remain unreached in that run.
Retained run 38560940328 completes 493 cases in 40.533s: 491 pass, one assertion fails and one existing Apple-only case skips. Both collision cases pass. Native text and UIA name/text are exact Unicode; all four nodes expose Pane/value-null, while raw/progress and exit 0 agree. This projection mismatch precedes explicit real built-in provider registration and the later passing EDIT/value/role regression. Missing descendants and lost Unicode are refuted; the original form timeout remains unattributed.
Equivalent-runtime run 38563038128 completes 493 cases in 39.354s: 491 pass, one error and one existing Apple-only case skips. Matching assembly loading passes, but direct SDK registration throws NullReferenceException before any query; no inner framework stack is retained. Typed public MethodInfo invocation, complete original exception retention and an owned UTF-16LE/BOM script launched through -File define the correction. Latest native execution [run 38565495242](https://github.com/ryancinsight/metis/actions/runs/38565495242) completes 493 cases in 37.921s: 492 pass and one existing Apple-only case skips; the real Unicode EDIT/value/Edit-role case passes in 6.312691s. All 827 source identities, 73 artifact members, two actor programs and four prepared originals bind; Ready and five resource responses pass. The selected form reaches 60.0935128s while polling weight: active request 103 follows admitted request 102 with focused value `60` against required `64`, exact patient text and no submitted calculation. Late native stages continue at node 41; no single blocked getter or input-loss mechanism is established. Three actual headless outputs pass; the prior inner exception site remains unobserved.
Input-boundary replay [38586243542](https://github.com/ryancinsight/metis/actions/runs/38586243542), [verified source commit](https://github.com/ryancinsight/metis/commit/41101f3691a5d1abce92467ee003d22b95982a42), passes 493 of 494 scripts in 33.768s with the existing Apple-only skip; 827 reported sources, 75 artifact members, two actor programs and four prepared originals bind. Actual native/CDP calculations 0.48/0.96, 0.6/2.4 and 0.09/0.72, invalid concentration 0x3002, and Light/High Contrast selection pass. First Alt+Shift+D Dark waits to the unchanged 60-second deadline. Its last text-input snapshot is 34.96s old and cannot establish shortcut delivery. The old native sender supplies zero scans while Rust requires browser physical codes: the [physical-key contract](../../../../../../../scripts/native_input/design.md#physical-key-contract) defines actual-recipient HKL mapping, immutable scan/E0/release records, missing/E1 rejection, all-caller migration and real message verification. Independent source review and the full 496-case local replay pass in 17.015s: 493 pass, three native Windows cases skip, and all 994 source identities remain exact. The activation tap explicitly selects the actual foreground queue HKL before focus; held scopes preserve primary errors and release ownership. Fresh native execution must determine whether this correction closes Dark; the prior weight stall is not attributed. Later journeys, 847 listed debug archive executions, release and MSI remain unreached.
Retained native form [run 38685234146](https://github.com/ryancinsight/metis/actions/runs/38685234146), exact ae1eb71, binds 837 reported inputs to 1,003 committed files, 80 original artifact members, four prepared files and three actor programs. All 502 Windows scripts finish in 40.796s: 501 pass and the Apple-only case skips. Physical keys pass in 80.134ms, plain EDIT/UIA in 4.916s and both RichEdit replacements in 1.740s. Both nine-unit native values, distinct ValuePattern stories, 24 recorded native messages and full selections retain exact values with no observer errors. Ready and exact patient text plus weight 64 occur; dose requests 0.25 while focused/enabled/onscreen Spinner ValuePattern remains 0.2. Selection requests 20/21/22 retain (0,-1,0,-1), an ordering vector rather than distances. Actual [48,46,50,53] units, owned recipient and modifier observations do not establish consumption. Reply 115/pending 116 retain ongoing queries over 48.388s; the 60.101s timeout identifies no single hung getter. No calculation is submitted; three independent headless rational cases pass. Ordinary form, later native journeys, 847 debug archive executions, release and MSI remain unreached. The changed failing field under unchanged runtime sources proves no product fix. The initial diagnostic source 924dd8ac70d6 in [Moirai PR 622](https://github.com/ryancinsight/Moirai/pull/622) passes real Ctrl+A/Escape, unchanged Handled and owned cleanup in Windows run 38743789587; that replay selects focused candidate bd88d2c56f9d. This is an explicit candidate rather than protected-default adoption; Apple facade 0.2.0 and unrelated lock records stay exact. Retained 53af Apple execution passes 777 tests, 42 doctests and three bundles; five guards and fourteen actual fuzz campaigns pass with 101,685,436 executions in 704 reported seconds. Root/fuzz rustls 0.23.46 retains other 331/250 records; root audit passes all four classes. Standalone fuzz NCSA/metadata license admission stays open without a waiver in [METIS-FUZZ-LICENSE-001](../../../../../../../backlog.md#METIS-FUZZ-LICENSE-001).
Current diagnostic [run 38745314161](https://github.com/ryancinsight/metis/actions/runs/38745314161), source 05a2e7c, executes 502 Windows script cases in 34.004s (501 pass, one Apple-only skip) and submits exact `患者-🦀-one` with 64/2/0.25; the native result and DOM agree on 0.48/0.96. Before patient two, the retained parent FocusGained/FocusLost pair and controller LostFocus (sequence 58) precede complete absence of further controller key callbacks. UIA still reports the patient field focused and ValuePattern `患者-🦀-one` after requested `患者-🦀-two`; native focus stays on owned Chrome_RenderWidgetHostHWND. These disagreeing focus layers identify a controller-receipt discontinuity; renderer consumption for that edit remains unestablished, and its Rust submit handler is not reached. No callback loss, getter error or quota record occurs. Provider baseline run 38748681812 fails the real parent-focus assertion in 1.957s because the recipient remains the parent HWND. The provider synchronous-entry regression passes, but [Metis b7ad7405 run 38750658426](https://github.com/ryancinsight/metis/actions/runs/38750658426) still fails the second dose replacement (0.25 retained after 0.5 requested) at 60.0879709s. Its 502 script cases finish in 44.777s: 501 pass and one Apple-only case skips. The native controller ends with Lost/Got/Lost, last two 5.5us apart, sequence108; native focus stays on the owned renderer while all later key receipt stops. UIA selection comparisons remain (1,1,0,-1), admitted replacement units are [48,46,53], and the actor remains STA. Only request1 submits 0.48/0.96; second submit, ordinary form and later native/archive/release/MSI execution remain unreached. A real native MTA UIA isolation is now under validation, with the actual actor/input/interleaving difference explicit; all original input and acceptance limits stay unchanged. Independently matched 837 reported source digests for each of runs 38678882731, 38682223774 and 38685234146 canonicalize CRLF/CR even for binary inputs, so these digests alone do not prove raw-byte identity. The [case and budget contract](../../../../../../../scripts/webview_acceptance/design.md#selected-and-ordinary-production-form-execution) still requires both full form journeys and every original native scenario.

Provider9cf4d47a bounds recurring work but startup-only fails. Provider9aca/run38785314523 captures timely live concentration2 and UIA2 after requested4, with admitted physical input, owned focus and active sampler; no Rust form handler exists in the fixture. That point excludes stale UIA alone and does not establish transient assignment history or routing cause. Provideraa532/run38788877397 still fails startup-only concentration0/UIA4 after10/40; its independent controller variant passes16/64 without triggering a snapshot. The necessary Gaia0.6/Eunomia0.10/Hermes0.9/Consus0.2.1 cohort changes37/28 related Git records; all295/223 others remain exact. Strict Clippy,777 local Rust cases and canonical module preparation pass.

Metis source5e5c4e2/run38792889172 passes502 Windows scripts in37.299s and
completes four exact Unicode submissions: three successful calculation pairs
and the expected0x3002 zero-concentration rejection. The selected journey then
retains Dark after Alt+Shift+S and exceeds the original60-second worker limit.
Controller quota exhaustion precedes submit4; no late keyboard/DOM receipt
establishes where that shortcut stops. The final UIA result and exact inputs
match the third successful calculation; a separate runtime ledger4 value is
not retained. Both locks now pin provider2fcef45d, whose independently
reproduced ready-event/deadline correction changes15/11 provider SHA records
and no other lock bytes. All Metis input, protocol and acceptance source stay
exact. Corrected provider native execution, both complete form journeys and
package execution remain required.

Admission-only sourcead328/run38799513267 again reaches all four submissions
and exact results/rejection, then retains Dark on Alt+Shift+S;502 Windows
scripts pass55.690s. Its actualmergefb14678/tree6e33d4f5 binds all1003 Git blobs
and837 normalized run-source digests. The observed page module identity is the
original inventory SHA256, distinct from the separately retained WASM-file SHA.
The latest UIA query166 is sent without a final response; query165 retains Dark.
The exhausted controller quota cannot attribute the late shortcut.

Provider2fcef's separate failed probe has a complete unexhausted controller
history: successful keys use one owned HWND, then Lost/Got/LostFocus ends at
another owned HWND, where4 select-all and18 Unicode records are admitted with
no later controller key receipt. Both targets share browser process, thread
and layout. A timely live snapshot and UIA retain the previous patient. The
new explicit native-parent entry policy addresses that setup gap through the
existing programmatic controller route. It performs one real SetFocus, verifies
an owned non-root descendant and preserves the selected UIA field; it does not
repair LostFocus asynchronously or retry input. Local504 scripts pass with six
Windows-only skips; corrected native forms remain unverified. Provider6343610d
adds the matching real fixture entry and16/12 computed entry checks while
preserving its16 replacements/64 field checks. Both consumer locks change only
15/11 provider SHA records from2fcef; full Windows and package gates remain.

Provider6343610d [run38803044085](https://github.com/ryancinsight/Moirai/actions/runs/38803044085)
passes 69 actual Windows cases, including all eight original replacement modes,
the separate probe and actual Rust bridge. Each replacement invocation checks
16 native entries, 12 non-first entries, 16 replacements and 64 field-preservation
checks. The separate probe does not trigger a new DOM snapshot; exhausted
startup/controller histories remain incomplete.

Consumer add41427 [run38804568395](https://github.com/ryancinsight/metis/actions/runs/38804568395)
completes 504 Windows scripts in 40.426s (503 pass, Apple-only skip) and all four
exact Unicode submissions, then retains Dark after Alt+Shift+S at the unchanged
60-second worker deadline. Actual merge c1dadecf/tree91d14f7d binds 1,003 raw Git
blobs and 837 normalized run-source digests; the normalized digests alone do not
establish raw Windows filesystem identity. Latest UIA request 241 is sent after
reply 240 still reports Dark. Command status confirms the Dark command executes;
owned recipient and released modifiers do not prove System renderer receipt.
Controller quota exhausts before submit4, so absent late records are censored.
The coordinator remains strongly owned in the document's thread-local PAGE;
JavaScript handle collection is not its lifetime mechanism. The next diagnostic
retains existing native observations from actual VK_D down onward only for the
observed form, with independent loss/quota/closure evidence. No input, assertion,
selection observation or time bound changes. Ordinary form and later native,
archive, release and MSI gates remain unreached.
Both locks now select provider d1658acc with exactly 15/11 source-hash
substitutions from 6343610d and all other bytes exact. Its private configured
anchor adds no native callback work. The consumer's actual logger regression
fails on retained span fields in the original INFO formatter, then passes with
selected target routing through the existing context-free layer. All four
logger tests and twelve startup/process tests pass. Windows late-key execution
and both complete forms remain required; this is evidence retention, not an
established correction for System.
The exact diagnostic consumer passes strict workspace and Windows-app Clippy,
778 active Rust tests in 9.242s, canonical preparation and all 504 local script
cases in 15.629s (498 pass, six Windows-only skips). All 1,003 tracked hashes
remain stable during the script run. A fresh external Git consumer resolves
the exact d1658acc package graph and compiles for MSVC without path/patch
overrides. The first sandboxed Rust attempt denies 28 real loopback listeners;
the unchanged replay with loopback access passes, without a leak report.
Two direct environment-helper entries remain ignored; the real two-hop tests
exercise their workflow. None of these portable checks proves Windows receipt.

The actual 561189 replay/run38812113624 binds merge4f7142a5/treeadee45b9,
1,003 raw Git blobs and837 normalized run-source digests. Its original ZIP
12010865923 hashes3d708059… and log hashesf75cd52c…. All504 Windows scripts
finish46.278s (503 pass, Apple-only skip), and all four exact Unicode submissions
produce their expected results/rejection. Native late records329–337 contain
S down335/up336, scan31, Alt+Shift high bits set, Ctrl clear and Handled=false.
Both use focus197106/foreground1114418; the down/up interval is0.5073ms on
the same native clock. The collector retains19 records/5,868 encoded bytes,
zero observed loss/error through337 and no closing record. Positive S receipt
is established; complete-history or S-absence claims are not.

The latest complete UIA response153 reports Dark; request154 is sent and its
full-tree traversal remains incomplete at collection. Response153's receipt
time is not retained, so its placement relative to S is unknown. Adjacent actor
call markers are0.5234ms apart on that actor's clock; they identify no blocked
theme getter. Four correct submits do not close either complete form journey.
The next selected observation records actual Rust key entry, metadata/admission,
command decision and original handler return. Return success establishes that
result only, without a post-mutation DOM or rendered-frame claim.

The next document-observation candidate passes808 Rust tests in9.580s, retaining
the two existing helper-entry ignores. All504 local script cases complete in
14.759s (498 pass, six Windows-only skips). Strict workspace/MSVC diagnostics,
workspace formatting, the actual release WASM build and canonical packaged
preparation pass. A real414-byte ClosingIntent with valid JSON whitespace
reproduces rejection under the old canonical-terminal hard limit, despite room
under the total byte cap. The same codec/receiver assertions pass after treating
413 bytes as reserved local-terminal capacity rather than a received-format
limit. Both aggregate caps remain exact. Native source-first routing and the
entire original functional dispatcher admit byte-exact inverse-diff proofs.
The first script attempt retains three sandbox PermissionErrors (socket/ps);
the unchanged unrestricted replay passes under the original60s. The first
MSVC lint failure is resolved by extracting only the diagnostic routing helper.
These checks do not establish native document delivery or complete acceptance.

Actual sourcef1523a4/run38821192294 binds merge92d8d78f/tree4c55ba20,
1,025 raw Git blobs and859 normalized source digests. ZIP12014106461 is
1,373,527 bytes/SHA25698f02c54715aee20ecfbce67d3ba446e3427ff8419f0bc1423a6acca68db0cfb;
all83 unique members pass CRC. The original90,276-byte job log hashes
445b2eb758885433b108e7888bbd460745458a7c7415cd5c3ef05ef03120383e.
The504-case Windows script suite completes in43.301s (503 pass, Apple-only skip); four exact
Unicode/rational submissions and the zero-concentration rejection pass.

The actual private document stream retains nine contiguous records/2,945 raw
encoded bytes: Selected; D/KeyD with Alt+Shift, Ctrl/Meta clear, trusted and
nonrepeat, Ready admission, Dark decision and original Ok return; then real
AltLeft and ShiftLeft entry/metadata/ignored/Ok. It records no quota, error or
terminal. The retained prefix contains no S record; missing closure leaves
events beyond that prefix unknown. Native S335/336 remains positive at scan31,
Handled=false, Alt+Shift held, Ctrl clear and focus66068. Its19-row suffix
retains5,860 encoded bytes/9,282 physical LF bytes with zero observed loss/error
through337 and no closing record. No complete-history claim follows.

UIA response193 reports Dark without a retained receipt/capture time; pending194
is a full-tree query at node24 whose identity is not retained. The Python input
receipt to request194 interval is21.4349863s on that clock only. No single
blocked getter, simultaneous stale UIA value or browser-interception cause is
established. The original60s form worker expires; the20-minute job limit does
not fire. Ordinary form, later native scenarios, actual archived test execution,
release and MSI acceptance remain unreached. Next: compare real renderer key
delivery with only per-key WebView2 browser handling varied; retain every input,
assertion, observation and existing limit before selecting a production change.

Provider2efcc148/run38829030763 binds merge319fd90/tree37e40b6b. Its24
nextest processes contain98 executions: all91 original executions and four new
portable checks pass; three new native controls fail. Keyboard ZIP12016187300
is213,592 bytes/SHA256f02ec2cc51f9d59061820650d75fdc3f326026e9644877a7753f1656c0e9475d;
the original bridge ZIP12016247474 hashes
3fc11ee1722de8e792cf2d85a1c5884576345ddf4d676b2e783c2c41459ba6bb.
All28 members verify. Actual owning runtime154.0.4258.62, mapped strokes and
payloads agree across Default, EnableS and SkipS. All three retain native S
down/up, D/P Rust notices and Closed with eight actual key callbacks, no
publication error and zero listener owners. S is unobserved in those bounded
document histories. SkipS records one successful true→false setter and unchanged
Handled=false, without observed delivery benefit.

Each native control fails host.close on a synthesized E_ABORT after a matching
PID/non-NORMAL BrowserProcessExitKind branch; the numeric PID/kind are not
retained. E_ABORT is not an OS/browser exit code. Native destruction is1, but
failed shutdown retains the owned profile. The later value/stroke assertions,
including SkipS's required S assertion, remain unreached; no comparison.json
runs. A logical pending=true collector bit does not prove an SDK call remains
in flight. Successor a0467cfe/run38831422438 binds mergecb3100bd/tree62683329
and all1,140 Git/physical source identities. Its101 nextest executions contain
95 pass and six fail: all91 original executions remain exact and pass, alongside
four portable additions. All six selected/observer-free native controls fail
host cleanup before final assertions. Each reports matching browser PIDs and
SDK exit kind1/non-normal, with no getter failures; each retains the same eight
D/P key records without S. The startup collector is unnecessary for either
reproduced failure. All40 original ZIP members pass CRC/source/payload audits.
Keyboard ZIP12017565895 hashes7dbe2936dfef77c16941be11646882d32efb66a2cf17361da2293e78e10b0ae4.
No comparator receives admission. Next: review a test-only browser-wide
true/false contrast before first navigation, as required by the Microsoft
sample. All six controls, cleanup gates and limits remain mandatory. No
production setting or Metis pin advances.

Provider14cb3fdf/run38835227528 binds mergee6b9c43/treeb3a0fbb2 and all1,142
source blobs/modes. Its29 nextest processes contain105 executions:97 pass/eight
fail, including all91 original executions and six portable passes. All eight
native controls exit100 without timeout/driver error. Seven fail host close
before assert_values. Globalfalse records before=true, setter success, after
and post-navigation=false, actual event defaults=false and no SDK getter error.
It observes matching PID/SDK kind0/NORMAL, host/profile cleanup and Destroyed1,
then fails required KeyS at keyboard_fixture.rs325. Its later Closed-count/owner,
stroke/startup/publication assertions remain unexecuted. No triplet/pair/cross
comparison is admitted. All eight raw documents retain D/P, eight actual Key
callbacks and Closed/zero owners, with S unobserved in those bounded histories.
Forensic identity covers24 chords/144 admitted strokes/96 native accelerators;
no complete keyboard/IPC drain claim follows. Abnormal shutdown is unnecessary
for this retained missing-S symptom. The setting supports no S correction.

Keyboard ZIP12018957213 is242,333 bytes/SHA256
43c281d16f9f167baa561967e2187aca7846a3edba981f99985f24039726e7cd;
bridge ZIP12018877499 hashes84b38de053402d6ce52d964974f01102dae87cc5f054c79ee0a00f9b779e03a2.
All48 members,20 producer-source bindings and ten actual payload files verify.
The original12,388,874-byte log hashes
98a911813655677cde58fa8508034cefd6ff86600203d98b5dda1380cd29b7a7.
Native observations establish controller receipt and a retained document gap;
they do not identify a precise browser/Win32 mechanism or a Metis full-form fix.
Read-only parent inspection finds no S-specific branch: system keys retain
DefWindowProc behavior and existing Alt+F4/system-close coverage. No evidence
justifies changing that processing. Next: review a complete correction against
existing physical input, closed protocol and lifecycle contracts before any
production change. Metis remains on d1658acc; both complete forms remain open.

[WebView2 issue5549](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5549)
is an open primary bug report. A May20 firsthand comment reproduces missing
capture-phase S and ineffective per-event disabling on148.0.3967.70. Its Visual
Search explanation is unconfirmed; no supported correction or maintainer
confirmation appears. It matches the observed symptom, without establishing
the precise mechanism in our154.0.4258.62 execution.

Budget baseline [run 38681175478](https://github.com/ryancinsight/metis/actions/runs/38681175478) stops at the unchanged 60-second script limit after 435/502 cases: 434 pass and one skips; its active visual case is not an attributed hang. Plain EDIT/UIA costs 16.696s versus 6.521s previously, while RichEdit passes in 16.830s. All 837 sources and 26 artifact members bind; preparation and native form do not run. The message-aware fixture wait now removes avoidable per-message sleep without extending the existing 25ms completion poll or dropping observations. Source-bound 53af replay completes all 502 cases in 50.180s, with plain EDIT/UIA 8.781s and RichEdit 2.361s; the earlier startup/getter cost split and application failure mechanism remain unproven. Closure commit 3d4103f exposes one board-schema regression: completed status is outside the canonical todo/blocked set. Restoring todo preserves the genuinely open startup/provider/query attribution scope and the checked native-wait subtask; neither the checker nor its baseline changes.
Portable 777 Rust cases, 42 doctests and strict Clippy pass with the patched locks; the pre-input-correction script replay passes 492 of 494 with two Windows-only skips in 18.447s. These local values do not establish real Windows key delivery.
Advisory API checker exit 101/root zero pass and 260 skip remains excluded from compatibility proof.

