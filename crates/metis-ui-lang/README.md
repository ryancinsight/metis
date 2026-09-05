# metis-ui-lang

A bounded XML-like markup parser and sequential row/column software layout.
This is a declarative presentation subset, not an HTML/CSS browser engine.

```rust
let document = metis_ui_lang::parse_markup("<label>42</label>")?;
let display = metis_ui_lang::compute_layout(&document, 320, 240)?;
assert_eq!(display.commands.len(), 1);
# Ok::<(), metis_core::error::MetisError>(())
```

Parsing supports one root, case-sensitive element names, quoted and boolean
attributes, literal text, comments, and self-closing elements. It rejects
trailing roots and malformed closing tags. Input is limited to one MiB,
4,096 nodes, 64 nesting levels, and 64 attributes per element. Entity decoding,
scripts, and browser error recovery are not implemented.

Layout supports sequential row/column flow, explicit and percentage dimensions,
automatic width/content height, spacing, colors, square borders, and bitmap text.
Stored alignment, minimum-size, font-weight, and radius declarations currently
have no rendering effect. The font and glyph coverage belong to metis-platform.
Layout rejects coordinate overflow and invalid dimensions. Application-built
DOMs should observe the parser limits; direct DOM construction does not validate
them until layout, and recursive DOM utility operations assume bounded trees.

The software framebuffer implements Iris `RenderBackend<DisplayList>`. Rendering
returns a slice borrowed from the existing pixel storage, preserving the same
clipped rasterization path without allocating another frame.

```rust
use iris::render::RenderBackend;
let document = metis_ui_lang::parse_markup("<root style='height:8px;background:#123456'/>")?;
let display = metis_ui_lang::compute_layout(&document, 8, 8)?;
let mut framebuffer = metis_platform::Framebuffer::new(8, 8)?;
let pixels = framebuffer.render(&display).expect("clipped rendering is infallible");
assert_eq!(pixels[0], 0xff12_3456);
# Ok::<(), metis_core::error::MetisError>(())
```
