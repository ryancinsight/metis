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

```bash
python scripts/starter.py build
python scripts/starter.py serve
```

`build` compiles the crate for `wasm32-unknown-unknown`, generates the loader
with `wasm-bindgen`, and assembles `output/starter/`. `serve` serves that
directory on loopback and prints its address. `check`, given a W3C `WebDriver`
endpoint, opens the page, greets a name, verifies the reply and captures the
page; see [the application manual](../../docs/manual/applications.md).

The page's stylesheet is adapted from the create-tauri-app template
(MIT OR Apache-2.0); `frontend/styles.css` records the source and the changes.
