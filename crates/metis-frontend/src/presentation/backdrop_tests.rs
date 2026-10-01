//! The backdrop clear is skipped only where the display list proves it is
//! overwritten, and every skip leaves the pixels a clearing paint would.

use super::{paint, replaces_clip};
use crate::presentation::BACKDROP;
use iris::render::RenderBackend;
use metis_platform::rasterizer::CornerRadius;
use metis_platform::{Color, Framebuffer, Rect};
use metis_ui_lang::{DisplayCommand, DisplayList};

const WIDTH: u32 = 8;
const HEIGHT: u32 = 6;
/// Stands for the previous frame's pixels: neither the backdrop nor any
/// fill color used below.
const STALE: Color = Color::rgb(1, 2, 3);
const ROOT: Color = Color::rgb(10, 20, 30);
const CARD: Color = Color::rgb(200, 100, 50);

fn surface() -> Framebuffer {
    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT).expect("small surface");
    framebuffer.clear(STALE);
    framebuffer
}

fn whole() -> Rect {
    Rect::new(0, 0, 8, 6)
}

fn fill(rect: Rect, color: Color) -> DisplayCommand {
    DisplayCommand::FillRect {
        rect,
        radius: CornerRadius::SQUARE,
        color,
    }
}

fn list(commands: Vec<DisplayCommand>) -> DisplayList {
    DisplayList { commands }
}

/// Paints `display` into `region` of a stale surface.
fn painted(display: &DisplayList, region: Rect) -> Framebuffer {
    let mut framebuffer = surface();
    framebuffer.render_clipped(region, |framebuffer| paint(framebuffer, display));
    framebuffer
}

/// What a paint that always clears the region first leaves.
fn cleared_first(display: &DisplayList, region: Rect) -> Framebuffer {
    let mut framebuffer = surface();
    framebuffer.render_clipped(region, |framebuffer| {
        framebuffer.clear(BACKDROP);
        framebuffer
            .render(display)
            .unwrap_or_else(|never| match never {});
    });
    framebuffer
}

fn replaces(display: &DisplayList, region: Rect) -> bool {
    surface().render_clipped(region, |framebuffer| {
        replaces_clip(display, framebuffer.clip())
    })
}

fn assert_matches_a_clearing_paint(display: &DisplayList, region: Rect) {
    assert_eq!(
        painted(display, region).pixels(),
        cleared_first(display, region).pixels()
    );
}

#[test]
fn an_opaque_root_over_the_surface_replaces_the_clear() {
    let card = Rect::new(2, 2, 3, 2);
    let display = list(vec![fill(whole(), ROOT), fill(card, CARD)]);
    assert!(replaces(&display, whole()));
    let result = painted(&display, whole());
    assert_matches_a_clearing_paint(&display, whole());
    assert_eq!(result.get_pixel(0, 0), ROOT);
    assert_eq!(result.get_pixel(7, 5), ROOT);
    assert_eq!(result.get_pixel(2, 2), CARD);
    assert_eq!(result.get_pixel(4, 3), CARD);
    assert_eq!(result.get_pixel(5, 3), ROOT);
}

#[test]
fn a_translucent_root_keeps_the_clear() {
    let veil = Color::rgba(10, 20, 30, 128);
    let display = list(vec![fill(whole(), veil)]);
    assert!(!replaces(&display, whole()));
    assert_matches_a_clearing_paint(&display, whole());

    let over = |base: Color| {
        let mut reference = Framebuffer::new(1, 1).expect("one pixel");
        reference.clear(base);
        reference.blend_pixel(0, 0, veil);
        reference.get_pixel(0, 0)
    };
    let result = painted(&display, whole());
    assert_eq!(result.get_pixel(3, 3), over(BACKDROP));
    assert_ne!(over(BACKDROP), over(STALE), "the stale pixels would show");
}

#[test]
fn a_root_short_of_the_surface_keeps_the_clear() {
    let display = list(vec![fill(Rect::new(0, 0, 5, 6), ROOT)]);
    assert!(!replaces(&display, whole()));
    let result = painted(&display, whole());
    assert_matches_a_clearing_paint(&display, whole());
    assert_eq!(result.get_pixel(4, 5), ROOT);
    assert_eq!(result.get_pixel(5, 0), BACKDROP);
    assert_eq!(result.get_pixel(7, 5), BACKDROP);
}

#[test]
fn a_root_off_one_edge_keeps_the_clear() {
    for rect in [
        Rect::new(1, 0, 7, 6),
        Rect::new(0, 1, 8, 5),
        Rect::new(0, 0, 8, 5),
        Rect::new(0, 0, 7, 6),
    ] {
        let display = list(vec![fill(rect, ROOT)]);
        assert!(!replaces(&display, whole()), "{rect:?}");
        assert_matches_a_clearing_paint(&display, whole());
    }
}

#[test]
fn a_rounded_opaque_root_keeps_the_clear() {
    let radius = CornerRadius::clamped(3, whole());
    assert!(!radius.is_square());
    let display = list(vec![DisplayCommand::FillRect {
        rect: whole(),
        radius,
        color: ROOT,
    }]);
    assert!(!replaces(&display, whole()));
    let result = painted(&display, whole());
    assert_matches_a_clearing_paint(&display, whole());
    assert_ne!(result.get_pixel(0, 0), STALE);
    assert_eq!(result.get_pixel(4, 3), ROOT);
}

#[test]
fn an_empty_list_clears_to_the_backdrop() {
    let display = list(Vec::new());
    assert!(!replaces(&display, whole()));
    let result = painted(&display, whole());
    assert_matches_a_clearing_paint(&display, whole());
    assert_eq!(result.get_pixel(0, 0), BACKDROP);
    assert_eq!(result.get_pixel(7, 5), BACKDROP);
    assert_eq!(result.get_pixel(3, 2), BACKDROP);
}

#[test]
fn a_cover_hides_the_commands_before_it_and_composes_with_those_after() {
    let veil = Color::rgba(250, 0, 0, 100);
    let display = list(vec![
        fill(whole(), veil),
        fill(whole(), ROOT),
        fill(Rect::new(2, 2, 2, 2), veil),
    ]);
    assert!(replaces(&display, whole()));
    let result = painted(&display, whole());
    assert_matches_a_clearing_paint(&display, whole());
    assert_eq!(result.get_pixel(0, 0), ROOT);
    assert_ne!(result.get_pixel(2, 2), ROOT);
}

#[test]
fn a_root_covering_only_the_clip_replaces_the_clear_inside_it() {
    let region = Rect::new(2, 1, 4, 3);
    let display = list(vec![fill(region, ROOT)]);
    assert!(replaces(&display, region));
    assert!(!replaces(&display, whole()));
    let result = painted(&display, region);
    assert_matches_a_clearing_paint(&display, region);
    assert_eq!(result.get_pixel(2, 1), ROOT);
    assert_eq!(result.get_pixel(5, 3), ROOT);
    assert_eq!(
        result.get_pixel(1, 1),
        STALE,
        "outside the clip is untouched"
    );
    assert_eq!(result.get_pixel(6, 3), STALE);
}
