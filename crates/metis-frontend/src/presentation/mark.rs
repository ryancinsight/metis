//! Application mark painted into the header's authored anchor box.
//!
//! The mark is the committed `examples/browser/assets/metis-mark.png` asset
//! that packaging, the browser workbench and this software path share. The
//! anchor box is authored markup, so the layout owns the mark's geometry
//! and it moves with alignment and display scaling like any other element;
//! this overlay only fills the box the layout reserved.
//!
//! Image admission is native (`metis-ui-lang`'s decoder is not in the
//! WebAssembly graph), so this overlay is compiled for the software host
//! only. The browser host renders the same mark from its own HTML/CSS, which
//! is a target composition split rather than a hidden no-op.

use crate::presentation::RenderCache;
use metis_core::{ErrorCode, MetisError, Result};
use metis_platform::Rect;
use metis_ui_lang::image::{ImagePlacement, ImageSampling};
use metis_ui_lang::{DisplayList, RasterImage};
use std::sync::{Arc, OnceLock};

/// Authored identifier of the header box the mark paints into.
pub const APP_MARK_ID: &str = "app-mark";

/// The committed Metis mark shared with packaging and the browser host.
pub(super) const APP_MARK_PNG: &[u8] =
    include_bytes!("../../../../examples/browser/assets/metis-mark.png");

/// Paints the application mark into the authored [`APP_MARK_ID`] box.
///
/// The mark decodes once per process and its placement is reused from the
/// render cache while the anchor box does not move, so a repaint requests
/// no image memory; the command it appends paints over the header it sits
/// in and nothing else.
///
/// # Errors
/// Rejects a document without the anchor box, a mark that does not decode,
/// or a placement outside its geometry contract.
pub(super) fn append_cached_app_mark(
    cache: &mut RenderCache,
    display: &mut DisplayList,
) -> Result<()> {
    let destination = display
        .element_rect(APP_MARK_ID)
        .ok_or_else(|| mark_error("Authored form is missing the application mark box"))?;
    display.append_image(cache.mark(destination)?)
}

/// Appends the application mark into `display`'s authored [`APP_MARK_ID`]
/// box.
///
/// This is the overlay the form paints every frame, published so a frame
/// derived independently of the form — a reference surface compared against
/// what the form presented — presents the committed mark from the same
/// authored anchor contract. The form's own render path goes through the
/// render cache instead, reusing the placement while the anchor box does
/// not move.
///
/// # Errors
/// Rejects a display list without the anchor box, a mark that does not
/// decode, or a placement outside its geometry contract.
pub fn append_app_mark(display: &mut DisplayList) -> Result<()> {
    let destination = display
        .element_rect(APP_MARK_ID)
        .ok_or_else(|| mark_error("Authored form is missing the application mark box"))?;
    display.append_image(placement(destination)?)
}

/// A placement of the whole committed mark into `destination`.
///
/// # Errors
/// Rejects a mark that does not decode or a destination with non-positive
/// dimensions.
pub(super) fn placement(destination: Rect) -> Result<Arc<ImagePlacement>> {
    let image = decoded_mark()?;
    let source = Rect::new(
        0,
        0,
        i32::try_from(image.width()).map_err(|_| mark_error("Application mark is oversized"))?,
        i32::try_from(image.height()).map_err(|_| mark_error("Application mark is oversized"))?,
    );
    Ok(Arc::new(ImagePlacement::new(
        image,
        source,
        destination,
        ImageSampling::Nearest,
    )?))
}

/// The committed mark, decoded on first use and shared thereafter.
fn decoded_mark() -> Result<Arc<RasterImage>> {
    static MARK: OnceLock<std::result::Result<Arc<RasterImage>, String>> = OnceLock::new();
    match MARK.get_or_init(|| {
        RasterImage::decode(APP_MARK_PNG)
            .map(Arc::new)
            .map_err(|error| error.to_string())
    }) {
        Ok(image) => Ok(Arc::clone(image)),
        Err(cause) => Err(mark_error(&format!(
            "Application mark failed to decode: {cause}"
        ))),
    }
}

fn mark_error(message: &str) -> MetisError {
    MetisError::ui(ErrorCode::RenderFailure, message)
}
