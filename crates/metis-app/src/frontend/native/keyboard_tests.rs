//! Native keyboard focus navigation, activation and text routing.

use super::super::{NativeForm, command_rect, native_accessibility};
use super::{PATIENT_INPUT, SPACE_KEY, TAB_KEY};
use metis_frontend::{ApplicationCommand, ApplicationTheme, FrontendApp};
use metis_ipc::MemoryTransport;
use metis_platform::native::{
    AccessibilityAction, AccessibilityActionRequest, ModifierState, MouseButton, NativeApplication,
    WindowEvent,
};

fn form() -> NativeForm<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("form");
    let patient_id = app.inputs().patient_id.clone();
    NativeForm {
        app,
        pid: 1,
        patient_id,
        focused: true,
    }
}

fn key(form: &mut NativeForm<MemoryTransport>, virtual_key: u32, modifiers: ModifierState) {
    form.handle_events(&[WindowEvent::KeyDown {
        virtual_key,
        repeated: false,
        modifiers,
    }])
    .expect("key event");
}

#[test]
fn tab_and_shift_tab_move_focus_with_a_ring() {
    let mut form = form();
    assert_eq!(form.app.focused_control(), PATIENT_INPUT);
    key(&mut form, TAB_KEY, ModifierState::NONE);
    assert_eq!(form.app.focused_control(), "btn-calc");
    assert!(form.app.focus_visible());
    key(&mut form, TAB_KEY, ModifierState::SHIFT);
    assert_eq!(form.app.focused_control(), PATIENT_INPUT);
    // Control+Tab and Alt+Tab belong to the host, not to focus navigation.
    key(&mut form, TAB_KEY, ModifierState::CONTROL);
    key(
        &mut form,
        TAB_KEY,
        ModifierState::ALT | ModifierState::SHIFT,
    );
    assert_eq!(form.app.focused_control(), PATIENT_INPUT);
}

#[test]
fn space_on_the_menu_button_opens_the_menu_on_its_first_item() {
    let mut form = form();
    key(&mut form, TAB_KEY, ModifierState::SHIFT);
    key(&mut form, TAB_KEY, ModifierState::SHIFT);
    assert_eq!(form.app.focused_control(), "command-menu-toggle");
    key(&mut form, SPACE_KEY, ModifierState::NONE);
    assert!(form.app.command_menu_open());
    assert_eq!(
        form.app.focused_control(),
        ApplicationCommand::ThemeDark.id()
    );
    // Tab walks the open menu's items; Enter on one applies it, closes the
    // menu and returns focus to the button.
    key(&mut form, TAB_KEY, ModifierState::NONE);
    assert_eq!(
        form.app.focused_control(),
        ApplicationCommand::ThemeSystem.id()
    );
    key(&mut form, TAB_KEY, ModifierState::SHIFT);
    key(&mut form, super::RETURN_KEY, ModifierState::NONE);
    assert_eq!(form.app.theme(), ApplicationTheme::Dark);
    assert!(!form.app.command_menu_open());
    assert_eq!(form.app.focused_control(), "command-menu-toggle");
    assert!(form.app.focus_visible());
}

#[test]
fn typing_reaches_the_patient_reference_only_while_it_has_focus() {
    let mut form = form();
    let before = form.app.inputs().patient_id.clone();
    key(&mut form, TAB_KEY, ModifierState::NONE);
    form.handle_events(&[WindowEvent::TextInput { character: 'x' }])
        .expect("text on a button");
    assert_eq!(form.app.inputs().patient_id, before);

    let patient = command_rect(&form.app, PATIENT_INPUT).expect("patient surface");
    form.handle_events(&[WindowEvent::PointerUp {
        x: patient.x,
        y: patient.y,
        button: MouseButton::Left,
    }])
    .expect("press the patient reference");
    assert_eq!(form.app.focused_control(), PATIENT_INPUT);
    assert!(!form.app.focus_visible());
    form.handle_events(&[WindowEvent::TextInput { character: 'x' }])
        .expect("text on the patient reference");
    assert_eq!(form.app.inputs().patient_id, format!("{before}x"));
}

#[test]
fn a_pointer_press_focuses_its_control_without_a_ring() {
    let mut form = form();
    let toggle = command_rect(&form.app, "command-menu-toggle").expect("toggle surface");
    form.handle_events(&[WindowEvent::PointerUp {
        x: toggle.x,
        y: toggle.y,
        button: MouseButton::Left,
    }])
    .expect("press the menu button");
    assert!(form.app.command_menu_open());
    assert_eq!(form.app.focused_control(), "command-menu-toggle");
    assert!(!form.app.focus_visible());
}

#[test]
fn an_accessibility_focus_request_moves_focus_with_a_ring() {
    let mut form = form();
    let request = |target_node| WindowEvent::AccessibilityAction {
        request: AccessibilityActionRequest {
            target_node,
            action: AccessibilityAction::Focus,
            value: None,
            delta: None,
        },
    };
    form.handle_events(&[request(native_accessibility::control_identity("btn-calc"))])
        .expect("focus request");
    assert_eq!(form.app.focused_control(), "btn-calc");
    assert!(form.app.focus_visible());
    // A closed menu's item cannot take focus; focus stays put.
    form.handle_events(&[request(native_accessibility::control_identity(
        ApplicationCommand::ThemeDark.id(),
    ))])
    .expect("focus request for a hidden item");
    assert_eq!(form.app.focused_control(), "btn-calc");
}
