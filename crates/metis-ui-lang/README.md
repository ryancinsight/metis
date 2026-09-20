# metis-ui-lang

A bounded XML-like markup parser and sequential row/column software layout.
This is a declarative presentation subset, not an HTML/CSS browser engine.

```rust
let document = metis_ui_lang::parse_markup("<label>42</label>")?;
let display = metis_ui_lang::compute_layout(
    &document,
    metis_ui_lang::LayoutViewport::new(320, 240),
)?;
assert_eq!(display.commands.len(), 1);
# Ok::<(), metis_core::error::MetisError>(())
```

Parsing supports one root, case-sensitive element names, quoted and boolean
attributes, literal text, comments, and self-closing elements. It rejects
trailing roots and malformed closing tags. Input is limited to one MiB,
4,096 nodes, 64 nesting levels, and 64 attributes per element. Entity decoding,
scripts, and browser error recovery are not implemented.

The same document can be projected into a bounded [`SemanticTree`] before a
host paints it. Roles are inferred from the admitted element vocabulary or an
explicit `role`, names resolve through `aria-label`/`aria-labelledby` and text,
and states/actions cover focus, disabled, hidden, value, selection and
activation. Duplicate IDs, unresolved references, unknown roles, malformed
state values and oversized semantic text fail with typed UI errors. The tree
is a host-neutral contract: native UIA, `NSAccessibility` or AT-SPI bridges
and browser accessibility remain host responsibilities, and tree presence
alone does not establish spoken screen-reader support.

```rust
use metis_ui_lang::{SemanticAction, SemanticRole, SemanticTree, parse_markup};

let document = parse_markup("<screen><button id='open'>Open study</button></screen>")?;
let tree = SemanticTree::from_document(&document)?;
assert_eq!(tree.root.children[0].role, SemanticRole::Button);
assert_eq!(tree.root.children[0].actions, vec![SemanticAction::Activate]);
# Ok::<(), metis_core::error::MetisError>(())
```

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

`LayoutViewport` carries the physical framebuffer dimensions and a validated
`metis_platform::DisplayScale`. Explicit pixel dimensions, spacing, automatic
extents and bitmap text are mapped with that scale; percentages resolve once
against the physical viewport. Native hosts can repaint after a DPI event and
reuse the resulting display list for hit testing without a second coordinate
conversion.

The display list admits one-pixel line segments through
`DisplayList::append_line` and width-aware paths through
`DisplayList::append_polyline`. Both use metis-platform clipping and
source-over rules, so format-neutral overlays can share one painter order as
fills, text and images. `StrokeWidth` rejects zero, and each path is bounded to
4,096 vertices; caps and joins are explicit rather than hidden style defaults.

```rust
use metis_platform::{Color, Framebuffer};
use metis_ui_lang::{DisplayList, LineCap, LineJoin, StrokeWidth};

let mut display = DisplayList::default();
display.append_polyline(
    &[(2, 2), (12, 2), (12, 10)],
    StrokeWidth::new(2)?,
    LineCap::Square,
    LineJoin::Round,
    Color::BLUE,
)?;
let mut framebuffer = Framebuffer::new(16, 12)?;
display.render_to(&mut framebuffer);
assert_eq!(framebuffer.get_pixel(2, 2), Color::BLUE);
# Ok::<(), metis_core::error::MetisError>(())
```

The software framebuffer implements Iris `RenderBackend<DisplayList>`. Rendering
returns a slice borrowed from the existing pixel storage, preserving the same
clipped rasterization path without allocating another frame.

Raster presentation uses [`RasterImage`](https://docs.rs/metis-ui-lang/latest/metis_ui_lang/struct.RasterImage.html)
and [`ImagePlacement`](https://docs.rs/metis-ui-lang/latest/metis_ui_lang/struct.ImagePlacement.html).
Images validate dimensions and row-major pixel storage at construction;
`RasterImage::from_rgba_bytes` converts a bounded RGBA byte boundary once.
Placements validate the source crop, clip the destination to the framebuffer
and composite with source-over alpha. A placement can apply an identity,
horizontal or vertical flip, a quarter-turn, or a validated arbitrary affine
mapping through [`ImageTransform`](https://docs.rs/metis-ui-lang/latest/metis_ui_lang/enum.ImageTransform.html).
`AffineTransform` expresses a finite, invertible source-to-destination mapping
in normalized crop coordinates; nearest-neighbor sampling reads the shared
source without allocating a transformed copy. `ImagePlacement::contain` swaps
the effective extents for quarter-turns and centers an aspect-preserving fit,
leaving letterboxing untouched. Its integer fit loses less than one pixel on
the non-limiting axis; affine mappings use explicit placement instead.

Native hosts can call `RasterImage::decode_png` or capability-checked
`RasterImage::load_png`. Admission bounds encoded input to 64 MiB, each edge to
16,384 and output to 16,777,216 pixels. PNG CRCs, complete zlib termination,
Adler checksum and exact scanline length are required. The static PNG subset
admits IHDR/PLTE/tRNS/IDAT/IEND, depths up to eight bits, and Adam7; indexed
images require complete palettes. EXIF, animation, color profiles and physical
spacing metadata are rejected rather than ignored. Samples retain their
straight RGBA values; this is not a color-management API. Clinical image
decoding and orientation metadata remain RITK-owned. The [native gallery](../../docs/manual/native.md#native-image-assets)
shows the Windows presentation and rejection state.

```rust
use metis_platform::{Color, Framebuffer, Rect};
use metis_ui_lang::{
    DisplayList, ImagePlacement, ImageSampling, ImageTransform, RasterImage,
};

let image = RasterImage::new(1, 1, vec![Color::RED])?;
let placement = ImagePlacement::new(
    image,
    Rect::new(0, 0, 1, 1),
    Rect::new(0, 0, 2, 2),
    ImageSampling::Nearest,
)?;
let mut display = DisplayList::default();
display.append_image(placement)?;
let rotated = ImagePlacement::new(
    RasterImage::new(1, 1, vec![Color::RED])?,
    Rect::new(0, 0, 1, 1),
    Rect::new(0, 0, 2, 2),
    ImageSampling::Nearest,
)?.with_transform(ImageTransform::RotateClockwise);
display.append_image(rotated)?;
let affine = ImagePlacement::new(
    RasterImage::new(1, 1, vec![Color::RED])?,
    Rect::new(0, 0, 1, 1),
    Rect::new(0, 0, 2, 2),
    ImageSampling::Nearest,
)?
.with_transform(ImageTransform::Affine(
    metis_ui_lang::AffineTransform::identity(),
));
display.append_image(affine)?;
let mut framebuffer = Framebuffer::new(2, 2)?;
display.render_to(&mut framebuffer);
assert_eq!(framebuffer.get_pixel(1, 1), Color::RED);
# Ok::<(), metis_core::error::MetisError>(())
```

```rust
use iris::render::RenderBackend;
let document = metis_ui_lang::parse_markup("<root style='height:8px;background:#123456'/>")?;
let display = metis_ui_lang::compute_layout(
    &document,
    metis_ui_lang::LayoutViewport::new(8, 8),
)?;
let mut framebuffer = metis_platform::Framebuffer::new(8, 8)?;
let pixels = framebuffer.render(&display).expect("clipped rendering is infallible");
assert_eq!(pixels[0], 0xff12_3456);
# Ok::<(), metis_core::error::MetisError>(())
```
