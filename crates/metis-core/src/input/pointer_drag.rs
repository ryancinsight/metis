//! Press-relative displacement in an application-selected coordinate space.

use super::PointerDragError;

/// A validated pointer press and its captured coordinate scale.
///
/// Displacement is computed from the press coordinate `p`, current coordinate
/// `q`, and captured scale `s` as `(q - p) * s` independently on each axis.
/// Native hosts use `[1.0, 1.0]` for screen-pixel coordinates. A host that
/// reports normalized coordinates can capture its backing extent as `s`.
/// Later layout changes do not alter the captured scale.
///
/// # Examples
///
/// ```
/// use metis_core::input::PointerDrag;
///
/// let drag = PointerDrag::try_new([0.25, 0.5], [800.0, 600.0])?;
/// assert_eq!(drag.displacement([0.5, 0.25])?, [200.0, -150.0]);
/// # Ok::<(), metis_core::input::PointerDragError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct PointerDrag {
    origin: [f64; 2],
    units_per_coordinate: [f64; 2],
}

impl PointerDrag {
    /// Captures a finite press coordinate and positive coordinate scale.
    ///
    /// # Errors
    ///
    /// Returns [`PointerDragError::InvalidCoordinate`] for a non-finite origin,
    /// or [`PointerDragError::InvalidScale`] unless both scale components are
    /// finite and greater than zero.
    pub fn try_new(
        origin: [f64; 2],
        units_per_coordinate: [f64; 2],
    ) -> Result<Self, PointerDragError> {
        let [origin_x, origin_y] = origin;
        if !origin_x.is_finite() || !origin_y.is_finite() {
            return Err(PointerDragError::InvalidCoordinate);
        }
        let [units_x, units_y] = units_per_coordinate;
        if !units_x.is_finite() || !units_y.is_finite() || units_x <= 0.0 || units_y <= 0.0 {
            return Err(PointerDragError::InvalidScale);
        }
        Ok(Self {
            origin,
            units_per_coordinate,
        })
    }

    /// Returns the scaled displacement from the captured press coordinate.
    ///
    /// # Errors
    ///
    /// Returns [`PointerDragError::InvalidCoordinate`] for a non-finite current
    /// coordinate. Returns [`PointerDragError::UnrepresentableDisplacement`] if
    /// subtraction or scaling produces a non-finite value.
    pub fn displacement(self, current: [f64; 2]) -> Result<[f64; 2], PointerDragError> {
        let [current_x, current_y] = current;
        if !current_x.is_finite() || !current_y.is_finite() {
            return Err(PointerDragError::InvalidCoordinate);
        }
        let [origin_x, origin_y] = self.origin;
        let delta_x = current_x - origin_x;
        let delta_y = current_y - origin_y;
        if !delta_x.is_finite() || !delta_y.is_finite() {
            return Err(PointerDragError::UnrepresentableDisplacement);
        }
        let [units_x, units_y] = self.units_per_coordinate;
        let displacement_x = delta_x * units_x;
        let displacement_y = delta_y * units_y;
        if !displacement_x.is_finite() || !displacement_y.is_finite() {
            return Err(PointerDragError::UnrepresentableDisplacement);
        }
        Ok([displacement_x, displacement_y])
    }

    /// Returns the largest square displacement contained in the raw drag.
    ///
    /// For raw displacement `d`, the shared magnitude is
    /// `m = min(abs(d[0]), abs(d[1]))`. Each nonzero result component has
    /// magnitude `m` and keeps the corresponding raw sign. A zero magnitude
    /// returns `[0.0, 0.0]`, including when either raw component is negative
    /// zero. The calculation uses the coordinate scale captured by this drag,
    /// so the square is expressed in that output coordinate space. Callers
    /// retain ownership of gesture state and persistence decisions.
    ///
    /// # Errors
    ///
    /// Returns the same [`PointerDragError`] as [`Self::displacement`] for the
    /// same `current` coordinate.
    ///
    /// # Examples
    ///
    /// ```
    /// use metis_core::input::PointerDrag;
    ///
    /// let drag = PointerDrag::try_new([10.0, -3.0], [2.0, 0.5])?;
    /// assert_eq!(drag.displacement([4.0, 5.0])?, [-12.0, 4.0]);
    /// assert_eq!(drag.square_displacement([4.0, 5.0])?, [-4.0, 4.0]);
    /// # Ok::<(), metis_core::input::PointerDragError>(())
    /// ```
    pub fn square_displacement(self, current: [f64; 2]) -> Result<[f64; 2], PointerDragError> {
        let [raw_x, raw_y] = self.displacement(current)?;
        let magnitude = raw_x.abs().min(raw_y.abs());
        if magnitude <= 0.0 {
            return Ok([0.0, 0.0]);
        }
        Ok([magnitude.copysign(raw_x), magnitude.copysign(raw_y)])
    }
}
