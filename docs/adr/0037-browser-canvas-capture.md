# ADR 0037: Context-aware browser canvas capture

Status: Accepted

Date: 2026-09-16

Driver: `METIS-BROWSER-001`

Revised 2026-11-24: the runner decodes the baseline PNG and refuses a single
uniform colour, so a blank frame cannot equal itself across the rejection
probes; it still claims no RGBA equivalence. Evidence: the unit tests in
`scripts/tests/test_browser_drop.py` and ryancinsight/metis#591.

## Context

The generic browser runner's exact RGBA oracle reads a two-dimensional canvas
context. A consumer that presents through WebGPU still needs a real element
image and a proof that the requested context was selected. Calling the 2D
readback API against a WebGPU canvas returns no context and turns a valid
presentation into a misleading runner failure.

## Decision

`browser_drop.py` keeps `--canvas-capture rgba` as its default and adds an
explicit `--canvas-capture screenshot --canvas-context <name>` mode. Screenshot
mode validates the bounded intrinsic dimensions, checks the named context on
each consumer canvas, captures the element PNG through WebDriver, and records
its digest and dimensions. It supports one lifecycle and compares those
digests after the rejection probes. Before those probes the runner decodes the
saved baseline PNG from the served `output/browser` tree and refuses a single
uniform colour, so `--output` must lie under that tree; argument checking
rejects any other path before a file transfer. Beyond that check the runner
does not interpret the pixels: consumers own pixel interpretation and any
cross-provider comparison.

## Alternatives rejected

1. Calling `getContext("2d")` for every canvas fails on WebGPU and hides the
   selected provider.
2. Replacing the RGBA oracle globally would weaken existing 2D evidence.
3. Rendering a hidden 2D copy would duplicate consumer work and could pass
   while the visible provider is broken.

## Failure modes and limits

The runner fails when the named context is unavailable, intrinsic dimensions
are outside the capture bound, WebDriver cannot return a valid PNG, the
baseline PNG is a single uniform colour, the browser cannot decode the
baseline within its 10 s deadline, `--output` lies outside `output/browser`,
or a rejection probe changes the captured digest. An element PNG does not establish
RGBA readback equivalence, compositor timing, device performance or memory
use. Those claims require a consumer-owned artifact and an independent oracle.

## Verification

The dependency-free Python suite covers the closed capture-mode set and
context-name boundary. The full Metis gate runs the unchanged RGBA workflow
and the new parser/source checks. RITK uses screenshot mode with its explicit
`renderer=webgpu` query and supplies the DICOM attributes and visual review.
