use super::display::{DisplayCommand, DisplayList};
use crate::dom::{DomDocument, DomElement, DomNode};
use crate::parser::{copy_text, limit_error};
use crate::style::{AlignItems, ComputedStyle, Display, FlexDirection, JustifyContent};
use metis_core::error::Result;
use metis_platform::DisplayScale;
use metis_platform::framebuffer::Rect;
use metis_platform::rasterizer::CornerRadius;

use super::device::{
    add, device_shadow, dimension, minimum, scaled_geometry, sub, text_style, whole_pixels,
};
use super::intrinsic::{Sizing, is_visible_popover, max_content_width};
use super::limits::validate_layout_tree;
use super::popover::popover_error;

/// Logical viewport dimensions and the host's device-pixel scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutViewport {
    width: i32,
    height: i32,
    scale: DisplayScale,
}

impl LayoutViewport {
    /// Creates a viewport at the default 96-DPI scale.
    #[must_use]
    pub const fn new(width: i32, height: i32) -> Self {
        Self::with_scale(width, height, DisplayScale::ONE)
    }

    /// Creates a viewport with an explicit validated device scale.
    #[must_use]
    pub const fn with_scale(width: i32, height: i32, scale: DisplayScale) -> Self {
        Self {
            width,
            height,
            scale,
        }
    }

    /// Horizontal physical pixel count supplied by the host.
    #[must_use]
    pub const fn width(self) -> i32 {
        self.width
    }

    /// Vertical physical pixel count supplied by the host.
    #[must_use]
    pub const fn height(self) -> i32 {
        self.height
    }

    /// Device scale used to map authored dimensions to physical pixels.
    #[must_use]
    pub const fn scale(self) -> DisplayScale {
        self.scale
    }
}

/// Computes a bounded display list for a nonnegative viewport.
///
/// Programmatic DOMs have the same depth, node, and relevant content-byte limits
/// as parsed DOMs.
/// Integer overflow is rejected rather than wrapped; percentage sizes round toward zero.
///
/// # Errors
/// Rejects negative viewport or explicit dimensions, non-finite percentages,
/// coordinate overflow, excessive tree resources, failed allocations, a popover
/// used as the document root, or a visible popover whose anchor has not been laid
/// out.
pub fn compute_layout(doc: &DomDocument, viewport: LayoutViewport) -> Result<DisplayList> {
    if viewport.width < 0 || viewport.height < 0 {
        return Err(limit_error("Viewport dimensions must be nonnegative"));
    }
    validate_layout_tree(&doc.root)?;
    let mut list = DisplayList::default();
    if is_visible_popover(&doc.root) {
        return Err(popover_error(
            "The document root cannot be an anchored popover",
        ));
    }
    list.element(
        &doc.root,
        Rect::new(0, 0, viewport.width, viewport.height),
        viewport.scale,
        Sizing::Fill,
    )?;
    list.popovers(&doc.root, viewport)?;
    Ok(list)
}

impl DisplayList {
    fn text(
        &mut self,
        text: &str,
        style: &ComputedStyle,
        x: i32,
        y: i32,
        display_scale: DisplayScale,
    ) -> Result<(i32, i32)> {
        let text_style = text_style(style, display_scale)?;
        // Extents round up so the box always holds the glyphs it measures.
        let text_width = whole_pixels(text_style.advance(text))?;
        let text_height = whole_pixels(text_style.line_height())?;
        self.push(DisplayCommand::DrawText {
            text: copy_text(text)?,
            x,
            y,
            style: text_style,
        })?;
        Ok((text_width, text_height))
    }

    fn children_extent(
        &mut self,
        element: &DomElement,
        layout: ChildLayout<'_>,
    ) -> Result<ChildExtent> {
        let ChildLayout {
            style,
            content_x,
            content_y,
            content_width,
            available_height,
            row,
            gap,
            display_scale,
        } = layout;
        let mut child_x = content_x;
        let mut child_y = content_y;
        let mut cross_size = 0;
        let mut placements = Vec::new();
        let mut visible_children = 0;
        for child in &element.children {
            if matches!(child, DomNode::Element(child) if child.computed_style.display == Display::None || is_visible_popover(child))
            {
                continue;
            }
            if visible_children > 0 {
                if row {
                    child_x = add(child_x, gap)?;
                } else {
                    child_y = add(child_y, gap)?;
                }
            }
            let first_command = self.commands.len();
            let (child_width, child_height) = match child {
                DomNode::Element(child) => {
                    let available_width = if row {
                        sub(content_width, sub(child_x, content_x)?)?.max(0)
                    } else {
                        content_width
                    };
                    // A column that does not stretch its children sizes an
                    // automatic-width child to its content, so cross-axis
                    // alignment has free space to place it in.
                    let sizing = if !row && style.align_items != AlignItems::Stretch {
                        Sizing::Content
                    } else {
                        Sizing::Fill
                    };
                    let child_rect = self.element(
                        child,
                        Rect::new(child_x, child_y, available_width, available_height),
                        display_scale,
                        sizing,
                    )?;
                    (child_rect.width, child_rect.height)
                }
                DomNode::Text(text) => self.text(text, style, child_x, child_y, display_scale)?,
            };
            placements
                .try_reserve(1)
                .map_err(|_| limit_error("Child placement allocation failed"))?;
            placements.push(ChildPlacement {
                first_command,
                end_command: self.commands.len(),
                cross: if row { child_height } else { child_width },
            });
            if row {
                child_x = add(child_x, child_width)?;
                cross_size = cross_size.max(child_height);
            } else {
                child_y = add(child_y, child_height)?;
                cross_size = cross_size.max(child_width);
            }
            visible_children += 1;
        }
        let content_height = if row {
            cross_size
        } else {
            sub(child_y, content_y)?
        };
        // Gaps belong to the occupied main extent, so the cursor delta is the
        // span alignment redistributes around.
        let main_used = if row {
            sub(child_x, content_x)?
        } else {
            sub(child_y, content_y)?
        };
        Ok(ChildExtent {
            content_height,
            main_used,
            placements,
        })
    }

    /// Places children within the free space the container leaves.
    ///
    /// Children paint while they are measured, so redistribution translates
    /// what they already emitted. Start and stretch alignment produce zero
    /// offsets, which is why a document that declares neither moves at all.
    fn align_children(
        &mut self,
        style: &crate::style::ComputedStyle,
        extent: &ChildExtent,
        content_main: i32,
        content_cross: i32,
        row: bool,
    ) -> Result<()> {
        let free = sub(content_main, extent.main_used)?.max(0);
        let count = i32::try_from(extent.placements.len())
            .map_err(|_| limit_error("Child count exceeds coordinate range"))?;
        for (index, placement) in extent.placements.iter().enumerate() {
            let ordinal = i32::try_from(index)
                .map_err(|_| limit_error("Child index exceeds coordinate range"))?;
            let main_offset = match style.justify_content {
                JustifyContent::Center => free / 2,
                JustifyContent::FlexEnd => free,
                // The first child keeps the start edge and the last reaches the
                // end edge, so each step is one share of the free space.
                JustifyContent::SpaceBetween if count > 1 => mul(free, ordinal)? / (count - 1),
                // A single child has no gap to distribute into, so it sits
                // where start alignment puts it.
                JustifyContent::FlexStart | JustifyContent::SpaceBetween => 0,
            };
            let cross_free = sub(content_cross, placement.cross)?.max(0);
            let cross_offset = match style.align_items {
                AlignItems::FlexStart | AlignItems::Stretch => 0,
                AlignItems::Center => cross_free / 2,
                AlignItems::FlexEnd => cross_free,
            };
            if main_offset == 0 && cross_offset == 0 {
                continue;
            }
            let (dx, dy) = if row {
                (main_offset, cross_offset)
            } else {
                (cross_offset, main_offset)
            };
            for command in &mut self.commands[placement.first_command..placement.end_command] {
                command.translate(dx, dy)?;
            }
        }
        Ok(())
    }

    pub(super) fn element(
        &mut self,
        element: &DomElement,
        available: Rect,
        display_scale: DisplayScale,
        sizing: Sizing,
    ) -> Result<Rect> {
        let style = &element.computed_style;
        if style.display == Display::None {
            return Ok(Rect::new(available.x, available.y, 0, 0));
        }
        let geometry = scaled_geometry(style, display_scale)?;
        let fill = sub(
            available.width,
            add(geometry.margin.left, geometry.margin.right)?,
        )?
        .max(0);
        let automatic = match sizing {
            Sizing::Fill => fill,
            Sizing::Content => max_content_width(element, display_scale)?.min(fill),
        };
        let width = dimension(style.width, available.width, automatic, display_scale)?
            .max(minimum(style.min_width, available.width, display_scale)?);
        let x = add(available.x, geometry.margin.left)?;
        let y = add(available.y, geometry.margin.top)?;
        let content_x = add(add(x, geometry.padding.left)?, geometry.border.left)?;
        let content_y = add(add(y, geometry.padding.top)?, geometry.border.top)?;
        let horizontal_edges = add(
            add(geometry.padding.left, geometry.padding.right)?,
            add(geometry.border.left, geometry.border.right)?,
        )?;
        let content_width = sub(width, horizontal_edges)?.max(0);
        let row = style.flex_direction == FlexDirection::Row;
        // Reserve the parent's painter position before descendants; its auto height is
        // filled after child layout, so siblings never paint behind earlier siblings.
        let slots = self.reserve_box(style, Rect::new(x, y, width, 0), display_scale)?;
        let element_rect = if let Some(id) = element.id() {
            let index = self.commands.len();
            self.push(DisplayCommand::ElementRect {
                id: copy_text(id)?,
                rect: Rect::new(x, y, width, 0),
            })?;
            Some(index)
        } else {
            None
        };
        let child_extent = self.children_extent(
            element,
            ChildLayout {
                style,
                content_x,
                content_y,
                content_width,
                available_height: available.height,
                row,
                gap: geometry.gap,
                display_scale,
            },
        )?;
        let vertical_edges = add(
            add(geometry.padding.top, geometry.padding.bottom)?,
            add(geometry.border.top, geometry.border.bottom)?,
        )?;
        let height = dimension(
            style.height,
            available.height,
            add(child_extent.content_height, vertical_edges)?.max(0),
            display_scale,
        )?
        .max(minimum(style.min_height, available.height, display_scale)?);
        let rect = Rect::new(x, y, width, height);
        // Free space exists only once the container's own extent is final: an
        // automatic height is derived from the children that just painted.
        let content_height_box = sub(height, vertical_edges)?.max(0);
        let (content_main, content_cross) = if row {
            (content_width, content_height_box)
        } else {
            (content_height_box, content_width)
        };
        self.align_children(style, &child_extent, content_main, content_cross, row)?;
        // The radius is clamped against the final rectangle, whose height is
        // known only after the children have been laid out.
        let radius = CornerRadius::clamped(display_scale.scale_extent(style.border_radius)?, rect);
        self.settle_box(slots, rect, radius);
        if let Some(index) = element_rect {
            let DisplayCommand::ElementRect { rect: target, .. } = &mut self.commands[index] else {
                unreachable!("invariant: element rectangle index names element metadata");
            };
            *target = rect;
        }
        if geometry.border.top > 0 {
            self.push(DisplayCommand::DrawBorder {
                rect,
                width: geometry.border.top,
                radius,
                color: style.border_color,
            })?;
        }
        Ok(rect)
    }

    /// Reserves painter slots for the element's shadow and background.
    ///
    /// The outer shadow sits immediately below the background (CSS
    /// Backgrounds 3 §6.1.3), so its slot is reserved first; the background
    /// image paints over the background color (§3.1). Every slot carries a
    /// placeholder until [`Self::settle_box`] writes the final rectangle.
    fn reserve_box(
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
    fn settle_box(&mut self, slots: BoxSlots, rect: Rect, radius: CornerRadius) {
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

/// Painter slots an element reserves before its children paint.
#[derive(Clone, Copy)]
struct BoxSlots {
    shadow: Option<usize>,
    background: Option<usize>,
    gradient: Option<usize>,
}

/// Where one child's commands and extents landed during child layout.
struct ChildPlacement {
    /// First command this child emitted, in painter order.
    first_command: usize,
    /// One past this child's last command.
    end_command: usize,
    /// Extent along the container's cross axis.
    cross: i32,
}

/// What child layout leaves for the container to redistribute.
struct ChildExtent {
    /// Content height the container uses for its automatic height.
    content_height: i32,
    /// Main-axis span the children occupied, gaps included.
    main_used: i32,
    /// One record per visible child, in painter order.
    placements: Vec<ChildPlacement>,
}

#[derive(Clone, Copy)]
struct ChildLayout<'style> {
    style: &'style ComputedStyle,
    content_x: i32,
    content_y: i32,
    content_width: i32,
    available_height: i32,
    row: bool,
    gap: i32,
    display_scale: DisplayScale,
}

fn mul(left: i32, right: i32) -> Result<i32> {
    left.checked_mul(right)
        .ok_or_else(|| limit_error("Layout text extent overflow"))
}
