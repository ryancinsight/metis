//! Stores filled by asynchronous work started from another store.

use super::{Readable, Store, Writable};
use std::cell::Cell;
use std::rc::{Rc, Weak};

/// The state of a [`Resource`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceState<T, E> {
    /// Work for the current source value has started and not finished.
    Pending,
    /// The work for the current source value succeeded.
    Ready(T),
    /// The work for the current source value failed.
    Failed(E),
}

/// A store holding the outcome of work started for each source value, the
/// counterpart of Dioxus's `use_resource` or Svelte's `{#await}`.
pub type Resource<T, E> = Readable<ResourceState<T, E>>;

/// Delivers the outcome of the work started for one source value.
///
/// A completion is single-use and runtime-agnostic: the caller starts the
/// work on any executor and completes it when the work finishes. A
/// completion whose source value has since changed, or whose resource was
/// dropped, is ignored, so a slow response can never overwrite a newer one.
#[must_use = "an uncompleted resource stays pending"]
pub struct Completion<T, E> {
    target: Weak<super::store::Shared<ResourceState<T, E>>>,
    generation: Rc<Cell<u64>>,
    issued: u64,
}

impl<T, E> Completion<T, E>
where
    T: Clone + PartialEq + 'static,
    E: Clone + PartialEq + 'static,
{
    /// Whether this completion still belongs to the current source value.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.generation.get() == self.issued && self.target.strong_count() > 0
    }

    /// Records the outcome; returns whether it was current and applied.
    pub fn complete(self, outcome: Result<T, E>) -> bool {
        if !self.is_current() {
            return false;
        }
        let Some(shared) = self.target.upgrade() else {
            return false;
        };
        let state = match outcome {
            Ok(value) => ResourceState::Ready(value),
            Err(error) => ResourceState::Failed(error),
        };
        let _ = Writable::from_shared(shared).set(state);
        true
    }
}

/// A resource that calls `start` with each source value and a completion
/// for it, and is [`ResourceState::Pending`] until that completion arrives.
pub fn resource<S, T, E>(
    source: &impl Store<S>,
    start: impl Fn(&S, Completion<T, E>) + 'static,
) -> Resource<T, E>
where
    S: 'static,
    T: Clone + PartialEq + 'static,
    E: Clone + PartialEq + 'static,
{
    let store = Writable::new(ResourceState::Pending);
    let generation = Rc::new(Cell::new(0_u64));
    let target = store.downgrade();
    let subscription = source.subscribe(move |value| {
        let Some(shared) = target.upgrade() else {
            return;
        };
        let issued = generation.get().wrapping_add(1);
        generation.set(issued);
        let _ = Writable::from_shared(shared).set(ResourceState::Pending);
        start(
            value,
            Completion {
                target: target.clone(),
                generation: Rc::clone(&generation),
                issued,
            },
        );
    });
    Readable::with_sources(store, subscription)
}
