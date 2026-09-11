//! Moirai-backed browser canvas surface.

use super::CanvasFrame;
use moirai_pal::wasm::{CanvasSize, RgbaFrame, WebCanvas, WebDocument};
use std::io;

/// A browser canvas surface owned by the Metis host.
pub struct CanvasSurface {
    canvas: WebCanvas,
}

impl CanvasSurface {
    /// Resolves a canvas from the current browser document by identifier.
    ///
    /// # Errors
    /// Returns a typed I/O error when no browser document exists, the element
    /// is absent, is not a canvas, or cannot provide a two-dimensional context.
    pub fn from_current_document(id: &str) -> io::Result<Self> {
        let document = WebDocument::current()?;
        Self::from_document(&document, id)
    }

    /// Resolves a canvas from a document by its stable identifier.
    ///
    /// # Errors
    /// Returns a typed I/O error when the element is absent, is not a canvas,
    /// or cannot provide a two-dimensional rendering context.
    pub fn from_document(document: &WebDocument, id: &str) -> io::Result<Self> {
        Ok(Self {
            canvas: document.canvas_by_id(id)?,
        })
    }

    /// Returns the resolved canvas identifier.
    #[must_use]
    pub fn id(&self) -> String {
        self.canvas.id()
    }

    /// Presents one borrowed RGBA8 frame without retaining its bytes.
    ///
    /// Dimensions and exact storage length are validated by Moirai's shared
    /// browser frame contract before the Web API upload. The consumer remains
    /// the owner of the source bytes and any domain meaning attached to them.
    ///
    /// # Errors
    /// Returns [`io::ErrorKind::InvalidInput`] for zero, oversized, or
    /// inconsistent dimensions/storage, or when the browser rejects the
    /// upload.
    pub fn present<F>(&self, frame: &F) -> io::Result<()>
    where
        F: CanvasFrame + ?Sized,
    {
        let size = CanvasSize::new(frame.width(), frame.height())?;
        let frame = RgbaFrame::new(size, frame.rgba())?;
        self.canvas.present(frame)
    }
}
