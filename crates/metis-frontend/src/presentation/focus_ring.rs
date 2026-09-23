//! Keyboard focus ring painted over the laid-out form.

use super::theme::ThemePalette;
use crate::app::FrontendApp;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::IpcTransport;
use metis_platform::Rect;
use metis_platform::rasterizer::CornerRadius;
use metis_ui_lang::DisplayList;

/// Ring stroke width in authored pixels; display scaling never rounds a
/// positive extent to zero, so the ring stays visible at every scale.
const RING_WIDTH: i32 = 2;
/// Authored gap between the control's border box and the ring, so the ring
/// stays distinct from the control's own fill and border.
const RING_GAP: i32 = 2;

impl<T: IpcTransport> FrontendApp<T> {
    /// Appends the focus ring around the focused control when focus is
    /// visible and the control is laid out.
    ///
    /// The ring follows the control's corner rounding, grown by its offset,
    /// and paints after the document so nothing covers it.
    pub(super) fn append_focus_ring(&self, display: &mut DisplayList) -> Result<()> {
        if !self.focus_visible() {
            return Ok(());
        }
        let Some(control) = display.element_rect(self.focused_control()) else {
            return Ok(());
        };
        let width = self.display_scale.scale_extent(RING_WIDTH)?;
        let reach = width + self.display_scale.scale_extent(RING_GAP)?;
        let grow = |value: i32, by: i32| value.checked_add(by).ok_or_else(ring_overflow);
        let span = reach.checked_mul(2).ok_or_else(ring_overflow)?;
        let ring = Rect::new(
            grow(control.x, -reach)?,
            grow(control.y, -reach)?,
            grow(control.width, span)?,
            grow(control.height, span)?,
        );
        let authored_radius = self
            .doc
            .find_element_by_id(self.focused_control())
            .map_or(0, |element| element.computed_style.border_radius);
        let radius = grow(self.display_scale.scale_extent(authored_radius)?, reach)?;
        display.append_border(
            ring,
            width,
            CornerRadius::clamped(radius, ring),
            ThemePalette::for_theme(self.theme).focus,
        )
    }
}

fn ring_overflow() -> MetisError {
    MetisError::ui(
        ErrorCode::LayoutOverflow,
        "Focus ring exceeds the layout coordinate range",
    )
}
