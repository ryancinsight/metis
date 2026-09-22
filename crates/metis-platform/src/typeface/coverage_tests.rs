//! Coverage rules and composite placement.
//!
//! The overlap tests check the premise that selects area accumulation — no
//! overlapping contours in a single component — and bound the nonzero rule
//! against an independent point-sampled reference. The composite tests build
//! a font byte by byte, so every transform branch is checked against bounds
//! and areas derived by hand from the OpenType definition.

use super::glyf::{Point, Transform};
use super::raster::{Canvas, NONZERO_SUBROWS, Outline};
use super::{GlyphId, Typeface, TypefaceError, bold, regular};

/// Crossings of a horizontal line with the segments, sorted by x, each with
/// its winding direction.
fn crossings(lines: &[(Point, Point)], y: f64) -> Vec<(f64, i32)> {
    let mut found: Vec<(f64, i32)> = lines
        .iter()
        .filter(|(from, to)| (from.y <= y) != (to.y <= y))
        .map(|(from, to)| {
            let x = (y - from.y) / (to.y - from.y) * (to.x - from.x) + from.x;
            (x, if from.y < to.y { 1 } else { -1 })
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found
}

fn outline_of(face: &Typeface<'_>, glyph: GlyphId, size: f64) -> Outline {
    let scale = size / f64::from(face.units_per_em());
    let mut outline = Outline::default();
    face.outline(glyph, &Transform::device(scale, 3.3, 40.7), &mut outline)
        .expect("embedded glyph decodes");
    outline
}

#[test]
fn single_component_glyphs_are_overlap_free() {
    // Accumulation takes the magnitude of the summed signed area. It equals
    // the nonzero rule exactly wherever the winding number is 0 or one fixed
    // sign, so checking that on dense rows of every simple glyph proves the
    // premise that selects it.
    for face in [regular(), bold()] {
        for index in 0..face.glyph_count {
            let outline = outline_of(face, GlyphId(index), 24.0);
            if outline.components() > 1 {
                continue;
            }
            let Some(bounds) = outline.bounds() else {
                continue;
            };
            let mut sign = 0;
            for sample in 0..bounds.height() * 8 {
                let y = f64::from(bounds.top)
                    + (f64::from(u32::try_from(sample).expect("small")) + 0.5) / 8.0;
                let mut winding = 0;
                for (_, direction) in crossings(outline.lines(), y) {
                    winding += direction;
                    if winding != 0 {
                        assert_eq!(winding.abs(), 1, "glyph {index}: winding {winding}");
                        assert!(sign == 0 || sign == winding, "glyph {index}: mixed signs");
                        sign = winding;
                    }
                }
            }
        }
    }
}

#[test]
fn overlapping_components_cover_their_union() {
    // The ring of 'Å' and the cedilla of 'Ç' overlap their base letters. The
    // reference samples the winding number on the rasterizer's own sample
    // rows, 64 points across each pixel, so the two agree row by row except
    // for the reference's horizontal sampling: at most half a sample column
    // for each edge crossing the pixel on that row.
    let samples = 64_usize;
    let size = 9.0;
    for face in [regular(), bold()] {
        for character in ['\u{c5}', '\u{c7}'] {
            let outline = outline_of(face, face.glyph(character), size);
            assert!(outline.components() > 1, "{character:?} is a composite");
            let bounds = outline.bounds().expect("inked glyph");
            let mut canvas = Canvas::default();
            outline.rasterize(bounds, &mut canvas);
            let (width, height) = (bounds.width(), bounds.height());
            let per_pixel = f64::from(u32::try_from(samples).expect("small"));
            let offset =
                |index: usize| (f64::from(u32::try_from(index).expect("small")) + 0.5) / per_pixel;
            let mut covered = vec![0_u32; width * height];
            let mut crossing_count = vec![0_u32; width * height];
            for row in 0..height {
                let top = f64::from(bounds.top) + f64::from(u32::try_from(row).expect("small"));
                for sample in 0..samples {
                    let y = (f64::from(u32::try_from(sample).expect("small")) + 0.5)
                        .mul_add(1.0 / f64::from(NONZERO_SUBROWS), top);
                    let found = crossings(outline.lines(), y);
                    for (at, _) in &found {
                        let column = (at - f64::from(bounds.left)).floor();
                        if (0.0..f64::from(u32::try_from(width).expect("small"))).contains(&column)
                        {
                            #[expect(
                                clippy::cast_possible_truncation,
                                clippy::cast_sign_loss,
                                reason = "a whole column checked to lie inside the bounds"
                            )]
                            let column = column as usize;
                            crossing_count[row * width + column] += 1;
                        }
                    }
                    for column in 0..width {
                        let left = f64::from(bounds.left)
                            + f64::from(u32::try_from(column).expect("small"));
                        for point in 0..samples {
                            let x = left + offset(point);
                            let winding: i32 =
                                found.iter().filter(|(at, _)| *at < x).map(|(_, d)| d).sum();
                            covered[row * width + column] += u32::from(winding != 0);
                        }
                    }
                }
            }
            let points = per_pixel * per_pixel;
            for (index, value) in canvas.coverage.iter().enumerate() {
                let reference = f64::from(covered[index]) / points;
                let bound = f64::from(crossing_count[index]) / (2.0 * points) + 1e-9;
                assert!(
                    (value - reference).abs() <= bound,
                    "{character:?} pixel {index}: {value} against {reference} (bound {bound})"
                );
            }
        }
    }
}

// A minimal TrueType font assembled table by table.

const ARGS_ARE_WORDS: u16 = 0x0001;
const ARGS_ARE_XY_VALUES: u16 = 0x0002;
const HAVE_A_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const HAVE_X_AND_Y_SCALE: u16 = 0x0040;
const HAVE_TWO_BY_TWO: u16 = 0x0080;
const SCALED_COMPONENT_OFFSET: u16 = 0x0800;

fn be16(value: i32) -> [u8; 2] {
    i16::try_from(value)
        .map(i16::to_be_bytes)
        .or_else(|_| u16::try_from(value).map(u16::to_be_bytes))
        .expect("a 16-bit field")
}

/// F2DOT14 encoding of a fixed-point scale.
fn fixed(value: f64) -> [u8; 2] {
    let raw = (value * 16_384.0).round();
    assert!((-32_768.0..32_768.0).contains(&raw));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole value checked to lie in the i16 range"
    )]
    let raw = raw as i32;
    be16(raw)
}

/// A one-contour rectangle `[0, width] x [0, height]` in font units.
fn rectangle(width: i32, height: i32) -> Vec<u8> {
    let mut glyph = Vec::new();
    for value in [1, 0, 0, width, height, 3, 0] {
        glyph.extend(be16(value));
    }
    glyph.extend([1, 1, 1, 1]);
    for delta in [0, width, 0, -width, 0, 0, height, 0] {
        glyph.extend(be16(delta));
    }
    glyph
}

/// One placement in a composite: flags, component, arguments and scale bytes.
struct Placement {
    flags: u16,
    glyph: u16,
    arguments: Vec<u8>,
    scale: Vec<u8>,
}

fn composite(placements: &[Placement]) -> Vec<u8> {
    let mut glyph = Vec::new();
    for value in [-1, 0, 0, 0, 0] {
        glyph.extend(be16(value));
    }
    for (index, placement) in placements.iter().enumerate() {
        let more = if index + 1 < placements.len() {
            MORE_COMPONENTS
        } else {
            0
        };
        glyph.extend((placement.flags | more).to_be_bytes());
        glyph.extend(placement.glyph.to_be_bytes());
        glyph.extend(&placement.arguments);
        glyph.extend(&placement.scale);
    }
    glyph
}

fn words(x: i32, y: i32) -> Vec<u8> {
    [be16(x), be16(y)].concat()
}

/// Assembles a font whose glyphs map from `A` onward, glyph 0 being blank.
fn font(glyphs: &[Vec<u8>]) -> Vec<u8> {
    let count = i32::try_from(glyphs.len()).expect("small");
    let (mut glyf, mut loca) = (Vec::new(), Vec::new());
    for glyph in glyphs {
        loca.extend(be16(i32::try_from(glyf.len() / 2).expect("small")));
        glyf.extend(glyph);
        if glyf.len() % 2 == 1 {
            glyf.push(0);
        }
    }
    loca.extend(be16(i32::try_from(glyf.len() / 2).expect("small")));
    let mut head = vec![0; 54];
    head[..4].copy_from_slice(&0x0001_0000_u32.to_be_bytes());
    head[18..20].copy_from_slice(&be16(1000));
    let mut hhea = vec![0; 36];
    hhea[4..6].copy_from_slice(&be16(800));
    hhea[6..8].copy_from_slice(&be16(-200));
    hhea[34..36].copy_from_slice(&be16(count));
    let mut maxp = vec![0; 6];
    maxp[4..6].copy_from_slice(&be16(count));
    let hmtx: Vec<u8> = (0..count)
        .flat_map(|_| [be16(500), be16(0)].concat())
        .collect();
    let last = 0x41 + count - 2;
    let mut cmap = Vec::new();
    for value in [0, 1, 3, 1] {
        cmap.extend(be16(value));
    }
    cmap.extend(12_u32.to_be_bytes());
    for value in [
        4,
        32,
        0,
        4,
        4,
        1,
        0,
        last,
        0xFFFF,
        0,
        0x41,
        0xFFFF,
        1 - 0x41 + 65_536,
        1,
        0,
        0,
    ] {
        cmap.extend(be16(value));
    }
    let tables: [(&[u8; 4], &Vec<u8>); 7] = [
        (b"cmap", &cmap),
        (b"glyf", &glyf),
        (b"head", &head),
        (b"hhea", &hhea),
        (b"hmtx", &hmtx),
        (b"loca", &loca),
        (b"maxp", &maxp),
    ];
    let mut bytes = Vec::new();
    bytes.extend(0x0001_0000_u32.to_be_bytes());
    for value in [7, 0, 0, 0] {
        bytes.extend(be16(value));
    }
    let mut offset = 12 + 16 * tables.len();
    let mut body: Vec<u8> = Vec::new();
    for (tag, data) in tables {
        bytes.extend(tag);
        bytes.extend(0_u32.to_be_bytes());
        bytes.extend(u32::try_from(offset).expect("small").to_be_bytes());
        bytes.extend(u32::try_from(data.len()).expect("small").to_be_bytes());
        body.extend(data.iter());
        offset += data.len();
    }
    bytes.extend(body);
    bytes
}

/// Outline bounds at one pixel per unit, y flipped back to font orientation.
fn placed(face: &Typeface<'_>, glyph: u16) -> Result<([i32; 4], f64), TypefaceError> {
    let mut outline = Outline::default();
    face.outline(
        GlyphId(glyph),
        &Transform::device(1.0, 0.0, 0.0),
        &mut outline,
    )?;
    let bounds = outline.bounds().expect("inked composite");
    let mut canvas = Canvas::default();
    outline.rasterize(bounds, &mut canvas);
    let area = canvas.coverage.iter().sum();
    Ok((
        [bounds.left, -bounds.bottom, bounds.right, -bounds.top],
        area,
    ))
}

#[test]
fn composite_transforms_place_components_as_the_specification_defines() {
    let shear = Placement {
        flags: ARGS_ARE_WORDS | ARGS_ARE_XY_VALUES | HAVE_TWO_BY_TWO,
        glyph: 1,
        arguments: words(10, 20),
        // xscale, scale01, scale10, yscale: x' = x + 10, y' = 0.5x + y + 20.
        scale: [fixed(1.0), fixed(0.5), fixed(0.0), fixed(1.0)].concat(),
    };
    let stretch = Placement {
        flags: ARGS_ARE_WORDS | ARGS_ARE_XY_VALUES | HAVE_X_AND_Y_SCALE | SCALED_COMPONENT_OFFSET,
        glyph: 1,
        arguments: words(100, 40),
        // The offset scales with the component: (150, 20).
        scale: [fixed(1.5), fixed(0.5)].concat(),
    };
    let halved = Placement {
        flags: ARGS_ARE_XY_VALUES | HAVE_A_SCALE,
        glyph: 1,
        arguments: vec![0xf8, 5],
        scale: fixed(0.5).to_vec(),
    };
    let nested = Placement {
        flags: ARGS_ARE_WORDS | ARGS_ARE_XY_VALUES,
        glyph: 2,
        arguments: words(0, 300),
        scale: Vec::new(),
    };
    let missing = Placement {
        flags: ARGS_ARE_WORDS | ARGS_ARE_XY_VALUES,
        glyph: 99,
        arguments: words(0, 0),
        scale: Vec::new(),
    };
    let cyclic = Placement {
        flags: ARGS_ARE_WORDS | ARGS_ARE_XY_VALUES,
        glyph: 7,
        arguments: words(0, 0),
        scale: Vec::new(),
    };
    let bytes = font(&[
        Vec::new(),
        rectangle(200, 100),
        composite(&[shear]),
        composite(&[stretch]),
        composite(&[halved]),
        composite(&[nested]),
        composite(&[missing]),
        composite(&[cyclic]),
    ]);
    let face = Typeface::parse(&bytes).expect("synthetic font parses");
    assert_eq!(face.glyph('B'), GlyphId(2));

    // Corners (0,0), (200,0), (200,100), (0,100) map through each transform;
    // the area scales by the determinant of the 2 x 2 part.
    let cases = [
        (1, [0, 0, 200, 100], 20_000.0),
        (2, [10, 20, 210, 220], 20_000.0),
        (3, [150, 20, 450, 70], 15_000.0),
        (4, [-8, 5, 92, 55], 5_000.0),
        (5, [10, 320, 210, 520], 20_000.0),
    ];
    for (glyph, bounds, area) in cases {
        let (found, covered) = placed(&face, glyph).expect("composite decodes");
        assert_eq!(found, bounds, "glyph {glyph}");
        // The rectangle's edges land on whole pixels except where the shear
        // slants them, and coverage is exact area either way.
        assert!(
            (covered - area).abs() < 1e-6,
            "glyph {glyph}: {covered} against {area}"
        );
    }
    assert!(matches!(
        placed(&face, 6),
        Err(TypefaceError::Invalid {
            field: "glyph index",
            ..
        })
    ));
    assert!(matches!(
        placed(&face, 7),
        Err(TypefaceError::Invalid {
            field: "component depth",
            ..
        })
    ));
}
