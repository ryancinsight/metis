use super::super::{PointerDrag, PointerDragError};

fn assert_bits_eq(actual: [f64; 2], expected: [f64; 2]) {
    assert_eq!(
        [actual[0].to_bits(), actual[1].to_bits()],
        [expected[0].to_bits(), expected[1].to_bits()]
    );
}

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

#[test]
fn square_displacement_uses_the_largest_contained_output_square() {
    let drag = PointerDrag::try_new([10.0, -3.0], [2.0, 0.5]).expect("valid drag basis");
    let raw = drag.displacement([4.0, 5.0]).expect("finite displacement");
    let square = drag
        .square_displacement([4.0, 5.0])
        .expect("finite square displacement");

    assert_bits_eq(raw, [-12.0, 4.0]);
    assert_bits_eq(square, [-4.0, 4.0]);
    assert!(square[0].abs() <= raw[0].abs());
    assert!(square[1].abs() <= raw[1].abs());
}

#[test]
fn square_displacement_preserves_each_nonzero_quadrant_sign() {
    let drag = PointerDrag::try_new([0.0, 0.0], [1.0, 1.0]).expect("valid drag basis");
    for (current, expected) in [
        ([3.0, 2.0], [2.0, 2.0]),
        ([2.0, 3.0], [2.0, 2.0]),
        ([-3.0, 2.0], [-2.0, 2.0]),
        ([3.0, -2.0], [2.0, -2.0]),
        ([-3.0, -2.0], [-2.0, -2.0]),
        ([3.0, -3.0], [3.0, -3.0]),
    ] {
        assert_bits_eq(
            drag.square_displacement(current)
                .expect("finite square displacement"),
            expected,
        );
    }
}

#[test]
fn square_displacement_canonicalizes_axis_zero_to_positive_zero() {
    let drag = PointerDrag::try_new([0.0, 0.0], [1.0, 1.0]).expect("valid drag basis");
    for current in [[0.0, 5.0], [-0.0, -5.0], [4.0, 0.0], [-4.0, -0.0]] {
        assert_bits_eq(
            drag.square_displacement(current)
                .expect("finite degenerate displacement"),
            [0.0, 0.0],
        );
    }
}

#[test]
fn square_displacement_is_translation_invariant_and_idempotent() {
    let original = PointerDrag::try_new([10.0, -3.0], [2.0, 0.5])
        .expect("valid original drag basis")
        .square_displacement([4.0, 5.0])
        .expect("finite original square");
    let translated = PointerDrag::try_new([20.0, 7.0], [2.0, 0.5])
        .expect("valid translated drag basis")
        .square_displacement([14.0, 15.0])
        .expect("finite translated square");
    let projected = PointerDrag::try_new([0.0, 0.0], [1.0, 1.0])
        .expect("valid projection basis")
        .square_displacement(original)
        .expect("finite repeated projection");

    assert_bits_eq(original, [-4.0, 4.0]);
    assert_bits_eq(translated, original);
    assert_bits_eq(projected, original);
}

#[test]
fn square_displacement_preserves_displacement_failures() {
    let drag =
        PointerDrag::try_new([f64::MAX, -f64::MAX], [1.0, 1.0]).expect("finite boundary origin");
    assert_eq!(
        drag.square_displacement([f64::INFINITY, 0.0]),
        Err(PointerDragError::InvalidCoordinate)
    );
    assert_eq!(
        drag.square_displacement([f64::NAN, 0.0]),
        Err(PointerDragError::InvalidCoordinate)
    );
    assert_eq!(
        drag.square_displacement([-f64::MAX, f64::MAX]),
        Err(PointerDragError::UnrepresentableDisplacement)
    );

    let scaled = PointerDrag::try_new([0.0, 0.0], [f64::MAX, 1.0]).expect("finite positive scales");
    assert_eq!(
        scaled.square_displacement([2.0, 1.0]),
        Err(PointerDragError::UnrepresentableDisplacement)
    );
}
