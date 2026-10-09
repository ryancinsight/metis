//! What a render derives from the document, kept while the document stays
//! as it was.

use crate::document::TrackedDocument;
use crate::focus::focusable_ids;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ui_lang::{DisplayList, LayoutViewport, SemanticTree, compute_layout};

/// Room a frame keeps beyond the cached layout, for the application mark
/// and the focus ring.
const OVERLAY_COMMANDS: usize = 2;

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
    /// The laid-out document, without overlays.
    layout: DisplayList,
    layout_key: Option<(u64, LayoutViewport)>,
    /// Buffers the form's projected text is written into, so a render that
    /// leaves the text as it was requests no memory for it.
    pub(super) text: [String; 2],
    /// The application mark placement for its anchor box, reused while the
    /// box does not move, so a repaint requests no placement memory.
    #[cfg(not(target_arch = "wasm32"))]
    mark: Option<(
        metis_platform::Rect,
        std::sync::Arc<metis_ui_lang::image::ImagePlacement>,
    )>,
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

    /// A display list of `document` at `viewport`, laid out only when either
    /// changed since the last call.
    ///
    /// The list is a copy that owns room for the overlays a host paints over
    /// it, so the caller may extend it without disturbing the cached layout.
    /// The copy shares every id, text and payload with the cached commands,
    /// so its command storage is the only memory it requests.
    ///
    /// # Errors
    /// Returns the layout error of [`compute_layout`], or a bounded error
    /// when the command storage cannot be allocated.
    pub(super) fn frame(
        &mut self,
        document: &TrackedDocument,
        viewport: LayoutViewport,
    ) -> Result<DisplayList> {
        let key = Some((document.revision(), viewport));
        if self.layout_key != key {
            self.layout = compute_layout(document, viewport)?;
            self.layout_key = key;
        }
        let mut commands = Vec::new();
        commands
            .try_reserve_exact(self.layout.commands.len() + OVERLAY_COMMANDS)
            .map_err(|_| {
                MetisError::ui(
                    ErrorCode::LayoutOverflow,
                    "Display command allocation failed",
                )
            })?;
        commands.extend_from_slice(&self.layout.commands);
        Ok(DisplayList { commands })
    }

    /// The application mark placement for `destination`, derived only when
    /// the anchor box moved or no placement was derived yet.
    ///
    /// # Errors
    /// Rejects a mark that does not decode or a placement outside its
    /// geometry contract.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn mark(
        &mut self,
        destination: metis_platform::Rect,
    ) -> Result<std::sync::Arc<metis_ui_lang::image::ImagePlacement>> {
        if self
            .mark
            .as_ref()
            .is_none_or(|(anchor, _)| *anchor != destination)
        {
            self.mark = Some((
                destination,
                crate::presentation::mark::placement(destination)?,
            ));
        }
        let (_, placement) = self.mark.as_ref().expect("placement derived above");
        Ok(std::sync::Arc::clone(placement))
    }
}
