# Create a presentation

Metis accepts a bounded XML-like document and a CSS-inspired style subset. It is
not an HTML5 browser engine: browser DOM APIs, script execution, entity decoding,
Unicode text shaping and general CSS layout are not available.

Start with the runnable [presentation example](../../examples/presentation.rs).
It parses the form's [application markup](../../crates/metis-frontend/src/presentation.rs),
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
Alignment, minimum-size, font-weight and radius declarations are outside this
software subset and fail with the typed `ERR_INVALID_CSS_STYLE` diagnostic.
Unknown properties, malformed declarations and invalid values use the same
diagnostic; programmatically constructed DOMs receive it during layout. The
software parser never silently changes a style. The exact implementation
limits are documented in [metis-ui-lang](../../crates/metis-ui-lang/README.md),
and the migration steps are in [Migrate presentation styles](style-migration.md).

Parsing admits at most one MiB of markup, 4,096 nodes, 64 nesting levels and 64
attributes per element. Framebuffers admit at most 16,777,216 pixels. These are
failure boundaries, not recommendations to construct maximum-sized documents.

## Update application state

`FrontendApp::set_inputs(...)?` updates the form values and labels, clears any
previous result and renders immediately. `submit_calculation` paints pending
before sending the values through IPC, then paints success or failure. The
synchronous client blocks while waiting; the Windows native application role
uses Moirai's finite event wait to keep its host loop responsive.

Match `state()` to distinguish a backend response, peer rejection, request
preparation failure and disconnection. Full typed diagnostics remain available
there; the software status line shows the error code. Failed requests and edits
display no old result or MAC. A successful response belongs to the exact current
`inputs()`; the document and framebuffer are available through read-only accessors.
Long identifiers show an explicit ellipsis in the software preview, while the
request retains the full identifier.

Correct rejected inputs and submit again to recover on the same session. A
transport or reply-decoding failure closes that session because the backend may
already have processed the request. Reconnect by constructing a new app with a
new authenticated transport. A repeated handshake is also rejected and closes
the session. The [migration contract](../adr/0004-form-state.md) lists the API changes.

`PlatformEvent` represents pointer, key, character, resize and quit events, and
`PlatformSurface` continues to receive application-supplied events only. On
Windows, [`metis_platform::native::NativeSurface`] presents the same framebuffer
through a real HWND and returns Moirai's complete `WindowEvent` values, including
focus, key-up and DPI events. The `metis-app --metis-native-window` role connects
those events to the authored form and private IPC workflow; non-Windows system
WebView targets and the OS permission boundary remain separate host work. See
the [captured Windows workflows](native.md#captured-windows-workflows)
for the current native and packaged-page evidence.

Keep calculation rules in the backend. The frontend dependency closure excludes
`metis-backend`; presentation code submits values and renders responses.
