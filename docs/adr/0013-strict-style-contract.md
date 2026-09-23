# 0013 — Strict presentation style contract

Status: Accepted

Date: 2026-09-07

Driver: [METIS-UI-001](../../backlog.md#METIS-UI-001).

Revision: 2026-09-09 — [METIS-LAYOUT-001](../../backlog.md#METIS-LAYOUT-001)
closes the silent custom-renderer style gap by rejecting declarations without
software-renderer semantics.

Revision: 2026-09-22 — [METIS-LAYOUT-ALIGN-001](../../backlog.md#METIS-LAYOUT-ALIGN-001)
admits `justify-content` and `align-items`
([ADR 0045](0045-flex-alignment-redistribution.md)), which empties this
decision's rejection category: every declaration the style model carries is now
painted. `ComputedStyle::validate_renderer_support` and `unsupported_style` are
deleted rather than kept as an empty call, because a check that can no longer
fail is not a guard.

What remains is the part that was always doing the work: `parse` still rejects
unknown properties, malformed declarations and values outside each admitted
keyword or length grammar. The decision therefore stands with its original
intent intact — a declaration is either painted or a typed error, never
silently dropped — and the renderer simply caught up with the subset.

Revision: 2026-09-22 — [METIS-LAYOUT-MINSIZE-001](../../backlog.md#METIS-LAYOUT-MINSIZE-001)
admits `min-width` and `min-height`. A minimum sizes one box, which layout
already does; it needs no space redistribution. `justify-content` and
`align-items` keep their rejection because they do, and that is a separate
layout capability rather than a longer length list.

Revision: 2026-09-22 — [METIS-TYPOGRAPHY-WEIGHT-001](../../backlog.md#METIS-TYPOGRAPHY-WEIGHT-001)
admits `font-weight` now that rasterization applies a stroke weight. The subset
stays bounded to the two weights the renderer can paint: `normal`/`400` and
`bold`/`700`. Any other weight is a typed error rather than a silent rounding
to the nearest paintable one, which would be the silent-drop the contract
exists to prevent.

Revision: 2026-09-22 — [METIS-RASTER-ROUND-002](../../backlog.md#METIS-RASTER-ROUND-002)
admits `border-radius` now that the software renderer paints it
([ADR 0043](0043-rounded-rectangle-paint.md)). This is the contract working
rather than a reversal: the rejection exists to stop a declaration being
silently dropped, so it lifts exactly when the renderer gains the semantics —
one property at a time, each with the paint evidence that earns it.

Revision: 2026-09-22 — [METIS-RASTER-SHADOW-001](https://github.com/ryancinsight/metis/pull/370)
admits `box-shadow` for one outer shadow of two offsets, an optional blur and a
color ([ADR 0046](0046-gaussian-box-shadows.md)). `inset`, a spread distance
and comma-separated lists stay typed errors: the renderer has no semantics for
them, so the admission is bounded the way `font-weight` is.

Revision: 2026-09-23 — [METIS-RASTER-GRADIENT-001](https://github.com/ryancinsight/metis/pull/376)
admits `background-image` and the gradient form of `background` for one
`linear-gradient()` of an angle or side keyword and two to eight hex stops with
optional percentage positions ([ADR 0048](0048-linear-gradient-backgrounds.md)).
Corner keywords, other angle units, color hints, length positions and the other
gradient functions stay typed errors.

## Context

`metis-ui-lang` parses a bounded CSS-inspired subset for the software
presentation path. Its parser previously ignored unknown properties, malformed
declarations and invalid values. That behavior makes a typo or a CSS feature
outside the admitted subset look accepted while silently changing the rendered
result. The existing `InvalidCssStyle` error code has no producer, so the
manual's typed unsupported-style diagnostic cannot be demonstrated.

The browser host remains the HTML5/CSS compatibility path. This decision covers
only the owned software presentation subset; it does not turn the subset into a
browser CSS engine or change browser DOM parsing.

## Decision

`ComputedStyle::parse` returns `metis_core::error::Result<ComputedStyle>`.

- An empty declaration list and empty declarations caused by a trailing
  semicolon are valid.
- Property names are case-insensitive after ASCII lower-casing, and duplicate
  admitted properties keep CSS-like last-declaration-wins behavior.
- The admitted properties and values are parsed strictly. Unknown properties,
  missing separators, empty values, invalid enum values, malformed pixel or
  percentage dimensions, negative spacing, malformed edge lists, invalid
  colors and unsupported font weights return `ErrorCode::InvalidCssStyle`.
- Every declaration in the admitted subset is painted since the 2026-09-22
  revisions; values outside an admitted keyword set or length grammar remain
  `ErrorCode::InvalidCssStyle`. `min-width` and `min-height` are admitted since
  the 2026-09-22 revision on the same length grammar and display scaling as
  `width`/`height`. `font-weight` is admitted
  since the 2026-09-22 revision for the two paintable weights. `border-radius` is
  admitted since the 2026-09-22 revision: it parses as a nonnegative pixel
  length on the same grammar as `gap` and `border-width`, scales by the host
  display scale and clamps to half the shorter side of the laid-out rectangle.
  `box-shadow` is admitted since the 2026-09-22 revision as `none` or
  `<x> <y> [<blur>] <color>`; layout scales it and reports a blur past the
  renderer's bound as a layout overflow.
- `parse_markup` propagates style errors at the element boundary. Layout keeps
  ownership of representability errors for programmatically constructed DOMs,
  including non-finite percentages and coordinate overflow.

The error is typed before layout so applications can distinguish an unsupported
style from a malformed document or a representability failure and present a
useful migration diagnostic.

## Alternatives

Keeping permissive parsing was rejected because it masks unsupported CSS and
cannot produce the promised diagnostic. Implementing a complete CSS parser and
layout engine was rejected because browser HTML5/CSS compatibility belongs to
the browser or system WebView path and would duplicate that infrastructure.
Adding a second parser or a compatibility wrapper was rejected because it
would create two style contracts and delay migration of callers.

## Migration

This is a breaking pre-release API change. Callers that invoke
`ComputedStyle::parse` must handle its `Result` with `?`, `match` or an
application-specific error boundary. Callers of `parse_markup` already receive
the typed error through its existing `Result` return. Replace declarations that
are not in the admitted subset with supported properties or move the UI to the
browser HTML5/CSS path. No permissive fallback or forwarding parser remains.

The user-facing steps and examples are in
[the style migration guide](../manual/style-migration.md).

## Verification

Style tests cover valid admitted declarations, unsupported rendering
declarations, unknown properties, malformed separators and invalid numeric,
edge and color values. Parser tests verify propagation as
`ERR_INVALID_CSS_STYLE`. Layout tests reject unsupported programmatic styles and
retain a non-finite percentage case classified as `ERR_LAYOUT_OVERFLOW`. The
presentation example uses only declarations with software-renderer semantics;
the manual records the typed rejection path. Focused nextest, strict Clippy,
documentation and the full visual gate run against the delivered revision.

## Limits

This contract covers only inline declarations consumed by the software
renderer. It does not implement selectors, inheritance, cascading stylesheets,
browser layout, text shaping, accessibility or native input. The browser and
desktop host gaps remain on their owning backlog items.
