//! Portable complete-byte identity, independent of filesystem/process APIs.
use super::{Digest, inventory::invalid};
use std::io;

/// Exact file length and SHA-256, verified from the complete byte stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileIdentity {
    /// Complete file length.
    pub bytes: u64,
    /// Complete SHA-256.
    pub digest: Digest,
}
impl FileIdentity {
    /// Hashes an in-memory complete file value.
    ///
    /// # Errors
    /// Returns an error if its length cannot fit the wire length type.
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        Ok(Self {
            bytes: u64::try_from(bytes.len()).map_err(|_| invalid("file length overflow"))?,
            digest: Digest::hash(bytes),
        })
    }
}
