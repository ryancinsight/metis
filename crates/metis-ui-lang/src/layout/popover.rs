//! Anchored popover measurement and placement.

use super::device::{add, sub};
use super::display::DisplayList;
use super::geometry::LayoutViewport;
use super::intrinsic::Sizing;
use crate::dom::{DomElement, DomNode};
use crate::style::Display;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_platform::framebuffer::Rect;

impl DisplayList {
    pub(super) fn popovers(
        &mut self,
        element: &DomElement,
        viewport: LayoutViewport,
    ) -> Result<()> {
        if element.computed_style.display == Display::None {
            return Ok(());
        }
        if let Some(anchor_id) = element.attributes.get("popover-anchor") {
            let anchor = self.element_rect(anchor_id).ok_or_else(|| {
                popover_error("A visible popover references an anchor absent from layout")
            })?;
            let measured = {
                let mut measurement = Self::default();
                measurement.element(
                    element,
                    Rect::new(0, 0, viewport.width(), viewport.height()),
                    viewport.scale(),
                    Sizing::Content,
                )?
            };
            let x = popover_x(anchor.x, measured.width, viewport.width())?;
            let y = popover_y(anchor, measured.height, viewport.height())?;
            self.element(
                element,
                Rect::new(
                    sub(x, measured.x)?,
                    sub(y, measured.y)?,
                    viewport.width(),
                    viewport.height(),
                ),
                viewport.scale(),
                Sizing::Content,
            )?;
        }
        for child in &element.children {
            if let DomNode::Element(child) = child {
                self.popovers(child, viewport)?;
            }
        }
        Ok(())
    }
}

pub(super) fn popover_error(message: &'static str) -> MetisError {
    MetisError::ui(ErrorCode::MalformedMarkup, message)
}

fn popover_x(anchor_x: i32, popover_width: i32, viewport_width: i32) -> Result<i32> {
    if popover_width >= viewport_width {
        return Ok(0);
    }
    Ok(anchor_x.clamp(0, sub(viewport_width, popover_width)?))
}

fn popover_y(anchor: Rect, popover_height: i32, viewport_height: i32) -> Result<i32> {
    let below = add(anchor.y, anchor.height)?;
    let bottom = add(below, popover_height)?;
    if bottom > viewport_height && popover_height <= anchor.y {
        sub(anchor.y, popover_height)
    } else {
        Ok(below)
    }
}
