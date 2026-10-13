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
