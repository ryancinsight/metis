//! Contrast of white labels over the themed fills.

use super::super::{BADGE_CLOSED, BADGE_READY};
use super::ThemePalette;
use crate::commands::ApplicationTheme;
use metis_ui_lang::Color;

/// WCAG 2.2 relative luminance of an opaque sRGB color.
fn luminance(color: Color) -> f64 {
    let linear = |channel: u8| {
        let value = f64::from(channel) / 255.0;
        if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126f64.mul_add(
        linear(color.r),
        0.7152f64.mul_add(linear(color.g), 0.0722 * linear(color.b)),
    )
}

/// WCAG 2.2 contrast ratio between two opaque colors.
fn contrast(first: Color, second: Color) -> f64 {
    let (first, second) = (luminance(first), luminance(second));
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

fn white_contrast(background: Color) -> f64 {
    contrast(Color::WHITE, background)
}

/// Fractions sampled along each fill; the extremes are the stops and the
/// interior points guard the premultiplied interpolation between them.
const SAMPLES: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

#[test]
fn white_labels_keep_minimum_contrast_across_every_themed_fill() {
    // Criterion 1.4.3: 4.5:1 for text below 18.66 px bold. Control labels
    // are 14 px bold; the header's 22 px bold title would need only 3:1.
    for theme in [ApplicationTheme::System, ApplicationTheme::Dark] {
        let palette = ThemePalette::for_theme(theme);
        for (name, fill) in [("control", &palette.control), ("header", &palette.header)] {
            for fraction in SAMPLES {
                let color = fill.color_at_fraction(fraction);
                let ratio = white_contrast(color);
                assert!(
                    ratio >= 4.5,
                    "{theme:?} {name} at {fraction}: {color:?} gives {ratio:.2}:1"
                );
            }
        }
    }
}

#[test]
fn status_badge_colors_stay_legible_over_both_headers() {
    // 13 px regular text needs 4.5:1 across the whole header gradient.
    for theme in [ApplicationTheme::System, ApplicationTheme::Dark] {
        let header = ThemePalette::for_theme(theme).header;
        for badge in [BADGE_READY, BADGE_CLOSED] {
            for fraction in SAMPLES {
                let background = header.color_at_fraction(fraction);
                let ratio = contrast(badge, background);
                assert!(
                    ratio >= 4.5,
                    "{theme:?} {badge:?} over {background:?}: {ratio:.2}:1"
                );
            }
        }
    }
}

#[test]
fn contrast_matches_the_published_reference_points() {
    // WCAG's end points: white on black is 21:1 and white on white 1:1.
    assert!((white_contrast(Color::BLACK) - 21.0).abs() < 1e-12);
    assert!((white_contrast(Color::WHITE) - 1.0).abs() < 1e-12);
}
