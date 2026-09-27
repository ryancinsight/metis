//! Intrinsic sizes: how large an element is when sized to its content.
//!
//! A flex container that does not stretch its children across the cross axis
//! sizes an automatic-width child to its content, as CSS flex layout does; in
//! a column that is the child's max-content width, the width its content
//! occupies without wrapping.

use super::device::{add, dimension, minimum, scaled_geometry, sub, text_style, whole_pixels};
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
            DomNode::Element(child)
                if child.computed_style.display == Display::None || is_visible_popover(child) =>
            {
                continue;
            }
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

/// The border-box width layout gives `element` within `available_width`.
///
/// An automatic width fills the width left after margins, or with
/// [`Sizing::Content`] takes the max-content width within it; a declared width
/// resolves against `available_width`; the declared minimum applies last.
/// Layout sizes every element through this, so a flex base size measured here
/// is the width the element takes when laid out.
///
/// # Errors
/// Returns a layout limit error when a scaled length leaves the coordinate
/// range.
pub(super) fn border_box_width(
    element: &DomElement,
    available_width: i32,
    sizing: Sizing,
    display_scale: DisplayScale,
) -> Result<i32> {
    let style = &element.computed_style;
    let margin = scaled_geometry(style, display_scale)?.margin;
    let fill = sub(available_width, add(margin.left, margin.right)?)?.max(0);
    let automatic = match sizing {
        Sizing::Fill => fill,
        Sizing::Content => max_content_width(element, display_scale)?.min(fill),
    };
    Ok(
        dimension(style.width, available_width, automatic, display_scale)?.max(minimum(
            style.min_width,
            available_width,
            display_scale,
        )?),
    )
}

/// The border-box height `element` takes when laid out.
///
/// Heights do not depend on widths — text runs do not wrap — so this follows
/// layout's height rule directly: a text run is one line tall, a column stacks
/// its children and gaps, a row takes its tallest child, and padding, borders,
/// a declared height and the declared minimum apply as layout applies them.
/// Children resolve percentages against the same `available_height` their
/// parent did, as layout passes it down. Growth never changes the result: a
/// column's children grow only into the height this already accounts for.
///
/// # Errors
/// Returns a layout limit error when a scaled length or the summed height
/// leaves the coordinate range.
pub(super) fn max_content_height(
    element: &DomElement,
    available_height: i32,
    display_scale: DisplayScale,
) -> Result<i32> {
    let style = &element.computed_style;
    if style.display == Display::None {
        return Ok(0);
    }
    let geometry = scaled_geometry(style, display_scale)?;
    let row = style.flex_direction == FlexDirection::Row;
    let (mut total, mut tallest, mut count) = (0_i32, 0_i32, 0_i32);
    for child in &element.children {
        let height = match child {
            DomNode::Element(child)
                if child.computed_style.display == Display::None || is_visible_popover(child) =>
            {
                continue;
            }
            DomNode::Element(child) => max_content_height(child, available_height, display_scale)?,
            DomNode::Text(_) => whole_pixels(text_style(style, display_scale)?.line_height())?,
        };
        total = add(total, height)?;
        tallest = tallest.max(height);
        count = add(count, 1)?;
    }
    let content = if row {
        tallest
    } else {
        let gaps = geometry
            .gap
            .checked_mul((count - 1).max(0))
            .ok_or_else(|| limit_error("Intrinsic gap height exceeds coordinate range"))?;
        add(total, gaps)?
    };
    let edges = add(
        add(geometry.padding.top, geometry.padding.bottom)?,
        add(geometry.border.top, geometry.border.bottom)?,
    )?;
    Ok(dimension(
        style.height,
        available_height,
        add(content, edges)?.max(0),
        display_scale,
    )?
    .max(minimum(style.min_height, available_height, display_scale)?))
}

pub(super) fn is_visible_popover(element: &DomElement) -> bool {
    element.computed_style.display != Display::None
        && element.attributes.contains_key("popover-anchor")
}
