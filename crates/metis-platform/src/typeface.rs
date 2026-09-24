//! TrueType typefaces: parsing, glyph outlines and antialiased text.
//!
//! The parser reads the subset a horizontal text run needs — `head`, `hhea`,
//! `maxp`, `hmtx`, `loca`, `glyf` and a format 4 `cmap` — from the OpenType
//! specification (Microsoft Typography, "OpenType specification" version
//! 1.9, chapters of the same names). Every read is bounds-checked, so a
//! malformed font is a typed [`TypefaceError`], never a panic. Hinting
//! instructions and GPOS kerning are not interpreted: outlines are rasterized
//! unhinted with exact area coverage, and glyphs advance by their `hmtx`
//! widths.

mod cmap;
mod faces;
mod glyf;
mod glyph_cache;
mod raster;
mod reader;
mod text;

use std::fmt;

pub use faces::{bold, regular};
pub use text::{GlyphWeight, TextSize, TextStyle, draw_text};

use cmap::CharacterMap;
use reader::{Reader, offset};

/// A font whose bytes cannot be read as the supported TrueType subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypefaceError {
    /// A read ran past the end of `table` at byte `offset`.
    Truncated {
        /// Table being read.
        table: &'static str,
        /// Offset of the failed read within that table.
        offset: usize,
    },
    /// A required table is absent.
    MissingTable(&'static str),
    /// A field holds a value outside the specification or this subset.
    Invalid {
        /// Table holding the field.
        table: &'static str,
        /// Field that failed validation.
        field: &'static str,
    },
}

impl fmt::Display for TypefaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { table, offset } => {
                write!(
                    formatter,
                    "font table '{table}' ends before offset {offset}"
                )
            }
            Self::MissingTable(table) => write!(formatter, "font has no '{table}' table"),
            Self::Invalid { table, field } => {
                write!(formatter, "font table '{table}' has an invalid '{field}'")
            }
        }
    }
}

impl std::error::Error for TypefaceError {}

/// Glyph identifier within one typeface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphId(u16);

impl GlyphId {
    /// The missing-character glyph every TrueType font places first.
    pub const NOTDEF: Self = Self(0);

    /// The raw glyph index.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.0
    }
}

/// How `loca` stores glyph offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocationFormat {
    /// Offsets divided by two, as `u16`.
    Short,
    /// Byte offsets as `u32`.
    Long,
}

/// A parsed TrueType font over borrowed bytes.
///
/// # Examples
///
/// ```
/// use metis_platform::typeface;
///
/// let face = typeface::regular();
/// let glyph = face.glyph('A');
/// assert_ne!(glyph, typeface::GlyphId::NOTDEF);
/// // Advances are in font units; this face has 1000 units per em.
/// assert_eq!(face.units_per_em(), 1000);
/// assert!(face.advance(glyph) > 0);
/// ```
#[derive(Clone)]
pub struct Typeface<'font> {
    units_per_em: u16,
    /// Union of every glyph's box from its origin (`head.xMin`, `yMin`,
    /// `xMax`, `yMax`), y up.
    min_x: i16,
    min_y: i16,
    max_x: i16,
    max_y: i16,
    ascender: i16,
    descender: i16,
    line_gap: i16,
    glyph_count: u16,
    horizontal_metric_count: u16,
    location_format: LocationFormat,
    hmtx: Reader<'font>,
    loca: Reader<'font>,
    glyf: Reader<'font>,
    cmap: CharacterMap<'font>,
}

impl fmt::Debug for Typeface<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Typeface")
            .field("units_per_em", &self.units_per_em)
            .field("glyph_count", &self.glyph_count)
            .finish_non_exhaustive()
    }
}

impl<'font> Typeface<'font> {
    /// Parses a TrueType font.
    ///
    /// # Errors
    ///
    /// Returns [`TypefaceError`] when a required table is missing, truncated
    /// or holds a value outside the supported subset.
    pub fn parse(bytes: &'font [u8]) -> Result<Self, TypefaceError> {
        let font = Reader::new(bytes, "offset table");
        let version = font.read::<u32>(0)?;
        // 0x00010000 marks TrueType outlines; 'true' is Apple's equivalent.
        if version != 0x0001_0000 && version != u32::from_be_bytes(*b"true") {
            return Err(TypefaceError::Invalid {
                table: "offset table",
                field: "sfntVersion",
            });
        }
        let table_count = offset(font.read::<u16>(4)?);
        let table = |tag: &[u8; 4], name: &'static str| -> Result<Reader<'font>, TypefaceError> {
            for index in 0..table_count {
                let record = 12 + 16 * index;
                if font.slice(record, 4)? == tag {
                    let start = offset(font.read::<u32>(record + 8)?);
                    let length = offset(font.read::<u32>(record + 12)?);
                    return font.sub(start, length, name);
                }
            }
            Err(TypefaceError::MissingTable(name))
        };
        let head = table(b"head", "head")?;
        let hhea = table(b"hhea", "hhea")?;
        let maxp = table(b"maxp", "maxp")?;

        let units_per_em = head.read::<u16>(18)?;
        // The specification's valid range for unitsPerEm.
        if !(16..=16_384).contains(&units_per_em) {
            return Err(TypefaceError::Invalid {
                table: "head",
                field: "unitsPerEm",
            });
        }
        let location_format = match head.read::<i16>(50)? {
            0 => LocationFormat::Short,
            1 => LocationFormat::Long,
            _ => {
                return Err(TypefaceError::Invalid {
                    table: "head",
                    field: "indexToLocFormat",
                });
            }
        };
        let glyph_count = maxp.read::<u16>(4)?;
        let horizontal_metric_count = hhea.read::<u16>(34)?;
        if glyph_count == 0 || horizontal_metric_count == 0 || horizontal_metric_count > glyph_count
        {
            return Err(TypefaceError::Invalid {
                table: "hhea",
                field: "numberOfHMetrics",
            });
        }
        let face = Self {
            units_per_em,
            min_x: head.read::<i16>(36)?,
            min_y: head.read::<i16>(38)?,
            max_x: head.read::<i16>(40)?,
            max_y: head.read::<i16>(42)?,
            ascender: hhea.read::<i16>(4)?,
            descender: hhea.read::<i16>(6)?,
            line_gap: hhea.read::<i16>(8)?,
            glyph_count,
            horizontal_metric_count,
            location_format,
            hmtx: table(b"hmtx", "hmtx")?,
            loca: table(b"loca", "loca")?,
            glyf: table(b"glyf", "glyf")?,
            cmap: CharacterMap::parse(table(b"cmap", "cmap")?)?,
        };
        // Validate the metric and location arrays once, so lookups within
        // the glyph count cannot run past them later.
        let metrics =
            4 * offset(horizontal_metric_count) + 2 * offset(glyph_count - horizontal_metric_count);
        face.hmtx.slice(0, metrics)?;
        let entry = match location_format {
            LocationFormat::Short => 2,
            LocationFormat::Long => 4,
        };
        face.loca.slice(0, entry * (offset(glyph_count) + 1))?;
        Ok(face)
    }

    /// Font design units per em square.
    #[must_use]
    pub const fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// Distance from the baseline to the top of the line box, in font units.
    #[must_use]
    pub const fn ascender(&self) -> i16 {
        self.ascender
    }

    /// Leftmost extent of any glyph relative to its origin, in font units.
    ///
    /// Negative when some glyph overhangs to the left of its pen position.
    #[must_use]
    pub const fn min_x(&self) -> i16 {
        self.min_x
    }

    /// Rightmost extent of any glyph relative to its origin, in font units.
    #[must_use]
    pub const fn max_x(&self) -> i16 {
        self.max_x
    }

    /// Lowest extent of any glyph relative to the baseline, in font units,
    /// negative below it.
    #[must_use]
    pub const fn min_y(&self) -> i16 {
        self.min_y
    }

    /// Highest extent of any glyph relative to the baseline, in font units.
    #[must_use]
    pub const fn max_y(&self) -> i16 {
        self.max_y
    }

    /// Height of one line box — ascender, descender and line gap — in font
    /// units.
    #[must_use]
    pub fn line_height(&self) -> i32 {
        i32::from(self.ascender) - i32::from(self.descender) + i32::from(self.line_gap)
    }

    /// Glyph for a character; characters the font lacks map to
    /// [`GlyphId::NOTDEF`].
    #[must_use]
    pub fn glyph(&self, character: char) -> GlyphId {
        self.cmap
            .lookup(character)
            .filter(|glyph| *glyph < self.glyph_count)
            .map_or(GlyphId::NOTDEF, GlyphId)
    }

    /// Horizontal advance of a glyph in font units.
    ///
    /// # Panics
    ///
    /// Does not panic: parsing validated the `hmtx` array for every glyph
    /// index this lookup can reach.
    #[must_use]
    pub fn advance(&self, glyph: GlyphId) -> u16 {
        // Glyphs past the last full metric repeat its advance.
        let index = glyph.0.min(self.horizontal_metric_count - 1);
        self.hmtx
            .read::<u16>(4 * offset(index))
            .expect("invariant: parse validated the hmtx array for every glyph")
    }

    /// Byte range of a glyph's outline within `glyf`; empty for a glyph
    /// without contours.
    fn outline_range(&self, glyph: GlyphId) -> Result<(usize, usize), TypefaceError> {
        // A component naming a glyph past the count is malformed, not a
        // request for the last glyph.
        if glyph.0 >= self.glyph_count {
            return Err(TypefaceError::Invalid {
                table: "glyf",
                field: "glyph index",
            });
        }
        let index = offset(glyph.0);
        let (start, end) = match self.location_format {
            LocationFormat::Short => (
                2 * offset(self.loca.read::<u16>(2 * index)?),
                2 * offset(self.loca.read::<u16>(2 * index + 2)?),
            ),
            LocationFormat::Long => (
                offset(self.loca.read::<u32>(4 * index)?),
                offset(self.loca.read::<u32>(4 * index + 4)?),
            ),
        };
        if end < start {
            return Err(TypefaceError::Invalid {
                table: "loca",
                field: "offsets",
            });
        }
        Ok((start, end))
    }

    /// The bounding box a glyph's header declares, in font units, or `None`
    /// for a glyph without contours.
    #[cfg(test)]
    fn glyph_bounds(&self, glyph: GlyphId) -> Result<Option<[i16; 4]>, TypefaceError> {
        let (start, end) = self.outline_range(glyph)?;
        if start == end {
            return Ok(None);
        }
        let header = self.glyf.sub(start, 10, "glyf")?;
        Ok(Some([
            header.read::<i16>(2)?,
            header.read::<i16>(4)?,
            header.read::<i16>(6)?,
            header.read::<i16>(8)?,
        ]))
    }
}

#[cfg(test)]
#[path = "typeface_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "typeface/coverage_tests.rs"]
mod coverage_tests;
