# ADR 0028: Bounded vector stroke

Status: Accepted

Date: 2026-09-12

Driver: [METIS-GRAPHICS-001](../../backlog.md#METIS-GRAPHICS-001)

## Context

The format-neutral software renderer already composes rectangles, text and
validated raster images through one Iris display list. Viewer overlays and
custom application controls also need a line primitive. The operation must keep
the existing framebuffer ownership and clipping contract, remain usable by
RITK without importing DICOM semantics into Métis, and reject no valid image
because an endpoint lies outside the viewport.

## Decision

Add `metis_platform::draw_line` and `DisplayCommand::DrawLine`. The command
accepts two signed endpoint coordinates and one straight RGBA color. A bounded
Cohen–Sutherland clip in widened integer arithmetic runs before integer
Bresenham traversal. Every visited pixel uses the framebuffer's existing
source-over compositor, and no intermediate pixel storage is allocated. The
display-list `append_line` method preserves painter order and the Iris backend
continues to return a borrow of the framebuffer's storage.

This increment admits one-pixel segments. Stroke width, joins, caps, affine
transforms and device acceleration remain separate operations with their own
geometry and performance evidence. RITK may use this seam for format-neutral
overlay presentation; DICOM scanning, geometry and clinical meaning remain in
RITK.

## Alternatives

Drawing lines in each application would duplicate clipping and alpha behavior
and would make visual parity depend on the caller. A floating-point clip or a
generic path tessellator would add rounding and allocation policy before the
current one-pixel contract requires it. A GPU-only path would remove the
software oracle and leave the WASM and deterministic hosts without a renderer.

## Threat model and limits

Endpoints and colors are application inputs. Widened arithmetic, finite clip
iterations and the visible-surface traversal bound prevent overflow and
off-screen work amplification. The function has no file, network, process or
authority access. The current operation does not establish device-loss
recovery, font/vector parity in a browser, or a performance advantage over
egui, GPUI or Tauri.

## Verification

`metis-platform` tests cover diagonal pixels, translucent source-over blending
and `i32`-extreme endpoints. `metis-ui-lang` tests exercise the public display
command through the same framebuffer. The image example now composes two
visible line commands with a raster image and regenerates the inspected
`docs/manual/images/image-placement.svg` artifact. Focused strict Clippy,
format and nextest runs pass 45 tests (one platform-specific test skipped).
