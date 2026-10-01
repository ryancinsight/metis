//! The host replaces the surface's accessibility tree only with a tree the
//! application took.

use super::tests::{ProbeError, RecordingSurface};
use super::*;
use crate::native::{AccessibilityNode, AccessibilityRole};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn tree_named(name: &str) -> AccessibilityTree {
    let root = AccessibilityNode::new(1, AccessibilityRole::Application, name)
        .expect("bounded accessibility root");
    AccessibilityTree::from_nodes(1, 1, vec![root]).expect("validated accessibility tree")
}

/// Answers each accessibility take with the next scripted outcome, and `None`
/// once the script ends.
struct AccessibilityApplication {
    framebuffer: Framebuffer,
    script: VecDeque<Option<AccessibilityTree>>,
    taken: Arc<AtomicUsize>,
}

impl AccessibilityApplication {
    fn new(
        script: impl IntoIterator<Item = Option<AccessibilityTree>>,
    ) -> (Self, Arc<AtomicUsize>) {
        let taken = Arc::new(AtomicUsize::new(0));
        (
            Self {
                framebuffer: Framebuffer::new(2, 2).expect("bounded framebuffer"),
                script: script.into_iter().collect(),
                taken: Arc::clone(&taken),
            },
            taken,
        )
    }
}

impl NativeApplication for AccessibilityApplication {
    type Error = ProbeError;

    fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    fn take_accessibility(&mut self) -> Result<Option<AccessibilityTree>, Self::Error> {
        self.taken.fetch_add(1, Ordering::SeqCst);
        Ok(self.script.pop_front().flatten())
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

#[test]
fn generic_host_replaces_the_accessibility_tree_only_with_a_tree_the_application_took() {
    let changed = tree_named("changed");
    let (application, taken) =
        AccessibilityApplication::new([None, None, Some(changed.clone()), None]);
    let focus = || vec![WindowEvent::FocusGained];
    let (surface, trace) = RecordingSurface::new([
        focus(),
        Vec::new(),
        focus(),
        Vec::new(),
        vec![WindowEvent::CloseRequested],
    ]);

    run_application_loop(surface, application, Duration::ZERO).expect("recording host loop");

    let updates = trace
        .accessibility_updates
        .lock()
        .expect("accessibility trace lock remains healthy");
    assert_eq!(updates.as_slice(), [changed]);
    assert_eq!(taken.load(Ordering::SeqCst), 4);
}

#[test]
fn generic_host_makes_no_accessibility_update_over_empty_batches() {
    let (application, taken) = AccessibilityApplication::new([]);
    let mut batches: Vec<Vec<WindowEvent>> = vec![Vec::new(); 6];
    batches.push(vec![WindowEvent::CloseRequested]);
    let (surface, trace) = RecordingSurface::new(batches);

    run_application_loop(surface, application, Duration::ZERO).expect("recording host loop");

    assert!(
        trace
            .accessibility_updates
            .lock()
            .expect("accessibility trace lock remains healthy")
            .is_empty()
    );
    assert_eq!(taken.load(Ordering::SeqCst), 6);
}
