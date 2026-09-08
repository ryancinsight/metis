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

The starter mark is local artwork under `examples/browser/assets/`. The browser
build copies the PNG and multi-resolution ICO; `metis.json` declares both
destinations so portable and MSI payloads retain the resources. The optional
manifest `icon` identifies the ICO used for native shell branding. The CLI
validates its bounded entry table, PNG chunks, CRCs, dimensions and ranges
before an MSI is written. The MSI `Icon` table and `Shortcut.Icon_` reference
the validated stream. No browser or installer asset is fetched from a remote
origin.

## Alternatives

Per-application hardcoded colors would duplicate the palette contract and make
theme selection impossible to test through the host. A runtime CSS parser in
Rust would duplicate the browser's CSS engine and add an unbounded surface to
the trusted state path. A remote stock icon or stylesheet would violate the
same-origin asset policy and make builds depend on network availability.
Windows-specific icon conversion would duplicate the source artwork and could
drift from browser branding. A bounded local ICO keeps one project asset while
the MSI schema supplies a separate native shell reference.

## Threat model and limits

Theme option values are parsed at the Rust boundary and unknown values leave
the previous state unchanged. CSS changes presentation only; they do not grant
file, network, process or backend authority. The browser resource is local and
bounded by the existing asset policy and package limits. The generated starter
mark is replaceable project artwork; its provenance and replacement path are
documented in the user manual.

This decision does not establish screen-reader speech, forced-colors runtime
behavior, high-DPI geometry, cross-engine CSS parity or native WebView2
integration. Those remain in the linked layout and host verification items.

## Verification

`metis-web` unit tests cover every mode, stable CSS value and invalid option.
The browser asset tests cover the semantic variables, all mode selectors,
favicon and focus-order markup, while the build script requires the copied PNG
and ICO. `metis-cli` tests exercise the generated ICO, malformed header and
dimension rejection, and MSI `Icon`/`Shortcut` rows. The manual includes the
mark and a reproducible mode-by-mode capture procedure.
Runtime captures must record the browser engine, viewport, scale factor and
host presentation settings before they can close the remaining V04/V06 gaps.

## Revision — 2026-09-08

`METIS-ASSETS-001` closed the native icon wiring gap. The local mark is emitted
as a seven-resolution PNG-in-ICO asset; the manifest and MSI packaging path
validate it before persistence, and the Start Menu shortcut points to the
embedded `MetisIcon` row. The acceptance evidence is the focused CLI suite,
the browser asset tests and the full gate at the delivery revision.
