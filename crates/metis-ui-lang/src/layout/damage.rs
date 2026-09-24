//! The device region a display list changes relative to the one painted
//! before it.
//!
//! A pixel's value depends only on the commands covering it, in painter
//! order. When two lists have the same length, every pixel outside the paint
//! bounds of the commands that differ is covered by identical commands and
//! keeps its value, so repainting both lists' bounds of those commands, with
//! every command clipped to that region, reproduces a full repaint.

use super::display::{DisplayCommand, DisplayList};
use metis_platform::framebuffer::{Damage, Rect};
use metis_platform::rasterizer::polyline_extent;

impl DisplayCommand {
    /// A device rectangle holding every pixel this command can change, or
    /// `None` for a command that paints nothing.
    fn paint_bounds(&self) -> Option<Rect> {
        match self {
            Self::ElementRect { .. } => None,
            Self::DrawShadow { rect, shadow, .. } => Some(shadow.extent(*rect)),
            Self::FillRect { rect, .. }
            | Self::FillGradient { rect, .. }
            | Self::DrawBorder { rect, .. } => Some(*rect),
            Self::DrawLine { start, end, .. } => Some(span(
                i64::from(start.0.min(end.0)),
                i64::from(start.1.min(end.1)),
                i64::from(start.0.max(end.0)) + 1,
                i64::from(start.1.max(end.1)) + 1,
            )),
            Self::DrawPolyline {
                points,
                width,
                join,
                ..
            } => polyline_extent(points, *width, *join),
            Self::DrawText { text, x, y, style } => style.extent(*x, *y, text),
            Self::DrawImage { placement } => Some(placement.destination()),
        }
    }
}

impl DisplayList {
    /// The region repainting `self` over `surface` showing `painted` must
    /// cover.
    ///
    /// Once the changed commands cover the whole surface the answer is
    /// [`Damage::Full`] and the remaining commands are not examined, so a
    /// change that recolors everything costs no more to classify than the
    /// first command that covers the surface.
    ///
    /// # Examples
    ///
    /// ```
    /// use metis_platform::framebuffer::Damage;
    /// use metis_platform::rasterizer::CornerRadius;
    /// use metis_ui_lang::{Color, DisplayCommand, DisplayList, Rect};
    ///
    /// let fill = |x, color| DisplayCommand::FillRect {
    ///     rect: Rect::new(x, 0, 10, 10),
    ///     radius: CornerRadius::SQUARE,
    ///     color,
    /// };
    /// let surface = Rect::new(0, 0, 40, 10);
    /// let painted = DisplayList { commands: vec![fill(0, Color::BLACK), fill(20, Color::BLACK)] };
    /// let next = DisplayList { commands: vec![fill(0, Color::BLACK), fill(20, Color::WHITE)] };
    /// assert_eq!(next.damage_since(&painted, surface), Damage::Region(Rect::new(20, 0, 10, 10)));
    /// assert_eq!(painted.damage_since(&painted, surface), Damage::Unchanged);
    /// let recolored = DisplayList { commands: vec![fill(0, Color::WHITE), fill(20, Color::WHITE)] };
    /// assert_eq!(recolored.damage_since(&painted, Rect::new(0, 0, 10, 10)), Damage::Full);
    /// ```
    #[must_use]
    pub fn damage_since(&self, painted: &Self, surface: Rect) -> Damage {
        if self.commands.len() != painted.commands.len() {
            return Damage::Full;
        }
        let mut region: Option<Rect> = None;
        let changed = self
            .commands
            .iter()
            .zip(&painted.commands)
            .filter(|(next, before)| next != before)
            .flat_map(|(next, before)| [next.paint_bounds(), before.paint_bounds()])
            .flatten()
            .filter(|rect| rect.width > 0 && rect.height > 0);
        for rect in changed {
            let grown = region.map_or(rect, |region| region.union(rect));
            if grown.covers(surface) {
                return Damage::Full;
            }
            region = Some(grown);
        }
        region.map_or(Damage::Unchanged, Damage::Region)
    }
}

/// The half-open rectangle `[left, right) × [top, bottom)`, saturated to the
/// `i32` coordinate range.
fn span(left: i64, top: i64, right: i64, bottom: i64) -> Rect {
    let fit = |value: i64| {
        i32::try_from(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)))
            .expect("invariant: a value clamped to the i32 range fits i32")
    };
    let (left, top) = (fit(left), fit(top));
    Rect::new(
        left,
        top,
        fit(right - i64::from(left)),
        fit(bottom - i64::from(top)),
    )
}

#[cfg(test)]
#[path = "damage_tests.rs"]
mod tests;
