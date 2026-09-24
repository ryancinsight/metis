# metis-starter

The Metis starter application: the page create-tauri-app's vanilla template
produces, a heading, a logo row, a name field and a **Greet** button, with the
greeting computed in Rust.

In the Tauri template, `main.js` wires the form and calls the Rust `greet`
command over IPC. Here the Rust code is compiled to WebAssembly and owns the
page itself: it binds the form, reads the name, and writes the reply. The
page's only JavaScript is the generated module loader and one call:

```js
import init from "./metis_starter.js";

(await init()).metis_starter_start();
```

```rust
assert_eq!(
    metis_starter::greet("World"),
    "Hello, World! You've been greeted from Rust!"
);
```

## Run it

Install the `metis` command and the WebAssembly prerequisites once:

```bash
cargo install --path crates/metis-cli --locked
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
```

Then, in this directory:

```bash
metis serve
```

`metis serve` builds the page and serves it on `http://127.0.0.1:1420/`, the
port a Tauri development server uses, until interrupted; `--port` selects
another. `metis build` only builds. Both read `metis.json` here, whose
`frontend` names the page directory and this package. The build compiles the
package for `wasm32-unknown-unknown`, generates the loader with the
`wasm-bindgen` CLI whose version equals the locked `wasm-bindgen` crate
(`WASM_BINDGEN` overrides `PATH`), and stages the page in `dist/app` with a
file inventory in `dist/inventory.json`. See
[the application manual](../../docs/manual/applications.md) for the browser
check that greets a name and captures the page.

## Assets

The page's stylesheet is adapted from the create-tauri-app template
(MIT OR Apache-2.0); `frontend/styles.css` records the source and the changes.

- `frontend/assets/webassembly.svg`: the WebAssembly logo by Carlos Baraza,
  dedicated to the public domain under CC0 1.0, copied unmodified from
  `dist/icon/web-assembly-icon.svg` at
  [carlosbaraza/web-assembly-logo](https://github.com/carlosbaraza/web-assembly-logo)
  commit `f0f411529c1dafffa233be1bd95b80b79144b675`.
- `frontend/assets/metis-mark.svg`: a copy of
  `examples/browser/assets/metis-mark.svg`, kept here so the starter is a
  self-contained template, as each create-tauri-app template carries its own
  logos. `scripts/tests/test_starter.py` requires the two copies to be
  byte-identical.
