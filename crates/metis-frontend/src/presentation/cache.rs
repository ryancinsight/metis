//! Render state kept between renders.

/// Buffers and derived values a render reuses instead of requesting memory
/// again.
#[derive(Default)]
pub(crate) struct RenderCache {
    /// Buffers the form's projected text is written into, so a render that
    /// leaves the text as it was requests no memory for it.
    pub(super) text: [String; 2],
}
