//! Accessibility application and tree for host tests.

use crate::Framebuffer;
use crate::native::application::{NativeApplication, NativeFlow};
use crate::native::application::tests::probe::ProbeError;
use crate::native::{AccessibilityNode, AccessibilityRole, AccessibilityTree, WindowEvent};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Projects one tree, then reports no semantic change, and counts the
/// projections so a push the host did not need is visible.
pub struct OnceAccessibilityApplication {
    pub framebuffer: Framebuffer,
    pub projections: Arc<AtomicUsize>,
    pub changed: bool,
}

impl NativeApplication for OnceAccessibilityApplication {
    type Error = ProbeError;

    fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    fn accessibility_tree(&self) -> Result<Option<AccessibilityTree>, Self::Error> {
        self.projections.fetch_add(1, Ordering::SeqCst);
        Ok(Some(accessibility_tree()))
    }

    fn take_accessibility_damage(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
        if events
            .iter()
            .any(|event| matches!(event, WindowEvent::CloseRequested))
        {
            return Ok(NativeFlow::Exit);
        }
        Ok(NativeFlow::Continue { repaint: false })
    }
}

/// A validated tree a host can push, as a native provider receives one.
pub fn accessibility_tree() -> AccessibilityTree {
    let mut button = AccessibilityNode::new(2, AccessibilityRole::Button, "Submit")
        .expect("accessibility button");
    button.set_focusable(true);
    let mut root = AccessibilityNode::new(1, AccessibilityRole::Application, "Metis")
        .expect("accessibility root");
    root.set_children(vec![2]).expect("root children");
    AccessibilityTree::from_nodes(1, 2, vec![root, button]).expect("validated tree")
}