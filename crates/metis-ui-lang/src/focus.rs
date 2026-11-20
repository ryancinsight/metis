//! Sequential keyboard focus over an authored control order.

use std::borrow::Borrow;

/// A borrowed document-order view of focusable control identifiers.
///
/// Traversal itself returns identifiers from the borrowed slice without
/// allocating; caller-supplied [`Borrow`] and equality implementations retain
/// their own behavior. When `current` is absent, forward traversal enters at
/// the first control and backward traversal enters at the last control. Both
/// directions wrap at the opposite endpoint. If identifiers repeat, forward
/// traversal uses the first matching position and backward traversal uses the
/// last matching position.
///
/// # Examples
///
/// ```
/// use metis_ui_lang::TabOrder;
///
/// let controls = ["name", "email", "save"];
/// let order = TabOrder::new(&controls);
/// assert_eq!(order.forward_from("email"), Some(&"save"));
/// assert_eq!(order.backward_from("name"), Some(&"save"));
/// ```
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct TabOrder<'order, Id> {
    controls: &'order [Id],
}

impl<'order, Id> TabOrder<'order, Id> {
    /// Borrows the control identifiers in sequential navigation order.
    pub const fn new(controls: &'order [Id]) -> Self {
        Self { controls }
    }

    /// Selects the control after `current`, wrapping to the first control.
    ///
    /// An absent `current` enters the order at its first control. When an
    /// identifier repeats, its first position is the traversal origin. An
    /// empty order returns `None`.
    #[must_use]
    pub fn forward_from<Current>(&self, current: &Current) -> Option<&'order Id>
    where
        Id: Borrow<Current>,
        Current: Eq + ?Sized,
    {
        let first = self.controls.first()?;
        Some(
            self.controls
                .iter()
                .skip_while(|control| <Id as Borrow<Current>>::borrow(*control) != current)
                .nth(1)
                .unwrap_or(first),
        )
    }

    /// Selects the control before `current`, wrapping to the last control.
    ///
    /// An absent `current` enters the order at its last control. When an
    /// identifier repeats, its last position is the traversal origin. An
    /// empty order returns `None`.
    #[must_use]
    pub fn backward_from<Current>(&self, current: &Current) -> Option<&'order Id>
    where
        Id: Borrow<Current>,
        Current: Eq + ?Sized,
    {
        let last = self.controls.last()?;
        Some(
            self.controls
                .iter()
                .rev()
                .skip_while(|control| <Id as Borrow<Current>>::borrow(*control) != current)
                .nth(1)
                .unwrap_or(last),
        )
    }
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
