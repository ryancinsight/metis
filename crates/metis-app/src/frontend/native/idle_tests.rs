//! An idle event batch allocates nothing and replaces no accessibility tree,
//! and a press that hits nothing allocates nothing.

use super::alloc_counter::ALLOCATIONS;
use super::{NativeForm, native_accessibility};
use metis_frontend::FrontendApp;
use metis_ipc::MemoryTransport;
use metis_platform::native::{MouseButton, NativeApplication, NativeFlow, WindowEvent};

pub(super) fn form() -> NativeForm<MemoryTransport> {
    let (transport, _peer) = MemoryTransport::pair();
    NativeForm {
        app: FrontendApp::new(transport, 800, 600).expect("form"),
        pid: 1,
        patient_id: String::new(),
        focused: true,
    }
}

fn press(x: i32, y: i32) -> WindowEvent {
    WindowEvent::PointerUp {
        x,
        y,
        button: MouseButton::Left,
    }
}

#[test]
fn idle_batches_take_no_accessibility_tree_and_allocate_nothing() {
    let mut form = form();
    let first = form
        .take_accessibility()
        .expect("first take")
        .expect("the native provider is opted into at startup");
    let source = form.app.semantic_tree().expect("semantic tree");
    assert_eq!(
        first,
        native_accessibility::project(&source, form.app.focused_control()).expect("projection")
    );

    ALLOCATIONS.set(0);
    for _ in 0..16 {
        let flow = form.handle_events(&[]).expect("empty batch");
        assert!(matches!(flow, NativeFlow::Continue { repaint: false }));
        assert!(form.take_accessibility().expect("idle take").is_none());
    }
    assert_eq!(ALLOCATIONS.get(), 0);
}

#[test]
fn a_change_replaces_the_accessibility_tree_once() {
    let mut form = form();
    let first = form.take_accessibility().expect("first take");
    let toggle = form
        .app
        .element_rect("command-menu-toggle")
        .expect("toggle surface");
    let flow = form
        .handle_events(&[press(toggle.x, toggle.y)])
        .expect("press the menu button");
    assert!(matches!(flow, NativeFlow::Continue { repaint: true }));

    let changed = form
        .take_accessibility()
        .expect("take after a change")
        .expect("a change reports its tree");
    let source = form.app.semantic_tree().expect("semantic tree");
    assert_eq!(
        changed,
        native_accessibility::project(&source, form.app.focused_control()).expect("projection")
    );
    assert_ne!(Some(changed), first);
    assert!(
        form.take_accessibility()
            .expect("take after taking")
            .is_none()
    );
}

#[test]
fn a_press_that_hits_nothing_allocates_nothing() {
    let mut form = form();
    ALLOCATIONS.set(0);
    let flow = form
        .handle_events(&[press(1, 1)])
        .expect("press the margin");
    assert!(matches!(flow, NativeFlow::Continue { repaint: false }));
    assert_eq!(ALLOCATIONS.get(), 0);

    form.app.toggle_command_menu().expect("open the menu");
    let menu = form.app.element_rect("command-menu").expect("menu surface");
    ALLOCATIONS.set(0);
    let flow = form
        .handle_events(&[press(menu.x + 1, menu.y + 1)])
        .expect("press the menu padding");
    assert!(matches!(flow, NativeFlow::Continue { repaint: false }));
    assert_eq!(ALLOCATIONS.get(), 0);
    assert!(form.app.command_menu_open());
}
