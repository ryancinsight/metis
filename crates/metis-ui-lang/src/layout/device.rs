//! Resolution of authored lengths to device pixels.
//!
//! Every authored length passes through the host display scale exactly once,
//! here, so layout arithmetic downstream works in one unit.

use crate::parser::limit_error;
use crate::style::{ComputedStyle, EdgeValues, Shadow, Size};
use metis_core::error::Result;
use metis_platform::DisplayScale;
use metis_platform::rasterizer::BoxShadow;

/// Box-model edges and gap resolved to device pixels.
#[derive(Clone, Copy)]
pub(super) struct ScaledGeometry {
    pub(super) margin: EdgeValues,
    pub(super) padding: EdgeValues,
    pub(super) border: EdgeValues,
    pub(super) gap: i32,
}

pub(super) fn scaled_geometry(
    style: &ComputedStyle,
    display_scale: DisplayScale,
) -> Result<ScaledGeometry> {
    Ok(ScaledGeometry {
        margin: scale_edges(style.margin, display_scale)?,
        padding: scale_edges(style.padding, display_scale)?,
        border: scale_edges(style.border_width, display_scale)?,
        gap: display_scale.scale_coordinate(style.gap)?,
    })
}

fn scale_edges(edges: EdgeValues, display_scale: DisplayScale) -> Result<EdgeValues> {
    Ok(EdgeValues {
        top: display_scale.scale_coordinate(edges.top)?,
        right: display_scale.scale_coordinate(edges.right)?,
        bottom: display_scale.scale_coordinate(edges.bottom)?,
        left: display_scale.scale_coordinate(edges.left)?,
    })
}

/// Resolves the floor an extent may not fall below.
///
/// `Size::Auto` states no minimum. A declared minimum uses the same length
/// grammar and display scaling as `width`/`height`, so a minimum and an extent
/// expressed the same way resolve to the same number.
pub(super) fn minimum(size: Size, available: i32, display_scale: DisplayScale) -> Result<i32> {
    match size {
        Size::Auto => Ok(0),
        declared => dimension(declared, available, 0, display_scale),
    }
}

pub(super) fn dimension(
    size: Size,
    available: i32,
    automatic: i32,
    display_scale: DisplayScale,
) -> Result<i32> {
    match size {
        Size::Auto => Ok(automatic),
        Size::Px(value) if value >= 0 => display_scale.scale_extent(value),
        Size::Percent(percent) if percent.is_finite() && percent >= 0.0 => {
            // The percentage contract is f32; multiplication remains in that precision.
            #[expect(
                clippy::cast_precision_loss,
                reason = "CSS percentage sizing uses f32 coordinates"
            )]
            let value = available as f32 * percent;
            #[expect(
                clippy::cast_precision_loss,
                reason = "i32::MAX rounds to the first excluded f32 coordinate"
            )]
            if !value.is_finite() || value >= i32::MAX as f32 {
                return Err(limit_error("Percentage size exceeds coordinate range"));
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "Finite nonnegative value is checked below the i32 upper bound; fractional pixels truncate"
            )]
            Ok(value as i32)
        }
        _ => Err(limit_error("Size must be finite and nonnegative")),
    }
}

/// Scales a shadow's offsets and blur to device pixels.
///
/// # Errors
/// Returns a layout limit error when a scaled offset leaves the coordinate
/// range or the scaled blur exceeds [`BoxShadow::MAX_BLUR`].
pub(super) fn device_shadow(shadow: Shadow, display_scale: DisplayScale) -> Result<BoxShadow> {
    // `scale_extent` rejects a negative extent, so the scaled blur is its
    // own magnitude.
    let blur = display_scale.scale_extent(shadow.blur)?.unsigned_abs();
    BoxShadow::new(
        display_scale.scale_coordinate(shadow.offset_x)?,
        display_scale.scale_coordinate(shadow.offset_y)?,
        blur,
        shadow.color,
    )
    .ok_or_else(|| limit_error("Shadow blur exceeds the renderer's blur bound"))
}
