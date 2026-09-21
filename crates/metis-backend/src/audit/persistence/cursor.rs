use super::storage_error;
use metis_core::error::{ErrorCode, Result};

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    pub(super) offset: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| storage_error(ErrorCode::FrameTruncated))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| storage_error(ErrorCode::FrameTruncated))?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn byte(&mut self) -> Result<u8> {
        self.take(1)?
            .first()
            .copied()
            .ok_or_else(|| storage_error(ErrorCode::FrameTruncated))
    }

    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }
}
