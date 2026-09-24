//! Value tests for the authored form's projection and paint.

use super::{BADGE_CLOSED, BADGE_READY, CLINICAL_SCREEN_XML, bounded_accessible_value};
use metis_platform::{Color, Framebuffer};
use metis_ui_lang::{
    DisplayCommand, LayoutViewport, MAX_SEMANTIC_TEXT_BYTES, compute_layout, parse_markup,
};

#[test]
fn accessible_patient_value_is_bounded_without_splitting_utf8() {
    let short = "PT-9042-ALPHA";
    assert_eq!(bounded_accessible_value(short), short);
    let exact = "x".repeat(MAX_SEMANTIC_TEXT_BYTES);
    assert_eq!(bounded_accessible_value(&exact), exact);

    let oversized = "é".repeat(MAX_SEMANTIC_TEXT_BYTES);
    let bounded = bounded_accessible_value(&oversized);
    assert!(bounded.len() <= MAX_SEMANTIC_TEXT_BYTES);
    assert!(bounded.ends_with("..."));
    assert!(bounded.is_char_boundary(bounded.len() - 3));
}

/// Inked pixels inside `bounds` counted by the candidate ink that best
/// explains each.
///
/// Antialiased strokes at small sizes may never reach full coverage, so a
/// single probe pixel is not an oracle, and a nearest-color vote misreads
/// half-covered edges, whose blend with the background can lie nearer
/// another candidate than the ink that produced them. Each pixel is instead
/// fitted as a blend of the background toward each ink: the run's own ink
/// fits its strokes exactly and collects them, and the others collect
/// nothing.
fn ink_counts<const N: usize>(
    framebuffer: &Framebuffer,
    bounds: (i32, i32, i32, i32),
    inks: [Color; N],
) -> [usize; N] {
    let (x, y, width, height) = bounds;
    // The header just left of the run is the background the glyphs blend
    // over; sampling it keeps the model true on a gradient header.
    let background = framebuffer.get_pixel(x - 4, y + height / 2);
    let channels = |color: Color| [color.r, color.g, color.b].map(f64::from);
    let base = channels(background);
    let mut counts = [0; N];
    for row in y..y + height {
        for column in x..x + width {
            let pixel = channels(framebuffer.get_pixel(column, row));
            // An antialiased pixel is `background + a * (ink - background)`:
            // project onto each ink's blend line and keep the closest.
            let fits = inks.map(|ink| {
                let ink = channels(ink);
                let (mut along, mut length) = (0.0, 0.0);
                for channel in 0..3 {
                    along += (pixel[channel] - base[channel]) * (ink[channel] - base[channel]);
                    length += (ink[channel] - base[channel]).powi(2);
                }
                let coverage = (along / length).clamp(0.0, 1.0);
                let residual: f64 = (0..3)
                    .map(|channel| {
                        let blend = coverage.mul_add(ink[channel] - base[channel], base[channel]);
                        (pixel[channel] - blend).powi(2)
                    })
                    .sum();
                (coverage, residual)
            });
            let (index, (coverage, _)) = fits
                .iter()
                .enumerate()
                .min_by(|left, right| left.1.1.total_cmp(&right.1.1))
                .expect("invariant: at least one ink");
            // Pixels at least half inked count; fainter edges carry too little
            // of the ink to tell candidates apart.
            if *coverage >= 0.5 {
                counts[index] += 1;
            }
        }
    }
    counts
}

/// Rounds a small nonnegative extent up to a whole pixel count.
fn whole(extent: f64) -> i32 {
    let rounded = extent.ceil();
    assert!((0.0..4096.0).contains(&rounded), "extent {extent}");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole value checked to lie in 0..4096"
    )]
    let pixels = rounded as i32;
    pixels
}

#[test]
fn authored_form_text_and_status_fit_the_viewport() {
    let document = parse_markup(CLINICAL_SCREEN_XML).expect("authored markup");
    let display =
        compute_layout(&document, LayoutViewport::new(800, 600)).expect("authored layout");
    let mut text_runs = Vec::new();
    for command in &display.commands {
        if let DisplayCommand::DrawText { text, x, y, style } = command {
            let width = style.advance(text);
            let height = style.line_height();
            assert!(
                *x >= 0 && f64::from(*x) + width <= 800.0,
                "horizontal clipping: {text} x={x} width={width}"
            );
            assert!(
                *y >= 0 && f64::from(*y) + height <= 600.0,
                "vertical clipping: {text}"
            );
            text_runs.push((text.as_str(), *x, *y));
        }
    }
    assert!(
        text_runs
            .iter()
            .any(|run| run.0 == "METIS FORM DEMONSTRATION")
    );
    assert!(text_runs.iter().any(|run| run.0 == "SYSTEM READY"));
    assert!(
        text_runs
            .iter()
            .any(|run| run.0 == "Patient Demographics and Drug Prescription")
    );
    let mut framebuffer = Framebuffer::new(800, 600).expect("presentation surface");
    display.render_to(&mut framebuffer);
    // The status run paints in the ready color over the header.
    let status = display
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::DrawText { text, x, y, style } if text == "SYSTEM READY" => {
                Some((*x, *y, style.advance(text), style.line_height()))
            }
            _ => None,
        })
        .expect("status run");
    let bounds = (status.0, status.1, whole(status.2), whole(status.3));
    let [ready, closed] = ink_counts(&framebuffer, bounds, [BADGE_READY, BADGE_CLOSED]);
    assert!(ready > 20 && closed == 0, "ready {ready}, closed {closed}");
    assert_eq!(framebuffer.get_pixel(799, 599), Color::rgb(240, 244, 248));
}
