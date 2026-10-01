//! The painted rectangle of an element is the rectangle of a fresh layout.

use super::repaint_tests::Edit;
use crate::{ApplicationCommand, FocusDirection, FrontendApp};
use metis_core::ErrorCode;
use metis_ipc::MemoryTransport;
use metis_ui_lang::{LayoutViewport, compute_layout};

const FORM_IDS: [&str; 6] = [
    "command-menu-toggle",
    "command-focus-patient",
    "label-patient",
    "btn-calc",
    "results-card",
    "status-badge",
];

/// Surfaces the form lays out only while its command menu is open.
const MENU_IDS: [&str; 3] = ["command-menu", "command-theme-dark", "command-theme-system"];

fn form() -> FrontendApp<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    FrontendApp::new(transport, 800, 600).expect("initial form")
}

/// The rectangle of `id` in a fresh layout of the document at the surface
/// size and display scale.
fn laid_out_rect(app: &FrontendApp<MemoryTransport>, id: &str) -> Option<metis_platform::Rect> {
    let width = i32::try_from(app.framebuffer().width()).expect("width fits i32");
    let height = i32::try_from(app.framebuffer().height()).expect("height fits i32");
    compute_layout(
        app.document(),
        LayoutViewport::with_scale(width, height, app.display_scale()),
    )
    .expect("layout")
    .element_rect(id)
}

fn assert_rects_match(app: &FrontendApp<MemoryTransport>, case: &str) {
    for id in FORM_IDS.into_iter().chain(MENU_IDS) {
        let expected = laid_out_rect(app, id);
        let laid_out = FORM_IDS.contains(&id) || app.command_menu_open();
        assert_eq!(expected.is_some(), laid_out, "{case}: {id} is laid out");
        assert_eq!(app.element_rect(id).ok(), expected, "{case}: {id}");
    }
}

#[test]
fn the_painted_rectangle_is_the_laid_out_rectangle_after_every_edit() {
    let mut app = form();
    assert_rects_match(&app, "initial");
    for edit in [
        Edit::Menu,
        Edit::Focus(FocusDirection::Forward),
        Edit::Keystroke("PT-9042-ALPHAB"),
        Edit::Composition(Some("ひら")),
        Edit::Command(ApplicationCommand::ThemeDark),
        Edit::Resize(640, 520),
        Edit::Scale(1_250),
        Edit::Menu,
        Edit::Resize(1_024, 300),
    ] {
        edit.apply(&mut app);
        assert_rects_match(&app, &format!("{edit:?}"));
    }
}

#[test]
fn a_form_without_a_painted_frame_lays_the_rectangle_out_afresh() {
    let mut app = form();
    Edit::Menu.apply(&mut app);
    Edit::Scale(1_500).apply(&mut app);
    let rects = |app: &FrontendApp<MemoryTransport>| {
        FORM_IDS
            .into_iter()
            .chain(MENU_IDS)
            .map(|id| app.element_rect(id).ok())
            .collect::<Vec<_>>()
    };
    let painted = rects(&app);
    app.painted = None;
    let laid_out = rects(&app);
    assert_eq!(laid_out, painted);
    assert!(laid_out.iter().all(Option::is_some));
}

#[test]
fn an_element_the_form_does_not_author_is_a_markup_error() {
    let mut app = form();
    let error = app.element_rect("no-such-element").expect_err("missing id");
    assert_eq!(error.code, ErrorCode::MalformedMarkup);
    app.painted = None;
    let error = app.element_rect("no-such-element").expect_err("missing id");
    assert_eq!(error.code, ErrorCode::MalformedMarkup);
}
