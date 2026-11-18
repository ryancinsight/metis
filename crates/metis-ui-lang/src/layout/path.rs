//! Bounded pointer-path capture and screen-space picking.

use metis_platform::rasterizer::MAX_STROKE_POINTS;
mod picking;

pub use picking::{
    PathTopology, PolylineError, SegmentHit, VertexHit, nearest_polyline_segment,
    nearest_polyline_vertex, polyline_distance,
};

/// Computes the two screen-space wing endpoints for a directional arrow.
///
/// The returned values are the two wing endpoints; callers draw each segment
/// from `tip` to one endpoint. `depth` and `half_width` use the same
/// units as the points. Depth is the distance from the tip back along the
/// shaft. The width is twice the perpendicular offset from its centerline.
/// Returns `None` for coincident points, non-positive dimensions, or
/// non-finite arithmetic. Returns `None` when rounding collapses the wing
/// separation or the head depth.
///
/// ```
/// use metis_ui_lang::arrow_wings;
/// let wings = arrow_wings([0.0, 0.0], [10.0, 0.0], 4.0, 2.0)
///     .expect("a finite non-zero arrow is renderable");
/// assert_eq!(wings, [[6.0, -2.0], [6.0, 2.0]]);
/// ```
#[must_use]
pub fn arrow_wings(
    tail: [f64; 2],
    tip: [f64; 2],
    depth: f64,
    half_width: f64,
) -> Option<[[f64; 2]; 2]> {
    if !tail.into_iter().chain(tip).all(f64::is_finite)
        || !depth.is_finite()
        || !half_width.is_finite()
        || depth <= 0.0
        || half_width <= 0.0
    {
        return None;
    }
    let direction = [tip[0] - tail[0], tip[1] - tail[1]];
    let magnitude = direction[0].abs().max(direction[1].abs());
    if !magnitude.is_finite() || magnitude == 0.0 {
        return None;
    }
    // At least one scaled component has unit magnitude. The norm cannot
    // underflow for subnormal directions or overflow for finite directions.
    let [x, y] = direction.map(|value| value / magnitude);
    let norm = x.hypot(y);
    let unit = [x / norm, y / norm];
    let base = [tip[0] - unit[0] * depth, tip[1] - unit[1] * depth];
    let offset = [unit[1] * half_width, -unit[0] * half_width];
    let first = [base[0] + offset[0], base[1] + offset[1]];
    let second = [base[0] - offset[0], base[1] - offset[1]];
    let separated = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()) > 0.0;
    ([base, first, second]
        .into_iter()
        .flatten()
        .all(f64::is_finite)
        && separated(base, tip)
        && separated(first, second))
    .then_some([first, second])
}

/// The outcome of appending a point to a captured path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathPush {
    /// A point different from the previous sample was retained.
    Added,
    /// The point matched the previous sample and was ignored.
    Duplicate,
    /// The path already contains [`MAX_STROKE_POINTS`] samples.
    Full,
}

/// A bounded ordered path of caller-owned coordinate values.
///
/// The capture stores the initial point and retains later points only when
/// they differ from the preceding point. It owns the samples after capture,
/// so a host can release its pointer-event storage immediately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCapture<T> {
    points: Vec<T>,
}

impl<T> PathCapture<T> {
    /// Starts a path with one sample.
    ///
    /// ```
    /// use metis_ui_lang::{PathCapture, PathPush};
    ///
    /// let mut path = PathCapture::new([0, 0]);
    /// assert_eq!(path.push([0, 0]), PathPush::Duplicate);
    /// assert_eq!(path.push([1, 0]), PathPush::Added);
    /// assert_eq!(path.into_vec(), vec![[0, 0], [1, 0]]);
    /// ```
    #[must_use]
    pub fn new(start: T) -> Self {
        Self {
            points: vec![start],
        }
    }

    /// Appends a distinct point up to the renderer's vertex budget.
    ///
    /// A repeated point returns [`PathPush::Duplicate`] even at capacity.
    /// A distinct point at capacity returns [`PathPush::Full`]. Both leave
    /// the captured path unchanged.
    #[must_use]
    pub fn push(&mut self, point: T) -> PathPush
    where
        T: PartialEq,
    {
        if self.points.last() == Some(&point) {
            return PathPush::Duplicate;
        }
        if self.points.len() == MAX_STROKE_POINTS {
            return PathPush::Full;
        }
        self.points.push(point);
        PathPush::Added
    }

    /// Borrows the captured samples in input order.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.points
    }

    /// Consumes the capture and returns its samples.
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.points
    }
}

#[cfg(test)]
mod tests {
    use super::{PathCapture, PathPush, arrow_wings};
    use metis_platform::rasterizer::MAX_STROKE_POINTS;

    #[test]
    fn capture_retains_start_and_skips_duplicate_samples() {
        let mut capture = PathCapture::new((1_i32, 2_i32));
        assert_eq!(capture.push((1, 2)), PathPush::Duplicate);
        assert_eq!(capture.push((3, 4)), PathPush::Added);
        assert_eq!(capture.as_slice(), &[(1, 2), (3, 4)]);
    }

    #[test]
    fn capture_rejects_samples_after_the_rendering_bound() {
        let mut capture = PathCapture::new(0_u32);
        for point in 1..MAX_STROKE_POINTS {
            let point = u32::try_from(point).expect("invariant: path bound fits u32");
            assert_eq!(capture.push(point), PathPush::Added);
        }
        let point = u32::try_from(MAX_STROKE_POINTS).expect("invariant: path bound fits u32");
        assert_eq!(capture.push(point - 1), PathPush::Duplicate);
        assert_eq!(capture.push(point), PathPush::Full);
        assert_eq!(capture.as_slice().len(), MAX_STROKE_POINTS);
    }

    #[test]
    fn arrow_wings_follow_horizontal_reversed_and_rotated_directions() {
        assert_eq!(
            arrow_wings([0.0, 0.0], [10.0, 0.0], 4.0, 2.0),
            Some([[6.0, -2.0], [6.0, 2.0]])
        );
        assert_eq!(
            arrow_wings([10.0, 0.0], [0.0, 0.0], 4.0, 2.0),
            Some([[4.0, 2.0], [4.0, -2.0]])
        );
        assert_eq!(
            arrow_wings([0.0, 0.0], [0.0, 10.0], 4.0, 2.0),
            Some([[2.0, 6.0], [-2.0, 6.0]])
        );
    }

    #[test]
    fn arrow_wings_normalize_subnormal_diagonals() {
        let tiny = f64::from_bits(1);
        let unit = 1.0 / 2.0_f64.sqrt();
        // A unit diagonal has equal components. Each wing adds these
        // components on one coordinate and cancels them on the other.
        assert_eq!(
            arrow_wings([-tiny, -tiny], [0.0, 0.0], 1.0, 1.0),
            Some([[0.0, -2.0 * unit], [-2.0 * unit, 0.0]])
        );
    }

    #[test]
    fn arrow_wings_reject_non_renderable_geometry() {
        assert_eq!(arrow_wings([1.0, 1.0], [1.0, 1.0], 4.0, 2.0), None);
        assert_eq!(arrow_wings([0.0, 0.0], [1.0, 1.0], f64::NAN, 2.0), None);
        assert_eq!(
            arrow_wings([0.0, 0.0], [1.0, f64::INFINITY], 4.0, 2.0),
            None
        );
        assert_eq!(arrow_wings([0.0, 0.0], [1.0, 1.0], 0.0, 2.0), None);
        // Adjacent representable coordinates below 2^54 differ by 2.
        let boundary = 2.0_f64.powi(54);
        assert_eq!(arrow_wings([0.0, 0.0], [boundary, 0.0], 1.0, 2.0), None);
    }
}
