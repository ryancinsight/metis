# 0058 — Opt-in series selection demonstration

Status: Accepted

Date: 2026-10-13

Driver: `METIS-SELECTION-001` [minor].

## Context

PR [#498](https://github.com/ryancinsight/metis/pull/498) preserves the original
commit and its static DICOM interface. Current main
`6662f14` excludes that shell. ADR 0035 assigns DICOM processing and viewer
semantics to RITK. Recovering selection must not turn the preserved labels or
the shared illustrative image into evidence of clinical loading or rendering.

## Decision

- Recover one opt-in `metis-web` example, packaged through the existing explicit
  consumer-gallery seam. The default workbench and public library API retain
  their current contracts.
- A Rust const-generic reducer owns the active viewport and per-viewport
  assignments. Selection is derived from the active assignment. Moirai owns
  the DOM/listener lifecycle; native buttons supply keyboard semantics.
- The page identifies its study/series as demonstration fixtures and presents
  assignment text. It neither loads DICOM nor displays decoded images. The
  four preserved files and the preservation branch remain intact.

The co-located [design and algebra](../../crates/metis-web/examples/series_selection/README.md)
specify transitions, rejection, variation by viewport count and preservation
comparison. There is one reducer for all counts, with no scalar, backend or
execution-policy variation in this discrete selection operation.

## Alternatives

Replacing the default workbench with the preservation shell violates ADR 0035
and introduces nonfunctional imaging controls. A second JavaScript state model
duplicates Rust-owned state. Extending RITK decoding or MPR exceeds this
selection increment and cannot establish recovered-shell behavior in isolation.

## Failure modes and verification

Malformed, unknown and out-of-range actions preserve populated state and report
distinct errors. Missing DOM elements produce presentation/mount errors.
Remount releases the prior listener and starts with unassigned viewports.
Native value tests cover counts 1, 2 and 4; browser acceptance covers real WASM,
mouse/descendant clicks, Enter/Space, selected attributes, assignment text,
focus preservation, rejection, stop/remount and a 360-pixel-wide layout.

## Remaining capabilities

This increment establishes selection/assignment behavior only. File loading,
DICOM decoding, validated geometry, image-plane rendering and reconstruction
remain RITK work. No release, preservation-PR merge or imaging capability claim
follows from passing selection tests.
