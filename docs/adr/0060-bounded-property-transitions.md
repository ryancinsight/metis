# 0060 — Bounded property transitions

Status: Proposed

Date: 2026-10-22

Driver: `METIS-ANIMATION-001`.

Revision: 2026-10-22 — vocabulary, sampler and grammar admission are delivered
(`metis_ui_lang::transition`, [ADR 0013](0013-strict-style-contract.md) revision);
the overlay, `next_tick` clock and reduced-motion policy remain (decision 8).

## Context

The [Freya comparison](0003-framework-conformance/freya.md) opened this gap:
Freya ships `freya-animation` as a first-class crate, and `metis-ui-lang`
declares no transition, animation or easing property, so a state change paints
the new value immediately with nothing between the two frames. Four constraints
already in the tree shape any answer:

- [ADR 0051](0051-damage-limited-repaint.md) repaints only the region whose
  commands changed, comparing two display lists command by command, so a
  recolored command damages its own extent and an animated frame is already
  cheap to *paint*: a keystroke repaint measures 0.40 ms at 800×600 and 0.49 ms
  at 1600×1200, and a 300×40 region costs 0.025–0.050 ms to present.
- `RenderCache::frame` lays out once per `(document revision, viewport)` and
  returns a copy whose command storage is the only memory it requests;
  `METIS-MEM-RENDER-002` measures 350 allocations for a one-character edit.
- The native host pushes the accessibility tree on **every** event batch,
  before checking for a repaint (`application.rs:195-206`);
  `METIS-PERF-IDLE-001` measures that rebuild at ~430 allocations per batch.
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
   compositing, which ADR 0051 rejected on memory grounds.
3. **Sampling seam.** A transition is sampled when the display list is built,
   after `RenderCache::frame` and before `damage_since`. The document revision
   does not change, so layout, the semantic projection and the accessibility
   tree are untouched for the whole transition.
4. **Attribution.** Layout records the owning element for every command it
   pushes. Commands are not contiguous per element — a parent's box is reserved
   before its children and its border is pushed after them — so ownership
   cannot be inferred from order; the single push funnel writes it, and a test
   asserts one owner per command.
5. **Sampler.** `fn sample(elapsed: Duration) -> Color` is pure: each channel is
   interpolated in the stored 8-bit sRGB encoding CSS interpolates legacy colors
   in, computed in `f64` from the eased fraction and rounded once, half away from
   zero, with both endpoints pinned exactly. The named easings are CSS
   cubic-bezier curves solved by a fixed iteration count, so the same input
   yields the same output on every host. A second transition on the same element
   and property starts from the current sampled value, so an interruption does
   not jump.
6. **Clock.** The host owns time. `NativeApplication` gains
   `fn next_tick(&self) -> Option<Duration>` (default `None`); the host waits
   `min(wait, next_tick)`. A tick is the empty event batch the host already
   documents for timer work, so no second event loop is introduced, and an
   idle surface reports `None` and is never woken.
7. **Policy.** A host-supplied `MotionPreference` of `Reduced` applies the
   target immediately and schedules no tick; `Full` runs the transition. The
   DOM host keeps its own stylesheet transition, authored in the same
   vocabulary, and Rust writes no per-frame DOM state: the browser interpolates
   without Rust frames, and the policy there is the `prefers-reduced-motion`
   media query.
8. **Prerequisite.** `METIS-PERF-IDLE-001`'s conditional accessibility push
   lands first: the host pushes the tree on every batch today, so a 60 Hz tick
   would multiply a measured 430-allocation rebuild by the frame rate. Until it
   lands, `transition` is accepted and applied as an immediate change — the
   reduced-motion outcome — and no tick is scheduled, so the declaration is
   honoured in its outcome throughout and only the cadence is gated.

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

Each rejected row either moves the animation into the document and pays the
rebuild or duplicates the paint contract and drifts from it; sampling at the
paint boundary keeps ADR 0051's damage model the sole authority over repaint.

## Verification

- `samples_pin_both_endpoints`: for each easing, `t = 0` is the source color and
  `t = 1` the target, exactly, not within an epsilon (**delivered**).
- `samples_match_the_declared_curve`: for each named easing, ten sampled
  fractions match a hand-computed reference table within one channel step, and
  the DOM host's stylesheet fixture declares the same curve name. The browser's
  interpolation is not sampled, so the shared claim is the curve and the
  endpoints, not a pixel value.
- `a_transition_damages_only_its_element`: an animating label's damage equals
  the union of its old and new text extents; every other command compares equal.
- `a_transition_allocates_nothing_per_frame`: a counting allocator sees zero
  allocations across the ticks of one transition after the scratch list has
  grown once.
- `an_idle_surface_schedules_no_tick`: five seconds with no events calls
  `next_tick` and observes `None`, and the host performs no wakeup.
- `reduced_motion_applies_the_target_in_one_frame`: one render, no tick, and
  the target color is painted.
- `interruption_starts_from_the_sampled_value`: a second transition at t = 0.5
  of the first starts at the first's sampled color, not at the old target.
- A committed capture records a multi-step transition; the visual gate compares it.

## Migration

`transition` is a new admitted declaration: no existing document changes meaning
and no caller migrates, and a document that declares it is rejected today but
parses after this decision. `DisplayList` gains an owner vector, which breaks
the ten `DisplayList { commands }` literals in five files — `RenderCache::frame`,
three test modules and the doctests in `layout/damage.rs` — and they migrate to
a constructor enforcing one owner per command, so the type checks the invariant.
The doctests are published API examples, so this is a documented construction
change, not an internal refactor.

## Limits

No group or subtree opacity, no gradient-stop interpolation, no repeating or
looping animation, no layout-property animation (width, height, flex grow — each
reintroduces the per-frame relayout this decision avoids), and no animation of
the focus ring or the canvas. A theme change recolors every command and would
repaint in full for the whole duration, so the theme path sets colors directly
and never starts a transition. Only the native host samples; the DOM host's
curve is the browser's, so a cross-host pixel comparison of a transition is not
an oracle — only the endpoints and the declared curve are.

This decision establishes no memory, latency or frame-rate advantage over Freya,
and it does not close `METIS-PERF-IDLE-001`; it depends on it.
