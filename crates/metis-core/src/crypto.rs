//! Protocol framing integrity primitive.
//!
//! Authentication hashing is owned by [`moirai_crypto`]. This module retains
//! CRC-32 because it detects accidental frame corruption and is not an
//! authentication primitive.

/// Computes CRC-32 using the IEEE 802.3 Ethernet/PNG polynomial.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::crc32;

    #[test]
    fn crc32_matches_ieee_reference_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
