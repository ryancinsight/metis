//! Character-to-glyph mapping from a format 4 `cmap` subtable.

use super::TypefaceError;
use super::reader::{Reader, offset};

/// A validated format 4 subtable: segments of Basic Multilingual Plane code
/// points, each mapped by a delta or through a glyph index array.
#[derive(Clone, Copy)]
pub(super) struct CharacterMap<'font> {
    subtable: Reader<'font>,
    segment_count: usize,
}

impl<'font> CharacterMap<'font> {
    /// Selects the Windows Unicode BMP (3, 1) or Unicode BMP (0, 3) subtable.
    pub(super) fn parse(cmap: Reader<'font>) -> Result<Self, TypefaceError> {
        let count = offset(cmap.read::<u16>(2)?);
        let mut chosen = None;
        for index in 0..count {
            let record = 4 + 8 * index;
            let platform = cmap.read::<u16>(record)?;
            let encoding = cmap.read::<u16>(record + 2)?;
            if matches!((platform, encoding), (3, 1) | (0, 3)) {
                chosen = Some(offset(cmap.read::<u32>(record + 4)?));
                // The Windows subtable is the conventional choice when both exist.
                if platform == 3 {
                    break;
                }
            }
        }
        let start = chosen.ok_or(TypefaceError::MissingTable("cmap format 4 subtable"))?;
        let header = cmap.tail(start)?;
        if header.read::<u16>(0)? != 4 {
            return Err(TypefaceError::Invalid {
                table: "cmap",
                field: "format",
            });
        }
        let length = offset(header.read::<u16>(2)?);
        let subtable = cmap.sub(start, length, "cmap")?;
        let doubled = offset(subtable.read::<u16>(6)?);
        if doubled == 0 || doubled % 2 != 0 {
            return Err(TypefaceError::Invalid {
                table: "cmap",
                field: "segCountX2",
            });
        }
        // Four parallel arrays and the reserved pad must fit the subtable.
        subtable.slice(14, 4 * doubled + 2)?;
        Ok(Self {
            subtable,
            segment_count: doubled / 2,
        })
    }

    /// The glyph index for `character`, if its segment maps it.
    pub(super) fn lookup(&self, character: char) -> Option<u16> {
        let code = u16::try_from(u32::from(character)).ok()?;
        let table = self.subtable;
        let count = self.segment_count;
        let end_codes = 14;
        let start_codes = end_codes + 2 * count + 2;
        let deltas = start_codes + 2 * count;
        let range_offsets = deltas + 2 * count;
        // End codes ascend, so the first segment ending at or after the code
        // is the only one that can hold it.
        let (mut low, mut high) = (0, count);
        while low < high {
            let middle = low + (high - low) / 2;
            if table.read::<u16>(end_codes + 2 * middle).ok()? < code {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let segment = low;
        if segment == count {
            return None;
        }
        let start = table.read::<u16>(start_codes + 2 * segment).ok()?;
        if code < start {
            return None;
        }
        let delta = table.read::<u16>(deltas + 2 * segment).ok()?;
        let range_offset = table.read::<u16>(range_offsets + 2 * segment).ok()?;
        if range_offset == 0 {
            // Deltas are applied modulo 65536.
            return Some(code.wrapping_add(delta));
        }
        // The offset is relative to its own position in the range array.
        let address = range_offsets + 2 * segment + offset(range_offset) + 2 * offset(code - start);
        let glyph = table.read::<u16>(address).ok()?;
        (glyph != 0).then(|| glyph.wrapping_add(delta))
    }
}
