//! SHA-256 identity and its canonical lowercase hexadecimal wire representation.
use std::{fmt, io, str::FromStr};

/// A SHA-256 digest with domain-separated type identity and no representation cost.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest([u8; 32]);

impl Digest {
    /// Hashes the complete supplied bytes with the canonical crypto provider.
    #[must_use]
    pub fn hash(bytes: &[u8]) -> Self {
        Self(moirai_crypto::sha256(bytes))
    }
    /// Retains a computed digest's exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Borrows the digest's exact binary representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    /// Returns the digest's exact binary representation.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for Digest {
    type Err = io::Error;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "digest requires 64 lowercase hexadecimal bytes",
            ));
        }
        let mut bytes = [0; 32];
        for (output, pair) in bytes.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
            let digit = |byte: u8| {
                if byte <= b'9' {
                    byte - b'0'
                } else {
                    byte - b'a' + 10
                }
            };
            *output = digit(pair[0]) * 16 + digit(pair[1]);
        }
        Ok(Self(bytes))
    }
}
