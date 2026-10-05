//! What the render cache serves equals what a fresh derivation gives.

use crate::{ApplicationCommand, FocusDirection, FrontendApp};
use metis_ipc::MemoryTransport;
use metis_platform::DisplayScale;
use metis_ui_lang::{DisplayCommand, LayoutViewport, compute_layout};

type App = FrontendApp<MemoryTransport>;
/// One user-visible change to the form.
type Step = fn(&mut App);

fn form() -> App {
    let (transport, _peer) = MemoryTransport::pair();
    FrontendApp::new(transport, 800, 600).expect("invariant: 800x600 is a valid form surface")
}

fn fresh_layout(app: &App) -> Vec<DisplayCommand> {
    let viewport = LayoutViewport::with_scale(
        i32::try_from(app.framebuffer().width()).expect("width fits i32"),
        i32::try_from(app.framebuffer().height()).expect("height fits i32"),
        app.display_scale(),
    );
    compute_layout(app.document(), viewport)
        .expect("invariant: the authored form lays out")
        .commands
}

#[test]
fn every_edit_paints_the_layout_a_fresh_computation_gives() {
    let edits: [(&str, Step); 9] = [
        ("keystroke", |app| {
            app.set_inputs("PT-9042-ALPHAB", 72.5, 4.0, 0.5)
                .expect("input");
        }),
        ("equal keystroke", |app| {
            app.set_inputs("PT-9042-ALPHAB", 72.5, 4.0, 0.5)
                .expect("input");
        }),
        ("composition", |app| {
            app.set_composition(Some("ひら".to_owned()))
                .expect("composition");
        }),
        ("focus", |app| {
            app.move_focus(FocusDirection::Forward).expect("focus");
        }),
        ("menu", |app| app.toggle_command_menu().expect("menu")),
        ("theme", |app| {
            app.activate_command(ApplicationCommand::ThemeDark)
                .expect("theme");
        }),
        ("resize", |app| app.resize(640, 520).expect("resize")),
        ("scale", |app| {
            app.set_display_scale(DisplayScale::from_milli(1_250).expect("scale"))
                .expect("scale");
        }),
        ("shorter patient", |app| {
            app.set_inputs("PT-1", 70.0, 4.0, 0.5).expect("input");
        }),
    ];
    let mut app = form();
    for (name, edit) in edits {
        edit(&mut app);
        let fresh = fresh_layout(&app);
        let painted = &app.painted.as_ref().expect("a rendered frame").commands;
        // A focus ring is the one command a frame adds to the layout.
        assert!(
            painted.starts_with(&fresh) && painted.len() - fresh.len() <= 1,
            "{name}: the painted frame is not the fresh layout"
        );
    }
}

#[test]
fn the_document_revision_moves_only_when_the_form_changes() {
    let mut app = form();
    let settled = app.doc.revision();
    app.render().expect("render");
    app.set_inputs("PT-9042-ALPHA", 72.5, 4.0, 0.5)
        .expect("equal inputs");
    assert_eq!(
        app.doc.revision(),
        settled,
        "unchanged state is not an edit"
    );

    app.set_inputs("PT-9042-ALPHAB", 72.5, 4.0, 0.5)
        .expect("input");
    assert_ne!(app.doc.revision(), settled, "a changed label is an edit");
}

#[test]
fn the_cached_focus_order_follows_the_document() {
    let mut app = form();
    let closed = app.focus_order().expect("focus order");
    assert_eq!(app.cache.focusable(&app.doc).expect("cached"), closed);

    app.toggle_command_menu().expect("menu opens");
    let open = app.focus_order().expect("focus order");
    assert_ne!(open, closed, "an open menu adds its items");
    assert_eq!(app.cache.focusable(&app.doc).expect("cached"), open);

    app.toggle_command_menu().expect("menu closes");
    assert_eq!(app.cache.focusable(&app.doc).expect("cached"), closed);
}
