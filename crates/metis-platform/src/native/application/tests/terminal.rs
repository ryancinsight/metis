//! Terminal event classification tests.

use crate::native::application::is_terminal;
use crate::native::application::is_destroyed;
use crate::native::WindowEvent;

#[test]
fn terminal_event_classification_is_explicit() {
    let close = [WindowEvent::CloseRequested];
    let destroyed = [WindowEvent::Destroyed];
    let input = [WindowEvent::FocusGained];
    assert!(close.iter().any(is_terminal));
    assert!(destroyed.iter().any(is_destroyed));
    assert!(!input.iter().any(is_terminal));
}