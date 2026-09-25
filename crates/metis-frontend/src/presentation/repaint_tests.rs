//! A damage-limited repaint leaves the surface exactly as a full repaint of
//! the same frame would.

use super::BACKDROP;
use crate::{ApplicationCommand, FocusDirection, FrontendApp};
use iris::render::RenderBackend;
use metis_ipc::MemoryTransport;
use metis_platform::{Damage, DisplayScale, Framebuffer, Rect};

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
enum Edit {
    Keystroke(&'static str),
    Composition(Option<&'static str>),
    Focus(FocusDirection),
    Menu,
    Command(ApplicationCommand),
    Resize(u32, u32),
    Scale(u32),
}

impl Edit {
    fn apply(self, app: &mut FrontendApp<MemoryTransport>) {
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
