//! A bounded, conflict-free accelerator-to-command registry.

use super::{Accelerator, ShortcutError};

/// Upper bound on bindings in one [`ShortcutMap`].
pub const MAX_SHORTCUTS: usize = 64;

/// Binds accelerators to application commands.
///
/// Each accelerator resolves to at most one command, and conflicts are
/// refused when bound rather than resolved by order at key-press time. The
/// map is small and bounded, so lookups scan contiguous storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutMap<C> {
    bindings: Vec<(Accelerator, C)>,
}

impl<C> Default for ShortcutMap<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> ShortcutMap<C> {
    /// An empty map; storage is allocated on the first binding.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Number of bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether the map has no bindings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// The bindings in the order they were made.
    #[must_use = "iterators are lazy"]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (Accelerator, &C)> {
        self.bindings
            .iter()
            .map(|(accelerator, command)| (*accelerator, command))
    }

    /// Binds `accelerator` to `command`.
    ///
    /// # Errors
    /// Returns [`ShortcutError::Conflict`] when the accelerator is already
    /// bound, or [`ShortcutError::Full`] at [`MAX_SHORTCUTS`]; the map is
    /// unchanged in both cases.
    pub fn bind(&mut self, accelerator: Accelerator, command: C) -> Result<(), ShortcutError> {
        if self.resolve(accelerator).is_some() {
            return Err(ShortcutError::Conflict);
        }
        if self.bindings.len() >= MAX_SHORTCUTS {
            return Err(ShortcutError::Full);
        }
        self.bindings.push((accelerator, command));
        Ok(())
    }

    /// Removes the binding for `accelerator`, returning its command.
    pub fn unbind(&mut self, accelerator: Accelerator) -> Option<C> {
        let index = self
            .bindings
            .iter()
            .position(|(bound, _)| *bound == accelerator)?;
        Some(self.bindings.remove(index).1)
    }

    /// The command bound to `accelerator`.
    #[must_use]
    pub fn resolve(&self, accelerator: Accelerator) -> Option<&C> {
        self.bindings
            .iter()
            .find_map(|(bound, command)| (*bound == accelerator).then_some(command))
    }
}

impl<C: PartialEq> ShortcutMap<C> {
    /// The first accelerator bound to `command`, for menu labels.
    #[must_use]
    pub fn accelerator_of(&self, command: &C) -> Option<Accelerator> {
        self.bindings
            .iter()
            .find_map(|(accelerator, bound)| (bound == command).then_some(*accelerator))
    }
}
