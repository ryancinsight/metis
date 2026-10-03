//! What a render derives from the document, kept while the document stays
//! as it was.

use crate::document::TrackedDocument;
use crate::focus::focusable_ids;
use metis_core::Result;
use metis_ui_lang::SemanticTree;

/// Derived render state, each part valid for the document revision it was
/// derived at.
///
/// A change of revision drops nothing eagerly: the next request for the
/// part recomputes it, and a failed recomputation leaves the previous part
/// under its previous revision, so a stale part is never served.
#[derive(Default)]
pub(crate) struct RenderCache {
    /// Authored ids of the focusable controls in navigation order.
    focusable: Vec<String>,
    focusable_revision: Option<u64>,
    /// Buffers the form's projected text is written into, so a render that
    /// leaves the text as it was requests no memory for it.
    pub(super) text: [String; 2],
}

impl RenderCache {
    /// The focusable controls of `document`, projected only when the
    /// document changed since the last call.
    ///
    /// # Errors
    /// Rejects a document whose semantics cannot be projected.
    pub(super) fn focusable(&mut self, document: &TrackedDocument) -> Result<&[String]> {
        if self.focusable_revision != Some(document.revision()) {
            self.focusable = focusable_ids(&SemanticTree::from_document(document)?);
            self.focusable_revision = Some(document.revision());
        }
        Ok(&self.focusable)
    }
}
