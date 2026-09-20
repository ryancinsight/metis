# ADR 0041: Host-neutral accessibility semantics

Status: Accepted

Date: 2026-09-20

Driver: [METIS-A11Y-001](../../backlog.md#METIS-A11Y-001).

## Decision

`metis-ui-lang` derives a bounded `SemanticTree` from every declarative
document before the software renderer presents it. The tree carries the
admitted role vocabulary, explicit identity, accessible name and description,
value and boolean states, focusability, child order and typed host actions.
IDs are unique, ARIA references resolve within the document, role/state values
are validated, and parser depth/node limits plus semantic text limits remain
active for application-built DOMs.

`metis-frontend::FrontendApp::render` validates this projection before
painting, so the custom renderer cannot present a malformed identity or action
surface as if it were accessible. The projection is format-neutral and owns no
screen-reader or operating-system API. Each native host must translate it to
its platform provider; browser hosts continue to use the browser DOM tree.

## Alternatives

- Keep semantics only in each host. Rejected because native, software and
  future hosts would infer different roles and action sets from the same DOM.
- Add an AccessKit/UIA dependency to the UI-language crate. Rejected because
  the core renderer must remain host-neutral and WASM-compatible; native
  bridges belong at the host boundary.
- Treat a tree snapshot as screen-reader support. Rejected because tree
  presence does not establish spoken output, host preference enablement or
  platform action delivery.

## Invariants and verification

- Source IDs are unique and bounded; references fail closed when unresolved.
- Unknown roles, malformed state values, invalid tabindex values and oversized
  semantic text return typed UI errors.
- Hidden subtrees inherit effective visibility: a descendant cannot restore a
  focus or action surface with `aria-hidden="false"`; hidden or disabled nodes
  expose no focus or action surface.
- The tree preserves source child order and all exposed strings are bounded.
- `metis-ui-lang` tests cover role/name/reference/state/action derivation,
  rejection cases and hidden/disabled interaction; `metis-frontend` tests
  exercise the authored form through `FrontendApp::semantic_tree`.

The remaining METIS-A11Y-001 work is the native OS bridge and supported
screen-reader evidence. This ADR does not claim those capabilities.
