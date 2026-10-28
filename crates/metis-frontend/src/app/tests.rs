use super::{FrontendApp, MAX_COMPOSITION_BYTES};
use crate::commands::ApplicationCommand;
use metis_core::ErrorCode;
use metis_ipc::MemoryTransport;
use metis_platform::DisplayScale;
use metis_ui_lang::{DisplayCommand, TransitionProperty};
use std::time::Duration;

/// The color the shown frame paints for the badge.
fn painted_badge(app: &FrontendApp<MemoryTransport>) -> metis_ui_lang::Color {
    let display = app.painted_display().expect("the form has painted a frame");
    let badge = display
        .element_index("status-badge")
        .expect("the badge declares a transition");
    display
        .element_color(badge, TransitionProperty::Color)
        .expect("the badge paints a text color")
}

#[test]
fn the_authored_badge_declares_the_transition_it_is_sampled_with() {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("initial form");
    let display = app.painted_display().expect("initial frame");
    let badge = display
        .element_index("status-badge")
        .expect("the badge is the authored animated element");
    let transition = display
        .element_transition(badge)
        .expect("the badge declares a transition");
    assert_eq!(transition.property(), TransitionProperty::Color);
    assert_eq!(transition.duration(), Duration::from_millis(150));
    // No clock was supplied, so the declared target paints in one frame.
    assert_eq!(painted_badge(&app), super::super::BADGE_READY);
    assert_eq!(app.next_tick(), None);
}

#[test]
fn a_palette_change_settles_instead_of_interpolating() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("initial form");
    app.set_now(Duration::ZERO);
    app.activate_command(ApplicationCommand::ThemeDark)
        .expect("dark theme");

    // A palette change recolors every command, so it paints the new palette
    // rather than interpolating towards it, and schedules no frame.
    assert_eq!(app.next_tick(), None);
}

#[test]
fn resize_replaces_surface_and_preserves_it_on_invalid_dimensions() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("initial form");
    app.resize(640, 480).expect("valid resize");
    assert_eq!(app.framebuffer().width(), 640);
    assert_eq!(app.framebuffer().height(), 480);

    let error = app.resize(0, 480).expect_err("zero width");
    assert_eq!(error.code, ErrorCode::SurfaceAllocationError);
    assert_eq!(app.framebuffer().width(), 640);
    assert_eq!(app.framebuffer().height(), 480);
}

#[test]
fn composition_is_transient_and_bounded() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("initial form");
    app.set_composition(Some("東京".to_owned()))
        .expect("bounded composition");
    assert_eq!(app.composition(), Some("東京"));
    assert_eq!(
        app.document()
            .find_element_by_id("label-patient")
            .expect("patient label")
            .text_content(),
        "Patient ID: PT-9042-ALPHA [東京]"
    );

    let error = app
        .set_composition(Some("x".repeat(MAX_COMPOSITION_BYTES + 1)))
        .expect_err("oversized composition");
    assert_eq!(error.code, ErrorCode::PayloadTooLarge);
    assert_eq!(app.composition(), Some("東京"));

    app.set_inputs("PT-2", 70.0, 4.0, 0.5)
        .expect("committed input edit");
    assert_eq!(app.composition(), None);
}

#[test]
fn display_scale_repaints_text_and_preserves_the_new_capability() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("initial form");
    let scale = DisplayScale::from_milli(1_500).expect("150 percent");
    app.set_display_scale(scale).expect("scaled form");
    assert_eq!(app.display_scale(), scale);
    let command = app.document().clone();
    let display = metis_ui_lang::compute_layout(
        &command,
        metis_ui_lang::LayoutViewport::with_scale(800, 600, scale),
    )
    .expect("scaled layout");
    assert!(display.commands().iter().any(|command| matches!(
        command,
        // The default 14-pixel size at 150 percent is 21 device
        // pixels per em, exact in binary.
        DisplayCommand::DrawText { style, .. }
            if (style.size.pixels() - 21.0).abs() < f64::EPSILON
    )));
}

#[test]
fn authored_form_exposes_host_neutral_semantics() {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("initial form");
    let tree = app.semantic_tree().expect("semantic tree");
    assert_eq!(tree.root.id.as_deref(), Some("main-screen"));
    assert_eq!(tree.root.role, metis_ui_lang::SemanticRole::Application);
    let button = tree
        .root
        .children
        .iter()
        .flat_map(|node| node.children.iter())
        .flat_map(|node| node.children.iter())
        .find(|node| node.id.as_deref() == Some("btn-calc"))
        .expect("submit action");
    assert_eq!(button.role, metis_ui_lang::SemanticRole::Button);
    assert!(button.focusable);
    assert_eq!(
        button.actions,
        vec![metis_ui_lang::SemanticAction::Activate]
    );
    let patient = tree
        .root
        .children
        .iter()
        .flat_map(|node| node.children.iter())
        .flat_map(|node| node.children.iter())
        .find(|node| node.id.as_deref() == Some("label-patient"))
        .expect("patient input");
    assert_eq!(patient.role, metis_ui_lang::SemanticRole::TextBox);
    assert_eq!(patient.name, "Patient ID");
    assert_eq!(patient.value.as_deref(), Some("PT-9042-ALPHA"));
    assert!(patient.focusable);
    assert_eq!(
        patient.actions,
        vec![metis_ui_lang::SemanticAction::SetValue]
    );
}
