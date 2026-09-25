# 0051 — Damage-limited repaint

Status: Accepted

Date: 2026-09-24

Driver: METIS-PERF-DAMAGE-002; presentation by METIS-PERF-PRESENT-003.

Revised 2026-09-24: the native host presents only the damaged region
(METIS-PERF-PRESENT-003), replacing the limit that it presented whole.

## Context

`FrontendApp::render` cleared the framebuffer and rasterized the whole
display list on every state change. The whole-frame benchmark
(`crates/metis-frontend/benches/frame.rs`) put a frame of the authored form at
about 2.8 ms at 800×600 and 8 ms at 200% on 1600×1200, nearly all of it in
rasterization: box shadows, glyph coverage and gradients. A keystroke changes
one label, yet paid for the whole form.

## Decision

Render repaints only the region the change can affect.

- `Framebuffer` carries a `Clip`, the whole surface by default.
  `render_clipped(region, draw)` narrows it for one draw and restores it
  through a drop guard, so a panic cannot leave later writes narrowed. Every
  write path (`row_span_mut`, `composite_span`, `blend_pixel`, `set_pixel`,
  `clear`) honors the clip. The fills, rounded shapes, shadows, strokes,
  glyph runs and images bound their visible rows and columns by it, so work
  shrinks with the region: a glyph outside the clip is skipped before its
  outline is rasterized. Lines keep clipping to the surface, because a
  Bresenham segment clipped to a smaller rectangle would start on different
  pixels; the write paths clip them.
- Each primitive reports a conservative extent: `BoxShadow::extent`,
  `polyline_extent`, `TextStyle::extent` (the face's `head` box at the first
  and last pen positions, plus one pixel for antialiasing), and the rectangle
  or image destination otherwise.
- `DisplayList::damage_since(painted, surface)` compares two lists of equal
  length command by command and returns the union of the old and new extents
  of the commands that differ. That union is `Unchanged` when only element
  metadata changed, and `Full` when the lengths differ or the union covers
  the surface. A pixel's value depends only on the commands covering it, in
  order, so every pixel outside that union keeps its value.
- `FrontendApp` keeps the display list its framebuffer shows. A resize, which
  replaces the surface, forgets it, and the next frame repaints in full.
- `Damage` lives in `metis-platform` beside `Rect`, so the display list and the
  native host share it. `FrontendApp` merges each render's damage until the
  host takes it. `NativeApplication::take_damage` defaults to `Full`, which is
  correct for an application that does not track repaints. The host takes the
  damage at each requested repaint: it presents nothing for `Unchanged`, the
  whole frame for `Full`, and otherwise the region clipped to the frame through
  Moirai's `present_argb8888_region`. That call copies only the region's rows
  and invalidates only its rectangle. It presents whole when the frame's
  dimensions or the window's client size changed, because frame coordinates
  are then not client coordinates.

## Alternatives

Rendering the damaged region into a scratch surface translated to the origin
was rejected: primitives evaluate coverage from absolute coordinates, and a
translation perturbs the floating-point sums, so the result would not be
bitwise equal to a full repaint. Clipping keeps every coordinate as it was.

Retained layers per element were rejected for now. They cost memory
proportional to the painted area, and the diff already limits a keystroke to
its label's region.

## Verification

- `clipped_scenes_match_unclipped_inside_and_leave_the_rest`: 96 random
  scenes of every primitive, clipped to random regions, equal the unclipped
  render inside the region and leave every other pixel untouched.
- `reported_extents_hold_every_painted_pixel`: 256 random shadows, strokes and
  glyph runs (descenders, accents, symbols) paint only inside their extents.
- `repainting_the_damage_matches_a_full_repaint`: random display-list edits
  repainted through their damage equal a fresh full repaint, bitwise.
- `every_edit_leaves_the_surface_as_a_full_repaint_would`: keystrokes,
  composition, focus moves, the command menu, theme changes, a resize and a
  scale change on the real form. After each edit, every pixel that differs
  from the previous frame lies inside the damage the host takes. The test
  fails when renders stop merging their damage.
- `generic_host_presents_exactly_the_reported_damage`: the host presents the
  first frame whole, a region as that region, nothing for `Unchanged`, and
  `Full` whole. `frame_region_clips_damage_to_the_frame` covers regions that
  cross every edge, have negative extents or lie outside the frame.

Each of the first three tests failed when its guard was weakened on purpose,
as a check that it can fail: a glyph visibility test off by six pixels, text
reporting no extent.

Pinned to one performance core, alternating before and after binaries: a
keystroke repaint went from 2.8 to 0.40 ms at 800×600 and from 8.2 to
0.49 ms at 1600×1200. A theme change, which recolors every command and
repaints in full, measured about 3% slower. That is within this host's drift
for identical code; classifying it costs 18 ns.

## Limits

A change that shifts layout, such as a label that grows and moves its
siblings, damages the union of old and new positions of every moved command.
A command-count change repaints in full.

Presenting whole cost 0.53 ms at 640×480 and 1.32 ms at 1280×960. A 300×40
region cost 0.025 and 0.050 ms (Moirai PR #457: a visible window, 200
alternating frames per case). Those figures bound the per-keystroke
presentation saving; an end-to-end keystroke-to-screen measurement was not
taken.
