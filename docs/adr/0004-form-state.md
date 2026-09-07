# Form state ownership

Status: Accepted

Date: 2026-09-05

Driver: [METIS-STATE-001](../../backlog.md#METIS-STATE-001).

The form owns its inputs, transport, document and framebuffer. Only a successful
correlated backend response creates `FormState::Success`. Input edits and every
new request replace that state before rendering or dispatch. Immutable accessors
expose the current state and presentation without allowing a caller to retain a
displayed result while mutating the inputs it describes.

The current client is synchronous: its exclusive mutable borrow prevents edits
during a request. Pending is rendered before blocking IPC; it does not establish
responsive event handling or cancellation. The asynchronous phase must bind each
completion to the submitted input revision and discard obsolete completions.

Local failures before dispatch retain the session. Failures after dispatch close
it because the peer may have executed a request and there is no resynchronization
protocol. A failed handshake also closes the session. Reconnection constructs a
new app with a new authenticated transport; it never silently retries a request.
Peer rejections retain the complete wire code and message. They are not all
clinical rejections. No frontend calculation or MAC verification is introduced.

Presentation is derived from the state in one place. Pending, idle and failure
states carry no response or MAC. Error codes remain visible in the bounded status
line; full diagnostics remain in the typed state. Long patient identifiers use an
explicit ellipsis in the software preview; the submitted identifier remains exact.
Session status and MAC presence replace raw token identifiers and MAC fragments
in the product display: those bytes do not help a user interpret the result.
Input labels retain significant digits, padding minimum decimals without rounding
small accepted values to zero. Result labels normally show three decimal places
for volume rate and two for mass rate; when that would display a nonzero result
as zero, scientific notation preserves a nonzero mantissa with the same number
of fractional digits. The state retains the full backend numeric values.

Alternative rejected: retain publicly mutable fields and clear two optional
result/error fields in setters. Direct field mutation bypasses the invariant and
independent options admit contradictory state. The chosen change is a breaking
frontend API correction before publication; no registry release is authorized.

Migration: replace field writes with `set_inputs(...)?`; replace result/error
field reads with an exhaustive match on `state()` (including the non-exhaustive
fallback in external code). Use `document()` and `framebuffer()` for read-only
inspection. `new` now renders the initial state. All repository callers migrate
in this change; there is no forwarding compatibility API.
Import the markup constant from `metis_frontend::CLINICAL_SCREEN_XML`; its old
`app` module path is removed. Cargo-semver-checks 0.50.0 against `a2c7100` confirms
four major-change lint classes (field visibility/removal, private field additions
and removed module constant path); this is not a patch-compatible API release.

Threat boundary: backend responses and input strings remain untrusted. The client
validates framing/correlation and the backend validates domain inputs. A stale
result must never represent edited inputs, a transport failure must not display
success, and a displayed MAC must not imply local authentication. This state
contract provides no OS sandbox or browser-origin authorization.

Verification: real BackendService/Moirai/MemoryTransport traces exercise edit,
success, peer rejection, pre-dispatch failure, disconnect and recovery. Typed
state, values, audit records and rendered labels are asserted before real software
captures are compared exactly. Layout is computed before the framebuffer is
changed. If presentation itself fails, its returned error is explicit and the
operation outcome remains available in `state()`; a renderer failure is not
reported as successful display. Browser, OS event and asynchronous evidence remain
owned by their corresponding backlog items.
