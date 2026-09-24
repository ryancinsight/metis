//! Bounded width-aware polyline rasterization with explicit caps and joins.

use crate::framebuffer::{Color, Framebuffer, Rect};
use metis_core::error::{ErrorCode, MetisError, Result};
use std::num::NonZeroU32;

/// A positive device-space stroke width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct StrokeWidth(NonZeroU32);

impl StrokeWidth {
    /// Creates a stroke width measured in framebuffer pixels.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorCode::RenderFailure`] when `width` is zero.
    pub fn new(width: u32) -> Result<Self> {
        NonZeroU32::new(width).map(Self).ok_or_else(|| {
            MetisError::ui(
                ErrorCode::RenderFailure,
                "Stroke width must be greater than zero",
            )
        })
    }

    /// Returns the validated width in framebuffer pixels.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl TryFrom<u32> for StrokeWidth {
    type Error = MetisError;

    fn try_from(width: u32) -> Result<Self> {
        Self::new(width)
    }
}

/// Endpoint treatment for an open polyline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LineCap {
    /// Ends at the exact path endpoint.
    Butt,
    /// Extends the path by half the stroke width with square corners.
    Square,
    /// Extends the path by a semicircle with radius half the stroke width.
    Round,
}

/// Vertex treatment for an open polyline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LineJoin {
    /// Extends the outer edges to their intersection, bounded by the miter limit.
    Miter,
    /// Connects outer edges with a straight edge.
    Bevel,
    /// Connects outer edges with a circular arc.
    Round,
}

/// Maximum points retained by one display-list polyline command.
pub const MAX_STROKE_POINTS: usize = 4096;

const MITER_LIMIT: f64 = 4.0;

#[derive(Clone, Copy)]
struct StrokeGeometry {
    radius: f64,
    radius_squared: f64,
    cap: LineCap,
    join: LineJoin,
}

/// Draws a bounded, integer-coordinate polyline with a validated width.
///
/// The renderer evaluates each visible pixel once, so translucent strokes do
/// not become darker where segments or joins overlap. Endpoint caps and vertex
/// joins are selected independently. A one-point path is rendered as a marker
/// using the selected cap; an empty path has no effect. The scan is clipped to
/// the framebuffer and to a width-derived miter bound before any pixel test,
/// so off-screen coordinates cannot amplify work.
pub fn draw_polyline(
    fb: &mut Framebuffer,
    points: &[(i32, i32)],
    width: StrokeWidth,
    cap: LineCap,
    join: LineJoin,
    color: Color,
) {
    let Some((min_x, max_x, min_y, max_y)) = stroke_bounds(fb, points, width, join) else {
        return;
    };
    let radius = f64::from(width.get()) / 2.0;
    let geometry = StrokeGeometry {
        radius,
        radius_squared: radius * radius,
        cap,
        join,
    };
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if polyline_contains(f64::from(x), f64::from(y), points, geometry) {
                fb.blend_pixel(x, y, color);
            }
        }
    }
}

/// The device pixels [`draw_polyline`] can change, or `None` without points.
///
/// # Panics
///
/// Does not panic: each edge is clamped to the `i32` range before conversion.
#[must_use]
pub fn polyline_extent(points: &[(i32, i32)], width: StrokeWidth, join: LineJoin) -> Option<Rect> {
    let (min_x, max_x, min_y, max_y) = unclipped_bounds(points, width, join)?;
    let saturate = |value: i64| {
        i32::try_from(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)))
            .expect("invariant: a value clamped to the i32 range fits i32")
    };
    Some(Rect::new(
        saturate(min_x),
        saturate(min_y),
        saturate(max_x - min_x + 1),
        saturate(max_y - min_y + 1),
    ))
}

/// Inclusive `(min_x, max_x, min_y, max_y)` the stroke can reach.
fn unclipped_bounds(
    points: &[(i32, i32)],
    width: StrokeWidth,
    join: LineJoin,
) -> Option<(i64, i64, i64, i64)> {
    let first = points.first()?;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (
        i64::from(first.0),
        i64::from(first.0),
        i64::from(first.1),
        i64::from(first.1),
    );
    for &(x, y) in &points[1..] {
        min_x = min_x.min(i64::from(x));
        max_x = max_x.max(i64::from(x));
        min_y = min_y.min(i64::from(y));
        max_y = max_y.max(i64::from(y));
    }
    // The miter limit is four radii. The same widened bound covers square and
    // round caps while keeping the integer conversion below in range.
    let expansion = match join {
        LineJoin::Miter => i64::from(width.get()) * 2,
        LineJoin::Bevel | LineJoin::Round => i64::from(width.get()),
    };
    Some((
        min_x.saturating_sub(expansion),
        max_x.saturating_add(expansion),
        min_y.saturating_sub(expansion),
        max_y.saturating_add(expansion),
    ))
}

fn stroke_bounds(
    fb: &Framebuffer,
    points: &[(i32, i32)],
    width: StrokeWidth,
    join: LineJoin,
) -> Option<(i32, i32, i32, i32)> {
    let (min_x, max_x, min_y, max_y) = unclipped_bounds(points, width, join)?;
    let clip = fb.clip();
    let min_x = min_x.max(i64::from(clip.left()));
    let min_y = min_y.max(i64::from(clip.top()));
    let max_x = max_x.min(i64::from(clip.right()) - 1);
    let max_y = max_y.min(i64::from(clip.bottom()) - 1);
    if min_x > max_x || min_y > max_y {
        return None;
    }
    Some((
        i32::try_from(min_x).expect("invariant: framebuffer x bound fits i32"),
        i32::try_from(max_x).expect("invariant: framebuffer x bound fits i32"),
        i32::try_from(min_y).expect("invariant: framebuffer y bound fits i32"),
        i32::try_from(max_y).expect("invariant: framebuffer y bound fits i32"),
    ))
}

fn polyline_contains(x: f64, y: f64, points: &[(i32, i32)], geometry: StrokeGeometry) -> bool {
    if points.len() == 1 {
        return endpoint_contains(
            x,
            y,
            f64::from(points[0].0),
            f64::from(points[0].1),
            (0.0, 0.0),
            geometry,
            true,
        );
    }
    for (index, segment) in points.windows(2).enumerate() {
        let start_cap = index == 0;
        let end_cap = index + 2 == points.len();
        if segment_contains(x, y, segment[0], segment[1], geometry, start_cap, end_cap) {
            return true;
        }
    }
    points
        .windows(3)
        .any(|vertex| join_contains(x, y, vertex[0], vertex[1], vertex[2], geometry))
}

fn segment_contains(
    x: f64,
    y: f64,
    start: (i32, i32),
    end: (i32, i32),
    geometry: StrokeGeometry,
    start_cap: bool,
    end_cap: bool,
) -> bool {
    let start = (f64::from(start.0), f64::from(start.1));
    let end = (f64::from(end.0), f64::from(end.1));
    let direction = (end.0 - start.0, end.1 - start.1);
    let length_squared = direction.0.mul_add(direction.0, direction.1 * direction.1);
    if length_squared == 0.0 {
        return (start_cap || end_cap)
            && endpoint_contains(x, y, start.0, start.1, direction, geometry, start_cap);
    }
    let relative = (x - start.0, y - start.1);
    let projection = relative.0.mul_add(direction.0, relative.1 * direction.1) / length_squared;
    if projection < 0.0 {
        return start_cap && endpoint_contains(x, y, start.0, start.1, direction, geometry, true);
    }
    if projection > 1.0 {
        return end_cap && endpoint_contains(x, y, end.0, end.1, direction, geometry, false);
    }
    let distance_squared = cross(relative, direction).powi(2) / length_squared;
    distance_squared <= geometry.radius_squared
}

fn endpoint_contains(
    x: f64,
    y: f64,
    endpoint_x: f64,
    endpoint_y: f64,
    direction: (f64, f64),
    geometry: StrokeGeometry,
    start: bool,
) -> bool {
    let relative = (x - endpoint_x, y - endpoint_y);
    match geometry.cap {
        LineCap::Butt => {
            direction == (0.0, 0.0) && relative.0.abs() <= 0.5 && relative.1.abs() <= 0.5
        }
        LineCap::Round => {
            relative.0.mul_add(relative.0, relative.1 * relative.1) <= geometry.radius_squared
        }
        LineCap::Square => {
            let length = direction.0.hypot(direction.1);
            if length == 0.0 {
                return relative.0.abs() <= geometry.radius && relative.1.abs() <= geometry.radius;
            }
            let unit = (direction.0 / length, direction.1 / length);
            let longitudinal = relative.0.mul_add(unit.0, relative.1 * unit.1);
            let outward = if start { -longitudinal } else { longitudinal };
            let perpendicular = relative.0.mul_add(-unit.1, relative.1 * unit.0).abs();
            outward >= 0.0 && outward <= geometry.radius && perpendicular <= geometry.radius
        }
    }
}

fn join_contains(
    x: f64,
    y: f64,
    previous: (i32, i32),
    vertex: (i32, i32),
    next: (i32, i32),
    geometry: StrokeGeometry,
) -> bool {
    let vertex = (f64::from(vertex.0), f64::from(vertex.1));
    let incoming = (
        vertex.0 - f64::from(previous.0),
        vertex.1 - f64::from(previous.1),
    );
    let outgoing = (f64::from(next.0) - vertex.0, f64::from(next.1) - vertex.1);
    let incoming_length = incoming.0.hypot(incoming.1);
    let outgoing_length = outgoing.0.hypot(outgoing.1);
    if incoming_length == 0.0 || outgoing_length == 0.0 {
        return false;
    }
    let incoming = (incoming.0 / incoming_length, incoming.1 / incoming_length);
    let outgoing = (outgoing.0 / outgoing_length, outgoing.1 / outgoing_length);
    let turn = cross(incoming, outgoing);
    if turn == 0.0 {
        return false;
    }
    if matches!(geometry.join, LineJoin::Round) {
        let delta = (x - vertex.0, y - vertex.1);
        return delta.0.mul_add(delta.0, delta.1 * delta.1) <= geometry.radius_squared;
    }
    let side = if turn > 0.0 { -1.0 } else { 1.0 };
    let first = (
        vertex.0 - incoming.1 * geometry.radius * side,
        vertex.1 + incoming.0 * geometry.radius * side,
    );
    let second = (
        vertex.0 - outgoing.1 * geometry.radius * side,
        vertex.1 + outgoing.0 * geometry.radius * side,
    );
    let miter = line_intersection(first, incoming, second, outgoing);
    let use_miter = matches!(geometry.join, LineJoin::Miter)
        && miter.is_some_and(|point| {
            distance_squared(point, vertex) <= geometry.radius_squared * MITER_LIMIT * MITER_LIMIT
        });
    let corner = if use_miter {
        miter.expect("invariant: checked miter intersection")
    } else {
        vertex
    };
    triangle_contains((x, y), first, corner, second)
}

fn line_intersection(
    first: (f64, f64),
    first_direction: (f64, f64),
    second: (f64, f64),
    second_direction: (f64, f64),
) -> Option<(f64, f64)> {
    let denominator = cross(first_direction, second_direction);
    if denominator == 0.0 {
        return None;
    }
    let offset = (second.0 - first.0, second.1 - first.1);
    let distance = cross(offset, second_direction) / denominator;
    Some((
        first.0 + first_direction.0 * distance,
        first.1 + first_direction.1 * distance,
    ))
}

fn triangle_contains(
    point: (f64, f64),
    first: (f64, f64),
    second: (f64, f64),
    third: (f64, f64),
) -> bool {
    let first_turn = cross(
        (second.0 - first.0, second.1 - first.1),
        (point.0 - first.0, point.1 - first.1),
    );
    let second_turn = cross(
        (third.0 - second.0, third.1 - second.1),
        (point.0 - second.0, point.1 - second.1),
    );
    let third_turn = cross(
        (first.0 - third.0, first.1 - third.1),
        (point.0 - third.0, point.1 - third.1),
    );
    (first_turn >= 0.0 && second_turn >= 0.0 && third_turn >= 0.0)
        || (first_turn <= 0.0 && second_turn <= 0.0 && third_turn <= 0.0)
}

fn distance_squared(first: (f64, f64), second: (f64, f64)) -> f64 {
    let delta = (first.0 - second.0, first.1 - second.1);
    delta.0.mul_add(delta.0, delta.1 * delta.1)
}

fn cross(first: (f64, f64), second: (f64, f64)) -> f64 {
    first.0.mul_add(second.1, -(first.1 * second.0))
}

#[cfg(test)]
#[path = "stroke_tests.rs"]
mod tests;
