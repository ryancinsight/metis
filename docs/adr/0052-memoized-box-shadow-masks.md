# 0052 — Memoized box-shadow masks

Status: Accepted

Date: 2026-09-24

Driver: METIS-PERF-001 (box shadows as the dominant full-repaint cost); this change's pull request.

## Context

After damage-limited repaint (ADR 0051) and memoized glyph coverage, box
shadows were half of every full repaint. Pinned to one performance core,
a theme change on the demo form spent 860 µs of 1.69 ms in its six shadows
at 800×600, and 2.65 ms of 5.59 ms at 1600×1200 and 200%. Nearly all of that
is the blurred field: per output row, every arc row within reach, times every
edge column, plus the error-function profiles of the arc rows.

A theme change keeps every shadow's geometry. So does a damage-limited repaint
through a shadow. Each still recomputed the field from scratch.

## Decision

Each shadow's per-pixel alpha is rendered once into a mask and memoized.

- The mask holds one byte per pixel of the shadow's extent clamped to the
  surface: `coverage_alpha(color.a, value × uncovered)`, where `value` is the
  blurred field and `uncovered` is the share of the pixel outside the border
  box. That is exactly the alpha the direct path composited, so compositing the
  mask gives every pixel the same source as before, bit for bit. Pixels the
  border box covers hold zero, which leaves them untouched.
- The key is everything that determines those bytes: the border box's device
  position and size, the clamped radius, the offsets, the blur, the color's
  alpha, and the surface size. The color's channels are not in it, because
  they enter only at compositing.
- Masks live in a thread-local `GenerationalMemo` (`crate::memo`, the glyph
  cache's eviction policy), with two generations of 4 MiB. A shadow larger
  than one generation is rendered and composited but not retained. A shadow
  wholly outside the clip is neither rendered nor retained.
- Compositing blends a row of alpha bytes (`Framebuffer::composite_alpha_row`),
  in `u16` lanes when the row is opaque. Each mask row records the one run the
  border box fills, whose alphas are zero, and compositing skips it, as the
  direct path skipped solid columns; a card's interior is most of its
  shadow's extent.

## Alternatives

- **A position-independent key**, so moved or scrolled shadows would hit, was
  rejected. Arc extents are computed in absolute device coordinates
  (`cx − sqrt(…)`), so a translated shadow can round differently in the last
  ulp and flip a rounding half. The memo would then change output.
- **Caching the f64 field values** would be bit-exact without the alpha in the
  key, but costs eight bytes per pixel instead of one.
- **Caching f32 values** would not be bit-exact.
- **Making the field itself faster** is complementary, not a substitute. Only
  first paints, resizes, and changed geometry pay the field now.

## Consequences

- A repaint that keeps a shadow's geometry composites its mask instead of
  recomputing the field. A first paint does the same work as before plus one
  pass over the mask bytes.
- Memory is bounded at 8 MiB of masks per rendering thread. One generation
  of 4 MiB holds any shadow on a 1600×1200 surface, at most 1,920,000 bytes;
  on larger surfaces a shadow larger than a generation is rendered each time.
- Output is unchanged, so no golden image or frame hash moves.

## Verification

- `mask_tests.rs`: after every scene is cached, each scene, differing from the
  first in one keyed field at a time, renders exactly as it does on a fresh
  thread with an empty cache. A repeated shadow renders its mask once. A
  clipped repaint that hits the cache matches the full shadow inside the clip
  and leaves the rest untouched. A shadow outside the clip is never rendered.
- `an_alpha_row_matches_one_pixel_sources`: the alpha row matches a one-pixel
  source at every alpha, over opaque, translucent and transparent rows.
- The existing shadow oracles (the separable-versus-naive convolution, the
  five-percent corner bound, clipping by the border box) and the frame hash and
  repaint differential tests pass unchanged.
