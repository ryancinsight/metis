//! Painter slots a box reserves before its children paint.
//!
//! An element's shadow and backgrounds paint beneath its children, but its
//! height is known only after they are laid out, so the commands are pushed
//! with a placeholder rectangle and settled once the box is final.

use super::device::device_shadow;
use super::display::{DisplayCommand, DisplayList};
use crate::style::ComputedStyle;
use metis_core::error::Result;
use metis_platform::DisplayScale;
use metis_platform::framebuffer::Rect;
use metis_platform::rasterizer::CornerRadius;

/// Painter slots an element reserves before its children paint.
#[derive(Clone, Copy)]
pub(super) struct BoxSlots {
    shadow: Option<usize>,
    background: Option<usize>,
    gradient: Option<usize>,
}

impl DisplayList {
    /// Reserves painter slots for the element's shadow and background.
    ///
    /// The outer shadow sits immediately below the background (CSS
    /// Backgrounds 3 §6.1.3), so its slot is reserved first; the background
    /// image paints over the background color (§3.1). Every slot carries a
    /// placeholder until [`Self::settle_box`] writes the final rectangle.
    pub(super) fn reserve_box(
        &mut self,
        style: &ComputedStyle,
        placeholder: Rect,
        display_scale: DisplayScale,
    ) -> Result<BoxSlots> {
        let shadow = match style.box_shadow {
            Some(shadow) => {
                let index = self.commands.len();
                self.push(DisplayCommand::DrawShadow {
                    rect: placeholder,
                    radius: CornerRadius::SQUARE,
                    shadow: device_shadow(shadow, display_scale)?,
                })?;
                Some(index)
            }
            None => None,
        };
        let background = match style.background_color {
            Some(color) => {
                let index = self.commands.len();
                self.push(DisplayCommand::FillRect {
                    rect: placeholder,
                    radius: CornerRadius::SQUARE,
                    color,
                })?;
                Some(index)
            }
            None => None,
        };
        let gradient = match &style.background_gradient {
            Some(gradient) => {
                let index = self.commands.len();
                self.push(DisplayCommand::FillGradient {
                    rect: placeholder,
                    radius: CornerRadius::SQUARE,
                    gradient: gradient.clone(),
                })?;
                Some(index)
            }
            None => None,
        };
        Ok(BoxSlots {
            shadow,
            background,
            gradient,
        })
    }

    /// Writes the final border box into the reserved slots.
    pub(super) fn settle_box(&mut self, slots: BoxSlots, rect: Rect, radius: CornerRadius) {
        for index in [slots.shadow, slots.background, slots.gradient]
            .into_iter()
            .flatten()
        {
            let (DisplayCommand::DrawShadow {
                rect: target,
                radius: target_radius,
                ..
            }
            | DisplayCommand::FillRect {
                rect: target,
                radius: target_radius,
                ..
            }
            | DisplayCommand::FillGradient {
                rect: target,
                radius: target_radius,
                ..
            }) = &mut self.commands[index]
            else {
                unreachable!("invariant: reserve_box records only shadow and fill slots");
            };
            *target = rect;
            *target_radius = radius;
        }
    }
}
