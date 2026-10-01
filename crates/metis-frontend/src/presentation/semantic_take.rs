//! Semantic projection and painted-geometry queries over the owned form.
//!
//! The take/guard pair lets a host rebuild its accessibility tree only after
//! a batch that rendered, and hit-test the frame the user actually sees.

use super::layout_error;
use crate::FrontendApp;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::IpcTransport;
use metis_platform::Rect;
use metis_ui_lang::{DisplayList, LayoutViewport, SemanticTree, compute_layout};

impl<T: IpcTransport> FrontendApp<T> {
    /// Projects the current document into the host-neutral semantic tree.
    ///
    /// Native hosts can translate this value to an operating-system
    /// accessibility provider. Browser hosts continue to use their DOM
    /// accessibility tree; this method does not claim spoken or OS-level
    /// accessibility support.
    ///
    /// # Errors
    /// Returns a bounded UI error when the authored document has duplicate
    /// identities, unresolved references, malformed states or unsupported
    /// semantic values.
    pub fn semantic_tree(&self) -> Result<SemanticTree> {
        SemanticTree::from_document(&self.doc)
    }

    /// Takes the semantic tree if a render completed since the last take, so
    /// a host that takes after each event batch rebuilds the tree only after
    /// a batch that rendered.
    ///
    /// Every successful render marks the tree pending, whether or not it
    /// changed the document; the form is rendered on construction, so the
    /// first take reports the tree. A take that fails keeps the tree pending.
    ///
    /// # Errors
    /// Returns the semantic projection error of [`Self::semantic_tree`].
    pub fn take_semantic_tree(&mut self) -> Result<Option<SemanticTree>> {
        if !self.semantics_pending {
            return Ok(None);
        }
        let tree = self.semantic_tree()?;
        self.semantics_pending = false;
        Ok(Some(tree))
    }

    /// Returns the border rectangle of the authored element `id` as painted,
    /// so a host hit-tests the frame the user sees.
    ///
    /// Every successful render rebuilds the painted display list from the
    /// document, surface size and display scale, so the list is current after
    /// each one. A render that fails after editing the document leaves the
    /// list of the last successful render, which can lag the document. A
    /// resize that failed to render leaves no list, and the rectangle is then
    /// laid out afresh.
    ///
    /// # Errors
    /// Returns a layout error when the surface exceeds layout coordinates or
    /// layout fails, and a markup error when the form has no element `id`.
    pub fn element_rect(&self, id: &str) -> Result<Rect> {
        let laid_out;
        let display = if let Some(painted) = &self.painted {
            painted
        } else {
            laid_out = self.lay_out()?.0;
            &laid_out
        };
        display.element_rect(id).ok_or_else(|| {
            MetisError::ui(
                ErrorCode::MalformedMarkup,
                format!("Authored form is missing element {id}"),
            )
        })
    }

    /// Lays the document out over the framebuffer at the display scale,
    /// returning the display list and the surface rectangle.
    fn lay_out(&self) -> Result<(DisplayList, Rect)> {
        let width = i32::try_from(self.framebuffer.width()).map_err(|_| layout_error())?;
        let height = i32::try_from(self.framebuffer.height()).map_err(|_| layout_error())?;
        let display = compute_layout(
            &self.doc,
            LayoutViewport::with_scale(width, height, self.display_scale),
        )?;
        Ok((display, Rect::new(0, 0, width, height)))
    }
}
