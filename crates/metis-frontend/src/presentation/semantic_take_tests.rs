//! The semantic tree is taken once after each completed render.

use super::repaint_tests::Edit;
use crate::FrontendApp;
use metis_core::ErrorCode;
use metis_ipc::MemoryTransport;
use metis_ui_lang::DomElement;

fn form() -> FrontendApp<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    FrontendApp::new(transport, 800, 600).expect("initial form")
}

#[test]
fn the_semantic_tree_is_taken_once_per_render() {
    let mut app = form();
    assert_eq!(
        app.take_semantic_tree().expect("first take"),
        Some(app.semantic_tree().expect("semantic tree")),
        "the first take reports the constructed form"
    );
    assert_eq!(app.take_semantic_tree().expect("idle take"), None);
    assert_eq!(app.take_semantic_tree().expect("repeated idle take"), None);

    Edit::Menu.apply(&mut app);
    let changed = app.take_semantic_tree().expect("take after a change");
    assert_eq!(changed, Some(app.semantic_tree().expect("semantic tree")));
    assert_eq!(app.take_semantic_tree().expect("take after taking"), None);

    app.resize(640, 480).expect("resize");
    assert!(
        app.take_semantic_tree()
            .expect("take after resize")
            .is_some(),
        "every successful render marks the tree pending"
    );
}

fn submit(app: &mut FrontendApp<MemoryTransport>) -> &mut DomElement {
    app.doc
        .find_element_by_id_mut("btn-calc")
        .expect("submit control")
}

#[test]
fn a_failed_take_keeps_the_tree_pending() {
    let mut app = form();
    submit(&mut app)
        .attributes
        .insert("aria-labelledby".to_owned(), "no-such-element".to_owned());
    let error = app.take_semantic_tree().expect_err("unresolved reference");
    assert_eq!(error.code, ErrorCode::MalformedMarkup);

    submit(&mut app).attributes.remove("aria-labelledby");
    let taken = app.take_semantic_tree().expect("take after the repair");
    assert_eq!(taken, Some(app.semantic_tree().expect("semantic tree")));
    assert_eq!(app.take_semantic_tree().expect("take after taking"), None);
}
