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

## Native capture evidence — 2026-09-20

The `metis-app --metis-semantic-capture` role serializes the production
frontend's validated tree to a bounded schema-1 JSON artifact. The reviewed
34-element specimen is [the native semantic capture](../manual/images/native-semantic.json);
it is 16,333 bytes and has SHA-256
`0bb048549b94aabfda01c34d1c1239a35b797e67640b1b690fd2a21889bc4bd1`. The
oracle checks the application root, the focusable `label-patient` textbox with
its bounded value and `set_value` action, and the enabled, focusable `btn-calc`
button with its `activate` action. The command uses the production
`FrontendApp` with an in-memory transport and therefore does not claim backend
execution, an OS accessibility provider, spoken output or screen-reader
acceptance.

The remaining METIS-A11Y-001 work is supported screen-reader and host-preference
evidence. This ADR does not claim those capabilities.

## Revision 2026-09-22 — Command-surface role bridge

The admitted role vocabulary now includes navigation and complementary
landmarks, toolbars, menus and menu items. `metis-app` preserves these names in
semantic captures and maps them through the host-neutral native contract. The
Moirai PAL extension in PR [#432](https://github.com/ryancinsight/Moirai/pull/432)
(merge `0e2e1bbb2d81e16dd9c694ba46a9e9710e034417`) maps the same roles directly
to AccessKit, keeping operating-system types out of `metis-ui-lang` while
allowing native UI Automation consumers to retain command-surface semantics.
Menu items use the existing typed `Activate` action, and the hidden/disabled
guards remain enforced before any host projection.

## Revision 2026-09-21 — Windows native bridge

The native boundary now consumes the validated tree without adding operating
system dependencies to `metis-ui-lang`. `metis-platform::NativeSurface` accepts
an optional tree, creates the Moirai window hidden, installs the AccessKit
adapter, and shows the HWND only after installation. Subsequent trees replace
the provider state on the window thread, and `NativeSurface::reopen` preserves
the supplied tree. `metis-app::NativeForm` projects the authored frontend tree
with stable nonzero identities, bounded strings and collision checks; focus and
button activation actions return through `WindowEvent::AccessibilityAction`.
Unknown future semantic roles or actions fail projection with a typed platform
error instead of silently changing the role vocabulary.

Moirai PR [#410](https://github.com/ryancinsight/Moirai/pull/410) added the
format-neutral Windows provider at merge `bc6d100`; PR
[#411](https://github.com/ryancinsight/Moirai/pull/411) aligned WebView2 0.39.1
with the same Windows 0.62 API generation at merge `88f837e`. Metis native
tests exercise hidden installation, update, close and reopen, and the
application test dispatches focus to the authored submit control. These checks
establish provider and consumer contract behavior; an installed UI Automation
client, spoken screen-reader output and host preference enablement remain
required evidence under METIS-A11Y-001.

## Revision 2026-09-21 — Native editable action delivery

The authored `label-patient` control now carries a stable textbox identity,
`aria-label`, current bounded value and `SetValue` action in the format-neutral
tree. The Windows consumer routes provider `Focus` and `SetValue` requests for
that identity through the existing patient transition used by keyboard and IME
commits. The transition rejects values over the native 128-byte bound or values
containing control characters before mutating `FormInputs`; accepted values
invalidate the prior result and repaint the same surface. Native tests cover
focus, value replacement, semantic-value refresh and both rejection classes.
This strengthens the application action contract; it remains evidence of the
provider boundary, not proof of spoken output or installed screen-reader
acceptance.

## Revision 2026-09-22 — Native command state

The software-rendered native role now consumes the same typed
`metis_frontend::ApplicationCommand` identities used by the browser listener.
Pointer and AccessKit activation toggle the authored command menu, apply the
bounded dark/system palette and focus the patient reference. Escape closes an
open menu before the host exits, and hidden menu descendants expose no native
actions. The focused `metis-app` suite covers these transitions; the semantic
capture above records the navigation, toolbar, menu and menu-item nodes.
