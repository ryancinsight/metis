# ADR 0016: Theme and branding

Status: Accepted

Date: 2026-09-08

Driver: `METIS-LAYOUT-001`

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

The packaged Windows WebView2 form uses the same four values in a local
selector. Its page script applies the selected value to the document attribute
and its stylesheet resolves the palette through local CSS variables. The
selector is presentation-only: it never posts a bridge message and does not
change host or backend authority.

The external browser stylesheet owns semantic `--metis-*` variables for page,
surface, text, accent, focus, status and backdrop colors. Attribute selectors
provide the light, dark and high-contrast palettes, while reduced-motion and
forced-colors rules retain their host accessibility contracts. Applications
may override these variables in a same-origin stylesheet without changing the
Rust state machine, IPC messages or authority checks.

The starter mark is local artwork under `examples/browser/assets/`. The browser
build copies the SVG, PNG alternate and multi-resolution ICO; `metis.json`
declares all three destinations so portable and MSI payloads retain the
resources. The optional manifest `icon` identifies the ICO used for native
shell branding. The CLI validates the SVG's bounded root and path grammar and
the ICO's entry table, PNG chunks, CRCs, dimensions and ranges before an MSI is
written. The MSI `Icon` table and `Shortcut.Icon_` reference the validated
stream. No browser or installer asset is fetched from a remote origin.

## Alternatives

Per-application hardcoded colors would duplicate the palette contract and make
theme selection impossible to test through the host. A runtime CSS parser in
Rust would duplicate the browser's CSS engine and add an unbounded surface to
the trusted state path. A remote stock icon or stylesheet would violate the
same-origin asset policy and make builds depend on network availability.
Windows-specific icon conversion would duplicate the source artwork and could
drift from browser branding. A general SVG parser would enlarge the trusted
packaging surface with XML, style and resource semantics that the application
does not need. A bounded local SVG subset plus PNG alternate and ICO keeps one
project mark while the browser and MSI schemas select their required formats.

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
favicon and focus-order markup, while the build script requires the copied SVG,
PNG and ICO. `metis-cli` tests exercise the bounded SVG grammar, generated ICO,
malformed header and dimension rejection, and MSI `Icon`/`Shortcut` rows. The
manual includes the mark and reproducible browser and WebView2 mode-selection
procedures. The WebView2 asset contract tests every option and palette selector;
the packaged capture role writes and visually inspects one `CapturePreview` PNG
for each mode. The committed captures are 1025×769: system
(`15c88ffd69531b815e71e28951b2b2bb09e274f2dd6c4c7bc6155b684499e6a6`), light
(`ec3caec1fd604ffc1272cfdffc958661e40ed90057636c487c601fc601ab8b7e`), dark
(`75730d4d78b9b8cf499a15afb8d228c1b2ef71ecac9a0e081789ee582d418694`) and
high-contrast
(`d1523c8f8bb5daff330ac90131e7b316ce6ce4c89d94ef043a923c4db827b160`).
The capture role proves visible palette selection in the packaged provider;
screen-reader behavior, installed IME behavior, physical high-DPI transitions,
and other host presentation settings remain separate Windows evidence items.

## Revision — 2026-09-08

`METIS-ASSETS-001` closed the native icon wiring gap and added the browser
vector path. The local mark is emitted as a scriptless fixed-viewport SVG, a PNG
alternate and a seven-resolution PNG-in-ICO asset; the manifest and MSI
packaging path validate each format before persistence, and the Start Menu
shortcut points to the embedded `MetisIcon` row. The acceptance evidence is the
focused CLI suite, the browser asset tests and the full gate at the delivery
revision.

## Revision — 2026-09-20

The packaged WebView2 form now exposes the same four local theme modes as the
browser workbench. The selector changes only CSS variables and the document
theme attribute; no page-to-host message is introduced. Static asset tests and
the manual procedure cover the contract. The bounded
`--metis-webview-theme-capture` role now produces and commits inspected PNGs for
all four modes through the packaged provider; their hashes and dimensions are
recorded in the manual. This closes the visible palette-selection evidence
while leaving accessibility, IME and physical display-scale evidence open.
