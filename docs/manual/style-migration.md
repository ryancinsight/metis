# Migrate presentation styles

The software presentation path accepts a bounded CSS-inspired declaration
subset. `ComputedStyle::parse` is strict so unsupported declarations fail with
the typed `ERR_INVALID_CSS_STYLE` diagnostic instead of silently changing the
rendered form.

Handle the parser result at the application boundary:

```rust
use metis_ui_lang::ComputedStyle;

fn style() -> Result<ComputedStyle, metis_core::error::MetisError> {
    ComputedStyle::parse("display: column; padding: 8px")
}
```

`display: column` is intentionally rejected because `column` belongs to
`flex-direction`; the corrected declaration is:

```rust
let style = ComputedStyle::parse("display: flex; flex-direction: column; padding: 8px")?;
```

Unknown properties, missing `:`, empty values, invalid enum values, malformed
pixel or percentage dimensions, negative spacing, malformed edge lists,
invalid hex colors all return `ErrorCode::InvalidCssStyle`. An empty style and a
trailing semicolon are valid. Every admitted property is painted, including
`justify-content`, `align-items`, `min-width`, `min-height`, `font-weight`
(`normal`/`400` and `bold`/`700`), `border-radius`, `box-shadow` and
`background-image`. The shadow subset is one outer shadow, `none` or
`<x> <y> [<blur>] <color>`; `inset`, a spread distance and comma-separated
lists are rejected with the same error. `background` and `background-image`
take one `linear-gradient()`: an optional `<n>deg` or `to top|right|bottom|left`
direction, then two to eight hex colors, each with an optional `<n>%`
position, for example `linear-gradient(135deg, #1a365d, #2c5282)`. The
gradient spans the border box and paints over `background-color`; the
`background` shorthand sets one of the two and clears the other. Corner
keywords, other angle units, color hints and length positions are rejected. Use the browser HTML5/CSS path when selectors, inheritance or full
CSS layout are required.

`parse_markup` already propagates the same error while constructing an element,
so markup callers continue to use their existing `Result` boundary:

```rust
let document = metis_ui_lang::parse_markup(
    "<panel style='unknown-property: value'/>"
);
assert_eq!(document.expect_err("unsupported style").code,
           metis_core::error::ErrorCode::InvalidCssStyle);
```

The supported layout path and its actual software captures are documented in
[Create a presentation](presentation.md) and the [application gallery](applications.md).
