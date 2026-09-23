# 0044 — Anchored popovers

Status: Accepted

Date: 2026-09-22

Driver: `METIS-NATIVE-POPOVER-001`.

## Decision

A visible element with `popover-anchor="element-id"` is excluded from normal
flow, including intrinsic width, margins and gap counting, and painted after
the document. Its anchor is the laid-out element with that ID. Placement starts
below the anchor, fits horizontally within the viewport and moves above when
the lower edge would overflow and space exists. Framebuffer clipping remains
the final bound for oversized content.

Layout retains element rectangles by ID. Native controls use those rectangles
for hit testing, independent of their text or theme colors. An open menu
consumes outside clicks to dismiss without activating underlying controls;
Escape and window focus loss also dismiss it. Menu actions close it through
the existing frontend command transition.

## Alternatives and constraints

Ordinary flex children shift the form when shown. General CSS absolute
positioning would require offset and containing-block contracts beyond the
anchored-menu requirement. A dedicated anchor relation expresses the current
need without fixed offsets tied to font metrics. Missing anchors fail layout;
existing DOM limits bound traversal and display-list construction.

## Verification

Layout tests assert placement, paint ordering and unchanged sibling geometry.
Native event tests exercise selection across themes and outside dismissal
without sending a calculation. Render inspection checks the visual separation
between the menu and stationary form content.

## Revision history

- 2026-09-23: clarified that flow exclusion includes intrinsic sizing after an
  auto-width row regression exposed popover margins and gaps affecting siblings.
