//! Bounds-checked big-endian reads over font table bytes.

use super::TypefaceError;

/// A byte slice read at checked offsets.
///
/// Every read names the table it serves, so a truncation reports where the
/// font ended early instead of panicking on an out-of-range index.
#[derive(Clone, Copy)]
pub(super) struct Reader<'font> {
    bytes: &'font [u8],
    table: &'static str,
}

impl<'font> Reader<'font> {
    pub(super) const fn new(bytes: &'font [u8], table: &'static str) -> Self {
        Self { bytes, table }
    }

    /// The `length` bytes at `offset`.
    pub(super) fn slice(self, offset: usize, length: usize) -> Result<&'font [u8], TypefaceError> {
        offset
            .checked_add(length)
            .and_then(|end| self.bytes.get(offset..end))
            .ok_or(TypefaceError::Truncated {
                table: self.table,
                offset,
            })
    }

    /// A reader over the `length` bytes at `offset`, naming `table`.
    pub(super) fn sub(
        self,
        offset: usize,
        length: usize,
        table: &'static str,
    ) -> Result<Self, TypefaceError> {
        Ok(Self::new(self.slice(offset, length)?, table))
    }

    /// The bytes from `offset` to the end.
    pub(super) fn tail(self, offset: usize) -> Result<Self, TypefaceError> {
        let rest = self.bytes.get(offset..).ok_or(TypefaceError::Truncated {
            table: self.table,
            offset,
        })?;
        Ok(Self::new(rest, self.table))
    }

    /// The big-endian field at `offset`.
    pub(super) fn read<T: BigEndian>(self, offset: usize) -> Result<T, TypefaceError> {
        Ok(T::decode(self.slice(offset, T::SIZE)?))
    }
}

/// Widens a table offset or count read from the font.
pub(super) fn offset(value: impl Into<u32>) -> usize {
    usize::try_from(value.into()).expect("invariant: a u32 offset fits usize on supported targets")
}

/// A fixed-size big-endian font field.
pub(super) trait BigEndian: Sized {
    /// Bytes the field occupies.
    const SIZE: usize;

    /// Decodes exactly [`Self::SIZE`] bytes.
    fn decode(bytes: &[u8]) -> Self;
}

impl BigEndian for u8 {
    const SIZE: usize = 1;

    fn decode(bytes: &[u8]) -> Self {
        Self::from_be_bytes(field(bytes))
    }
}

impl BigEndian for u16 {
    const SIZE: usize = 2;

    fn decode(bytes: &[u8]) -> Self {
        Self::from_be_bytes(field(bytes))
    }
}

impl BigEndian for i16 {
    const SIZE: usize = 2;

    fn decode(bytes: &[u8]) -> Self {
        Self::from_be_bytes(field(bytes))
    }
}

impl BigEndian for u32 {
    const SIZE: usize = 4;

    fn decode(bytes: &[u8]) -> Self {
        Self::from_be_bytes(field(bytes))
    }
}

/// The bytes of a field whose length the reader already checked.
fn field<const N: usize>(bytes: &[u8]) -> [u8; N] {
    bytes
        .try_into()
        .expect("invariant: the reader slices exactly the field's size")
}
