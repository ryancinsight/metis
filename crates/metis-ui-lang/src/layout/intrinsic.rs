//! Intrinsic widths: how wide an element is when sized to its content.
//!
//! A flex container that does not stretch its children across the cross axis
//! sizes an automatic-width child to its content, as CSS flex layout does; in
//! a column that is the child's max-content width, the width its content
//! occupies without wrapping.

use super::device::{add, minimum, scaled_geometry, text_style, whole_pixels};
use crate::dom::{DomElement, DomNode};
use crate::parser::limit_error;
use crate::style::{Display, FlexDirection, Size};
use metis_core::error::Result;
use metis_platform::DisplayScale;

/// How an element with automatic width resolves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sizing {
    /// Fill the width available after margins.
    Fill,
    /// Take the content's max-content width, within the available width.
    Content,
}

/// The border-box width `element` occupies when sized to its content.
///
/// Text contributes its measured advance; a row sums its children and gaps; a
/// column takes its widest child; padding and borders are added and the
/// declared minimum applies. An explicit pixel width is taken as declared. A
/// percentage has nothing definite to resolve against here, so it sizes like
/// an automatic width, and a percentage minimum resolves against zero.
///
/// # Errors
/// Returns a layout limit error when a scaled length or the summed width
/// leaves the coordinate range.
pub(super) fn max_content_width(element: &DomElement, display_scale: DisplayScale) -> Result<i32> {
    let style = &element.computed_style;
    if style.display == Display::None {
        return Ok(0);
    }
    let floor = minimum(style.min_width, 0, display_scale)?;
    if let Size::Px(declared) = style.width {
        return Ok(display_scale.scale_extent(declared)?.max(floor));
    }
    let geometry = scaled_geometry(style, display_scale)?;
    let row = style.flex_direction == FlexDirection::Row;
    let (mut total, mut widest, mut count) = (0_i32, 0_i32, 0_i32);
    for child in &element.children {
        let width = match child {
            DomNode::Element(child) if child.computed_style.display == Display::None => continue,
            DomNode::Element(child) => {
                let margins = scaled_geometry(&child.computed_style, display_scale)?.margin;
                add(
                    max_content_width(child, display_scale)?,
                    add(margins.left, margins.right)?,
                )?
            }
            DomNode::Text(text) => whole_pixels(text_style(style, display_scale)?.advance(text))?,
        };
        total = add(total, width)?;
        widest = widest.max(width);
        count = add(count, 1)?;
    }
    let content = if row {
        let gaps = geometry
            .gap
            .checked_mul((count - 1).max(0))
            .ok_or_else(|| limit_error("Intrinsic gap width exceeds coordinate range"))?;
        add(total, gaps)?
    } else {
        widest
    };
    let edges = add(
        add(geometry.padding.left, geometry.padding.right)?,
        add(geometry.border.left, geometry.border.right)?,
    )?;
    Ok(add(content, edges)?.max(floor))
}
