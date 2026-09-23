use super::{
    MAX_PATIENT_ID_BYTES, NativeForm, append_patient_character, append_patient_text,
    command_menu_toggle_rect, input_limit_error, submit_rect,
};
use metis_core::ErrorCode;
use metis_frontend::{ApplicationTheme, FrontendApp};
use metis_ipc::MemoryTransport;
use metis_platform::DisplayScale;
use metis_platform::native::{
    AccessibilityAction, AccessibilityActionRequest, CompositionPhase, ModifierState,
    NativeApplication, NativeFlow, WindowEvent,
};

#[test]
fn patient_text_rejects_controls_and_bounded_overflow() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("form");
    let mut value = String::from("patient");
    assert!(!append_patient_character(&mut app, &mut value, '\n').expect("control input"));
    value = "x".repeat(MAX_PATIENT_ID_BYTES);
    assert_eq!(
        append_patient_character(&mut app, &mut value, 'y').expect_err("bounded input"),
        input_limit_error()
    );
}

#[test]
fn committed_composition_uses_the_same_patient_transition_as_text_input() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("form");
    let mut value = String::from("patient");
    app.set_inputs(&value, 70.0, 4.0, 0.5)
        .expect("initial patient value");
    app.set_composition(Some("東京".to_owned()))
        .expect("preedit value");
    append_patient_text(&mut app, &mut value, "東京").expect("commit value");
    assert_eq!(value, "patient東京");
    assert_eq!(app.inputs().patient_id, "patient東京");
    assert_eq!(app.composition(), None);
}

#[test]
fn native_composition_events_keep_preedit_transient_and_cancel_on_focus_loss() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("form");
    app.set_inputs("patient", 70.0, 4.0, 0.5)
        .expect("initial patient value");
    let mut form = NativeForm {
        app,
        pid: 1,
        patient_id: "patient".to_owned(),
        focused: true,
    };

    let flow = form
        .handle_events(&[
            WindowEvent::TextComposition {
                phase: CompositionPhase::Started,
                text: String::new(),
            },
            WindowEvent::TextComposition {
                phase: CompositionPhase::Updated,
                text: "東京".to_owned(),
            },
        ])
        .expect("preedit events");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert_eq!(form.app.composition(), Some("東京"));
    assert_eq!(form.patient_id, "patient");
    assert_eq!(form.app.inputs().patient_id, "patient");

    let flow = form
        .handle_events(&[WindowEvent::FocusLost])
        .expect("focus loss");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert_eq!(form.app.composition(), None);
    assert!(!form.focused);

    form.handle_events(&[
        WindowEvent::FocusGained,
        WindowEvent::TextComposition {
            phase: CompositionPhase::Started,
            text: String::new(),
        },
        WindowEvent::TextComposition {
            phase: CompositionPhase::Committed,
            text: "東京".to_owned(),
        },
    ])
    .expect("committed composition");
    assert_eq!(form.app.composition(), None);
    assert_eq!(form.patient_id, "patient東京");
    assert_eq!(form.app.inputs().patient_id, "patient東京");
}

#[test]
fn submit_hit_region_comes_from_the_authored_button_surface() {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("form");
    let button = submit_rect(&app).expect("authored submit surface");
    assert!(button.contains(button.x, button.y));
    assert!(!button.contains(button.x - 1, button.y));
    assert!(!button.contains(button.x, button.y - 1));
}

#[test]
fn native_accessibility_focus_targets_the_authored_submit_control() {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("form");
    let mut form = NativeForm {
        app,
        pid: 1,
        patient_id: "patient".to_owned(),
        focused: false,
    };
    let flow = form
        .handle_events(&[WindowEvent::AccessibilityAction {
            request: AccessibilityActionRequest {
                target_node: super::native_accessibility::submit_button_identity(),
                action: AccessibilityAction::Focus,
                value: None,
                delta: None,
            },
        }])
        .expect("accessibility focus");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert!(form.focused);
}

#[test]
fn native_accessibility_patient_input_accepts_focus_and_bounded_value() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("form");
    app.set_inputs("patient", 70.0, 4.0, 0.5)
        .expect("patient value");
    let mut form = NativeForm {
        app,
        pid: 1,
        patient_id: "patient".to_owned(),
        focused: false,
    };
    let flow = form
        .handle_events(&[WindowEvent::AccessibilityAction {
            request: AccessibilityActionRequest {
                target_node: super::native_accessibility::patient_input_identity(),
                action: AccessibilityAction::Focus,
                value: None,
                delta: None,
            },
        }])
        .expect("patient accessibility focus");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert!(form.focused);

    let flow = form
        .handle_events(&[WindowEvent::AccessibilityAction {
            request: AccessibilityActionRequest {
                target_node: super::native_accessibility::patient_input_identity(),
                action: AccessibilityAction::SetValue,
                value: Some("screen-reader-value".to_owned()),
                delta: None,
            },
        }])
        .expect("patient accessibility value");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert_eq!(form.patient_id, "screen-reader-value");
    assert_eq!(form.app.inputs().patient_id, "screen-reader-value");
    let tree = form.app.semantic_tree().expect("semantic tree");
    let patient = tree
        .root
        .children
        .iter()
        .flat_map(|node| node.children.iter())
        .flat_map(|node| node.children.iter())
        .find(|node| node.id.as_deref() == Some("label-patient"))
        .expect("patient input");
    assert_eq!(patient.value.as_deref(), Some("screen-reader-value"));
}

#[test]
fn native_accessibility_patient_input_rejects_invalid_values() {
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600).expect("form");
    app.set_inputs("patient", 70.0, 4.0, 0.5)
        .expect("patient value");
    let mut form = NativeForm {
        app,
        pid: 1,
        patient_id: "patient".to_owned(),
        focused: true,
    };
    let oversized = form
        .handle_events(&[WindowEvent::AccessibilityAction {
            request: AccessibilityActionRequest {
                target_node: super::native_accessibility::patient_input_identity(),
                action: AccessibilityAction::SetValue,
                value: Some("x".repeat(MAX_PATIENT_ID_BYTES + 1)),
                delta: None,
            },
        }])
        .expect_err("oversized accessibility value");
    assert_eq!(oversized.code, ErrorCode::PayloadTooLarge);
    assert_eq!(form.patient_id, "patient");

    let control = form
        .handle_events(&[WindowEvent::AccessibilityAction {
            request: AccessibilityActionRequest {
                target_node: super::native_accessibility::patient_input_identity(),
                action: AccessibilityAction::SetValue,
                value: Some("patient\n".to_owned()),
                delta: None,
            },
        }])
        .expect_err("control accessibility value");
    assert_eq!(control.code, ErrorCode::MalformedPayload);
    assert_eq!(form.patient_id, "patient");
}

#[test]
fn dpi_event_repaints_and_scales_the_submit_hit_region() {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("form");
    let initial = submit_rect(&app).expect("initial submit surface");
    let mut form = NativeForm {
        app,
        pid: 1,
        patient_id: "patient".to_owned(),
        focused: true,
    };
    let flow = form
        .handle_events(&[WindowEvent::DpiChanged { dpi: 144 }])
        .expect("DPI event");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert_eq!(
        form.app.display_scale(),
        DisplayScale::from_dpi(144).expect("144 DPI")
    );
    let scaled = submit_rect(&form.app).expect("scaled submit surface");
    // The hit region scales on both axes. Its left edge is not a scaling
    // signal: the control is centred, so its offset is half the space its
    // card has left over, and on a fixed physical surface the control grows
    // faster than that card does. A centred control therefore moves inward
    // as the scale rises, which is correct and was never what this asserted.
    assert!(scaled.height > initial.height);
    assert!(scaled.width > initial.width);
}

#[test]
fn enter_submission_accepts_plain_and_control_shortcuts() {
    assert_eq!(
        super::classify_submit_modifier_class(super::SubmitModifierClass::Plain),
        Some(super::SubmitShortcut::Plain)
    );
    assert_eq!(
        super::classify_submit_modifier_class(super::SubmitModifierClass::Control),
        Some(super::SubmitShortcut::Control)
    );
    assert_eq!(
        super::classify_submit_modifier_class(super::SubmitModifierClass::System),
        None
    );
    assert_eq!(
        super::submit_shortcut(ModifierState::NONE),
        Some(super::SubmitShortcut::Plain)
    );
}

#[test]
fn native_command_menu_pointer_and_escape_follow_host_neutral_state() {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("form");
    let mut form = NativeForm {
        app,
        pid: 1,
        patient_id: "patient".to_owned(),
        focused: true,
    };
    let toggle = command_menu_toggle_rect(&form.app).expect("command toggle surface");
    let flow = form
        .handle_events(&[WindowEvent::PointerUp {
            x: toggle.x,
            y: toggle.y,
            button: super::MouseButton::Left,
        }])
        .expect("open command menu");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert!(form.app.command_menu_open());

    let flow = form
        .handle_events(&[WindowEvent::KeyDown {
            virtual_key: super::ESCAPE_KEY,
            repeated: false,
            modifiers: ModifierState::NONE,
        }])
        .expect("close command menu");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert!(!form.app.command_menu_open());

    let flow = form
        .handle_events(&[WindowEvent::KeyDown {
            virtual_key: super::ESCAPE_KEY,
            repeated: false,
            modifiers: ModifierState::NONE,
        }])
        .expect("close native form");
    assert!(matches!(flow, NativeFlow::Exit));
}

#[test]
fn native_command_accessibility_action_applies_theme_only_when_menu_is_open() {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("form");
    let mut form = NativeForm {
        app,
        pid: 1,
        patient_id: "patient".to_owned(),
        focused: true,
    };
    let closed = form
        .handle_events(&[WindowEvent::AccessibilityAction {
            request: AccessibilityActionRequest {
                target_node: super::native_accessibility::theme_dark_identity(),
                action: AccessibilityAction::Activate,
                value: None,
                delta: None,
            },
        }])
        .expect("closed theme action");
    assert!(matches!(closed, NativeFlow::Continue { repaint: false }));
    assert_eq!(form.app.theme(), ApplicationTheme::System);

    form.app.toggle_command_menu().expect("open command menu");
    let open = form
        .handle_events(&[WindowEvent::AccessibilityAction {
            request: AccessibilityActionRequest {
                target_node: super::native_accessibility::theme_dark_identity(),
                action: AccessibilityAction::Activate,
                value: None,
                delta: None,
            },
        }])
        .expect("dark theme action");
    assert!(matches!(open, NativeFlow::Continue { repaint: true }));
    assert_eq!(form.app.theme(), ApplicationTheme::Dark);
    assert!(!form.app.command_menu_open());
}

#[test]
fn native_popover_dismisses_without_clicking_through_and_tracks_theme() {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("form");
    let mut form = NativeForm {
        app,
        pid: 1,
        patient_id: "patient".to_owned(),
        focused: true,
    };
    let submit = submit_rect(&form.app).expect("submit surface");
    form.app.toggle_command_menu().expect("open menu");
    assert_eq!(submit_rect(&form.app).expect("stationary submit"), submit);
    assert!(
        form.handle_pointer_up(submit.x, submit.y)
            .expect("dismiss without IPC")
    );
    assert!(!form.app.command_menu_open());
    assert_eq!(form.app.state(), &metis_frontend::FormState::Idle);

    form.app.toggle_command_menu().expect("open menu");
    let menu = super::command_rect(&form.app, "command-menu").expect("menu surface");
    assert!(
        !form
            .handle_pointer_up(menu.x + 1, menu.y + 1)
            .expect("menu padding")
    );
    assert!(form.app.command_menu_open());
    let dark = super::command_rect(&form.app, "command-theme-dark").expect("dark menu item");
    assert!(form.handle_pointer_up(dark.x, dark.y).expect("select dark"));
    assert_eq!(form.app.theme(), ApplicationTheme::Dark);
    assert!(!form.app.command_menu_open());

    let toggle = command_menu_toggle_rect(&form.app).expect("dark toggle");
    assert!(
        form.handle_pointer_up(toggle.x, toggle.y)
            .expect("reopen dark menu")
    );
    let system = super::command_rect(&form.app, "command-theme-system").expect("system menu item");
    assert!(
        form.handle_pointer_up(system.x, system.y)
            .expect("select system")
    );
    assert_eq!(form.app.theme(), ApplicationTheme::System);
    assert!(!form.app.command_menu_open());

    form.app
        .toggle_command_menu()
        .expect("open before focus loss");
    let flow = form
        .handle_events(&[WindowEvent::FocusLost])
        .expect("focus loss");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
    assert!(!form.app.command_menu_open());
}
