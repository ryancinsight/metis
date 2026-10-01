//! Painting a display list onto the surface behind the authored form.

use super::BACKDROP;
use iris::render::RenderBackend;
use metis_platform::{Clip, Framebuffer, Rect};
use metis_ui_lang::{DisplayCommand, DisplayList};

/// Paints `display` over the current clip of `framebuffer`.
///
/// The clip is first set to the backdrop color unless the list itself
/// replaces every pixel of it ([`replaces_clip`]), where that clear would
/// only be overwritten.
pub(super) fn paint(framebuffer: &mut Framebuffer, display: &DisplayList) {
    if !replaces_clip(display, framebuffer.clip()) {
        framebuffer.clear(BACKDROP);
    }
    framebuffer
        .render(display)
        .unwrap_or_else(|never| match never {});
}

/// Whether painting `display` writes every pixel of `clip` without reading
/// what the surface held before.
///
/// An opaque square [`DisplayCommand::FillRect`] stores its color outright
/// over the part of its rectangle inside the clip, so when that rectangle
/// contains the clip, each clip pixel's final value is fixed by that command
/// and by the commands after it. Every earlier command, and any initial
/// content, no longer shows. A translucent fill reads the destination, a
/// rounded fill blends its corner arcs into it, and a rectangle short of the
/// clip leaves pixels the list never writes; each of those keeps the clear.
pub(super) fn replaces_clip(display: &DisplayList, clip: Clip) -> bool {
    display.commands.iter().any(|command| match command {
        DisplayCommand::FillRect {
            rect,
            radius,
            color,
        } => radius.is_square() && color.a == u8::MAX && contains_clip(*rect, clip),
        _ => false,
    })
}

/// Whether `rect` spans every column and row of `clip`.
fn contains_clip(rect: Rect, clip: Clip) -> bool {
    let right = i64::from(rect.x) + i64::from(rect.width);
    let bottom = i64::from(rect.y) + i64::from(rect.height);
    i64::from(rect.x) <= i64::from(clip.left())
        && i64::from(rect.y) <= i64::from(clip.top())
        && right >= i64::from(clip.right())
        && bottom >= i64::from(clip.bottom())
}

#[cfg(test)]
#[path = "backdrop_tests.rs"]
mod tests;
