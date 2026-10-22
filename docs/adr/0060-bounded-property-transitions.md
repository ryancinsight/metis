# 0060 — Bounded property transitions

Status: Accepted

Date: 2026-10-22

Driver: `METIS-ANIMATION-001`.

Revision: 2026-10-22 — decisions 1–8 are delivered: vocabulary, sampler and
grammar admission (`metis_ui_lang::transition`), the paint-boundary overlay with
its owner index, the `next_tick` clock seam and `MotionPreference`.

## Context

The [Freya comparison](0003-framework-conformance/freya.md) opened this gap:
Freya ships `freya-animation` as a first-class crate, and `metis-ui-lang`
declares no transition, animation or easing property, so a state change paints
the new value immediately with nothing between the two frames. Four constraints
already in the tree shape any answer:

- [ADR 0051](0051-damage-limited-repaint.md) repaints only the region whose
  commands changed, comparing two display lists command by command, so an
  animated frame is already cheap to *paint*: a keystroke repaint measures
  0.40 ms at 800×600 and 0.49 ms at 1600×1200, a 300×40 region 0.025–0.050 ms.
- `RenderCache::frame` lays out once per `(document revision, viewport)` and
  returns a copy whose command storage is the only memory it requests;
  `METIS-MEM-RENDER-002` measures 350 allocations for a one-character edit.
- The native host pushed the accessibility tree on **every** event batch,
  before checking for a repaint (`application.rs:195-206`);
  `METIS-PERF-IDLE-001` measured that rebuild at ~430 allocations per batch and
  now gates the push on `take_accessibility_damage`.
- [ADR 0013](0013-strict-style-contract.md) admits one property at a time, each
  with the paint evidence that earns it, and rejects unadmitted syntax with a
  typed error rather than dropping it silently.

## Decision

Animate at the paint boundary, not in the document.

1. **Vocabulary.** `ComputedStyle::parse` admits `transition:
   <property> <duration> <easing>`: one property, one duration in whole
   milliseconds (0–2000), one easing from `linear`, `ease`, `ease-in`,
   `ease-out`, `ease-in-out`. Comma lists, `cubic-bezier()`, other properties,
   other units and durations outside the bound are `ErrorCode::InvalidCssStyle`,
   as in ADR 0013. `transition` is metadata: it never changes layout.
2. **Admitted properties in this increment: `color` and `border-color`.** Each
   is painted only by commands owned by its element — `DrawText` runs and the
   single `DrawBorder` — so a sample's paint result is unambiguous.
   `background-color` waits for the gradient-under-fill decision, `box-shadow`
   for its memoized mask interaction (ADR 0052) and `opacity` for group
   compositing, which ADR 0051 rejected.
3. **Sampling seam.** A transition is sampled when the display list is built,
   after `RenderCache::frame` and before `damage_since`. The document revision
   does not change, so layout, the semantic projection and the accessibility
   tree are untouched for the whole transition.
4. **Attribution.** Layout records the owning element for every command an
   animated element pushes, in a side index rather than a field of the command,
   so a command stays a fixed-size value that clones without requesting memory.
   Commands are not contiguous per element — a parent's box is reserved before
   its children and its border is pushed after them — so ownership cannot be
   inferred from order; the single push funnel writes it. The index is scoped to
   elements that declare a transition and is shared between a layout and the
   frames copied from it, so an unchanged render requests only its command
   storage whether or not the document animates.
5. **Sampler.** `fn sample(elapsed: Duration) -> Color` is pure: each channel is
   interpolated in the stored 8-bit sRGB encoding CSS interpolates legacy colors
   in, computed in `f64` from the eased fraction and rounded once, half away from
   zero, with both endpoints pinned exactly. Named easings are CSS cubic-bezier
   curves solved by a fixed iteration count, so one input yields one output on
   every host; a second transition on the same element and property starts from
   the current sampled value, so an interruption does not jump.
6. **Clock.** The host owns time. `NativeApplication` gains
   `fn next_tick(&self) -> Option<Duration>` (default `None`) and waits
   `min(wait, next_tick)`; a tick is the empty event batch the host already
   documents for timer work, so no second event loop is introduced.
7. **Policy.** A host-supplied `MotionPreference` of `Reduced` applies the
   target immediately and schedules no tick; `Full` runs the transition. The DOM
   host keeps its own stylesheet transition in the same vocabulary and Rust
   writes no per-frame DOM state: the browser interpolates without Rust frames,
   and its policy is the `prefers-reduced-motion` media query.
8. **Prerequisite (met).** The conditional accessibility push of
   `METIS-PERF-IDLE-001` gates a tick, because a 60 Hz tick would otherwise
   multiply a measured 430-allocation tree rebuild by the frame rate. It landed
   before the overlay, so a tick now rebuilds no tree.

## Alternatives

Scored on the four criteria this decision is asked to satisfy; the chosen
design is the last row.

| Approach | Ease of usage | Memory | Performance | Maintainability |
| --- | --- | --- | --- | --- |
| Interpolate into the element style per frame | One declaration; nothing new to learn | Transient only, but 350 allocations per frame (MEM-RENDER-002) | Full layout, semantics and accessibility rebuild per frame; ~430 more allocations from the tree push | One derivation path, but it rides on `METIS-MEM-RENDER-002` landing and multiplies the idle defect by the frame rate. Rejected |
| Retained layers per animated element | Same declaration | Proportional to painted area, held for the transition's life | Compositing only, cheapest per frame | A second compositor contract and a cache-eviction policy. Rejected on memory, as ADR 0051 already rejected it |
| CSS transitions only, no native path | Familiar to web authors | None in Rust | Native hosts keep stepping | Two behaviours behind one declaration: the DOM host would animate and the software host would not. Rejected |
| Timeline and keyframe engine (Freya/Bevy class) | Powerful | Clip storage plus an evaluator | Fine | Large surface with no stated consumer; imports an asset model Metis does not have. Rejected |
| **Paint-boundary sampling (chosen)** | One declaration, one clock, no new concepts for the caller | 4 bytes of owner per command, one reused scratch list; zero per-frame allocation after warmup | Region paint only; layout, semantics and accessibility are untouched for the transition | One pure sampler, one overlay pass, one owner invariant with a test. The cost is a new field on `DisplayList` and a `next_tick` seam |

Each rejected row either pays the document rebuild or duplicates the paint
contract; paint-boundary sampling keeps ADR 0051 the sole authority on repaint.

## Verification

Each oracle is a committed test; the test name is the citation.

- `samples_pin_both_endpoints`, `samples_match_the_declared_curve`: for each
  easing `t = 0` is the source and `t = 1` the target, exactly, and ten sampled
  fractions match a hand-computed table within one channel step. The browser's
  interpolation is not sampled, so the shared claim is the curve and the
  endpoints, not a pixel value.
- `every_command_of_an_animated_element_is_attributed`,
  `an_element_owns_the_commands_it_paints_and_no_others`,
  `a_document_without_a_transition_carries_no_attribution`: the index is
  complete for an animated element, exclusive, and absent when nothing animates.
- `a_transition_damages_only_its_element`: a sample's damage equals the recolor
  extent and is smaller than the surface.
- `a_tick_reuses_the_frame_buffer_it_samples`,
  `a_tick_without_a_live_transition_requests_nothing`: a tick keeps the command
  storage it samples and requests no memory.
- `a_moving_application_shortens_the_host_wait_to_its_tick`,
  `an_idle_or_slower_application_keeps_the_whole_host_wait`: the host waits
  `min(wait, tick)` and never lengthens a wait to a tick.
- `reduced_motion_applies_the_target_in_one_frame`,
  `a_host_without_a_clock_paints_the_target`: policy and no-clock fallback both
  paint the declared target in one frame.
- `interruption_starts_from_the_sampled_value`: a second transition at t = 0.5 of
  the first starts at the first's sampled color.
- Not delivered: a committed capture of a multi-step transition and its gate.

## Migration

`transition` is a new admitted declaration and no caller migrates. The authored
clinical form now declares one on its status badge, so the software host
interpolates that badge on a session change while the theme path settles.
`DisplayList`'s `commands` became private behind `commands()` and
`from_commands`, migrating the construction literals in five files, two of them
published doctests.

## Limits

No group or subtree opacity, no gradient-stop interpolation, no repeating or
looping animation, no layout-property animation (width, height, flex grow — each
reintroduces the per-frame relayout this decision avoids), and no animation of
the focus ring or the canvas. A theme change recolors every command and would
repaint in full for the whole duration, so the theme path sets colors directly
and never starts a transition. Only the native host samples; the DOM host's
curve is the browser's, so a cross-host pixel comparison of a transition is not
an oracle — only the endpoints and the declared curve are.

This decision establishes no memory, latency or frame-rate advantage over Freya.
