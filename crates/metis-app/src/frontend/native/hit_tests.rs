//! A pointer press lands on what a fresh layout of the same form says it
//! lands on.

use super::NativeForm;
use super::idle_tests::form;
use super::keyboard::PATIENT_INPUT;
use super::pointer::PointerHit;
use metis_core::ErrorCode;
use metis_frontend::{ApplicationCommand, FocusOrigin, FormState, FrontendApp};
use metis_ipc::MemoryTransport;
use metis_platform::{DisplayScale, Rect};
use metis_ui_lang::{DisplayList, LayoutViewport, compute_layout};

type Form = NativeForm<MemoryTransport>;

/// A form state a press can meet.
struct Scenario {
    name: &'static str,
    setup: fn(&mut Form),
}

const SCENARIOS: [Scenario; 6] = [
    Scenario {
        name: "initial form",
        setup: |_| {},
    },
    Scenario {
        name: "menu open",
        setup: |form| form.app.toggle_command_menu().expect("open the menu"),
    },
    Scenario {
        name: "scaled to 150 percent",
        setup: |form| {
            let scale = DisplayScale::from_milli(1_500).expect("150 percent");
            form.app.set_display_scale(scale).expect("scale");
        },
    },
    Scenario {
        name: "resized with the menu open",
        setup: |form| {
            form.app.resize(640, 480).expect("resize");
            form.app.toggle_command_menu().expect("open the menu");
        },
    },
    Scenario {
        name: "patient reference focused by keyboard",
        setup: |form| {
            form.app
                .focus_control(PATIENT_INPUT, FocusOrigin::Keyboard)
                .expect("focus");
        },
    },
    Scenario {
        name: "dark theme with the menu open",
        setup: |form| {
            form.app
                .activate_command(ApplicationCommand::ThemeDark)
                .expect("dark theme");
            form.app.toggle_command_menu().expect("open the menu");
        },
    },
];

const SURFACES: [&str; 7] = [
    "command-menu-toggle",
    "command-focus-patient",
    "btn-calc",
    PATIENT_INPUT,
    "command-menu",
    "command-theme-dark",
    "command-theme-system",
];

/// The layout of the form's document at its surface size and display scale.
fn fresh_layout(app: &FrontendApp<MemoryTransport>) -> DisplayList {
    let width = i32::try_from(app.framebuffer().width()).expect("width fits i32");
    let height = i32::try_from(app.framebuffer().height()).expect("height fits i32");
    compute_layout(
        app.document(),
        LayoutViewport::with_scale(width, height, app.display_scale()),
    )
    .expect("layout")
}

fn surface(layout: &DisplayList, id: &str) -> Rect {
    layout.element_rect(id).expect("authored surface")
}

/// What `layout` says a press at `(x, y)` lands on: the first control of the
/// menu state whose surface holds the point; else, with the menu open, its
/// surface or everything outside it; else the patient reference.
fn hit_by_layout(menu_open: bool, layout: &DisplayList, x: i32, y: i32) -> PointerHit {
    let targets: [&'static str; 3] = if menu_open {
        [
            "command-menu-toggle",
            ApplicationCommand::ThemeDark.id(),
            ApplicationCommand::ThemeSystem.id(),
        ]
    } else {
        [
            "command-menu-toggle",
            ApplicationCommand::FocusPatient.id(),
            "btn-calc",
        ]
    };
    for target in targets {
        if surface(layout, target).contains(x, y) {
            return PointerHit::Control(target);
        }
    }
    if menu_open {
        if surface(layout, "command-menu").contains(x, y) {
            PointerHit::MenuSurface
        } else {
            PointerHit::OutsideMenu
        }
    } else if surface(layout, PATIENT_INPUT).contains(x, y) {
        PointerHit::PatientInput
    } else {
        PointerHit::Nothing
    }
}

/// Points on and just outside the edges and corners of `rect`.
fn probes(rect: Rect) -> [(i32, i32); 5] {
    let (right, bottom) = (rect.x + rect.width, rect.y + rect.height);
    [
        (rect.x, rect.y),
        (rect.x - 1, rect.y),
        (right - 1, bottom - 1),
        (right, bottom - 1),
        (right - 1, bottom),
    ]
}

#[test]
fn a_press_lands_where_a_fresh_layout_says() {
    let mut seen = Vec::new();
    for Scenario { name, setup } in SCENARIOS {
        let mut form = form();
        setup(&mut form);
        let layout = fresh_layout(&form.app);
        let mut points = vec![(i32::MIN, 0), (0, i32::MIN), (i32::MAX, i32::MAX), (-1, -1)];
        for id in SURFACES {
            assert_eq!(
                form.app.element_rect(id).ok(),
                layout.element_rect(id),
                "{name}: {id}"
            );
            points.extend(layout.element_rect(id).into_iter().flat_map(probes));
        }
        let menu_open = form.app.command_menu_open();
        let mut controls = 0;
        for (x, y) in points {
            let hit = form.pointer_hit(x, y).expect("hit");
            assert_eq!(
                hit,
                hit_by_layout(menu_open, &layout, x, y),
                "{name}: ({x}, {y})"
            );
            controls += usize::from(matches!(hit, PointerHit::Control(_)));
            seen.push(hit);
        }
        assert!(
            controls >= 3,
            "{name}: only {controls} points hit a control"
        );
    }
    for id in [
        "command-menu-toggle",
        ApplicationCommand::FocusPatient.id(),
        "btn-calc",
        ApplicationCommand::ThemeDark.id(),
        ApplicationCommand::ThemeSystem.id(),
    ] {
        assert!(seen.contains(&PointerHit::Control(id)), "{id} is never hit");
    }
    for hit in [
        PointerHit::MenuSurface,
        PointerHit::OutsideMenu,
        PointerHit::PatientInput,
        PointerHit::Nothing,
    ] {
        assert!(seen.contains(&hit), "{hit:?} is never hit");
    }
}

fn centre(form: &Form, id: &str) -> (i32, i32) {
    let rect = form.app.element_rect(id).expect("surface");
    (rect.x + rect.width / 2, rect.y + rect.height / 2)
}

#[test]
fn a_press_applies_the_action_of_what_it_lands_on() {
    let mut form = form();
    let (x, y) = centre(&form, "command-menu-toggle");
    assert_eq!(form.handle_pointer_up(x, y), Ok(true));
    assert!(form.app.command_menu_open());
    assert_eq!(form.app.focused_control(), "command-menu-toggle");
    assert!(!form.app.focus_visible());

    assert_eq!(form.handle_pointer_up(-1, -1), Ok(true));
    assert!(!form.app.command_menu_open());
    assert_eq!(form.app.command_status(), "Commands closed");

    let (x, y) = centre(&form, ApplicationCommand::FocusPatient.id());
    assert_eq!(form.handle_pointer_up(x, y), Ok(true));
    assert_eq!(form.app.command_status(), "Patient reference focused");

    form.app
        .focus_control("btn-calc", FocusOrigin::Keyboard)
        .expect("focus the submit control");
    let (x, y) = centre(&form, PATIENT_INPUT);
    assert_eq!(form.handle_pointer_up(x, y), Ok(true));
    assert_eq!(form.app.focused_control(), PATIENT_INPUT);
    assert!(!form.app.focus_visible());
}

#[test]
fn a_press_on_the_submit_control_focuses_it_and_submits() {
    let mut form = form();
    let (x, y) = centre(&form, "btn-calc");
    let error = form
        .handle_pointer_up(x, y)
        .expect_err("the form holds no session, so the submission is refused");
    assert_eq!(error.code, ErrorCode::MissingCapability);
    assert_eq!(form.app.focused_control(), "btn-calc");
    assert!(!form.app.focus_visible());
    assert!(
        matches!(form.app.state(), FormState::Failed(failure) if failure.code == ErrorCode::MissingCapability),
        "the refused submission is recorded as the form state: {:?}",
        form.app.state()
    );
}
