//! Bounded pointer-path capture and screen-space polyline hit geometry.

use metis_platform::rasterizer::MAX_STROKE_POINTS;
use std::fmt;

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

/// The topology used when measuring a polyline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathTopology {
    /// The final sample is not connected to the initial sample.
    Open,
    /// The final sample is connected to the initial sample.
    Closed,
}

/// Errors returned by [`polyline_distance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PolylineError {
    /// The input contained fewer than two points.
    TooFewPoints,
    /// A point contained a NaN or infinity.
    NonFiniteCoordinate,
    /// Finite input produced an unrepresentable distance.
    NonFiniteDistance,
}

impl fmt::Display for PolylineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooFewPoints => "a polyline needs at least two points",
            Self::NonFiniteCoordinate => "a polyline or target coordinate is not finite",
            Self::NonFiniteDistance => {
                "the segment geometry exceeds representable distance arithmetic"
            }
        })
    }
}

impl std::error::Error for PolylineError {}

/// Returns the distance from a point to an open or closed polyline.
///
/// Coordinates are screen-space pixels. The iterator is consumed once, so a
/// caller can project a path without allocating a second coordinate buffer.
/// The closest point on each segment is clamped to that segment's endpoints.
/// A closed path also tests its final-to-initial segment.
///
/// # Errors
///
/// Returns [`PolylineError::TooFewPoints`] for fewer than two points,
/// [`PolylineError::NonFiniteCoordinate`] for NaN or infinite input, and
/// [`PolylineError::NonFiniteDistance`] when a segment displacement, projection
/// or distance exceeds the finite `f64` range.
///
/// # Examples
///
/// ```
/// use metis_ui_lang::{polyline_distance, PathTopology};
/// let distance = polyline_distance(
///     [[0.0, 0.0], [4.0, 0.0]],
///     [2.0, 3.0],
///     PathTopology::Open,
/// )?;
/// assert_eq!(distance, 3.0);
/// # Ok::<(), metis_ui_lang::PolylineError>(())
/// ```
pub fn polyline_distance(
    points: impl IntoIterator<Item = [f64; 2]>,
    target: [f64; 2],
    topology: PathTopology,
) -> Result<f64, PolylineError> {
    let mut points = points.into_iter();
    let first = finite_point(points.next().ok_or(PolylineError::TooFewPoints)?)?;
    let mut previous = finite_point(points.next().ok_or(PolylineError::TooFewPoints)?)?;
    let target = finite_point(target)?;
    let mut nearest = segment_distance(first, previous, target)?;
    for current in points {
        let current = finite_point(current)?;
        nearest = nearest.min(segment_distance(previous, current, target)?);
        previous = current;
    }
    if topology == PathTopology::Closed {
        nearest = nearest.min(segment_distance(previous, first, target)?);
    }
    Ok(nearest)
}

fn finite_point(point: [f64; 2]) -> Result<[f64; 2], PolylineError> {
    point
        .into_iter()
        .all(f64::is_finite)
        .then_some(point)
        .ok_or(PolylineError::NonFiniteCoordinate)
}

fn segment_distance(
    start: [f64; 2],
    end: [f64; 2],
    target: [f64; 2],
) -> Result<f64, PolylineError> {
    let direction = [end[0] - start[0], end[1] - start[1]];
    let length = direction[0].hypot(direction[1]);
    if !length.is_finite() {
        return Err(PolylineError::NonFiniteDistance);
    }
    if length == 0.0 {
        return finite_distance(start, target);
    }
    let offset = [target[0] - start[0], target[1] - start[1]];
    // Project onto a unit tangent so a representable segment never needs
    // its squared length. Clamping the distance first keeps the fraction ≤ 1.
    let unit = [direction[0] / length, direction[1] / length];
    let along = offset[0].mul_add(unit[0], offset[1] * unit[1]);
    if !along.is_finite() {
        return Err(PolylineError::NonFiniteDistance);
    }
    let t = along.clamp(0.0, length) / length;
    let closest = [
        direction[0].mul_add(t, start[0]),
        direction[1].mul_add(t, start[1]),
    ];
    finite_distance(closest, target)
}

fn finite_distance(first: [f64; 2], second: [f64; 2]) -> Result<f64, PolylineError> {
    let distance = (first[0] - second[0]).hypot(first[1] - second[1]);
    distance
        .is_finite()
        .then_some(distance)
        .ok_or(PolylineError::NonFiniteDistance)
}

#[cfg(test)]
mod tests {
    use super::{PathCapture, PathPush, PathTopology, PolylineError, polyline_distance};
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
    fn distance_clamps_and_distinguishes_open_from_closed_paths() {
        let line = [[0.0, 0.0], [4.0, 0.0]];
        assert_eq!(
            polyline_distance(line, [2.0, 3.0], PathTopology::Open),
            Ok(3.0)
        );
        assert_eq!(
            polyline_distance(line, [7.0, 4.0], PathTopology::Open),
            Ok(5.0)
        );
        assert_eq!(
            polyline_distance(
                [[1e200, 0.0], [2e200, 0.0]],
                [1e200, 3.0],
                PathTopology::Open
            ),
            Ok(3.0)
        );
        let triangle = [[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]];
        assert_eq!(
            polyline_distance(triangle, [0.0, 1.0], PathTopology::Open),
            Ok(1.0)
        );
        assert_eq!(
            polyline_distance(triangle, [0.0, 1.0], PathTopology::Closed),
            Ok(0.0)
        );
    }

    #[test]
    fn distance_reports_invalid_input() {
        assert_eq!(
            polyline_distance([[0.0, 0.0]], [0.0, 0.0], PathTopology::Open),
            Err(PolylineError::TooFewPoints)
        );
        assert_eq!(
            polyline_distance(
                [[f64::NAN, 0.0], [1.0, 1.0]],
                [0.0, 0.0],
                PathTopology::Open
            ),
            Err(PolylineError::NonFiniteCoordinate)
        );
    }
}
