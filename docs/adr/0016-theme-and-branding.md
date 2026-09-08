# ADR 0016: Theme and branding

Status: Accepted

Date: 2026-09-08

Driver: [METIS-LAYOUT-001](../../backlog.md#METIS-LAYOUT-001)

Related work: [METIS-ASSETS-001](../../backlog.md#METIS-ASSETS-001)

## Context

The browser workbench needs the presentation controls expected from a Tauri
application while keeping application state and authority in Rust. Its earlier
stylesheet used one fixed palette and the page had no project-owned mark. A
theme implementation must work with the existing HTML5/CSS route, remain
replaceable by an application, and avoid a remote asset or script dependency.
The software framebuffer has a separate style contract and is not changed by
this browser increment.

## Decision

`metis_web::Theme` is the bounded browser presentation mode with four values:
`System`, `Light`, `Dark` and `HighContrast`. The HTML option values are stable
lowercase strings. `System` follows `prefers-color-scheme`; the other values
select an explicit document mode. Each render writes the mode's CSS value to
`data-metis-theme` on both the document body and `#metis-app`.

The external browser stylesheet owns semantic `--metis-*` variables for page,
surface, text, accent, focus, status and backdrop colors. Attribute selectors
provide the light, dark and high-contrast palettes, while reduced-motion and
forced-colors rules retain their host accessibility contracts. Applications
may override these variables in a same-origin stylesheet without changing the
Rust state machine, IPC messages or authority checks.

The starter mark is a local PNG under `examples/browser/assets/`. The browser
build copies nested assets and requires that mark; `metis.json` declares the
same destination so portable and MSI payloads retain the resource. No browser
asset is fetched from a remote origin. Native installer shell icon conversion
is outside this increment and remains an asset-gallery follow-on.

## Alternatives

Per-application hardcoded colors would duplicate the palette contract and make
theme selection impossible to test through the host. A runtime CSS parser in
Rust would duplicate the browser's CSS engine and add an unbounded surface to
the trusted state path. A remote stock icon or stylesheet would violate the
same-origin asset policy and make builds depend on network availability. A
Windows `.ico` conversion now would conflate browser resource packaging with
MSI shell integration; the latter has a separate acceptance oracle.

## Threat model and limits

Theme option values are parsed at the Rust boundary and unknown values leave
the previous state unchanged. CSS changes presentation only; they do not grant
file, network, process or backend authority. The browser resource is local and
bounded by the existing asset policy and package limits. The generated starter
mark is replaceable project artwork; its provenance and replacement path are
documented in the user manual.

This decision does not establish screen-reader speech, forced-colors runtime
behavior, high-DPI geometry, cross-engine CSS parity, native WebView2
integration or MSI shortcut icon rendering. Those remain in the linked layout,
asset and host verification items.

## Verification

`metis-web` unit tests cover every mode, stable CSS value and invalid option.
The browser asset tests cover the semantic variables, all mode selectors,
favicon and focus-order markup, while the build script requires the copied PNG.
The manual includes the mark and a reproducible mode-by-mode capture procedure.
Runtime captures must record the browser engine, viewport, scale factor and
host presentation settings before they can close the remaining V04/V06 gaps.
