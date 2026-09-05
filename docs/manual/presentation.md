# Create a presentation

Metis accepts a bounded XML-like document and a CSS-inspired style subset. It is
not an HTML5 browser engine: browser DOM APIs, script execution, entity decoding,
Unicode text shaping and general CSS layout are not available.

Start with the runnable [presentation example](../../examples/presentation.rs).
It parses the form's [application markup](../../crates/metis-frontend/src/app.rs),
computes a display list and renders through Iris into a `Framebuffer`. The same
pixel buffer supplies both the BMP inspection artifact and the manual snapshot.

## Supported authoring

Use one root element, matching case-sensitive tags, quoted attributes and literal
text. Self-closing elements, comments and boolean attributes are supported. Use
the word “and” when needed: `&amp;` is preserved literally and the bitmap font
does not cover every punctuation character.

Layout supports sequential rows and columns, pixel/percentage dimensions,
automatic width/content height, padding, margins, gaps, colors and square borders.
Each row child with automatic width can consume the available width; assign
explicit widths or stack content in a column when that is the intended result.
Stored alignment, minimum-size, font-weight and radius declarations currently
have no rendering effect. The exact implementation limits are documented in
[metis-ui-lang](../../crates/metis-ui-lang/README.md).

Parsing admits at most one MiB of markup, 4,096 nodes, 64 nesting levels and 64
attributes per element. Framebuffers admit at most 16,777,216 pixels. These are
failure boundaries, not recommendations to construct maximum-sized documents.

## Update application state

`FrontendApp::set_inputs` updates the form values and labels. `render` computes
the display list and paints the current state. `submit_calculation` sends the
values through IPC and updates either the result or the error state.

`PlatformEvent` represents pointer, key, character, resize and quit events, but
`PlatformSurface` currently receives application-supplied events only. A rendered
button is not yet a native clickable control. Native input dispatch and widget
interaction require further implementation; a snapshot does not demonstrate them.

Keep calculation rules in the backend. The frontend dependency closure excludes
`metis-backend`; presentation code submits values and renders responses.
