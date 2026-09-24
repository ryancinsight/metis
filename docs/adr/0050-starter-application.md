# 0050 — Starter application

Status: Accepted

Date: 2026-09-23

Revised 2026-09-24: building and serving moved from `scripts/starter.py`
into `metis build` and `metis serve`, driven by the crate's `metis.json`.

Driver: [METIS-STARTER-001](https://github.com/ryancinsight/metis/pull/387).

## Context

Metis aims to be a near drop-in replacement for Tauri
([ADR 0002](0002-web-application-contract.md)). A Tauri project begins from
create-tauri-app, whose vanilla template is a centred page: a heading, a row of
logos, a name field and a **Greet** button. Its `main.js` sends the name to a
Rust `greet` command over Tauri's IPC and writes the returned string into the
page. The repository had no equivalent first application; the browser
workbench is a clinical demonstrator, not a starting point.

## Decision

`crates/metis-starter` reproduces that template's page and behavior with Rust
owning the page.

- The markup and stylesheet follow the template (create-tauri-app
  `890e6208661617b2e86a21f662b6efe9e3035788`, MIT OR Apache-2.0), with the
  source and changes recorded at the top of `frontend/styles.css`. `greet`
  keeps the template's wording, `Hello, {name}! You've been greeted from
  Rust!`.
- The crate compiles to WebAssembly and binds the page itself through Moirai's
  DOM handles. It cancels the form's navigation, reads the name, and writes
  the greeting. The page's only JavaScript is the generated wasm-bindgen
  loader and a two-line module that calls `metis_starter_start`. There is no
  IPC round trip, because the Rust code already runs in the page.
- Tauri's trademark policy does not license its logo, so the logo row shows the
  Metis mark and the WebAssembly logo (CC0 1.0), the analogues of the
  template's framework and language logos. Hover glows use each mark's color.
- The page carries a Content Security Policy admitting its own origin and
  WebAssembly compilation (`'wasm-unsafe-eval'`), with no inline script or
  style and `form-action 'none'`, so the form cannot navigate if the module
  fails to load. The template ships with no policy.
- The template's `outline: none` on inputs and buttons is replaced with a
  `:focus-visible` outline, keeping keyboard focus visible
  ([ADR 0049](0049-keyboard-focus-ring.md)).
- The crate carries a `metis.json` whose `frontend` names the page directory
  and the package, so `metis build` and `metis serve` build and serve it as
  `tauri build` and `tauri dev` do a Tauri project; the
  [distribution manual](../manual/distribution.md#build-and-serve-a-browser-application)
  owns the command contract. The page keeps its own copy of the Metis mark so
  the starter is a self-contained template, as each create-tauri-app template
  carries its own logos; a test holds it byte-identical to the canonical mark.
- `scripts/starter.py` checks the page: it runs `metis serve`, greets a name
  through W3C WebDriver and requires the reply to equal the Rust wording and
  the page to run exactly one script. With `--color-scheme` it pins light or
  dark rendering through Chromium's `preferredColorScheme` setting, so
  captures do not follow the host theme.

## Alternatives

Serving the template's `main.js` unchanged and answering `invoke("greet")`
from a Metis backend was rejected for this starter. It keeps the JavaScript
the starter exists to remove, and Tauri API compatibility is a separate
mapped item ([ADR 0003](0003-framework-conformance.md)).

Rendering the page through the software renderer was rejected: its markup
subset has no text-entry control, and the starter shows the HTML5 path a
migrating Tauri frontend keeps.

## Verification

The `greet` tests and doctests pin the wording, including an empty name and
untrimmed spaces. `scripts/tests/test_starter.py` checks that:

- the manifest builds this crate and a page with an `index.html`;
- the page's Metis mark equals the canonical mark;
- the page runs only the loader, which calls the export `browser.rs` defines;
- the policy admits WebAssembly and nothing inline;
- Rust binds exactly the ids the page declares;
- the checked reply is the Rust format string.

The gate's WebAssembly stage compiles the crate. A WebDriver run on Edge
155.0.4283.13 greeted `Metis` in both schemes and produced the manual's
captures, and repeated through `metis serve` after the commands replaced the
script's build and serve steps.

## Limits

The starter runs in a browser. Hosting the same page in the Windows WebView2
window needs that host to serve the module and admit `'wasm-unsafe-eval'`,
which it does not today.
