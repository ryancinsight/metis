# 0013 — Strict presentation style contract

Status: Accepted

Date: 2026-09-07

Driver: [METIS-UI-001](../../backlog.md#METIS-UI-001).

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
- Valid alignment, minimum-size, font-weight and radius declarations remain
  accepted even when the current software renderer stores them without using
  them. Their lack of rendering semantics stays explicit in the manual and
  crate README.
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

Style tests cover valid admitted declarations, stored-only declarations,
unknown properties, malformed separators and invalid enum, numeric, edge and
color values. Parser tests verify propagation as `ERR_INVALID_CSS_STYLE`.
Layout tests retain a programmatic non-finite percentage case and continue to
classify it as `ERR_LAYOUT_OVERFLOW`. The presentation example and V01/V04
software captures still render from the authored form; the manual records the
typed rejection path beside those captures. Focused nextest, strict Clippy,
documentation and the full visual gate run against the delivered revision.

## Limits

This contract covers only inline declarations consumed by the software
renderer. It does not implement selectors, inheritance, cascading stylesheets,
browser layout, text shaping, accessibility or native input. The browser and
desktop host gaps remain on their owning backlog items.
