//! Display list: the commands of one laid-out document and their owners.

use super::command::DisplayCommand;
use super::element::{AnimatedElement, ElementIndex};
use crate::image::ImagePlacement;
use crate::parser::limit_error;
use crate::style::Color;
use crate::transition::{Transition, TransitionProperty};
use metis_core::error::Result;
use metis_platform::framebuffer::{Framebuffer, Rect};
use metis_platform::rasterizer::{
    CornerRadius, LineCap, LineJoin, MAX_STROKE_POINTS, StrokeWidth, draw_box_shadow, draw_line,
    draw_polyline, draw_rect_outline, fill_gradient, fill_rect,
};
use metis_platform::typeface::draw_text;
use std::sync::Arc;

/// Drawing commands emitted by bounded layout.
///
/// A command of an element whose style declares a transition names that
/// element, so a host can change one element's paint without laying the
/// document out again. The attribution is a side index rather than a field of
/// the command: a command stays a fixed-size value that clones without
/// requesting memory.
///
/// The index is scoped to the elements a transition can address. A document
/// that declares none carries no attribution at all, so an unchanged render of
/// such a form still requests only its command storage.
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    /// Commands in painter order; public through [`Self::commands`], so a
    /// caller outside layout cannot desynchronize the owner index.
    pub(in crate::layout) commands: Vec<DisplayCommand>,
    /// `(command, element)` for every command an animated element paints, in
    /// command order.
    ///
    /// Shared between copies: attribution is a fact of the layout, and a frame
    /// only changes command colors, so a copy clones a pointer rather than the
    /// table.
    owned: Arc<Vec<(u32, u32)>>,
    /// The elements whose style declares a transition, in emission order.
    animated: Arc<Vec<AnimatedElement>>,
    /// The element the next push belongs to, while layout is inside one that
    /// declares a transition.
    owner: Option<u32>,
    /// One past the last command this list holds as current.
    ///
    /// A layout writes through this cursor, so laying a document out again
    /// into a list it laid out before overwrites the storage it already owns
    /// instead of growing a second one. It equals `commands.len()` for a list
    /// a host appends to.
    written: usize,
}

impl DisplayList {
    /// The commands in painter order.
    #[must_use]
    pub fn commands(&self) -> &[DisplayCommand] {
        &self.commands
    }

    /// One past the last command the current layout wrote.
    ///
    /// Layout reads this instead of `commands.len()`, because a list being
    /// laid out again holds commands the new layout has not reached yet.
    pub(in crate::layout) const fn written(&self) -> usize {
        self.written
    }

    /// The animated element that paints a command, or `None` when the command
    /// belongs to a host overlay or to an element that declares no transition.
    #[must_use]
    pub fn command_owner(&self, index: usize) -> Option<ElementIndex> {
        let index = u32::try_from(index).ok()?;
        self.owned
            .binary_search_by_key(&index, |(command, _)| *command)
            .ok()
            .map(|slot| ElementIndex(self.owned[slot].1))
    }

    /// The animated elements, in emission order.
    pub fn elements(&self) -> impl Iterator<Item = ElementIndex> + '_ {
        (0..self.animated.len()).filter_map(|index| u32::try_from(index).ok().map(ElementIndex))
    }

    /// The authored identifier of an element, when the document named it.
    #[must_use]
    pub fn element_id(&self, element: ElementIndex) -> Option<&str> {
        self.record(element)?.id.as_deref()
    }

    /// The transition the element's style declares.
    #[must_use]
    pub fn element_transition(&self, element: ElementIndex) -> Option<Transition> {
        Some(self.record(element)?.transition)
    }

    /// The last animated element the document named `id`.
    ///
    /// Later entries take precedence, matching the popup stacking
    /// [`Self::element_rect`] follows.
    #[must_use]
    pub fn element_index(&self, id: &str) -> Option<ElementIndex> {
        self.animated
            .iter()
            .rposition(|record| record.id.as_deref() == Some(id))
            .and_then(|index| u32::try_from(index).ok())
            .map(ElementIndex)
    }

    /// The color an element paints for `property`, taken from the first
    /// command that paints it.
    #[must_use]
    pub fn element_color(
        &self,
        element: ElementIndex,
        property: TransitionProperty,
    ) -> Option<Color> {
        self.owned
            .iter()
            .filter(|(_, owner)| *owner == element.0)
            .find_map(|(command, _)| {
                let command = usize::try_from(*command).ok()?;
                self.commands.get(command)?.animated_color(property)
            })
    }

    /// Writes `color` into every command the element paints for `property`,
    /// reporting the area those commands cover.
    ///
    /// A command that already holds `color` is left alone, so the report is
    /// the area whose paint actually changes. `None` therefore means the call
    /// changed no pixel, which is what a host needs to tell a moving frame
    /// from a settled one.
    pub fn recolor(
        &mut self,
        element: ElementIndex,
        property: TransitionProperty,
        color: Color,
    ) -> Option<Rect> {
        let mut covered: Option<Rect> = None;
        for (command, owner) in self.owned.iter() {
            if *owner != element.0 {
                continue;
            }
            let Some(slot) = usize::try_from(*command).ok() else {
                continue;
            };
            let Some(target) = self.commands.get_mut(slot) else {
                continue;
            };
            let Some(bounds) = target.paint_bounds() else {
                continue;
            };
            let Some(paint) = target.animated_color_mut(property) else {
                continue;
            };
            if *paint == color {
                continue;
            }
            *paint = color;
            covered = Some(covered.map_or(bounds, |area| area.union(bounds)));
        }
        covered
    }

    fn record(&self, element: ElementIndex) -> Option<&AnimatedElement> {
        usize::try_from(element.0)
            .ok()
            .and_then(|index| self.animated.get(index))
    }

    /// Opens an element: later commands belong to it until [`Self::end_element`].
    ///
    /// Only an element that declares a transition is addressable, so only such
    /// an element records itself and becomes the owner of what it paints.
    /// Returns the owner to restore, so nesting restores the parent exactly
    /// rather than by inference.
    pub(in crate::layout) fn begin_element(
        &mut self,
        id: Option<&str>,
        transition: Option<Transition>,
    ) -> Result<Option<u32>> {
        let previous = self.owner;
        self.owner = match transition {
            Some(transition) => {
                let animated = Arc::make_mut(&mut self.animated);
                let index = u32::try_from(animated.len()).map_err(|_| {
                    limit_error("Animated element count exceeds the display index range")
                })?;
                animated
                    .try_reserve(1)
                    .map_err(|_| limit_error("Display element allocation failed"))?;
                animated.push(AnimatedElement {
                    id: id.map(Arc::from),
                    transition,
                });
                Some(index)
            }
            None => None,
        };
        Ok(previous)
    }

    /// Closes the element [`Self::begin_element`] opened.
    pub(in crate::layout) fn end_element(&mut self, previous: Option<u32>) {
        self.owner = previous;
    }

    /// Executes the display list with framebuffer clipping.
    pub fn render_to(&self, fb: &mut Framebuffer) {
        for command in &self.commands {
            match command {
                DisplayCommand::ElementRect { .. } => {}
                DisplayCommand::DrawShadow {
                    rect,
                    radius,
                    shadow,
                } => draw_box_shadow(fb, *rect, *radius, *shadow),
                DisplayCommand::FillRect {
                    rect,
                    radius,
                    color,
                } => fill_rect(fb, *rect, *radius, *color),
                DisplayCommand::FillGradient {
                    rect,
                    radius,
                    gradient,
                } => fill_gradient(fb, *rect, *radius, gradient),
                DisplayCommand::DrawBorder {
                    rect,
                    width,
                    radius,
                    color,
                } => {
                    draw_rect_outline(fb, *rect, *width, *radius, *color);
                }
                DisplayCommand::DrawLine { start, end, color } => {
                    draw_line(fb, *start, *end, *color);
                }
                DisplayCommand::DrawPolyline {
                    points,
                    width,
                    cap,
                    join,
                    color,
                } => draw_polyline(fb, points, *width, *cap, *join, *color),
                DisplayCommand::DrawText { text, x, y, style } => {
                    draw_text(fb, *x, *y, text, *style);
                }
                DisplayCommand::DrawImage { placement } => placement.render_to(fb),
            }
        }
    }

    /// Returns the laid-out border rectangle for an authored element ID.
    ///
    /// Later painter entries take precedence, matching popup stacking when a
    /// programmatic DOM contains duplicate IDs. Semantic projection rejects
    /// duplicate IDs before accessibility exposure.
    #[must_use]
    pub fn element_rect(&self, id: &str) -> Option<Rect> {
        self.commands
            .iter()
            .rev()
            .find_map(|command| match command {
                DisplayCommand::ElementRect {
                    id: candidate,
                    rect,
                } if &**candidate == id => Some(*rect),
                _ => None,
            })
    }

    /// Appends a validated image command in painter order.
    ///
    /// # Errors
    /// Returns [`metis_core::error::ErrorCode::LayoutOverflow`] when the
    /// display command storage cannot grow.
    pub fn append_image(&mut self, placement: ImagePlacement) -> Result<()> {
        self.push(DisplayCommand::DrawImage {
            placement: Arc::new(placement),
        })
    }

    /// Appends a clipped one-pixel line command in painter order.
    ///
    /// Endpoints may be outside the target surface; the renderer clips them
    /// before traversing the segment.
    ///
    /// # Errors
    ///
    /// Returns [`metis_core::error::ErrorCode::LayoutOverflow`] when the
    /// display command storage cannot grow.
    pub fn append_line(&mut self, start: (i32, i32), end: (i32, i32), color: Color) -> Result<()> {
        self.push(DisplayCommand::DrawLine { start, end, color })
    }

    /// Appends an inward border of `width` device pixels in painter order.
    ///
    /// Hosts use it for overlays drawn over the laid-out document, such as a
    /// keyboard focus ring around an element's [`Self::element_rect`].
    ///
    /// # Errors
    ///
    /// Returns [`metis_core::error::ErrorCode::LayoutOverflow`] when the
    /// display command storage cannot grow.
    pub fn append_border(
        &mut self,
        rect: Rect,
        width: i32,
        radius: CornerRadius,
        color: Color,
    ) -> Result<()> {
        self.push(DisplayCommand::DrawBorder {
            rect,
            width,
            radius,
            color,
        })
    }

    /// Appends a bounded polyline stroke in painter order.
    ///
    /// At most [`MAX_STROKE_POINTS`] vertices are retained. The points are
    /// copied once into the display list so callers may release their input
    /// after this method returns.
    ///
    /// # Errors
    ///
    /// Returns [`metis_core::error::ErrorCode::LayoutOverflow`] for an empty
    /// or oversized path or when the display command storage cannot grow.
    /// Construct the [`StrokeWidth`] before calling this method to reject
    /// zero.
    pub fn append_polyline(
        &mut self,
        points: &[(i32, i32)],
        width: StrokeWidth,
        cap: LineCap,
        join: LineJoin,
        color: Color,
    ) -> Result<()> {
        if points.is_empty() || points.len() > MAX_STROKE_POINTS {
            return Err(limit_error("Polyline point limit exceeded"));
        }
        self.push(DisplayCommand::DrawPolyline {
            points: Arc::from(points),
            width,
            cap,
            join,
            color,
        })
    }

    /// A copy of this list with room for `extra` more commands.
    ///
    /// Every command and every attribution is shared with `self`; only the
    /// command storage is new, so a frame requests no memory for the payloads
    /// or the ownership table it repeats.
    ///
    /// # Errors
    /// Returns [`metis_core::error::ErrorCode::LayoutOverflow`] when the
    /// storage cannot be allocated or the counts exceed the addressable range.
    pub fn copy_with_room(&self, extra: usize) -> Result<Self> {
        let capacity =
            self.commands.len().checked_add(extra).ok_or_else(|| {
                limit_error("Display command count exceeds the addressable range")
            })?;
        let mut copy = Self {
            commands: Vec::new(),
            owned: Arc::clone(&self.owned),
            animated: Arc::clone(&self.animated),
            owner: None,
            written: 0,
        };
        copy.commands
            .try_reserve_exact(capacity)
            .map_err(|_| limit_error("Display command allocation failed"))?;
        copy.commands.extend_from_slice(&self.commands);
        copy.written = copy.commands.len();
        Ok(copy)
    }

    /// A list of commands no element paints.
    ///
    /// Hosts and tests build presentation commands outside a layout; a
    /// transition cannot address them, which is the intended treatment of an
    /// overlay that belongs to no element.
    #[must_use]
    pub fn from_commands(commands: Vec<DisplayCommand>) -> Self {
        Self {
            written: commands.len(),
            commands,
            ..Self::default()
        }
    }

    /// Prepares this list to receive a layout that replaces what it holds.
    ///
    /// The storage stays: a document laid out again writes over the commands
    /// it wrote before, so the second layout of an unchanged form requests no
    /// memory for command storage. Every reader sees only the commands the
    /// new layout wrote once [`Self::end_frame`] closes it.
    pub(in crate::layout) fn begin_frame(&mut self) {
        self.written = 0;
        self.owner = None;
        // Attribution is a fact of the layout being replaced. The table is
        // shared with the frames painted from the previous layout, so it is
        // cleared only when this list is its last holder.
        self.owned = Arc::new(Vec::new());
        self.animated = Arc::new(Vec::new());
    }

    /// Closes a layout [`Self::begin_frame`] opened, dropping the commands the
    /// new layout did not reach.
    pub(in crate::layout) fn end_frame(&mut self) {
        self.commands.truncate(self.written);
        self.owner = None;
    }

    /// The shared identifier this list already holds at the write position,
    /// when it names `id`.
    ///
    /// A layout that replaces a document writes over the commands it wrote
    /// before, so an identifier the previous layout interned is shared rather
    /// than interned again. The answer is a hint: a position that holds
    /// another command simply costs the request it would have cost anyway.
    pub(in crate::layout) fn held_id(&self, id: &str) -> Option<Arc<str>> {
        match self.commands.get(self.written) {
            Some(DisplayCommand::ElementRect { id: held, .. }) if &**held == id => {
                Some(Arc::clone(held))
            }
            _ => None,
        }
    }

    /// The shared text this list already holds at the write position, when it
    /// is `text`; see [`Self::held_id`].
    pub(in crate::layout) fn held_text(&self, text: &str) -> Option<Arc<str>> {
        match self.commands.get(self.written) {
            Some(DisplayCommand::DrawText { text: held, .. }) if &**held == text => {
                Some(Arc::clone(held))
            }
            _ => None,
        }
    }

    /// Appends a command for the element layout is inside, or for no element
    /// when a host appends one over the layout.
    pub(in crate::layout) fn push(&mut self, command: DisplayCommand) -> Result<()> {
        let index = self.written;
        if index == self.commands.len() {
            self.commands
                .try_reserve(1)
                .map_err(|_| limit_error("Display command allocation failed"))?;
            self.commands.push(command);
        } else {
            self.commands[index] = command;
        }
        self.written = index + 1;
        if let Some(owner) = self.owner {
            let index = u32::try_from(index).map_err(|_| {
                limit_error("Display command count exceeds the display index range")
            })?;
            let owned = Arc::make_mut(&mut self.owned);
            owned
                .try_reserve(1)
                .map_err(|_| limit_error("Display command allocation failed"))?;
            owned.push((index, owner));
        }
        Ok(())
    }
}
