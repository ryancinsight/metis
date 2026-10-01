//! A damage-limited repaint leaves the surface exactly as a full repaint of
//! the same frame would.

use super::{BACKDROP, clear_is_redundant, paint};
use crate::{ApplicationCommand, FocusDirection, FrontendApp};
use iris::render::RenderBackend;
use metis_ipc::MemoryTransport;
use metis_platform::rasterizer::CornerRadius;
use metis_platform::{Damage, DisplayScale, Framebuffer, Rect};
use metis_ui_lang::{Color, DisplayCommand, DisplayList};

fn full_repaint(app: &FrontendApp<MemoryTransport>) -> Framebuffer {
    let mut reference = app.framebuffer().clone();
    reference.clear(BACKDROP);
    reference
        .render(app.painted.as_ref().expect("a rendered frame"))
        .unwrap_or_else(|never| match never {});
    reference
}

/// One user-visible change to the form.
#[derive(Debug, Clone, Copy)]
pub(super) enum Edit {
    Keystroke(&'static str),
    Composition(Option<&'static str>),
    Focus(FocusDirection),
    Menu,
    Command(ApplicationCommand),
    Resize(u32, u32),
    Scale(u32),
}

impl Edit {
    pub(super) fn apply(self, app: &mut FrontendApp<MemoryTransport>) {
        match self {
            Self::Keystroke(patient) => app.set_inputs(patient, 72.5, 4.0, 0.5).expect("input"),
            Self::Composition(text) => app
                .set_composition(text.map(str::to_owned))
                .expect("composition"),
            Self::Focus(direction) => app.move_focus(direction).expect("focus"),
            Self::Menu => app.toggle_command_menu().expect("menu"),
            Self::Command(command) => app.activate_command(command).expect("command"),
            Self::Resize(width, height) => app.resize(width, height).expect("resize"),
            Self::Scale(milli) => app
                .set_display_scale(DisplayScale::from_milli(milli).expect("scale"))
                .expect("scale"),
        }
    }
}

#[test]
fn every_edit_leaves_the_surface_as_a_full_repaint_would() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("initial form");
    assert_eq!(app.take_damage(), Damage::Full);
    for edit in [
        Edit::Keystroke("PT-9042-ALPHAB"),
        Edit::Keystroke("PT-9042-ALPHABC"),
        Edit::Composition(Some("ひら")),
        Edit::Composition(None),
        Edit::Focus(FocusDirection::Forward),
        Edit::Focus(FocusDirection::Forward),
        Edit::Menu,
        Edit::Command(ApplicationCommand::ThemeDark),
        Edit::Command(ApplicationCommand::ThemeSystem),
        Edit::Resize(640, 520),
        Edit::Scale(1_250),
        Edit::Keystroke("PT-9042"),
    ] {
        let before = app.framebuffer().clone();
        edit.apply(&mut app);
        assert_eq!(
            app.framebuffer().pixels(),
            full_repaint(&app).pixels(),
            "{edit:?}: the damaged repaint diverged from a full repaint"
        );
        let damage = app.take_damage();
        assert_eq!(
            app.take_damage(),
            Damage::Unchanged,
            "{edit:?}: taking clears"
        );
        assert_presented_damage_covers_changes(&before, app.framebuffer(), damage, edit);
    }
}

/// Every pixel that differs from the previously presented frame lies in the
/// damage the host is told to present.
fn assert_presented_damage_covers_changes(
    before: &Framebuffer,
    after: &Framebuffer,
    damage: Damage,
    edit: Edit,
) {
    let resized = (before.width(), before.height()) != (after.width(), after.height());
    match damage {
        Damage::Full => {}
        _ if resized => panic!("{edit:?}: a resized frame reported {damage:?}"),
        Damage::Unchanged => assert_eq!(before.pixels(), after.pixels(), "{edit:?}"),
        Damage::Region(region) => {
            let width = usize::try_from(after.width()).expect("width fits usize");
            let changed = before.pixels().iter().zip(after.pixels()).enumerate();
            for (index, _) in changed.filter(|(_, (old, new))| old != new) {
                let x = i32::try_from(index % width).expect("column fits i32");
                let y = i32::try_from(index / width).expect("row fits i32");
                assert!(
                    region.covers(Rect::new(x, y, 1, 1)),
                    "{edit:?}: pixel ({x}, {y}) changed outside {region:?}"
                );
            }
        }
    }
}

#[test]
fn construction_and_resize_report_the_whole_frame() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 320, 240).expect("initial form");
    assert_eq!(app.take_damage(), Damage::Full);
    app.set_inputs("PT-9042-ALPHAB", 72.5, 4.0, 0.5)
        .expect("input");
    app.resize(400, 300).expect("resize");
    assert_eq!(
        app.take_damage(),
        Damage::Full,
        "a resize merges to the whole frame"
    );
}

/// Not opaque and not any palette color, so a pixel that keeps it was never
/// written by the repaint.
const STALE: u32 = 0x1234_5678;

fn stale_surface(width: u32, height: u32) -> Framebuffer {
    let mut surface = Framebuffer::new(width, height).expect("surface");
    surface.pixels_mut().fill(STALE);
    surface
}

/// The reference repaint: clear the region, then paint, with no skip.
fn cleared_repaint(surface: &Framebuffer, region: Rect, display: &DisplayList) -> Framebuffer {
    let mut repainted = surface.clone();
    repainted.render_clipped(region, |region| {
        region.clear(BACKDROP);
        region
            .render(display)
            .unwrap_or_else(|never| match never {});
    });
    repainted
}

fn painted_region(surface: &Framebuffer, region: Rect, display: &DisplayList) -> Framebuffer {
    let mut repainted = surface.clone();
    repainted.render_clipped(region, |region| paint(region, display));
    repainted
}

fn stale_count(surface: &Framebuffer) -> usize {
    surface
        .pixels()
        .iter()
        .filter(|&&pixel| pixel == STALE)
        .count()
}

#[test]
fn the_shipped_form_covers_every_repainted_region_so_no_clear_runs() {
    for (width, height, milli, dark) in [
        (800, 600, 1_000, false),
        (800, 600, 1_000, true),
        (640, 480, 1_000, false),
        (640, 520, 1_250, false),
        (1_600, 1_200, 2_000, false),
        (1_600, 1_200, 2_000, true),
    ] {
        let (transport, _peer) = MemoryTransport::pair();
        let mut app = FrontendApp::new(transport, width, height).expect("form");
        app.set_display_scale(DisplayScale::from_milli(milli).expect("scale"))
            .expect("scale");
        if dark {
            app.activate_command(ApplicationCommand::ThemeDark)
                .expect("theme");
        }
        let display = app.painted.as_ref().expect("a rendered frame");
        let (w, h) = (
            i32::try_from(width).expect("width"),
            i32::try_from(height).expect("height"),
        );
        let surface = stale_surface(width, height);
        for region in [
            Rect::new(0, 0, w, h),
            Rect::new(10, 10, 50, 40),
            Rect::new(-20, -20, 100, 100),
            Rect::new(w - 5, h - 5, 50, 50),
        ] {
            let mut probe = surface.clone();
            assert!(
                probe.render_clipped(region, |clipped| clear_is_redundant(clipped, display)),
                "{width}x{height}@{milli} dark={dark} {region:?}: no covering fill"
            );
            let repainted = painted_region(&surface, region, display);
            assert_eq!(
                repainted.pixels(),
                cleared_repaint(&surface, region, display).pixels(),
                "{width}x{height}@{milli} dark={dark} {region:?}"
            );
        }
    }
}

#[test]
fn a_full_render_onto_a_stale_surface_equals_a_cleared_repaint() {
    for dark in [false, true] {
        let (transport, _peer) = MemoryTransport::pair();
        let mut app = FrontendApp::new(transport, 320, 240).expect("form");
        if dark {
            app.activate_command(ApplicationCommand::ThemeDark)
                .expect("theme");
        }
        app.framebuffer.pixels_mut().fill(STALE);
        app.painted = None;
        app.render().expect("render");
        assert_eq!(
            app.framebuffer().pixels(),
            full_repaint(&app).pixels(),
            "dark={dark}"
        );
        assert_eq!(stale_count(app.framebuffer()), 0, "dark={dark}");
    }
}

fn fill(rect: Rect, radius: CornerRadius, color: Color) -> DisplayCommand {
    DisplayCommand::FillRect {
        rect,
        radius,
        color,
    }
}

#[test]
fn the_clear_is_skipped_only_when_an_opaque_square_fill_covers_the_region() {
    let (width, height) = (32, 24);
    let whole = Rect::new(0, 0, 32, 24);
    let inner = Rect::new(4, 4, 8, 8);
    let ink = Color::rgb(10, 20, 30);
    let square = CornerRadius::SQUARE;
    let rounded = CornerRadius::clamped(6, whole);
    let list = |commands: Vec<DisplayCommand>| DisplayList { commands };
    // (case, display list, redundant over the whole surface, over `inner`)
    let cases = [
        ("covering", list(vec![fill(whole, square, ink)]), true, true),
        (
            "overhanging",
            list(vec![fill(Rect::new(-5, -5, 99, 99), square, ink)]),
            true,
            true,
        ),
        (
            "covering after a small fill",
            list(vec![fill(inner, square, ink), fill(whole, square, ink)]),
            true,
            true,
        ),
        (
            "one column short",
            list(vec![fill(Rect::new(0, 0, 31, 24), square, ink)]),
            false,
            true,
        ),
        (
            "one row short",
            list(vec![fill(Rect::new(0, 0, 32, 23), square, ink)]),
            false,
            true,
        ),
        (
            "inside the region only",
            list(vec![fill(inner, square, ink)]),
            false,
            true,
        ),
        (
            "translucent",
            list(vec![fill(whole, square, Color::rgba(10, 20, 30, 254))]),
            false,
            false,
        ),
        (
            "rounded corners",
            list(vec![fill(whole, rounded, ink)]),
            false,
            false,
        ),
        (
            "stroke only",
            list(vec![DisplayCommand::DrawLine {
                start: (0, 0),
                end: (31, 23),
                color: ink,
            }]),
            false,
            false,
        ),
        ("empty", list(Vec::new()), false, false),
    ];
    let surface = stale_surface(width, height);
    for (case, display, over_whole, over_inner) in cases {
        for (region, redundant) in [(whole, over_whole), (inner, over_inner)] {
            let mut probe = surface.clone();
            assert_eq!(
                probe.render_clipped(region, |clipped| clear_is_redundant(clipped, &display)),
                redundant,
                "{case} over {region:?}"
            );
            let repainted = painted_region(&surface, region, &display);
            let reference = cleared_repaint(&surface, region, &display);
            assert_eq!(
                repainted.pixels(),
                reference.pixels(),
                "{case} over {region:?}: a pixel kept its stale value"
            );
            let outside = surface.pixels().len()
                - usize::try_from(region.width * region.height).expect("area");
            assert_eq!(stale_count(&repainted), outside, "{case} over {region:?}");
        }
    }
}
