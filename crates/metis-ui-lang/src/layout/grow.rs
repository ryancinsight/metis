//! `flex-grow`: sharing a container's free main-axis space among its children.
//!
//! A child's size is fixed while it paints, so growth is planned before the
//! children are laid out rather than applied afterwards the way alignment is.
//! Each child's flex base size comes from the intrinsic measures, which follow
//! the same sizing rules layout applies; the free space left along the main
//! axis is then divided by `flex-grow` weight (CSS Flexbox 1 §9.7, for a single
//! line with no shrinking).
//!
//! A container none of whose children grows is not planned at all, so every
//! document that does not declare `flex-grow` lays out exactly as before.

use super::device::{add, dimension, minimum, scaled_geometry, sub, text_style, whole_pixels};
use super::intrinsic::{Sizing, border_box_width, is_visible_popover, max_content_height};
use crate::dom::{DomElement, DomNode};
use crate::parser::limit_error;
use crate::style::{ComputedStyle, Display, FlexGrow};
use metis_core::error::Result;
use metis_platform::DisplayScale;

/// A main-axis extent a container fixes for one child before laying it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Grown {
    /// The child sizes itself as it would outside a growing container.
    Natural,
    /// The child's border box is exactly this wide.
    Width(i32),
    /// The child's border box is exactly this tall.
    Height(i32),
}

/// The container a plan is made for, as layout resolved it.
#[derive(Clone, Copy)]
pub(super) struct GrowContainer<'style> {
    /// The container's own computed style.
    pub(super) style: &'style ComputedStyle,
    /// Whether the main axis is horizontal.
    pub(super) row: bool,
    /// Width of the container's content box.
    pub(super) content_width: i32,
    /// Height available to the container, against which percentages resolve.
    pub(super) available_height: i32,
    /// Scaled gap between adjacent children.
    pub(super) gap: i32,
    /// Host display scale.
    pub(super) display_scale: DisplayScale,
}

/// Plans the main-axis extent of every child of `element`.
///
/// Returns `None` when no child in flow grows. Otherwise the plan has one entry
/// per entry of `element.children`. In a row every child in flow is fixed at
/// its flex base size — its content width, or its declared width — plus its
/// share, so a non-growing sibling is sized to its content rather than filling
/// the row. In a column only growing children are fixed; the rest keep their
/// natural height, which is their base size. When the base sizes already fill
/// or overflow the container, the shares are zero.
///
/// # Errors
/// Returns a layout limit error when a measured extent leaves the coordinate
/// range or the plan cannot be allocated.
pub(super) fn plan(
    element: &DomElement,
    container: GrowContainer<'_>,
) -> Result<Option<Vec<Grown>>> {
    if !element
        .children
        .iter()
        .any(|child| matches!(child, DomNode::Element(child) if in_flow(child) && child.computed_style.flex_grow.grows()))
    {
        return Ok(None);
    }
    let GrowContainer {
        style,
        row,
        content_width,
        available_height,
        gap,
        display_scale,
    } = container;
    let mut bases = Vec::new();
    bases
        .try_reserve_exact(element.children.len())
        .map_err(|_| limit_error("Flex-grow plan allocation failed"))?;
    let (mut consumed, mut count) = (0_i32, 0_i32);
    for child in &element.children {
        let (base, weight) = match child {
            DomNode::Element(child) if !in_flow(child) => {
                bases.push(None);
                continue;
            }
            DomNode::Element(child) => {
                let base = if row {
                    border_box_width(child, content_width, Sizing::Content, display_scale)?
                } else {
                    max_content_height(child, available_height, display_scale)?
                };
                (Some(base), child.computed_style.flex_grow)
            }
            DomNode::Text(text) => {
                let text_style = text_style(style, display_scale)?;
                let extent = if row {
                    whole_pixels(text_style.advance(text))?
                } else {
                    whole_pixels(text_style.line_height())?
                };
                consumed = add(consumed, extent)?;
                count = add(count, 1)?;
                bases.push(None);
                continue;
            }
        };
        if let Some(base) = base {
            consumed = add(consumed, base)?;
        }
        count = add(count, 1)?;
        bases.push(base.map(|base| (base, weight)));
    }
    let gaps = gap
        .checked_mul((count - 1).max(0))
        .ok_or_else(|| limit_error("Flex-grow gap span exceeds coordinate range"))?;
    let occupied = add(consumed, gaps)?;
    let content_main = if row {
        content_width
    } else {
        column_content_height(style, occupied, available_height, display_scale)?
    };
    let free = sub(content_main, occupied)?.max(0);
    let weights: Vec<FlexGrow> = bases.iter().flatten().map(|&(_, weight)| weight).collect();
    let mut shares = shares(free, &weights)?.into_iter();
    bases
        .into_iter()
        .map(|entry| {
            let Some((base, weight)) = entry else {
                return Ok(Grown::Natural);
            };
            let grown = add(base, shares.next().unwrap_or(0))?;
            Ok(if row {
                Grown::Width(grown)
            } else if weight.grows() {
                Grown::Height(grown)
            } else {
                Grown::Natural
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

/// Divides `free` pixels among `weights`, in the order given.
///
/// Shares follow the running total of the weights, and each is the difference
/// of two floored cumulative values, so rounding never loses or invents a
/// pixel: the shares sum to exactly the space distributed. That space is all of
/// `free` when the weights sum to at least one, and the weights' fraction of it
/// when they sum to less (CSS Flexbox 1 §9.7 step 4.2).
///
/// # Errors
/// Returns a layout limit error when a share leaves the coordinate range.
pub(super) fn shares(free: i32, weights: &[FlexGrow]) -> Result<Vec<i32>> {
    let total: u64 = weights
        .iter()
        .map(|weight| u64::from(weight.thousandths()))
        .sum();
    let denominator = i128::from(total.max(u64::from(FlexGrow::ONE)));
    let free = i128::from(free.max(0));
    let (mut running, mut previous) = (0_i128, 0_i128);
    weights
        .iter()
        .map(|weight| {
            running += i128::from(weight.thousandths());
            let reached = free * running / denominator;
            let share = i32::try_from(reached - previous)
                .map_err(|_| limit_error("Flex-grow share exceeds coordinate range"))?;
            previous = reached;
            Ok(share)
        })
        .collect()
}

/// The content-box height of a column whose children occupy `occupied` pixels.
///
/// An automatic height is the children's own; a declared height or minimum can
/// make the box taller, and that surplus is the space children grow into.
fn column_content_height(
    style: &ComputedStyle,
    occupied: i32,
    available_height: i32,
    display_scale: DisplayScale,
) -> Result<i32> {
    let geometry = scaled_geometry(style, display_scale)?;
    let vertical_edges = add(
        add(geometry.padding.top, geometry.padding.bottom)?,
        add(geometry.border.top, geometry.border.bottom)?,
    )?;
    let height = dimension(
        style.height,
        available_height,
        add(occupied, vertical_edges)?,
        display_scale,
    )?
    .max(minimum(style.min_height, available_height, display_scale)?);
    Ok(sub(height, vertical_edges)?.max(0))
}

fn in_flow(element: &DomElement) -> bool {
    element.computed_style.display != Display::None && !is_visible_popover(element)
}
