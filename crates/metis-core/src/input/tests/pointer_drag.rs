use super::super::{PointerDrag, PointerDragError};

#[test]
fn displacement_scales_each_axis_from_a_nonzero_origin() {
    let drag = PointerDrag::try_new([10.0, -3.0], [2.0, 0.5]).expect("valid drag basis");
    let [x, y] = drag.displacement([4.0, 5.0]).expect("finite displacement");
    assert_eq!(x.to_bits(), (-12.0f64).to_bits());
    assert_eq!(y.to_bits(), 4.0f64.to_bits());
}

#[test]
fn construction_rejects_non_finite_coordinates_and_non_positive_scales() {
    assert_eq!(
        PointerDrag::try_new([f64::NAN, 0.0], [1.0, 1.0]),
        Err(PointerDragError::InvalidCoordinate)
    );
    for scale in [
        [0.0, 1.0],
        [-0.0, 1.0],
        [-1.0, 1.0],
        [f64::INFINITY, 1.0],
        [1.0, f64::NAN],
    ] {
        assert_eq!(
            PointerDrag::try_new([0.0, 0.0], scale),
            Err(PointerDragError::InvalidScale)
        );
    }
}

#[test]
fn displacement_distinguishes_invalid_input_from_arithmetic_overflow() {
    let drag =
        PointerDrag::try_new([f64::MAX, -f64::MAX], [1.0, 1.0]).expect("finite boundary origin");
    assert_eq!(
        drag.displacement([f64::INFINITY, 0.0]),
        Err(PointerDragError::InvalidCoordinate)
    );
    assert_eq!(
        drag.displacement([-f64::MAX, f64::MAX]),
        Err(PointerDragError::UnrepresentableDisplacement)
    );

    let scaled = PointerDrag::try_new([0.0, 0.0], [f64::MAX, f64::MIN_POSITIVE])
        .expect("finite positive scales");
    assert_eq!(
        scaled.displacement([2.0, 0.0]),
        Err(PointerDragError::UnrepresentableDisplacement)
    );
    let [x, y] = scaled.displacement([0.0, 0.0]).expect("zero displacement");
    assert_eq!(x.to_bits(), 0.0f64.to_bits());
    assert_eq!(y.to_bits(), 0.0f64.to_bits());
}
