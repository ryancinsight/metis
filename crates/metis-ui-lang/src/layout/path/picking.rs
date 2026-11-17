//! Screen-space segment and vertex picking.

use std::fmt;

/// The topology used when measuring a polyline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathTopology {
    /// The final sample is not connected to the initial sample.
    Open,
    /// The final sample is connected to the initial sample.
    Closed,
}

/// Errors returned when picking a projected path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PolylineError {
    /// The input contained fewer than two points.
    TooFewPoints,
    /// A point contained a NaN or infinity.
    NonFiniteCoordinate,
    /// Finite input produced an unrepresentable distance.
    NonFiniteDistance,
    /// The input contained no vertices.
    EmptyPath,
}

impl fmt::Display for PolylineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyPath => "a path needs at least one vertex",
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
    nearest_polyline_segment(points, target, topology).map(SegmentHit::distance)
}

/// The closest point on a path segment, in path order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentHit {
    start_index: usize,
    fraction: f64,
    distance: f64,
}

impl SegmentHit {
    /// Index of the starting vertex. The closing segment starts at the last vertex.
    #[must_use]
    pub const fn start_index(self) -> usize {
        self.start_index
    }

    /// Interpolation fraction in `[0, 1]` from the start to the end of the segment.
    #[must_use]
    pub const fn fraction(self) -> f64 {
        self.fraction
    }

    /// Distance in screen pixels from the target to the selected point.
    #[must_use]
    pub const fn distance(self) -> f64 {
        self.distance
    }
}

/// Picks a segment and the location on it nearest to a screen point.
///
/// The query streams projected vertices in path order. Ties retain the first
/// segment, including when an endpoint is shared by two segments.
///
/// # Errors
///
/// Returns the same geometry errors as [`polyline_distance`].
///
/// # Examples
///
/// ```
/// use metis_ui_lang::{nearest_polyline_segment, PathTopology};
/// let hit = nearest_polyline_segment(
///     [[0.0, 0.0], [8.0, 0.0]],
///     [2.0, 3.0],
///     PathTopology::Open,
/// )?;
/// assert_eq!((hit.start_index(), hit.fraction(), hit.distance()), (0, 0.25, 3.0));
/// # Ok::<(), metis_ui_lang::PolylineError>(())
/// ```
pub fn nearest_polyline_segment(
    points: impl IntoIterator<Item = [f64; 2]>,
    target: [f64; 2],
    topology: PathTopology,
) -> Result<SegmentHit, PolylineError> {
    let mut points = points.into_iter().enumerate();
    let (_, first) = points.next().ok_or(PolylineError::TooFewPoints)?;
    let (mut index, previous) = points.next().ok_or(PolylineError::TooFewPoints)?;
    let first = finite_point(first)?;
    let mut previous = finite_point(previous)?;
    let target = finite_point(target)?;
    let mut nearest = segment_hit(0, first, previous, target)?;
    for (next_index, current) in points {
        let current = finite_point(current)?;
        let hit = segment_hit(index, previous, current, target)?;
        if hit.distance() < nearest.distance() {
            nearest = hit;
        }
        previous = current;
        index = next_index;
    }
    if topology == PathTopology::Closed {
        let closing = segment_hit(index, previous, first, target)?;
        if closing.distance() < nearest.distance() {
            nearest = closing;
        }
    }
    Ok(nearest)
}

/// The closest path vertex to a screen point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexHit {
    index: usize,
    distance: f64,
}

impl VertexHit {
    /// Index of the vertex in the input iterator.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    /// Distance in screen pixels from the target to the vertex.
    #[must_use]
    pub const fn distance(self) -> f64 {
        self.distance
    }
}

/// Picks the nearest vertex without allocating a projected path.
///
/// A tie picks the first vertex in path order. A path of one vertex is valid.
///
/// # Errors
///
/// Returns [`PolylineError::EmptyPath`] for empty input,
/// [`PolylineError::NonFiniteCoordinate`] for NaN or infinite coordinates, or
/// [`PolylineError::NonFiniteDistance`] for an unrepresentable distance.
///
/// # Examples
///
/// ```
/// use metis_ui_lang::nearest_polyline_vertex;
/// let hit = nearest_polyline_vertex([[1.0, 0.0], [4.0, 0.0]], [3.0, 0.0])?;
/// assert_eq!((hit.index(), hit.distance()), (1, 1.0));
/// # Ok::<(), metis_ui_lang::PolylineError>(())
/// ```
pub fn nearest_polyline_vertex(
    points: impl IntoIterator<Item = [f64; 2]>,
    target: [f64; 2],
) -> Result<VertexHit, PolylineError> {
    let mut points = points.into_iter().enumerate();
    let (index, first) = points.next().ok_or(PolylineError::EmptyPath)?;
    let target = finite_point(target)?;
    let mut nearest = VertexHit {
        index,
        distance: finite_distance(finite_point(first)?, target)?,
    };
    for (index, point) in points {
        let distance = finite_distance(finite_point(point)?, target)?;
        if distance < nearest.distance() {
            nearest = VertexHit { index, distance };
        }
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

fn segment_hit(
    start_index: usize,
    start: [f64; 2],
    end: [f64; 2],
    target: [f64; 2],
) -> Result<SegmentHit, PolylineError> {
    #[expect(
        clippy::float_cmp,
        reason = "coincident endpoints must agree exactly across adjacent segments"
    )]
    let endpoint = [(start, 0.0), (end, 1.0)]
        .into_iter()
        .find(|&(point, _)| point == target);
    if let Some((_, fraction)) = endpoint {
        return Ok(SegmentHit {
            start_index,
            fraction,
            distance: 0.0,
        });
    }
    let direction = [end[0] - start[0], end[1] - start[1]];
    let length = direction[0].hypot(direction[1]);
    if !length.is_finite() {
        return Err(PolylineError::NonFiniteDistance);
    }
    if length == 0.0 {
        return Ok(SegmentHit {
            start_index,
            fraction: 0.0,
            distance: finite_distance(start, target)?,
        });
    }
    let offset = [target[0] - start[0], target[1] - start[1]];
    // The unit tangent keeps the projection in pixel units without squaring
    // the segment length. Clamp before dividing to keep the fraction in range.
    let unit = [direction[0] / length, direction[1] / length];
    let along = offset[0].mul_add(unit[0], offset[1] * unit[1]);
    if !along.is_finite() {
        return Err(PolylineError::NonFiniteDistance);
    }
    let (fraction, closest) = if along <= 0.0 {
        (0.0, start)
    } else if along >= length {
        (1.0, end)
    } else {
        let fraction = along / length;
        (
            fraction,
            [
                direction[0].mul_add(fraction, start[0]),
                direction[1].mul_add(fraction, start[1]),
            ],
        )
    };
    Ok(SegmentHit {
        start_index,
        fraction,
        distance: finite_distance(closest, target)?,
    })
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
    use super::{
        PathTopology, PolylineError, nearest_polyline_segment, nearest_polyline_vertex,
        polyline_distance,
    };

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
    #[test]
    fn closing_segment_reports_last_index_and_fraction() {
        let points = [[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]];
        let hit = nearest_polyline_segment(points, [-2.0, 2.0], PathTopology::Closed)
            .expect("invariant: the finite fixture admits a path hit");
        assert_eq!(
            (hit.start_index(), hit.fraction(), hit.distance()),
            (2, 0.5, 2.0)
        );
    }

    #[test]
    fn segment_and_vertex_ties_retain_path_order() {
        let points = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]];
        let hit = nearest_polyline_segment(points, [4.0, 0.0], PathTopology::Open)
            .expect("invariant: the finite fixture admits a path hit");
        assert_eq!(
            (hit.start_index(), hit.fraction(), hit.distance()),
            (0, 1.0, 0.0)
        );
        let hit = nearest_polyline_vertex(points, [2.0, 0.0])
            .expect("invariant: the finite fixture admits a path hit");
        assert_eq!((hit.index(), hit.distance()), (0, 2.0));
        assert_eq!(
            nearest_polyline_vertex([], [0.0, 0.0]),
            Err(PolylineError::EmptyPath)
        );
    }
    #[test]
    fn a_shared_diagonal_vertex_retains_the_preceding_segment() {
        let hit = nearest_polyline_segment(
            [[0.0, 0.0], [1.0, 1.0], [2.0, 1.0]],
            [1.0, 1.0],
            PathTopology::Open,
        )
        .expect("invariant: target is a shared vertex of the path");
        assert_eq!(
            (hit.start_index(), hit.fraction(), hit.distance()),
            (0, 1.0, 0.0)
        );
    }
}
