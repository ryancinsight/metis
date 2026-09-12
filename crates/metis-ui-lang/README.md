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
Alignment, minimum-size, font-weight, and radius declarations are outside the
software renderer contract and return `ErrorCode::InvalidCssStyle`; direct DOM
construction receives the same diagnostic during layout. The font and glyph
coverage belong to metis-platform.
`ComputedStyle::parse` strictly rejects unknown properties, malformed
declarations and invalid values with `ErrorCode::InvalidCssStyle`; an empty
style and a trailing semicolon are valid. Layout rejects coordinate overflow
and invalid dimensions. Application-built DOMs should observe the parser
limits; direct DOM construction does not validate them until layout, and
recursive DOM utility operations assume bounded trees.

The display list also admits one-pixel line segments through
`DisplayList::append_line`. Lines use the metis-platform clipping and
source-over rules, so format-neutral overlays can share the same painter order
as fills, text and images without introducing a second renderer.

The software framebuffer implements Iris `RenderBackend<DisplayList>`. Rendering
returns a slice borrowed from the existing pixel storage, preserving the same
clipped rasterization path without allocating another frame.

Raster presentation uses [`RasterImage`](https://docs.rs/metis-ui-lang/latest/metis_ui_lang/struct.RasterImage.html)
and [`ImagePlacement`](https://docs.rs/metis-ui-lang/latest/metis_ui_lang/struct.ImagePlacement.html).
Images validate dimensions and row-major pixel storage at construction;
`RasterImage::from_rgba_bytes` converts a bounded RGBA byte boundary once.
Placements validate the source crop, clip the destination to the framebuffer
and composite with source-over alpha. Decoding formats and orientation metadata
remain an upstream asset-provider concern.

```rust
use metis_platform::{Color, Framebuffer, Rect};
use metis_ui_lang::{DisplayList, ImagePlacement, ImageSampling, RasterImage};

let image = RasterImage::new(1, 1, vec![Color::RED])?;
let placement = ImagePlacement::new(
    image,
    Rect::new(0, 0, 1, 1),
    Rect::new(0, 0, 2, 2),
    ImageSampling::Nearest,
)?;
let mut display = DisplayList::default();
display.append_image(placement)?;
let mut framebuffer = Framebuffer::new(2, 2)?;
display.render_to(&mut framebuffer);
assert_eq!(framebuffer.get_pixel(1, 1), Color::RED);
# Ok::<(), metis_core::error::MetisError>(())
```

```rust
use iris::render::RenderBackend;
let document = metis_ui_lang::parse_markup("<root style='height:8px;background:#123456'/>")?;
let display = metis_ui_lang::compute_layout(&document, 8, 8)?;
let mut framebuffer = metis_platform::Framebuffer::new(8, 8)?;
let pixels = framebuffer.render(&display).expect("clipped rendering is infallible");
assert_eq!(pixels[0], 0xff12_3456);
# Ok::<(), metis_core::error::MetisError>(())
```
