//! What the render cache serves equals what a fresh derivation gives.

use crate::FrontendApp;
use metis_ipc::MemoryTransport;

type App = FrontendApp<MemoryTransport>;

fn form() -> App {
    let (transport, _peer) = MemoryTransport::pair();
    FrontendApp::new(transport, 800, 600).expect("invariant: 800x600 is a valid form surface")
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
