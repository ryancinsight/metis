use std::cmp::Ordering;

use crate::parser::limit_error;
use metis_core::error::Result;

use super::{OriginBounds, PlacementRegion, Plane, Polygon};

impl PlacementRegion {
    /// Returns the exact number of admissible text anchors in the region.
    ///
    /// Obstacle boundary intersections partition the finite origin lattice into
    /// event rows and open slabs. Within each slab, forbidden interval edges retain
    /// their order, so signed Euclidean floor sums count every open gap without
    /// scanning rows or pixels. For `n` obstacles this uses `O(n^3 log n + n^3 log C)`
    /// work and `O(n^2)` storage. The compiled `|A|, |B| < 2^34`, `|C| < 2^70`
    /// bounds imply intersections `<2^105`, comparisons `<2^107`, and sums `<2^103`;
    /// all fit `i128` exactly.
    ///
    /// # Errors
    ///
    /// Returns a resource error on allocation or capacity failure; empty regions return zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use metis_platform::{RasterLine, Rect};
    /// use metis_ui_lang::{PlacementObstacle, PlacementRegion};
    ///
    /// let occupied = [PlacementObstacle::Line(RasterLine::new((2, 0), (2, 2))),
    ///     PlacementObstacle::Rect(Rect::new(4, 0, 2, 1))];
    /// let region = PlacementRegion::new(Rect::new(0, 0, 2, 1),
    ///     Rect::new(0, 0, 6, 3), &occupied)?;
    /// assert_eq!(region.count()?, 7);
    /// # Ok::<(), metis_core::error::MetisError>(())
    /// ```
    pub fn count(&self) -> Result<u64> {
        let Some(bounds) = self.bounds else {
            return Ok(0);
        };
        let events = events(&self.forbidden, bounds)?;
        let mut scratch = Scratch::new(self.forbidden.len())?;
        let mut total = 0_i128;
        for (index, &row) in events.iter().enumerate() {
            total += row_count(&self.forbidden, row, bounds, &mut scratch);
            if let Some(&next) = events.get(index + 1) {
                total += slab_count(&self.forbidden, row + 1, next - 1, bounds, &mut scratch);
            }
        }
        u64::try_from(total).map_err(|_| limit_error("Placement count exceeds its proven range"))
    }
}

#[derive(Clone, Copy)]
struct Edge {
    p: i128,
    q: i128,
    r: i128,
}

impl Edge {
    fn integer(value: i64) -> Self {
        Self {
            p: 0,
            q: i128::from(value),
            r: 1,
        }
    }

    fn from_plane(Plane(a, b, c): Plane) -> Self {
        let (mut p, mut q, mut r) = (-b, c, a);
        if r < 0 {
            (p, q, r) = (-p, -q, -r)
        }
        Self { p, q, r }
    }

    fn numerator(self, y: i64) -> i128 {
        self.p * i128::from(y) + self.q
    }

    fn cmp(self, other: Self, y: i64) -> Ordering {
        (self.numerator(y) * other.r).cmp(&(other.numerator(y) * self.r))
    }
}

#[derive(Clone, Copy)]
struct Interval(Edge, Edge);

#[derive(Clone, Copy)]
struct Gap {
    right: Edge,
    left: Edge,
}

struct Scratch {
    intervals: Vec<Interval>,
    gaps: Vec<Gap>,
}

impl Scratch {
    fn new(count: usize) -> Result<Self> {
        let gap_count = count
            .checked_add(1)
            .ok_or_else(|| limit_error("Placement count capacity overflow"))?;
        let mut intervals = Vec::new();
        let mut gaps = Vec::new();
        intervals
            .try_reserve_exact(count)
            .map_err(|_| limit_error("Placement count allocation failed"))?;
        gaps.try_reserve_exact(gap_count)
            .map_err(|_| limit_error("Placement count allocation failed"))?;
        Ok(Self { intervals, gaps })
    }
}

fn events(polygons: &[Polygon], bounds: OriginBounds) -> Result<Vec<i64>> {
    let line_count = polygons
        .len()
        .checked_mul(6)
        .and_then(|n| n.checked_add(4))
        .ok_or_else(|| limit_error("Placement count capacity overflow"))?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(line_count)
        .map_err(|_| limit_error("Placement count allocation failed"))?;
    for polygon in polygons {
        lines.extend(polygon.planes.iter().take(polygon.len).copied());
    }
    lines.extend([
        Plane(1, 0, i128::from(bounds.x.0 - 1)),
        Plane(1, 0, i128::from(bounds.x.1 + 1)),
        Plane(0, 1, i128::from(bounds.y.0)),
        Plane(0, 1, i128::from(bounds.y.1)),
    ]);
    let capacity = lines
        .len()
        .checked_mul(lines.len().saturating_sub(1))
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| limit_error("Placement count capacity overflow"))?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(capacity)
        .map_err(|_| limit_error("Placement count allocation failed"))?;
    rows.extend([bounds.y.0, bounds.y.1]);
    for (index, first) in lines.iter().enumerate() {
        for second in &lines[index + 1..] {
            let mut denominator = first.0 * second.1 - first.1 * second.0;
            if denominator == 0 {
                continue;
            }
            let mut numerator = first.0 * second.2 - second.0 * first.2;
            if denominator < 0 {
                (numerator, denominator) = (-numerator, -denominator)
            }
            for value in [
                numerator.div_euclid(denominator),
                -(-numerator).div_euclid(denominator),
            ] {
                if value >= i128::from(bounds.y.0) && value <= i128::from(bounds.y.1) {
                    rows.push(
                        i64::try_from(value).expect("invariant: a filtered event row fits i64"),
                    );
                }
            }
        }
    }
    rows.sort_unstable();
    rows.dedup();
    Ok(rows)
}

fn gaps<'a>(
    polygons: &[Polygon],
    y: i64,
    bounds: OriginBounds,
    scratch: &'a mut Scratch,
) -> &'a [Gap] {
    scratch.intervals.clear();
    scratch.gaps.clear();
    let left = Edge::integer(bounds.x.0 - 1);
    let right = Edge::integer(bounds.x.1 + 1);
    for polygon in polygons {
        let (mut lower, mut upper, mut active) = (left, right, true);
        for &plane in polygon.planes.iter().take(polygon.len) {
            let Plane(a, b, c) = plane;
            if a == 0 {
                active &= b * i128::from(y) <= c;
            } else {
                let edge = Edge::from_plane(plane);
                if a < 0 && edge.cmp(lower, y) == Ordering::Greater {
                    lower = edge
                }
                if a > 0 && edge.cmp(upper, y) == Ordering::Less {
                    upper = edge
                }
            }
        }
        if active && lower.cmp(upper, y) != Ordering::Greater {
            scratch.intervals.push(Interval(lower, upper));
        }
    }
    scratch
        .intervals
        .sort_unstable_by(|a, b| a.0.cmp(b.0, y).then_with(|| a.1.cmp(b.1, y)));
    let mut edge = left;
    for &Interval(lower, upper) in &scratch.intervals {
        if lower.cmp(edge, y) == Ordering::Greater {
            scratch.gaps.push(Gap {
                right: edge,
                left: lower,
            });
        }
        if upper.cmp(edge, y) == Ordering::Greater {
            edge = upper
        }
    }
    if edge.cmp(right, y) == Ordering::Less {
        scratch.gaps.push(Gap {
            right: edge,
            left: right,
        });
    }
    &scratch.gaps
}

fn row_count(polygons: &[Polygon], y: i64, bounds: OriginBounds, scratch: &mut Scratch) -> i128 {
    gap_count(gaps(polygons, y, bounds, scratch), y, y)
}

fn slab_count(
    polygons: &[Polygon],
    first: i64,
    last: i64,
    bounds: OriginBounds,
    scratch: &mut Scratch,
) -> i128 {
    if first > last {
        return 0;
    }
    gap_count(gaps(polygons, first, bounds, scratch), first, last)
}

fn gap_count(gaps: &[Gap], first: i64, last: i64) -> i128 {
    let rows = i128::from(last - first + 1);
    gaps.iter()
        .map(|gap| {
            let count = -floor_affine(-gap.left.p, -gap.left.q, gap.left.r, first, last)
                - floor_affine(gap.right.p, gap.right.q, gap.right.r, first, last)
                - rows;
            assert!(
                count >= 0,
                "invariant: an open rational gap has nonnegative occupancy"
            );
            count
        })
        .sum()
}

fn floor_affine(p: i128, q: i128, r: i128, first: i64, last: i64) -> i128 {
    floor_sum(
        i128::from(last - first + 1),
        r,
        p,
        p * i128::from(first) + q,
    )
}

fn floor_sum(mut n: i128, mut m: i128, mut a: i128, mut b: i128) -> i128 {
    // Signed normalization adapts AtCoder internal_math.hpp lines 169-194
    // (https://github.com/atcoder/ac-library/blob/master/atcoder/internal_math.hpp#L169-L194);
    // exact i128 arithmetic replaces that reference's wrapping u64 contract.
    let mut answer = a.div_euclid(m) * n * (n - 1) / 2 + b.div_euclid(m) * n;
    a = a.rem_euclid(m);
    b = b.rem_euclid(m);
    loop {
        answer += (a / m) * n * (n - 1) / 2 + (b / m) * n;
        a %= m;
        b %= m;
        let projected = a * n + b;
        if projected < m {
            return answer;
        }
        n = projected / m;
        b = projected % m;
        std::mem::swap(&mut m, &mut a);
    }
}

#[cfg(test)]
mod tests {
    use super::floor_sum;

    #[test]
    fn signed_floor_sum_matches_direct_and_large_affine_values() {
        for n in 0..=15 {
            for m in 1..=11 {
                for a in -12..=12 {
                    for b in -12..=12 {
                        let direct = (0..n).map(|k| (a * k + b).div_euclid(m)).sum();
                        assert_eq!(floor_sum(n, m, a, b), direct, "{n} {m} {a} {b}");
                    }
                }
            }
        }
        let (n, m) = ((1_i128 << 31) - 1, (1_i128 << 33) - 2);
        assert_eq!(floor_sum(n, m, m, 0), n * (n - 1) / 2);
        assert_eq!(floor_sum(n, m, -m, -1), -n * (n + 1) / 2);
        assert_eq!(floor_sum(n, m, 1, 0), 0);
        let constant = 1_i128 << 70;
        assert_eq!(floor_sum(n, m, 0, constant), n * (constant / m));
    }
}
