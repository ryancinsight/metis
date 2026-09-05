# metis-frontend

Declarative form state, software rendering and correlated IPC requests. The
dependency closure excludes backend calculation and audit storage. The current
binary exchanges requests over inherited pipes; native window and operating-system
sandbox integration remain unfinished.

```rust
use metis_frontend::CLINICAL_SCREEN_XML;
let document = metis_ui_lang::parse_markup(CLINICAL_SCREEN_XML)?;
let display = metis_ui_lang::compute_layout(&document, 800, 600)?;
// Submit the resulting display list to an Iris rendering backend.
# Ok::<(), metis_core::MetisError>(())
```

The renderer accepts the documented [presentation subset](../metis-ui-lang/README.md).
Backend responses carry a MAC which this frontend does not verify. See the
[process contract](../../docs/INTERFACE.md) and [verification](../../docs/VERIFICATION.md).
This package is unpublished.
