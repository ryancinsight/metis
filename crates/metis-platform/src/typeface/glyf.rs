//! Glyph outlines from the `glyf` table, emitted in device space.
//!
//! A simple glyph stores contours of on-curve and off-curve points; two
//! consecutive off-curve points imply an on-curve point midway between them,
//! and every off-curve point is the control of a quadratic Bézier. A
//! composite glyph places other glyphs through an affine transform.

use super::raster::Outline;
use super::reader::{Reader, offset};
use super::{GlyphId, Typeface, TypefaceError};

/// Composite nesting depth past which a glyph is rejected; real fonts nest a
/// few levels, and the bound keeps a cyclic reference from recursing forever.
const MAX_COMPOSITE_DEPTH: u8 = 8;

/// Components one outline may place in total. Depth alone does not bound the
/// work: a component list at every level multiplies, so a hostile font could
/// demand exponentially many placements. Accented letters place two or three.
const MAX_COMPONENTS: u16 = 64;

const ON_CURVE: u8 = 0x01;
const X_SHORT: u8 = 0x02;
const Y_SHORT: u8 = 0x04;
const REPEAT: u8 = 0x08;
const X_SAME_OR_POSITIVE: u8 = 0x10;
const Y_SAME_OR_POSITIVE: u8 = 0x20;

const ARGS_ARE_WORDS: u16 = 0x0001;
const ARGS_ARE_XY_VALUES: u16 = 0x0002;
const HAVE_A_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const HAVE_X_AND_Y_SCALE: u16 = 0x0040;
const HAVE_TWO_BY_TWO: u16 = 0x0080;
const SCALED_COMPONENT_OFFSET: u16 = 0x0800;

/// A point in device space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Point {
    pub(super) x: f64,
    pub(super) y: f64,
}

impl Point {
    fn midpoint(self, other: Self) -> Self {
        Self {
            x: f64::midpoint(self.x, other.x),
            y: f64::midpoint(self.y, other.y),
        }
    }
}

/// Affine map `(x, y) -> (xx·x + xy·y + dx, yx·x + yy·y + dy)`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Transform {
    xx: f64,
    xy: f64,
    yx: f64,
    yy: f64,
    dx: f64,
    dy: f64,
}

impl Transform {
    /// Font units to device pixels: `scale` pixels per unit, y flipped so the
    /// font's upward axis points down the surface, origin on the baseline.
    pub(super) const fn device(scale: f64, origin_x: f64, baseline: f64) -> Self {
        Self {
            xx: scale,
            xy: 0.0,
            yx: 0.0,
            yy: -scale,
            dx: origin_x,
            dy: baseline,
        }
    }

    fn apply(&self, x: f64, y: f64) -> Point {
        Point {
            x: self.xx.mul_add(x, self.xy * y) + self.dx,
            y: self.yx.mul_add(x, self.yy * y) + self.dy,
        }
    }

    /// This transform applied after `inner`.
    fn after(&self, inner: &Self) -> Self {
        Self {
            xx: self.xx.mul_add(inner.xx, self.xy * inner.yx),
            xy: self.xx.mul_add(inner.xy, self.xy * inner.yy),
            yx: self.yx.mul_add(inner.xx, self.yy * inner.yx),
            yy: self.yx.mul_add(inner.xy, self.yy * inner.yy),
            dx: self.xx.mul_add(inner.dx, self.xy * inner.dy) + self.dx,
            dy: self.yx.mul_add(inner.dx, self.yy * inner.dy) + self.dy,
        }
    }
}

impl Typeface<'_> {
    /// Emits the outline of `glyph` through `transform` into `outline`.
    pub(super) fn outline(
        &self,
        glyph: GlyphId,
        transform: &Transform,
        outline: &mut Outline,
    ) -> Result<(), TypefaceError> {
        let mut budget = MAX_COMPONENTS;
        self.outline_at_depth(glyph, transform, outline, 0, &mut budget)?;
        if outline.exceeded() {
            return Err(TypefaceError::Invalid {
                table: "glyf",
                field: "outline size",
            });
        }
        Ok(())
    }

    fn outline_at_depth(
        &self,
        glyph: GlyphId,
        transform: &Transform,
        outline: &mut Outline,
        depth: u8,
        budget: &mut u16,
    ) -> Result<(), TypefaceError> {
        if depth > MAX_COMPOSITE_DEPTH {
            return Err(TypefaceError::Invalid {
                table: "glyf",
                field: "component depth",
            });
        }
        let (start, end) = self.outline_range(glyph)?;
        if start == end {
            // No contours: a space or other blank glyph.
            return Ok(());
        }
        let data = self.glyf.sub(start, end - start, "glyf")?;
        let contours = data.read::<i16>(0)?;
        match usize::try_from(contours) {
            Ok(contours) => simple(data, contours, transform, outline),
            Err(_) => self.composite(data, transform, outline, depth, budget),
        }
    }

    fn composite(
        &self,
        data: Reader<'_>,
        transform: &Transform,
        outline: &mut Outline,
        depth: u8,
        budget: &mut u16,
    ) -> Result<(), TypefaceError> {
        let mut cursor = 10;
        loop {
            *budget = budget.checked_sub(1).ok_or(TypefaceError::Invalid {
                table: "glyf",
                field: "component count",
            })?;
            let flags = data.read::<u16>(cursor)?;
            let component = GlyphId(data.read::<u16>(cursor + 2)?);
            cursor += 4;
            if flags & ARGS_ARE_XY_VALUES == 0 {
                // Point-matching placement needs hinted point positions.
                return Err(TypefaceError::Invalid {
                    table: "glyf",
                    field: "point-matched component",
                });
            }
            let (dx, dy) = if flags & ARGS_ARE_WORDS == 0 {
                let x = i8::from_be_bytes([data.read::<u8>(cursor)?]);
                let y = i8::from_be_bytes([data.read::<u8>(cursor + 1)?]);
                cursor += 2;
                (f64::from(x), f64::from(y))
            } else {
                let x = data.read::<i16>(cursor)?;
                let y = data.read::<i16>(cursor + 2)?;
                cursor += 4;
                (f64::from(x), f64::from(y))
            };
            let fixed = |at: usize| -> Result<f64, TypefaceError> {
                // F2DOT14: a signed 2.14 fixed-point number.
                Ok(f64::from(data.read::<i16>(at)?) / 16_384.0)
            };
            let (xx, xy, yx, yy) = if flags & HAVE_A_SCALE != 0 {
                let scale = fixed(cursor)?;
                cursor += 2;
                (scale, 0.0, 0.0, scale)
            } else if flags & HAVE_X_AND_Y_SCALE != 0 {
                let pair = (fixed(cursor)?, fixed(cursor + 2)?);
                cursor += 4;
                (pair.0, 0.0, 0.0, pair.1)
            } else if flags & HAVE_TWO_BY_TWO != 0 {
                // Stored as xscale, scale01, scale10, yscale: the first two
                // are the images of the x axis.
                let matrix = (
                    fixed(cursor)?,
                    fixed(cursor + 2)?,
                    fixed(cursor + 4)?,
                    fixed(cursor + 6)?,
                );
                cursor += 8;
                (matrix.0, matrix.2, matrix.1, matrix.3)
            } else {
                (1.0, 0.0, 0.0, 1.0)
            };
            let (dx, dy) = if flags & SCALED_COMPONENT_OFFSET == 0 {
                (dx, dy)
            } else {
                (xx.mul_add(dx, xy * dy), yx.mul_add(dx, yy * dy))
            };
            let placement = Transform {
                xx,
                xy,
                yx,
                yy,
                dx,
                dy,
            };
            self.outline_at_depth(
                component,
                &transform.after(&placement),
                outline,
                depth + 1,
                budget,
            )?;
            if flags & MORE_COMPONENTS == 0 {
                return Ok(());
            }
        }
    }
}

/// Decodes a simple glyph of `contours` contours.
///
/// Flags and coordinates decode into the outline's reusable scratch, which is
/// returned to it whether or not decoding succeeds.
fn simple(
    data: Reader<'_>,
    contours: usize,
    transform: &Transform,
    outline: &mut Outline,
) -> Result<(), TypefaceError> {
    outline.begin_component();
    let mut flags = std::mem::take(&mut outline.flags);
    let mut coordinates = std::mem::take(&mut outline.coordinates);
    let result = decode_simple(
        data,
        contours,
        transform,
        &mut flags,
        &mut coordinates,
        outline,
    );
    outline.flags = flags;
    outline.coordinates = coordinates;
    result
}

fn decode_simple(
    data: Reader<'_>,
    contours: usize,
    transform: &Transform,
    flags: &mut Vec<u8>,
    coordinates: &mut Vec<(i32, i32)>,
    outline: &mut Outline,
) -> Result<(), TypefaceError> {
    if contours == 0 {
        return Ok(());
    }
    let invalid = |field| TypefaceError::Invalid {
        table: "glyf",
        field,
    };
    let points = offset(data.read::<u16>(10 + 2 * (contours - 1))?) + 1;
    let instructions = offset(data.read::<u16>(10 + 2 * contours)?);
    let mut cursor = 12 + 2 * contours + instructions;

    flags.clear();
    while flags.len() < points {
        let flag = data.read::<u8>(cursor)?;
        cursor += 1;
        flags.push(flag);
        if flag & REPEAT != 0 {
            let repeats = offset(data.read::<u8>(cursor)?);
            cursor += 1;
            if flags.len() + repeats > points {
                return Err(invalid("flag repeat"));
            }
            flags.extend(std::iter::repeat_n(flag, repeats));
        }
    }
    coordinates.clear();
    coordinates.resize(points, (0, 0));
    decode_axis(
        data,
        &mut cursor,
        flags,
        X_SHORT,
        X_SAME_OR_POSITIVE,
        |index, x| {
            coordinates[index].0 = x;
        },
    )?;
    decode_axis(
        data,
        &mut cursor,
        flags,
        Y_SHORT,
        Y_SAME_OR_POSITIVE,
        |index, y| {
            coordinates[index].1 = y;
        },
    )?;

    let mut first = 0;
    for contour in 0..contours {
        let end = offset(data.read::<u16>(10 + 2 * contour)?);
        if end < first || end >= points {
            return Err(invalid("endPtsOfContours"));
        }
        let contour_points = (first..=end).map(|index| {
            let (x, y) = coordinates[index];
            (
                transform.apply(f64::from(x), f64::from(y)),
                flags[index] & ON_CURVE != 0,
            )
        });
        emit_contour(contour_points, outline);
        first = end + 1;
    }
    Ok(())
}

/// Decodes one axis of delta-encoded coordinates, storing absolute values.
fn decode_axis(
    data: Reader<'_>,
    cursor: &mut usize,
    flags: &[u8],
    short: u8,
    same_or_positive: u8,
    mut store: impl FnMut(usize, i32),
) -> Result<(), TypefaceError> {
    let mut value = 0_i32;
    for (index, flag) in flags.iter().enumerate() {
        let delta = if flag & short != 0 {
            let magnitude = i32::from(data.read::<u8>(*cursor)?);
            *cursor += 1;
            if flag & same_or_positive == 0 {
                -magnitude
            } else {
                magnitude
            }
        } else if flag & same_or_positive != 0 {
            0
        } else {
            let delta = i32::from(data.read::<i16>(*cursor)?);
            *cursor += 2;
            delta
        };
        // Sixteen-bit deltas over at most 65,536 points stay inside i32.
        value += delta;
        store(index, value);
    }
    Ok(())
}

/// Emits one closed contour of on- and off-curve points.
fn emit_contour(points: impl Iterator<Item = (Point, bool)> + Clone, outline: &mut Outline) {
    let Some((first, first_on)) = points.clone().next() else {
        return;
    };
    let last = points.clone().last().unwrap_or((first, first_on));
    // The contour starts on-curve: at the first point, else the last, else
    // the midpoint the two off-curve ends imply.
    let (start, skip_first) = if first_on {
        (first, true)
    } else if last.1 {
        (last.0, false)
    } else {
        (first.midpoint(last.0), false)
    };
    let walk_length = if first_on || !last.1 {
        usize::MAX
    } else {
        // Starting at the last point, it must not be walked again.
        points.clone().count() - 1
    };
    let mut current = start;
    let mut control: Option<Point> = None;
    for (point, on_curve) in points.skip(usize::from(skip_first)).take(walk_length) {
        match (on_curve, control) {
            (true, Some(held)) => {
                outline.quadratic(current, held, point);
                current = point;
                control = None;
            }
            (true, None) => {
                outline.line(current, point);
                current = point;
            }
            (false, Some(held)) => {
                let implied = held.midpoint(point);
                outline.quadratic(current, held, implied);
                current = implied;
                control = Some(point);
            }
            (false, None) => control = Some(point),
        }
    }
    match control {
        Some(held) => outline.quadratic(current, held, start),
        None => outline.line(current, start),
    }
}
