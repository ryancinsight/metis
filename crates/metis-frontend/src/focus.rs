//! Keyboard focus among the authored form's controls.
//!
//! Focus moves in document order over the controls the semantic tree marks
//! focusable and does not hide or disable, wrapping at either end, as
//! sequential focus navigation does in HTML. How focus arrived decides
//! whether a ring is painted: only keyboard focus shows one, the rule CSS
//! `:focus-visible` follows, since a pointer press already shows where the
//! user acted.

use crate::app::FrontendApp;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::IpcTransport;
use metis_ui_lang::{DomElement, DomNode, SemanticNode, SemanticTree};

/// Control focused when the form opens: the patient reference, so typing
/// edits it without a prior focus move.
pub(crate) const INITIAL_FOCUS: &str = "label-patient";

/// How focus reached the focused control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FocusOrigin {
    /// A pointer press or the form's initial state; no ring is painted.
    Pointer,
    /// Sequential navigation, a keyboard activation or an assistive
    /// technology request; a ring is painted.
    Keyboard,
}

/// Direction of sequential focus navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    /// Next control in document order (Tab).
    Forward,
    /// Previous control in document order (Shift+Tab).
    Backward,
}

/// The focused control and how focus reached it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Focus {
    pub(crate) control: String,
    pub(crate) origin: FocusOrigin,
}

impl Focus {
    pub(crate) fn initial() -> Self {
        Self {
            control: INITIAL_FOCUS.to_owned(),
            origin: FocusOrigin::Pointer,
        }
    }
}

impl<T: IpcTransport> FrontendApp<T> {
    /// Authored id of the control holding keyboard focus.
    #[must_use]
    pub fn focused_control(&self) -> &str {
        &self.focus.control
    }

    /// Whether the focused control shows a focus ring.
    #[must_use]
    pub fn focus_visible(&self) -> bool {
        self.focus.origin == FocusOrigin::Keyboard
    }

    /// Authored ids of the focusable controls in navigation order.
    ///
    /// # Errors
    /// Rejects a document whose semantics cannot be projected.
    pub fn focus_order(&self) -> Result<Vec<String>> {
        Ok(focusable_ids(&self.semantic_tree()?))
    }

    /// Moves keyboard focus to the next or previous control, wrapping at
    /// either end, and repaints with the focus ring.
    ///
    /// # Errors
    /// Rejects a form with no focusable control, and preserves the prior
    /// focus when the surface cannot render.
    pub fn move_focus(&mut self, direction: FocusDirection) -> Result<()> {
        let order = self.focus_order()?;
        let count = order.len();
        if count == 0 {
            return Err(no_focusable_control());
        }
        let next = match order.iter().position(|id| *id == self.focus.control) {
            Some(index) => match direction {
                FocusDirection::Forward => (index + 1) % count,
                FocusDirection::Backward => (index + count - 1) % count,
            },
            None => match direction {
                FocusDirection::Forward => 0,
                FocusDirection::Backward => count - 1,
            },
        };
        self.set_focus(Focus {
            control: order[next].clone(),
            origin: FocusOrigin::Keyboard,
        })
    }

    /// Focuses the control `id` and repaints, returning whether it took
    /// focus: an id that is not a visible, enabled focusable control, such as
    /// an item of a closed menu, leaves focus where it was.
    ///
    /// # Errors
    /// Preserves the prior focus when the surface cannot render.
    pub fn focus_control(&mut self, id: &str, origin: FocusOrigin) -> Result<bool> {
        if !self.focus_order()?.iter().any(|control| control == id) {
            return Ok(false);
        }
        self.set_focus(Focus {
            control: id.to_owned(),
            origin,
        })?;
        Ok(true)
    }

    fn set_focus(&mut self, focus: Focus) -> Result<()> {
        let previous = std::mem::replace(&mut self.focus, focus);
        if let Err(error) = self.render() {
            self.focus = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Moves focus off a control that stopped being focusable, before the
    /// frame that no longer shows it is painted.
    ///
    /// Focus inside a popover returns to the control its `popover-anchor`
    /// names, as it returns to a menu button when its menu closes; any other
    /// control yields to the initial focus.
    pub(crate) fn reconcile_focus(&mut self, semantics: &SemanticTree) -> Result<()> {
        let order = focusable_ids(semantics);
        if order.contains(&self.focus.control) {
            return Ok(());
        }
        let anchor = popover_anchor(&self.doc.root, &self.focus.control)
            .filter(|anchor| order.contains(anchor));
        self.focus.control = anchor
            .or_else(|| order.iter().find(|id| *id == INITIAL_FOCUS).cloned())
            .or_else(|| order.first().cloned())
            .ok_or_else(no_focusable_control)?;
        Ok(())
    }
}

fn focusable_ids(semantics: &SemanticTree) -> Vec<String> {
    let mut order = Vec::new();
    collect_focusable(&semantics.root, &mut order);
    order
}

fn collect_focusable(node: &SemanticNode, order: &mut Vec<String>) {
    if node.hidden {
        return;
    }
    if node.focusable
        && !node.disabled
        && let Some(id) = &node.id
    {
        order.push(id.clone());
    }
    for child in &node.children {
        collect_focusable(child, order);
    }
}

/// The `popover-anchor` of the nearest popover enclosing `id`, if any.
fn popover_anchor(root: &DomElement, id: &str) -> Option<String> {
    let mut ancestors = Vec::new();
    if !ancestors_of(root, id, &mut ancestors) {
        return None;
    }
    ancestors
        .iter()
        .rev()
        .find_map(|element| element.attributes.get("popover-anchor").cloned())
}

/// Pushes the elements enclosing `id`, outermost first, and reports whether
/// `id` lies under `element`.
fn ancestors_of<'dom>(
    element: &'dom DomElement,
    id: &str,
    path: &mut Vec<&'dom DomElement>,
) -> bool {
    for child in &element.children {
        let DomNode::Element(child) = child else {
            continue;
        };
        path.push(element);
        if child.id() == Some(id) || ancestors_of(child, id, path) {
            return true;
        }
        path.pop();
    }
    false
}

fn no_focusable_control() -> MetisError {
    MetisError::ui(
        ErrorCode::MalformedMarkup,
        "The authored form has no focusable control",
    )
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
