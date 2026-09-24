# 0049 — Keyboard focus and its ring in the software form

Status: Accepted

Date: 2026-09-23

Driver: [METIS-NATIVE-FOCUS-001](https://github.com/ryancinsight/metis/pull/382).

## Context

The software-rendered form paints no focus indication. The native host keeps
one `focused: bool` for window focus and routes typing to the patient
reference, so a keyboard user can neither see nor move which control would
act. WCAG 2.2 criterion 2.4.7 requires a visible focus indicator for
keyboard-operable controls. Criterion 1.4.11 requires such an indicator to
reach 3:1 against the colors next to it.

The semantic projection ([ADR 0041](0041-host-neutral-accessibility-semantics.md))
already marks each control focusable or not, hidden or not and disabled or
not, so the navigation order can come from it rather than from a second list.

## Decision

`FrontendApp` owns focus: the focused control's authored id and how focus
reached it, `FocusOrigin::Pointer` or `FocusOrigin::Keyboard`.

- The navigation order is the semantic tree's pre-order over focusable
  controls that are neither hidden nor disabled, the document order HTML
  sequential navigation uses. `move_focus` steps forward or backward through
  it and wraps at either end. `focus_control` focuses a named control and
  reports `false` for one that cannot take focus, such as an item of a closed
  menu.
- Every render reconciles focus against the frame about to be painted. Focus
  on a control that stopped being focusable returns to the `popover-anchor` of
  the nearest popover that enclosed it, as focus returns to a menu button when
  its menu closes. Otherwise it goes to the initial control, the patient
  reference.
- Only keyboard focus paints a ring, the rule CSS `:focus-visible` applies,
  since a pointer press already shows where the user acted. The form opens
  with pointer-origin focus on the patient reference, so existing captures and
  typing behavior are unchanged.
- The ring is a two-pixel inward border drawn two pixels outside the control's
  border box, with the control's corner radius grown by that reach. It is
  appended after the document's commands, so nothing paints over it. Both
  lengths are authored pixels scaled by the display scale, which never rounds
  a positive extent to zero. `DisplayList::append_border` is the entry point.
- Each theme names a `focus` color. The theme tests hold it to 3:1 against the
  page, the command bar and the cards: 4.4:1 to 5.4:1 in the light theme and
  5.9:1 to 8.3:1 in the dark.
- The Focus patient command moves focus to the patient reference and keeps its
  origin.

The native host maps keys to that model:

- Tab and Shift+Tab call `move_focus`. With Control, Alt or the Windows key
  held, the key is left to other handlers.
- Enter and Space activate a focused control other than the patient
  reference, following the button pattern. Keyboard activation of the
  menu button opens the menu with focus on its first item, as the menu-button
  pattern does. With the patient reference focused, Enter keeps its submit
  shortcut and Space stays text.
- Typing, Backspace and IME composition reach the patient reference only while
  it holds focus.
- A pointer press focuses the control it hits with pointer origin, then acts.
- An assistive-technology focus request focuses its target with keyboard
  origin, or leaves focus in place when the target cannot take focus.
- The native accessibility tree reports the focused control as its focus.

Moirai `ModifierState` gained public `SHIFT`, `CONTROL`, `ALT` and `META`
constants ([Moirai PR 443](https://github.com/ryancinsight/Moirai/pull/443))
so these rules are tested with real modifier states.

## Alternatives

A focus list authored beside the markup was rejected: it duplicates the
semantic projection and drifts when controls are added or hidden.

A ring on every focus change, including pointer presses, was rejected. It
matches CSS `:focus` rather than `:focus-visible` and marks clicked buttons
for no information gain.

Recoloring the focused control's border instead of adding an outer ring was
rejected. The controls' own borders and fills differ per control and theme, so
one border color cannot hold 3:1 against all of them, and the ring would
change the control's appearance rather than frame it.

## Verification

`crates/metis-app/src/frontend/native/keyboard_tests.rs` covers Tab and
Shift+Tab, modified Tab left alone, Space opening the menu on its first item,
Enter applying an item and returning focus to the button, typing refused while
a button holds focus, pointer focus without a ring, and assistive-technology
focus requests. Letting typed text follow window focus alone fails the typing
test. `crates/metis-frontend/src/focus_tests.rs` covers the initial unringed focus,
the order with the menu closed and open, wrapping in both directions, and the
ring's pixels. The ring pixel carries the theme's focus color and the gap
pixel keeps the card fill under keyboard focus; the same pixel keeps the card
fill under pointer focus. It also covers return to the menu button when the
menu closes and the Focus patient command. Removing the popover-anchor
fallback fails the menu test. `theme_tests.rs` holds the 3:1 bound. The
`form-focus` capture shows the ring on the submit control after one Tab. Every
existing capture is unchanged.

## Limits

Focus traversal covers the controls this form authors. There is no `tabindex`
ordering beyond document order, no focus trap for modal content and no
roving focus inside the menu; the menu's items are ordinary stops.
