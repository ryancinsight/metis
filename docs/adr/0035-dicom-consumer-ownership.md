# 0035 — DICOM consumer ownership

Status: Accepted

Date: 2026-09-16

Drivers: [METIS-DICOM-005](../../backlog.md#METIS-DICOM-005),
[RITK-SNAP-DICOM-SUBSTRATE-001](../../ritk/backlog.md#RITK-SNAP-DICOM-SUBSTRATE-001).

## Context

Metis provides the browser host, bounded file handles and canvas transfer seam.
RITK provides the DICOM scanner, decoder, geometry, clinical display and saved-
study workflow. The historical browser gallery page lived under Metis because
the host packaging script copied it beside the generated Metis WASM. That placed
DICOM titles, controls and consumer canvas identifiers in a framework repository
and made the ownership boundary depend on comments rather than packaging inputs.

## Decision

The default Metis browser build contains only the generic workbench and host
assets. A consumer that needs a gallery supplies two explicit inputs to
`scripts/browser.py build`:

* `--consumer-package` is a bounded wasm-bindgen package containing one regular
  JavaScript module and one regular WebAssembly module.
* `--consumer-gallery` is a directory containing the consumer's `gallery.html`,
  `gallery.js` and `gallery.css`.

The packager validates the consumer page against Metis's canonical same-origin
content-security policy, rejects links and reparse points, and copies the page
and package into `output/browser`. Metis does not inspect or interpret the
consumer's data format. RITK stores its DICOM gallery under
`crates/ritk-snap/web/gallery` and supplies both inputs from its browser workflow.

The host contract remains format-neutral: it transfers bounded named bytes and
reports provider errors. DICOM classification, decoding, geometry, slice state,
clinical labels and image oracles remain in RITK. The consumer may configure a
format filter or label after `metis_start`; those values are not part of Metis's
host surface.

## Alternatives

Keeping the DICOM page in Metis was rejected because it gives a framework-owned
example authority over a consumer format and makes a completed ownership item
depend on a physical file location. Making Metis infer a consumer page or
format from the package was rejected because it hides the dependency and weakens
the explicit packaging boundary. Moving browser transport and provider handles
into RITK was rejected because it duplicates the first-party host seam.

## Failure modes

Missing or malformed consumer assets fail the build before output is published.
The canonical CSP check rejects a page that broadens script, style, object or
form authority. A consumer read or decode failure remains visible to that
consumer; Metis does not synthesize an image or downgrade the error.

## Verification

Metis tests assert that the default source tree has no consumer gallery and that
the explicit packaging path copies only validated assets. The standalone Metis
gate checks the generic workbench. RITK's locked browser workflow then builds
its WASM package, supplies its gallery, selects the saved MRI-DIR study and
checks the three decoded canvas oracles on each configured engine.

## Residuals

WebKit selected-file authorization, physical file-manager input and provider-
private browser resource measurements remain separate RITK/Moirai acceptance
items. This decision does not claim DICOM support in Metis or close those
external browser gaps.
