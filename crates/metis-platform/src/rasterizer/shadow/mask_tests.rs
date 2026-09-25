//! The memo returns what a fresh render would: a thread's cache is its own,
//! so a scene drawn on a new thread is the oracle for the same scene drawn
//! after unrelated shadows on this one.

use super::RENDERS;
use crate::framebuffer::{Color, Framebuffer, Rect};
use crate::rasterizer::{BoxShadow, CornerRadius, draw_box_shadow};

fn surface() -> Framebuffer {
    let mut fb = Framebuffer::new(160, 120).expect("test surface");
    fb.clear(Color::rgb(236, 240, 244));
    fb
}

fn shadow(offset: (i32, i32), blur: u32, alpha: u8) -> BoxShadow {
    BoxShadow::new(offset.0, offset.1, blur, Color::rgba(15, 23, 42, alpha)).expect("bounded blur")
}

/// Shadows that differ in every keyed field from the first one.
fn scenes() -> Vec<(Rect, i32, BoxShadow)> {
    let base = Rect::new(30, 20, 80, 50);
    vec![
        (base, 12, shadow((0, 4), 16, 40)),
        (base, 12, shadow((0, 4), 16, 200)),
        (base, 12, shadow((3, -2), 16, 40)),
        (base, 12, shadow((0, 4), 6, 40)),
        (base, 4, shadow((0, 4), 16, 40)),
        (Rect::new(31, 20, 80, 50), 12, shadow((0, 4), 16, 40)),
        (Rect::new(30, 20, 81, 50), 12, shadow((0, 4), 16, 40)),
    ]
}

fn draw(scene: &[(Rect, i32, BoxShadow)]) -> Framebuffer {
    let mut fb = surface();
    for &(rect, radius, described) in scene {
        draw_box_shadow(
            &mut fb,
            rect,
            CornerRadius::clamped(radius, rect),
            described,
        );
    }
    fb
}

fn fresh(scene: Vec<(Rect, i32, BoxShadow)>) -> Framebuffer {
    std::thread::spawn(move || draw(&scene))
        .join()
        .expect("fresh thread")
}

#[test]
fn every_keyed_field_selects_its_own_mask() {
    let scenes = scenes();
    // Populate this thread's cache with every scene, then draw each again.
    draw(&scenes);
    for scene in &scenes {
        let cached = draw(std::slice::from_ref(scene));
        assert_eq!(cached.pixels(), fresh(vec![*scene]).pixels(), "{scene:?}");
    }
}

#[test]
fn a_repeated_shadow_renders_once() {
    let scene = [(Rect::new(10, 10, 60, 40), 8, shadow((0, 2), 10, 31))];
    let before = RENDERS.get();
    let first = draw(&scene);
    let second = draw(&scene);
    assert_eq!(RENDERS.get() - before, 1);
    assert_eq!(first.pixels(), second.pixels());
}

#[test]
fn a_clipped_hit_matches_the_full_shadow_inside_the_clip() {
    let scene = [(Rect::new(40, 30, 70, 40), 10, shadow((0, 4), 12, 90))];
    let full = draw(&scene);
    let region = Rect::new(20, 50, 90, 30);
    let mut clipped = surface();
    clipped.render_clipped(region, |fb| {
        for &(rect, radius, described) in &scene {
            draw_box_shadow(fb, rect, CornerRadius::clamped(radius, rect), described);
        }
    });
    let blank = surface();
    for y in 0..120_i32 {
        for x in 0..160_i32 {
            let inside = (region.x..region.x + region.width).contains(&x)
                && (region.y..region.y + region.height).contains(&y);
            let expected = if inside {
                full.get_pixel(x, y)
            } else {
                blank.get_pixel(x, y)
            };
            assert_eq!(clipped.get_pixel(x, y), expected, "({x}, {y})");
        }
    }
}

#[test]
fn a_shadow_outside_the_clip_is_not_rendered() {
    let before = RENDERS.get();
    let mut fb = surface();
    fb.render_clipped(Rect::new(0, 0, 10, 10), |fb| {
        let rect = Rect::new(100, 80, 30, 20);
        draw_box_shadow(
            fb,
            rect,
            CornerRadius::clamped(4, rect),
            shadow((0, 2), 4, 60),
        );
    });
    assert_eq!(RENDERS.get(), before);
    assert_eq!(fb.pixels(), surface().pixels());
}
