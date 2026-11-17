//! Bounded pointer-path capture and screen-space picking.

use metis_platform::rasterizer::MAX_STROKE_POINTS;
mod picking;

pub use picking::{
    PathTopology, PolylineError, SegmentHit, VertexHit, nearest_polyline_segment,
    nearest_polyline_vertex, polyline_distance,
};

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
    use super::{PathCapture, PathPush};
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
}
