//! Finite binary64 wire values retain bits through a browser JSON object.
//!
//! JSON.stringify maps a numeric negative zero to zero. A canonical decimal
//! string preserves its sign while the exact Rust parser recovers every bit.
//! Binary64 needs at most ceil(1 + 53 log10(2)) = 17 significant decimal digits.
//! Its shortest scientific form requires at most 17 + sign + point + e +
//! exponent sign + 3 exponent digits = 24 UTF-8 bytes.

use serde::{
    Deserializer, Serializer,
    de::{self, Visitor},
    ser::Error,
};
use std::fmt;

const MAX_NUMBER_BYTES: usize = 24;

#[cfg_attr(
    target_pointer_width = "64",
    expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde's with callback receives a borrowed field; changing this ABI breaks serialization"
    )
)]
pub(super) fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    if !value.is_finite() {
        return Err(S::Error::custom("Page number is not finite"));
    }
    let text = serde_json::to_string(value).map_err(S::Error::custom)?;
    if text.len() > MAX_NUMBER_BYTES {
        return Err(S::Error::custom(
            "Page decimal exceeds its analytical bound",
        ));
    }
    serializer.serialize_str(&text)
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    struct Decimal;
    impl Visitor<'_> for Decimal {
        type Value = f64;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("canonical finite decimal number text")
        }
        fn visit_str<E: de::Error>(self, text: &str) -> Result<f64, E> {
            if text.is_empty() || text.len() > MAX_NUMBER_BYTES {
                return Err(E::custom("Page decimal exceeds its analytical bound"));
            }
            let value = serde_json::from_str::<f64>(text).map_err(E::custom)?;
            if !value.is_finite() || serde_json::to_string(&value).map_err(E::custom)? != text {
                return Err(E::custom("Page decimal is not canonical finite text"));
            }
            Ok(value)
        }
    }
    deserializer.deserialize_str(Decimal)
}
