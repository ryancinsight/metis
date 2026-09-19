# ADR 0039: Scoped native process provider

- Status: Accepted
- Date: 2026-09-19
- Item: [METIS-SERVICES-001](../../backlog.md#METIS-SERVICES-001)
- Upstream: Moirai transport `piped_stderr` provider increment

## Context

Metis needs a Tauri-like sidecar boundary without granting browser content a
shell, inherited credentials, arbitrary executable path, or unbounded pipe. The
existing Moirai supervisor owns process lifetime and finite cleanup, while
RITK and other applications own domain protocols. A provider in the platform
crate is the host policy boundary and must remain format-neutral.

## Decision

`ScopedProcessProvider` is constructed by trusted host code with one absolute
regular executable, a typed descendant-containment policy, and an argument
value allowlist. Each run requires a `RUN_PROCESS` capability witness. The
provider sends an empty environment and direct argument vectors to Moirai's
`ProcessSpec`; it never parses or executes shell strings. Argument count,
aggregate bytes, process lifetime and each output stream have fixed bounds.

Moirai's explicit `piped_stderr` option drains stderr into a bounded private
reader. Metis returns stdout and process status, but only a stderr byte count;
raw diagnostics never cross the host result boundary. Timeout cleanup calls the
supervisor's finite termination operation and reports failure when termination
cannot be confirmed. `Tree` containment is required where the host needs
descendant cleanup; `DirectChild` remains explicit for targets without an OS
job primitive and does not claim descendant containment.

## Alternatives

* **Shell command strings** — rejected because quoting, redirection and shell
  startup expand the authority surface and make argument injection a caller
  concern.
* **Browser-provided executable paths** — rejected because path validation
  cannot turn untrusted path selection into a stable host allowlist; the
  executable is fixed at provider construction.
* **Inherited environment and stderr** — rejected because credentials and
  diagnostics could cross the host boundary; the child starts with an empty
  environment and stderr is drained privately.
* **A Metis-local process supervisor** — rejected because it would duplicate
  lifecycle and OS containment logic already owned and tested by Moirai.

## Threat model and limits

The provider limits browser authority to the capability token, configured
executable, allowed argument values, empty environment, bounded output and
finite lifetime. It does not sandbox the executable's own OS privileges,
filesystem access or network access; deployments requiring those controls must
select an OS sandbox or sidecar service separately. `DirectChild` cannot contain
descendants on portable targets, and the provider exposes that policy rather
than implying stronger cleanup.

## Verification

Platform tests launch the real test executable, assert an allowlisted stdout
result, reject an unallowlisted argument and missing capability, capture and
discard a secret stderr marker, and terminate a child blocked on stdin at a
finite deadline. Moirai's process tests cover the native stderr pipe and
existing inherited behavior. Focused nextest, strict Clippy, format and lock
checks bind the result to the delivered revisions.
