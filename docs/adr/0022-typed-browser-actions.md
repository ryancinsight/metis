# ADR 0022: Authenticated typed browser actions

Status: Accepted

Date: 2026-09-09

Driver: [METIS-FRAGMENT-001](../../backlog.md#METIS-FRAGMENT-001).

## Context

The browser workbench already maps DOM events to Rust-owned state, but it did
not have a reusable request/target/update contract for actions whose result is
produced by the authorized backend. An htmx-style event → request → target →
swap model is useful for HTML5 applications, but accepting backend markup or
selectors would turn a presentation response into script, navigation and DOM
authority. A response also must not update a remounted document after an
in-flight request completes.

RITK remains the owner of DICOM scanning, decoding, geometry and viewer
semantics. This decision concerns only generic browser presentation messages;
it adds no format knowledge or DICOM dependency to Metis.

## Decision

Use the existing authenticated `PluginInvokeReq` transport rather than adding
another wire message type. The `ui` plugin declares one `action` command that
requires `CapabilityScope::UI_RENDER`. Browser handshakes carry the existing
calculation scope plus this presentation scope. The action body is a bounded
`FragmentAction` containing a non-zero mount generation, lower-case action and
target identifiers, and bounded UTF-8 input.

The plugin returns a bounded `FragmentPatchSet` with the same generation and
an ordered list of typed mutations. The patch-set boundary revalidates every
variant's identifier, attribute name, text bound and aggregate encoded size,
including values constructed inside the process, before encoding:

- `SetText` writes text content;
- `SetAttribute` writes an attribute after browser policy validation;
- `ReplaceChildren` is text-only and uses the same text-content setter.

There is no raw HTML, CSS declaration, URL, event-handler, script, unrestricted
selector or navigation operation in the contract. The browser keeps a closed
target allowlist for mounted Metis status and result elements and permits only
`aria-*`, `data-*`, `class` and `value` attributes. It validates every patch and
target before applying the first mutation, so an invalid response leaves the
previous DOM unchanged.

The browser action listener owns one bounded local task slot. It temporarily
owns the frontend application while the request is in flight, restores it only
for the current generation, and drops the task and application on unmount.
`metis_stop` advances the shared generation before dropping the application;
late responses are therefore discarded without DOM mutation. The existing
dialog opener is the first delegated action trigger, so the contract is
demonstrated without introducing an additional control or changing the static
application inventory.

## Alternatives

Adding `FragmentReq` and `FragmentResp` wire identifiers would duplicate the
existing plugin invocation envelope and expand the public message enum for one
extension. Accepting HTML from the backend would recreate the injection and
navigation authority that the CSP and host policy are intended to prevent.
Using an unrestricted CSS selector or `innerHTML` would make target ownership
runtime data rather than a checked browser contract. Keeping a second callback
or event runtime would duplicate Moirai's listener/task ownership and make
unmount cleanup non-local. Invoking the action in RITK would place presentation
transport in the DICOM/application owner and violate the stack boundary.

## Threat model and limits

The document and backend messages are untrusted until the existing host-bound
capability and session checks succeed. An attacker can submit malformed,
oversized, stale, unknown-target, event-handler or URL-bearing patch data. The
binary codecs bound lengths and counts; the backend checks the declared scope;
the browser rejects targets and attributes before mutation and uses text
content for all values. A generation mismatch, missing target or failed
attribute write is surfaced as a browser error and never partially applied.

The target set is intentionally tied to the current workbench markup. A future
application may add targets only by changing its own typed policy and tests;
there is no generic remote selector authority. The first plugin action reports
an input-sensitive status message; it is not a general application command
router. Cross-engine driver captures and provider-private listener/allocation
counts remain owned by `METIS-BROWSER-001` and Moirai.

## Verification

`metis-core` tests cover exact action and patch-set round trips, UTF-8 and
length bounds, unknown patch kinds, reserved bytes, trailing data, generation
and patch-count limits. `metis-backend` tests exercise the scoped `ui` plugin
through the real service handshake and verify an input-sensitive typed patch;
unknown actions are rejected. `metis-frontend` tests preserve missing-session
errors through the plugin invocation seam. `metis-web` policy tests cover the
closed target set, safe attributes and literal text values; the WASM path
preflights all patches before DOM mutation and generation checks run before
application.

The browser manual documents the action/target/swap workflow and the
authenticated demonstration. The locked WASM library build passes; a
configured WebDriver trace is still required for cross-engine visual evidence.
