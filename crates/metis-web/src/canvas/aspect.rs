//! Exact physical-aspect rounding for validated display spacing.

const FRACTION_BITS: u32 = 52;
const MIN_NORMAL_EXPONENT: i32 = -1022;
const MIN_SUBNORMAL_EXPONENT: i32 = -1074;
const MAX_EXPONENT: i32 = 1023;
const EXPONENT_BIAS: i32 = 1023;

#[derive(Clone, Copy)]
struct BinaryValue {
    significand: u128,
    exponent: i32,
}

/// Rounds `(width * column) / (height * row)` once to binary64.
///
/// A positive finite binary64 value is exactly `significand * 2^exponent`.
/// Multiplying its at-most-53-bit significand by a `u32` dimension needs at
/// most 85 bits, so both products fit in `u128`. Normalization shifts neither
/// product past 86 bits. Long division then emits 53 significant bits (the
/// leading bit and 52 following bits) by doubling a remainder smaller than
/// the normalized denominator. The final remainder selects round-to-nearest,
/// ties-to-even without a floating-point intermediate.
pub(super) fn physical_ratio(width: u32, height: u32, row: f64, column: f64) -> Option<f64> {
    let row = decode(row);
    let column = decode(column);
    let numerator = u128::from(width) * column.significand;
    let denominator = u128::from(height) * row.significand;
    rounded_ratio(numerator, denominator, column.exponent - row.exponent)
}

fn decode(value: f64) -> BinaryValue {
    let bits = value.to_bits();
    let stored_exponent = (bits >> FRACTION_BITS) & 0x7ff;
    let fraction = bits & ((1_u64 << FRACTION_BITS) - 1);
    if stored_exponent == 0 {
        BinaryValue {
            significand: u128::from(fraction),
            exponent: MIN_SUBNORMAL_EXPONENT,
        }
    } else {
        BinaryValue {
            significand: u128::from((1_u64 << FRACTION_BITS) | fraction),
            exponent: i32::try_from(stored_exponent)
                .expect("invariant: binary64 exponent field fits in i32")
                - EXPONENT_BIAS
                - i32::try_from(FRACTION_BITS)
                    .expect("invariant: binary64 fraction width fits in i32"),
        }
    }
}

fn rounded_ratio(numerator: u128, denominator: u128, exponent: i32) -> Option<f64> {
    let (normalized_numerator, normalized_denominator, quotient_exponent) =
        normalize(numerator, denominator);
    let result_exponent = exponent + quotient_exponent;
    let remainder = normalized_numerator - normalized_denominator;

    if result_exponent >= MIN_NORMAL_EXPONENT {
        return round_normal(remainder, normalized_denominator, result_exponent);
    }
    round_subnormal(remainder, normalized_denominator, result_exponent)
}

fn normalize(numerator: u128, denominator: u128) -> (u128, u128, i32) {
    let numerator_bits = u128::BITS - numerator.leading_zeros();
    let denominator_bits = u128::BITS - denominator.leading_zeros();
    let bit_delta = i32::try_from(numerator_bits).expect("invariant: u128 bit width fits in i32")
        - i32::try_from(denominator_bits).expect("invariant: u128 bit width fits in i32");
    let quotient_exponent = if bit_delta >= 0 {
        let scaled_denominator = denominator << bit_delta.unsigned_abs();
        if numerator < scaled_denominator {
            bit_delta - 1
        } else {
            bit_delta
        }
    } else {
        let scaled_numerator = numerator << bit_delta.unsigned_abs();
        if scaled_numerator < denominator {
            bit_delta - 1
        } else {
            bit_delta
        }
    };

    if quotient_exponent >= 0 {
        (
            numerator,
            denominator << quotient_exponent.unsigned_abs(),
            quotient_exponent,
        )
    } else {
        (
            numerator << quotient_exponent.unsigned_abs(),
            denominator,
            quotient_exponent,
        )
    }
}

fn append_bits(
    mut value: u128,
    mut remainder: u128,
    denominator: u128,
    count: u32,
) -> (u128, u128) {
    for _ in 0..count {
        value <<= 1;
        remainder <<= 1;
        if remainder >= denominator {
            remainder -= denominator;
            value |= 1;
        }
    }
    (value, remainder)
}

fn round_ties_even(value: u128, remainder: u128, denominator: u128) -> u128 {
    let doubled_remainder = remainder << 1;
    if doubled_remainder > denominator || (doubled_remainder == denominator && value & 1 == 1) {
        value + 1
    } else {
        value
    }
}

fn round_normal(remainder: u128, denominator: u128, mut exponent: i32) -> Option<f64> {
    if exponent > MAX_EXPONENT {
        return None;
    }
    let (significand, remainder) = append_bits(1, remainder, denominator, FRACTION_BITS);
    let mut significand = round_ties_even(significand, remainder, denominator);
    if significand == 1_u128 << (FRACTION_BITS + 1) {
        significand = 1_u128 << FRACTION_BITS;
        exponent += 1;
        if exponent > MAX_EXPONENT {
            return None;
        }
    }
    let stored_exponent = u64::try_from(exponent + EXPONENT_BIAS)
        .expect("invariant: normal result exponent is positive");
    let fraction = u64::try_from(significand - (1_u128 << FRACTION_BITS))
        .expect("invariant: rounded binary64 fraction fits in u64");
    Some(f64::from_bits(
        (stored_exponent << FRACTION_BITS) | fraction,
    ))
}

fn round_subnormal(remainder: u128, denominator: u128, exponent: i32) -> Option<f64> {
    if exponent < MIN_SUBNORMAL_EXPONENT - 1 {
        return None;
    }
    if exponent == MIN_SUBNORMAL_EXPONENT - 1 {
        return (remainder != 0).then_some(f64::from_bits(1));
    }
    let kept_bits = u32::try_from(exponent - MIN_SUBNORMAL_EXPONENT)
        .expect("invariant: subnormal kept-bit count is nonnegative");
    let (units, remainder) = append_bits(1, remainder, denominator, kept_bits);
    let units = round_ties_even(units, remainder, denominator);
    let bits = u64::try_from(units).expect("invariant: subnormal units fit in binary64 bits");
    (bits != 0).then_some(f64::from_bits(bits))
}

#[cfg(test)]
mod tests {
    use super::rounded_ratio;

    #[test]
    fn normal_rounding_is_ties_to_even() {
        let denominator = 1_u128 << 54;
        for (numerator, expected_fraction) in [
            (denominator + 1, 0_u64),
            (denominator + 2, 0),
            (denominator + 3, 1),
        ] {
            assert_eq!(
                rounded_ratio(numerator, denominator, 0)
                    .expect("representable normal")
                    .to_bits(),
                1.0_f64.to_bits() + expected_fraction
            );
        }
        assert_eq!(
            rounded_ratio((1_u128 << 53) + 3, 1_u128 << 53, 0)
                .expect("odd tie rounds upward")
                .to_bits(),
            1.0_f64.to_bits() + 2
        );
    }

    #[test]
    fn subnormal_rounding_preserves_even_ties_and_boundaries() {
        assert_eq!(rounded_ratio(1, 1, -1075), None);
        assert_eq!(
            rounded_ratio(3, 2, -1075)
                .expect("above half minimum subnormal")
                .to_bits(),
            1
        );
        assert_eq!(
            rounded_ratio(3, 2, -1074)
                .expect("odd subnormal tie")
                .to_bits(),
            2
        );
        assert_eq!(
            rounded_ratio(5, 2, -1074)
                .expect("even subnormal tie")
                .to_bits(),
            2
        );
        assert_eq!(
            rounded_ratio((1_u128 << 53) - 1, 1_u128 << 53, -1022)
                .expect("normal boundary tie")
                .to_bits(),
            f64::MIN_POSITIVE.to_bits()
        );
    }

    #[test]
    fn maximum_boundary_rejects_only_infinite_rounding() {
        assert_eq!(
            rounded_ratio((1_u128 << 53) - 1, 1_u128 << 52, 1023)
                .expect("exact maximum")
                .to_bits(),
            f64::MAX.to_bits()
        );
        assert_eq!(
            rounded_ratio((1_u128 << 55) - 3, 1_u128 << 54, 1023)
                .expect("below overflow midpoint")
                .to_bits(),
            f64::MAX.to_bits()
        );
        assert_eq!(rounded_ratio((1_u128 << 54) - 1, 1_u128 << 53, 1023), None);
    }

    #[test]
    fn power_of_two_exponent_boundaries_are_exact() {
        for (exponent, expected) in [
            (-1074, 1_u64),
            (-1022, f64::MIN_POSITIVE.to_bits()),
            (1023, 0x7fe0_0000_0000_0000),
        ] {
            assert_eq!(
                rounded_ratio(1, 1, exponent)
                    .expect("representable power of two")
                    .to_bits(),
                expected
            );
        }
        assert_eq!(rounded_ratio(1, 1, 1024), None);
    }
}
