//! Frame rendering and damage tracking for the form surface.

use crate::document::TrackedDocument;
use crate::{FormState, FrontendApp};
use iris::render::RenderBackend;
use metis_core::{ErrorCode, MetisError, Result};
use metis_platform::{Damage, Framebuffer, MotionPreference};
use metis_ui_lang::{
    Color, DisplayCommand, DisplayList, Edit, LayoutViewport, MAX_SEMANTIC_TEXT_BYTES,
};
use std::borrow::Cow;
use std::fmt::{self, Write};
use std::time::Duration;

/// Surface color behind the authored form.
const BACKDROP: Color = Color::rgb(240, 244, 248);

/// Paints `display` over the writable region, whatever the region held.
///
/// The backdrop clear is skipped when [`clear_is_redundant`] proves the
/// display list overwrites every pixel the clear would write.
pub(crate) fn paint(framebuffer: &mut Framebuffer, display: &DisplayList) {
    if !clear_is_redundant(framebuffer, display) {
        framebuffer.clear(BACKDROP);
    }
    framebuffer
        .render(display)
        .unwrap_or_else(|never| match never {});
}

/// Reports whether `display` holds an opaque, square-cornered rectangle fill
/// containing the whole writable region.
///
/// Such a fill replaces every writable pixel outright, so what those pixels
/// held before it, the backdrop clear included, cannot reach the result.
///
/// Commands are read in painter order. `DisplayCommand` is non-exhaustive
/// across crates, so a variant this function has not been taught ends the
/// scan with `false`: it may change how a later fill lands, and the clear then
/// runs. Every known variant is named, so a change to the list of them is a
/// deliberate edit here.
pub(crate) fn clear_is_redundant(framebuffer: &Framebuffer, display: &DisplayList) -> bool {
    for command in display.commands() {
        match command {
            DisplayCommand::FillRect {
                rect,
                radius,
                color,
            } if color.is_opaque() && radius.is_square() && framebuffer.clip_within(*rect) => {
                return true;
            }
            DisplayCommand::FillRect { .. }
            | DisplayCommand::ElementRect { .. }
            | DisplayCommand::DrawShadow { .. }
            | DisplayCommand::FillGradient { .. }
            | DisplayCommand::DrawBorder { .. }
            | DisplayCommand::DrawLine { .. }
            | DisplayCommand::DrawPolyline { .. }
            | DisplayCommand::DrawText { .. }
            | DisplayCommand::DrawImage { .. } => {}
            _ => return false,
        }
    }
    false
}