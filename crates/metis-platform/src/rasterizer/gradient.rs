//! Linear gradient paint after CSS Images 3, sections 3.1 and 3.4.
//!
//! A gradient is a direction and a list of color stops along the gradient
//! line. The line passes through the center of the box it paints, points in
//! the gradient's direction, and is long enough that its perpendiculars
//! through the ends touch opposite corners: `abs(W sin A) + abs(H cos A)` for a
//! `W` by `H` box and angle `A`. Each pixel takes the color at the projection
//! of its center onto that line.

use super::paint::Paint;
use crate::framebuffer::{Color, Framebuffer, SourceOver};

/// Most color stops one gradient carries.
///
/// Stop lookup is a linear scan per pixel, so the bound keeps it a handful of
/// comparisons.
pub const MAX_GRADIENT_STOPS: usize = 8;

/// A color stop as authored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    /// Straight RGBA color at the stop.
    pub color: Color,
    /// Position along the gradient line as a fraction of its length, or
    /// `None` to place the stop by the section 3.4.3 fixup.
    pub position: Option<f64>,
}

/// A stop after fixup, holding its color premultiplied by alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedStop {
    position: f64,
    /// Red, green and blue scaled by alpha, then alpha; each in `[0, 255]`.
    premultiplied: [f64; 4],
}

/// A validated linear gradient: a direction and between two and
/// [`MAX_GRADIENT_STOPS`] stops in nondecreasing position order.
///
/// # Examples
///
/// ```
/// use metis_platform::framebuffer::Color;
/// use metis_platform::rasterizer::{GradientStop, LinearGradient};
///
/// let stops = [
///     GradientStop { color: Color::WHITE, position: None },
///     GradientStop { color: Color::BLACK, position: None },
/// ];
/// // `to bottom`: white at the top edge, black at the bottom.
/// let gradient = LinearGradient::new(180.0, &stops).expect("two finite stops");
/// assert_eq!(gradient.color_at_fraction(0.0), Color::WHITE);
/// assert_eq!(gradient.color_at_fraction(1.0), Color::BLACK);
/// assert!(LinearGradient::new(180.0, &stops[..1]).is_none());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    /// Unit direction of the gradient line in device coordinates, whose
    /// vertical axis points down: `(sin A, -cos A)`.
    direction: (f64, f64),
    /// The first stop, held apart so a gradient cannot be empty.
    first: ResolvedStop,
    /// The remaining stops in nondecreasing position order; at least one.
    rest: Box<[ResolvedStop]>,
}

impl LinearGradient {
    /// Validates a gradient pointing at `degrees`, where 0 points up and
    /// angles increase clockwise, and resolves its stop positions.
    ///
    /// Returns `None` when the angle or a position is not finite, or when
    /// there are fewer than two or more than [`MAX_GRADIENT_STOPS`] stops.
    #[must_use]
    pub fn new(degrees: f64, stops: &[GradientStop]) -> Option<Self> {
        if !degrees.is_finite()
            || !(2..=MAX_GRADIENT_STOPS).contains(&stops.len())
            || stops
                .iter()
                .any(|stop| stop.position.is_some_and(|position| !position.is_finite()))
        {
            return None;
        }
        let mut resolved = resolve(stops).into_iter();
        let first = resolved.next()?;
        Some(Self {
            direction: direction(degrees),
            first,
            rest: resolved.collect(),
        })
    }

    /// Straight color at `fraction` of the gradient line, measured from its
    /// start.
    ///
    /// Before the first stop the color is the first stop's and after the last
    /// it is the last stop's; between stops the premultiplied channels
    /// interpolate linearly (section 3.4.2).
    #[must_use]
    pub fn color_at_fraction(&self, fraction: f64) -> Color {
        let mut from = self.first;
        if fraction <= from.position {
            return straight(from.premultiplied);
        }
        for to in &self.rest {
            // Earlier stops returned for any fraction below their position,
            // so `from.position <= fraction < to.position` and the span is
            // positive.
            if fraction < to.position {
                let weight = (fraction - from.position) / (to.position - from.position);
                let mut channels = [0.0; 4];
                for ((channel, start), end) in channels
                    .iter_mut()
                    .zip(from.premultiplied)
                    .zip(to.premultiplied)
                {
                    *channel = (end - start).mul_add(weight, start);
                }
                return straight(channels);
            }
            from = *to;
        }
        straight(from.premultiplied)
    }

    fn stops(&self) -> impl Iterator<Item = &ResolvedStop> {
        std::iter::once(&self.first).chain(&self.rest)
    }

    /// Whether every stop is fully opaque, so every pixel of the gradient is.
    fn is_opaque(&self) -> bool {
        self.stops().all(|stop| stop.premultiplied[3] >= 255.0)
    }

    /// Whether every stop is fully transparent.
    fn is_transparent(&self) -> bool {
        self.stops().all(|stop| stop.premultiplied[3] <= 0.0)
    }
}

/// Unit direction for a CSS angle, exact on the four axis angles so a
/// vertical or horizontal gradient varies along one axis only.
fn direction(degrees: f64) -> (f64, f64) {
    const AXES: [(f64, (f64, f64)); 4] = [
        (0.0, (0.0, -1.0)),
        (90.0, (1.0, 0.0)),
        (180.0, (0.0, 1.0)),
        (270.0, (-1.0, 0.0)),
    ];
    let turn = degrees.rem_euclid(360.0);
    #[expect(
        clippy::float_cmp,
        reason = "only an exact axis angle selects an exact unit vector"
    )]
    let axis = AXES.iter().find(|(angle, _)| turn == *angle);
    axis.map_or_else(
        || {
            let (sine, cosine) = turn.to_radians().sin_cos();
            (sine, -cosine)
        },
        |(_, unit)| *unit,
    )
}

/// Applies the section 3.4.3 fixup and premultiplies each color.
///
/// A missing first position becomes 0 and a missing last position 1; a
/// position below an earlier one rises to the largest earlier position; each
/// run of unpositioned stops spreads evenly between its positioned
/// neighbours.
fn resolve(stops: &[GradientStop]) -> Vec<ResolvedStop> {
    let last = stops.len() - 1;
    let mut positions: Vec<Option<f64>> = stops.iter().map(|stop| stop.position).collect();
    positions[0] = positions[0].or(Some(0.0));
    positions[last] = positions[last].or(Some(1.0));
    let mut largest = f64::NEG_INFINITY;
    for position in positions.iter_mut().flatten() {
        largest = largest.max(*position);
        *position = largest;
    }
    let stop_count =
        |count: usize| u32::try_from(count).expect("invariant: a gradient has at most eight stops");
    let mut index = 0;
    while index < last {
        let start = positions[index].expect("invariant: every run starts after a positioned stop");
        let end = (index + 1..=last)
            .find(|candidate| positions[*candidate].is_some())
            .expect("invariant: the last stop is positioned");
        let high = positions[end].expect("invariant: the search found a positioned stop");
        let span = f64::from(stop_count(end - index));
        for (step, position) in positions[index + 1..end].iter_mut().enumerate() {
            let share = f64::from(stop_count(step + 1)) / span;
            *position = Some((high - start).mul_add(share, start));
        }
        index = end;
    }
    stops
        .iter()
        .zip(positions)
        .map(|(stop, position)| ResolvedStop {
            position: position.expect("invariant: the fixup positions every stop"),
            premultiplied: premultiply(stop.color),
        })
        .collect()
}

fn premultiply(color: Color) -> [f64; 4] {
    let alpha = f64::from(color.a);
    let scale = alpha / 255.0;
    [
        f64::from(color.r) * scale,
        f64::from(color.g) * scale,
        f64::from(color.b) * scale,
        alpha,
    ]
}

/// Rounds premultiplied channels back to a straight byte color.
fn straight(channels: [f64; 4]) -> Color {
    let [red, green, blue, alpha] = channels;
    if alpha <= 0.0 {
        return Color::TRANSPARENT;
    }
    let unscale = 255.0 / alpha;
    // Interpolation between byte-valued stops keeps every channel in
    // [0, 255]; the clamp absorbs rounding at the ends.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a channel clamped to [0, 255] rounds to a byte"
    )]
    let byte = |value: f64| value.round().clamp(0.0, 255.0) as u8;
    Color::rgba(
        byte(red * unscale),
        byte(green * unscale),
        byte(blue * unscale),
        byte(alpha),
    )
}

/// A gradient placed over one box: the fraction along the gradient line is
/// affine in the pixel center.
#[derive(Debug, Clone, Copy)]
pub(super) struct PlacedGradient<'gradient> {
    gradient: &'gradient LinearGradient,
    /// Fraction at the origin pixel center `(0.5, 0.5)`.
    origin: f64,
    /// Change in fraction per column and per row.
    step: (f64, f64),
    /// Whether the gradient line is vertical, so each row holds one color.
    row_uniform: bool,
    opaque: bool,
}

impl<'gradient> PlacedGradient<'gradient> {
    /// Places `gradient` over the box with top-left corner `(left, top)` and
    /// positive extent `width` by `height`.
    pub(super) fn new(
        gradient: &'gradient LinearGradient,
        (left, top): (f64, f64),
        (width, height): (f64, f64),
    ) -> Self {
        let (along_x, along_y) = gradient.direction;
        // Positive for a nonempty box: one of the unit components is nonzero.
        let length = (width * along_x).abs() + (height * along_y).abs();
        let step = (along_x / length, along_y / length);
        let center = (width.mul_add(0.5, left), height.mul_add(0.5, top));
        let origin = step
            .1
            .mul_add(0.5 - center.1, step.0.mul_add(0.5 - center.0, 0.5));
        // The axis angles carry an exact zero component.
        let row_uniform = along_x == 0.0;
        Self {
            gradient,
            origin,
            step,
            row_uniform,
            opaque: gradient.is_opaque(),
        }
    }

    fn fraction(&self, column: u32, row: u32) -> f64 {
        self.step.1.mul_add(
            f64::from(row),
            self.step.0.mul_add(f64::from(column), self.origin),
        )
    }
}

impl Paint for PlacedGradient<'_> {
    fn is_transparent(&self) -> bool {
        self.gradient.is_transparent()
    }

    fn fill_run(&self, fb: &mut Framebuffer, row: u32, left: u32, right: u32) {
        if self.row_uniform {
            self.color_at(left, row).fill_run(fb, row, left, right);
            return;
        }
        if self.opaque {
            for (column, pixel) in (left..right).zip(fb.row_span_mut(row, left, right)) {
                *pixel = SourceOver::new(self.color_at(column, row)).packed();
            }
            return;
        }
        for column in left..right {
            self.color_at(column, row)
                .fill_run(fb, row, column, column + 1);
        }
    }

    fn color_at(&self, column: u32, row: u32) -> Color {
        self.gradient.color_at_fraction(self.fraction(column, row))
    }
}

#[cfg(test)]
#[path = "gradient_tests.rs"]
mod tests;
