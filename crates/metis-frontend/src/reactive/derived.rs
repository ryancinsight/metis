//! Stores computed from other stores.

use super::{Store, Subscription, Writable};
use std::rc::Rc;

/// A store whose value is computed; it cannot be set directly.
pub struct Readable<T> {
    store: Writable<T>,
    /// Keeps the source subscription alive exactly as long as this store.
    source: Rc<Subscription>,
}

impl<T> Clone for Readable<T> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            source: Rc::clone(&self.source),
        }
    }
}

impl<T> Readable<T> {
    /// A computed store that keeps `source` alive exactly as long as itself.
    pub(super) fn with_sources(store: Writable<T>, source: Subscription) -> Self {
        Self {
            store,
            source: Rc::new(source),
        }
    }
}

impl<T: Clone + PartialEq + 'static> Readable<T> {
    /// A copy of the current value.
    #[must_use]
    pub fn get(&self) -> T {
        self.store.get()
    }
}

impl<T: Clone + PartialEq + 'static> Store<T> for Readable<T> {
    fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        self.store.with(read)
    }

    fn subscribe(&self, listener: impl FnMut(&T) + 'static) -> Subscription {
        self.store.subscribe(listener)
    }
}

/// A store holding `compute` of `source`'s value, recomputed whenever the
/// source changes and notifying only when the computed value changes.
///
/// The derived store holds its source subscription; the source holds only a
/// weak reference back, so dropping every handle to the derived store
/// releases both.
pub fn derived<S, T>(source: &impl Store<S>, compute: impl Fn(&S) -> T + 'static) -> Readable<T>
where
    S: Clone + PartialEq + 'static,
    T: Clone + PartialEq + 'static,
{
    let store = Writable::new(source.with(|value| compute(value)));
    let target = store.downgrade();
    let subscription = source.subscribe(move |value| {
        if let Some(shared) = target.upgrade() {
            let store = Writable::<T>::from_shared(shared);
            // A cascade past the limit is reported to the setter of the
            // source; the derived value is already current.
            let _ = store.set(compute(value));
        }
    });
    Readable::with_sources(store, subscription)
}
