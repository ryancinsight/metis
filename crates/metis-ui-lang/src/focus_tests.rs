//! Sequential focus targets for ordered, missing and empty controls.

use super::TabOrder;
use std::borrow::Borrow;

const CONTROLS: [&str; 4] = ["menu", "name", "email", "save"];

#[test]
fn traversal_follows_authored_order_and_wraps() {
    let order = TabOrder::new(&CONTROLS);
    assert_eq!(order.forward_from("name"), Some(&"email"));
    assert_eq!(order.forward_from("save"), Some(&"menu"));
    assert_eq!(order.backward_from("email"), Some(&"name"));
    assert_eq!(order.backward_from("menu"), Some(&"save"));
}

#[test]
fn an_absent_current_control_enters_at_the_directional_endpoint() {
    let order = TabOrder::new(&CONTROLS);
    assert_eq!(order.forward_from("detached"), Some(&"menu"));
    assert_eq!(order.backward_from("detached"), Some(&"save"));
}

#[test]
fn an_empty_order_has_no_focus_target() {
    let order = TabOrder::<&str>::new(&[]);
    assert_eq!(order.forward_from("name"), None);
    assert_eq!(order.backward_from("name"), None);
}

#[test]
fn a_complete_cycle_returns_to_the_selected_control() {
    let order = TabOrder::new(&CONTROLS);
    let mut forward = "name";
    let mut backward = "name";
    for _ in &CONTROLS {
        forward = order
            .forward_from(forward)
            .copied()
            .expect("nonempty order has a forward target");
        backward = order
            .backward_from(backward)
            .copied()
            .expect("nonempty order has a backward target");
    }
    assert_eq!(forward, "name");
    assert_eq!(backward, "name");
}

#[derive(Debug, PartialEq, Eq)]
struct ControlId(&'static str);

impl Borrow<str> for ControlId {
    fn borrow(&self) -> &str {
        self.0
    }
}

#[test]
fn traversal_returns_the_borrowed_non_clone_identifier() {
    let controls = [ControlId("name"), ControlId("email")];
    let selected = TabOrder::new(&controls)
        .forward_from("name")
        .expect("nonempty order has a target");
    assert_eq!(selected, &ControlId("email"));
    assert!(std::ptr::eq(
        std::ptr::from_ref(selected),
        std::ptr::from_ref(&controls[1]),
    ));
}
