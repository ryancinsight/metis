//! What a render derives from the document, kept while the document stays
//! as it was.

use crate::document::TrackedDocument;
use crate::focus::focusable_order;
use metis_core::Result;
use metis_ui_lang::{DisplayList, LayoutViewport, SemanticTree, compute_layout_into};

/// Room a frame keeps beyond the cached layout, for the focus ring.
const OVERLAY_COMMANDS: usize = 1;

/// Derived render state, each part valid for the document revision it was
/// derived at.
///
/// A key change drops nothing eagerly: the next request for the part
/// recomputes it, and a failed recomputation leaves the previous part under
/// its previous key, so a stale part is never served.
#[derive(Default)]
pub(crate) struct RenderCache {
    /// Authored ids of the focusable controls in navigation order.
    focusable: Vec<String>,
    focusable_revision: Option<u64>,
    /// The semantic projection of the document, rebuilt for the elements an
    /// edit touched rather than for the form it did not.
    semantics: Option<SemanticTree>,
    /// The laid-out document, without overlays.
    layout: DisplayList,
    layout_key: Option<(u64, LayoutViewport)>,
    /// Buffers the form's projected text is written into, so a render that
    /// leaves the text as it was requests no memory for it.
    pub(super) text: [String; 2],
}

impl RenderCache {
    /// The focusable controls of `document`, projected only when the
    /// document changed since the last call.
    ///
    /// The projection is kept, so the render that follows an edit rebuilds the
    /// nodes the edit can have changed and reuses the rest.
    ///
    /// # Errors
    /// Rejects a document whose semantics cannot be projected.
    pub(super) fn focusable(&mut self, document: &mut TrackedDocument) -> Result<&[String]> {
        if self.focusable_revision != Some(document.revision()) {
            self.project(document)?;
            let semantics = self
                .semantics
                .as_ref()
                .expect("invariant: a successful projection stores the tree it projected");
            let order = focusable_order(semantics);
            // An edit that leaves the focusable controls where they were keeps
            // the copy the focus reconciler reads, so it requests nothing.
            if order.len() != self.focusable.len()
                || order
                    .iter()
                    .zip(&self.focusable)
                    .any(|(current, held)| *current != held.as_str())
            {
                self.focusable.clear();
                self.focusable
                    .extend(order.iter().map(|id| (*id).to_owned()));
            }
            self.focusable_revision = Some(document.revision());
        }
        Ok(&self.focusable)
    }

    /// Brings the projection up to `document`, rebuilding only what moved.
    ///
    /// A projection that cannot be refreshed is dropped rather than kept
    /// beside a revision it does not describe, so the next render projects the
    /// document from scratch.
    fn project(&mut self, document: &mut TrackedDocument) -> Result<()> {
        let changed = document.take_changed();
        let Some(tree) = self.semantics.take() else {
            self.semantics = Some(SemanticTree::from_document(document)?);
            return Ok(());
        };
        self.semantics = Some(tree.refresh(document, &changed)?);
        Ok(())
    }

    /// A display list of `document` at `viewport`, laid out only when either
    /// changed since the last call.
    ///
    /// The list is a copy that owns room for the overlays a host paints over
    /// it, so the caller may extend it without disturbing the cached layout.
    /// The copy shares every id, text and payload with the cached commands, so
    /// its command and owner storage is the only memory it requests.
    ///
    /// A relayout writes over the cached layout's own storage, so an edit
    /// requests memory for what it changed, not for the form it did not.
    ///
    /// # Errors
    /// Returns the layout error of [`compute_layout_into`], or a bounded error
    /// when the command storage cannot be allocated.
    pub(super) fn frame(
        &mut self,
        document: &TrackedDocument,
        viewport: LayoutViewport,
    ) -> Result<DisplayList> {
        let key = Some((document.revision(), viewport));
        if self.layout_key != key {
            // A failed layout leaves no frame, so the key it would have been
            // filed under is dropped rather than kept beside a stale list.
            if let Err(error) = compute_layout_into(document, viewport, &mut self.layout) {
                self.layout_key = None;
                return Err(error);
            }
            self.layout_key = key;
        }
        self.layout.copy_with_room(OVERLAY_COMMANDS)
    }
}
