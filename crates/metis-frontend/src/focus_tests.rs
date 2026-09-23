//! Keyboard focus order, reconciliation and the painted ring.

use super::{FocusDirection, FocusOrigin, INITIAL_FOCUS};
use crate::{ApplicationCommand, FrontendApp};
use metis_ipc::transport::MemoryTransport;
use metis_platform::{Color, Rect};
use metis_ui_lang::{LayoutViewport, compute_layout};

/// Ring width plus its gap in device pixels at display scale one.
const RING_REACH: i32 = 4;

fn app() -> FrontendApp<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    FrontendApp::new(transport, 800, 600).expect("initial form")
}

fn rect(app: &FrontendApp<MemoryTransport>, id: &str) -> Rect {
    compute_layout(app.document(), LayoutViewport::new(800, 600))
        .expect("layout")
        .element_rect(id)
        .expect("laid-out control")
}

/// A pixel on the ring's left edge, beside the control's vertical middle.
fn ring_pixel(app: &FrontendApp<MemoryTransport>, id: &str) -> Color {
    let control = rect(app, id);
    app.framebuffer()
        .get_pixel(control.x - RING_REACH + 1, control.y + control.height / 2)
}

#[test]
fn the_form_opens_with_unringed_focus_on_the_patient_reference() {
    let app = app();
    assert_eq!(app.focused_control(), INITIAL_FOCUS);
    assert!(!app.focus_visible());
    assert_eq!(ring_pixel(&app, INITIAL_FOCUS), Color::WHITE);
}

#[test]
fn focus_order_follows_the_document_and_the_open_menu() {
    let mut app = app();
    let closed = [
        "command-menu-toggle",
        "command-focus-patient",
        "label-patient",
        "btn-calc",
    ];
    assert_eq!(app.focus_order().expect("order"), closed);
    app.toggle_command_menu().expect("open menu");
    assert_eq!(
        app.focus_order().expect("order"),
        [
            "command-menu-toggle",
            "command-focus-patient",
            "command-theme-dark",
            "command-theme-system",
            "label-patient",
            "btn-calc",
        ]
    );
}

#[test]
fn sequential_navigation_wraps_in_both_directions() {
    let mut app = app();
    app.move_focus(FocusDirection::Forward).expect("tab");
    assert_eq!(app.focused_control(), "btn-calc");
    assert!(app.focus_visible());
    app.move_focus(FocusDirection::Forward).expect("tab");
    assert_eq!(app.focused_control(), "command-menu-toggle");
    app.move_focus(FocusDirection::Backward).expect("shift tab");
    assert_eq!(app.focused_control(), "btn-calc");
    app.move_focus(FocusDirection::Backward).expect("shift tab");
    assert_eq!(app.focused_control(), INITIAL_FOCUS);
}

#[test]
fn only_keyboard_focus_paints_a_ring_in_the_theme_color() {
    // The patient reference casts no shadow, so the pixels around it are the
    // card's own fill until a ring covers them.
    let mut app = app();
    assert!(
        app.focus_control(INITIAL_FOCUS, FocusOrigin::Pointer)
            .expect("focus")
    );
    assert_eq!(ring_pixel(&app, INITIAL_FOCUS), Color::WHITE);
    assert!(
        app.focus_control(INITIAL_FOCUS, FocusOrigin::Keyboard)
            .expect("focus")
    );
    assert_eq!(ring_pixel(&app, INITIAL_FOCUS), Color::rgb(43, 108, 176));
    // The ring stands clear of the control: the gap keeps the card fill.
    let control = rect(&app, INITIAL_FOCUS);
    assert_eq!(
        app.framebuffer()
            .get_pixel(control.x - 1, control.y + control.height / 2),
        Color::WHITE
    );
    app.activate_command(ApplicationCommand::ThemeDark)
        .expect("dark");
    assert!(
        app.focus_control(INITIAL_FOCUS, FocusOrigin::Keyboard)
            .expect("focus")
    );
    assert_eq!(ring_pixel(&app, INITIAL_FOCUS), Color::rgb(56, 189, 248));
}

#[test]
fn closing_the_menu_returns_focus_to_its_anchor() {
    let mut app = app();
    assert!(
        !app.focus_control("command-theme-dark", FocusOrigin::Keyboard)
            .expect("closed menu item")
    );
    assert_eq!(app.focused_control(), INITIAL_FOCUS);
    app.toggle_command_menu().expect("open menu");
    assert!(
        app.focus_control("command-theme-dark", FocusOrigin::Keyboard)
            .expect("open menu item")
    );
    assert!(app.close_command_menu().expect("close"));
    assert_eq!(app.focused_control(), "command-menu-toggle");
    assert!(app.focus_visible());
}

#[test]
fn the_focus_patient_command_moves_focus_and_keeps_its_origin() {
    let mut app = app();
    app.move_focus(FocusDirection::Backward).expect("shift tab");
    assert_eq!(app.focused_control(), "command-focus-patient");
    app.activate_command(ApplicationCommand::FocusPatient)
        .expect("focus patient");
    assert_eq!(app.focused_control(), INITIAL_FOCUS);
    assert!(app.focus_visible());
}
