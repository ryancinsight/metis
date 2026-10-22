use crate::image::ImagePlacement;
use crate::parser::limit_error;
use crate::style::{Color, LinearGradient};
use crate::transition::{Transition, TransitionProperty};
use metis_core::error::Result;
use metis_platform::framebuffer::{Framebuffer, Rect};
use metis_platform::rasterizer::{
    BoxShadow, CornerRadius, LineCap, LineJoin, MAX_STROKE_POINTS, StrokeWidth, draw_box_shadow,
    draw_line, draw_polyline, draw_rect_outline, fill_gradient, fill_rect,
};
use metis_platform::typeface::{TextStyle, draw_text};
use std::sync::Arc;

/// Primitive command in painter order.
///
/// Identifiers, text, gradients, polylines and image placements are shared
/// handles, so cloning a command copies a fixed-size value and requests no
/// heap memory; a display list is cloned to reuse a layout that did not
/// change. A const assertion bounds the size of the command.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DisplayCommand {
    /// Element rectangle metadata used by host hit testing.
    ElementRect {
        /// Authored element identifier.
        id: Arc<str>,
        /// Laid-out border rectangle in framebuffer coordinates.
        rect: Rect,
    },
    /// Blurred outer shadow of a border box, clipped inside that box.
    DrawShadow {
        /// Border box casting the shadow.
        rect: Rect,
        /// Corner rounding shared with the border box.
        radius: CornerRadius,
        /// Offsets, blur and color in device pixels.
        shadow: BoxShadow,
    },
    /// Rectangle fill.
    FillRect {
        /// Target rectangle.
        rect: Rect,
        /// Corner rounding; [`CornerRadius::SQUARE`] keeps square corners.
        radius: CornerRadius,
        /// Straight RGBA color.
        color: Color,
    },
    /// Linear gradient across a rectangle, painted over its background color.
    FillGradient {
        /// Target rectangle, which is also the gradient box.
        rect: Rect,
        /// Corner rounding; [`CornerRadius::SQUARE`] keeps square corners.
        radius: CornerRadius,
        /// Direction and color stops.
        gradient: Arc<LinearGradient>,
    },
    /// Uniform inward border following the fill it encloses.
    DrawBorder {
        /// Outer rectangle.
        rect: Rect,
        /// Border width.
        width: i32,
        /// Corner rounding; [`CornerRadius::SQUARE`] keeps square corners.
        radius: CornerRadius,
        /// Straight RGBA color.
        color: Color,
    },
    /// One-pixel line segment clipped to the framebuffer.
    DrawLine {
        /// Inclusive start coordinate.
        start: (i32, i32),
        /// Inclusive end coordinate.
        end: (i32, i32),
        /// Straight RGBA stroke color.
        color: Color,
    },
    /// Bounded multi-segment stroke with explicit width, caps and joins.
    DrawPolyline {
        /// Integer-coordinate path vertices in painter order.
        points: Arc<[(i32, i32)]>,
        /// Positive device-space stroke width.
        width: StrokeWidth,
        /// Endpoint treatment.
        cap: LineCap,
        /// Interior vertex treatment.
        join: LineJoin,
        /// Straight RGBA stroke color.
        color: Color,
    },
    /// Single horizontal antialiased text run.
    DrawText {
        /// Unicode text; characters the face lacks display as its
        /// missing-glyph box.
        text: Arc<str>,
        /// Left edge of the line box.
        x: i32,
        /// Top edge of the line box.
        y: i32,
        /// Color, device-pixel size and weight.
        style: TextStyle,
    },
    /// Raster image crop composited with source-over alpha.
    DrawImage {
        /// Validated source and destination placement.
        placement: Arc<ImagePlacement>,
    },
}

const _: () = assert!(size_of::<DisplayCommand>() <= 56);

/// Position of an element in a display list's element table.
///
/// Layout numbers each element as it emits it, so the number is stable for the
/// document revision and viewport the list was laid out at, which is exactly
/// the lifetime a paint-time transition overlay spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementIndex(u32);

impl ElementIndex {
    /// The element's position in emission order.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// An element whose style declares a transition.
#[derive(Debug, Clone)]
struct AnimatedElement {
    /// Authored identifier; `None` for an element the document did not name,
    /// which no run can follow across a relayout.
    id: Option<Arc<str>>,
    /// The transition the element's style declares.
    transition: Transition,
}

impl DisplayCommand {
    /// Moves every coordinate this command carries.
    ///
    /// Alignment redistributes free space after a child has already painted,
    /// so the child's commands move rather than being emitted twice. Every
    /// variant is matched without elision: a command kind added later must
    /// state how it moves, or a laid-out child would tear.
    pub(crate) fn translate(&mut self, dx: i32, dy: i32) -> Result<()> {
        let shift = |value: i32, delta: i32| {
            value
                .checked_add(delta)
                .ok_or_else(|| limit_error("Display coordinate exceeds coordinate range"))
        };
        let shift_rect = |rect: &mut Rect| -> Result<()> {
            *rect = Rect::new(
                shift(rect.x, dx)?,
                shift(rect.y, dy)?,
                rect.width,
                rect.height,
            );
            Ok(())
        };
        match self {
            Self::ElementRect { rect, .. }
            | Self::DrawShadow { rect, .. }
            | Self::FillRect { rect, .. }
            | Self::FillGradient { rect, .. }
            | Self::DrawBorder { rect, .. } => shift_rect(rect),
            Self::DrawLine { start, end, .. } => {
                *start = (shift(start.0, dx)?, shift(start.1, dy)?);
                *end = (shift(end.0, dx)?, shift(end.1, dy)?);
                Ok(())
            }
            Self::DrawPolyline { points, .. } => {
                *points = points
                    .iter()
                    .map(|point| Ok((shift(point.0, dx)?, shift(point.1, dy)?)))
                    .collect::<Result<_>>()?;
                Ok(())
            }
            Self::DrawText { x, y, .. } => {
                *x = shift(*x, dx)?;
                *y = shift(*y, dy)?;
                Ok(())
            }
            Self::DrawImage { placement } => Arc::make_mut(placement).translate(dx, dy),
        }
    }

    /// The color this command paints for `property`, when it paints one.
    fn animated_color(&self, property: TransitionProperty) -> Option<Color> {
        match (self, property) {
            (Self::DrawText { style, .. }, TransitionProperty::Color) => Some(style.color),
            (Self::DrawBorder { color, .. }, TransitionProperty::BorderColor) => Some(*color),
            _ => None,
        }
    }

    /// The same color as a write target.
    fn animated_color_mut(&mut self, property: TransitionProperty) -> Option<&mut Color> {
        match (self, property) {
            (Self::DrawText { style, .. }, TransitionProperty::Color) => Some(&mut style.color),
            (Self::DrawBorder { color, .. }, TransitionProperty::BorderColor) => Some(color),
            _ => None,
        }
    }
}

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
    pub(super) commands: Vec<DisplayCommand>,
    /// `(command, element)` for every command an animated element paints, in
    /// command order.
    owned: Vec<(u32, u32)>,
    /// The elements whose style declares a transition, in emission order.
    animated: Vec<AnimatedElement>,
    /// The element the next push belongs to, while layout is inside one that
    /// declares a transition.
    owner: Option<u32>,
}

impl DisplayList {
    /// The commands in painter order.
    #[must_use]
    pub fn commands(&self) -> &[DisplayCommand] {
        &self.commands
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
        for (command, owner) in &self.owned {
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
    pub(super) fn begin_element(
        &mut self,
        id: Option<&str>,
        transition: Option<Transition>,
    ) -> Result<Option<u32>> {
        let previous = self.owner;
        self.owner = match transition {
            Some(transition) => {
                let index = u32::try_from(self.animated.len()).map_err(|_| {
                    limit_error("Animated element count exceeds the display index range")
                })?;
                self.animated
                    .try_reserve(1)
                    .map_err(|_| limit_error("Display element allocation failed"))?;
                self.animated.push(AnimatedElement {
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
    pub(super) fn end_element(&mut self, previous: Option<u32>) {
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
    /// Every command and element is shared with `self`; only the command and
    /// owner storage is new, so a frame requests no memory for the payloads it
    /// repeats.
    ///
    /// # Errors
    /// Returns [`metis_core::error::ErrorCode::LayoutOverflow`] when the
    /// storage cannot be allocated or the counts exceed the addressable range.
    pub fn copy_with_room(&self, extra: usize) -> Result<Self> {
        let capacity =
            self.commands.len().checked_add(extra).ok_or_else(|| {
                limit_error("Display command count exceeds the addressable range")
            })?;
        let mut copy = Self::default();
        copy.commands
            .try_reserve_exact(capacity)
            .map_err(|_| limit_error("Display command allocation failed"))?;
        copy.owned
            .try_reserve_exact(self.owned.len())
            .map_err(|_| limit_error("Display command allocation failed"))?;
        copy.animated
            .try_reserve_exact(self.animated.len())
            .map_err(|_| limit_error("Display element allocation failed"))?;
        copy.commands.extend_from_slice(&self.commands);
        copy.owned.extend_from_slice(&self.owned);
        copy.animated.extend_from_slice(&self.animated);
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
            commands,
            ..Self::default()
        }
    }

    /// Appends a command for the element layout is inside, or for no element
    /// when a host appends one over the layout.
    pub(super) fn push(&mut self, command: DisplayCommand) -> Result<()> {
        let index = self.commands.len();
        self.commands
            .try_reserve(1)
            .map_err(|_| limit_error("Display command allocation failed"))?;
        self.commands.push(command);
        if let Some(owner) = self.owner {
            let index = u32::try_from(index).map_err(|_| {
                limit_error("Display command count exceeds the display index range")
            })?;
            self.owned
                .try_reserve(1)
                .map_err(|_| limit_error("Display command allocation failed"))?;
            self.owned.push((index, owner));
        }
        Ok(())
    }
}

impl iris::render::RenderBackend<DisplayList> for Framebuffer {
    type Error = std::convert::Infallible;
    type Frame<'frame> = &'frame [u32];

    fn render<'frame>(
        &'frame mut self,
        view: &DisplayList,
    ) -> std::result::Result<Self::Frame<'frame>, Self::Error> {
        view.render_to(self);
        Ok(self.pixels())
    }
}
