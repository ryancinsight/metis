//! Writable views of one field of a store.

use super::{CascadeLimit, Readable, Store, Subscription, Writable, derived};

/// A readable and writable view of one part of a [`Writable`], the
/// counterpart of a Dioxus store field or a Svelte writable derived from
/// another. Subscribers are notified only when the projected part changes,
/// and setting it updates the parent store in place.
pub struct Projection<T, F> {
    parent: Writable<T>,
    view: Readable<F>,
    field: fn(&mut T) -> &mut F,
}

/// A pair of accessors selecting the same part of a value, for reading and
/// for writing.
pub type Field<T, F> = (fn(&T) -> &F, fn(&mut T) -> &mut F);

impl<T, F> Clone for Projection<T, F> {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            view: self.view.clone(),
            field: self.field,
        }
    }
}

impl<T: Clone + PartialEq + 'static> Writable<T> {
    /// A writable view of the part of the value that `(read, write)`
    /// select, such as `(|s| &s.name, |s| &mut s.name)`.
    #[must_use]
    pub fn project<F>(&self, (read, field): Field<T, F>) -> Projection<T, F>
    where
        F: Clone + PartialEq + 'static,
    {
        let view = derived(self, move |value: &T| read(value).clone());
        Projection {
            parent: self.clone(),
            view,
            field,
        }
    }
}

impl<T, F> Projection<T, F>
where
    T: Clone + PartialEq + 'static,
    F: Clone + PartialEq + 'static,
{
    /// A copy of the projected part.
    #[must_use]
    pub fn get(&self) -> F {
        self.view.get()
    }

    /// Replaces the projected part of the parent value. Returns whether the
    /// parent changed.
    ///
    /// # Errors
    /// Returns [`CascadeLimit`] as [`Writable::set`] does.
    pub fn set(&self, value: F) -> Result<bool, CascadeLimit> {
        let field = self.field;
        self.parent.update(move |parent| *field(parent) = value)
    }

    /// Changes the projected part in place.
    ///
    /// # Errors
    /// Returns [`CascadeLimit`] as [`Writable::set`] does.
    pub fn update(&self, change: impl FnOnce(&mut F)) -> Result<bool, CascadeLimit> {
        let field = self.field;
        self.parent.update(move |parent| change(field(parent)))
    }
}

impl<T, F> Store<F> for Projection<T, F>
where
    T: Clone + PartialEq + 'static,
    F: Clone + PartialEq + 'static,
{
    fn with<R>(&self, read: impl FnOnce(&F) -> R) -> R {
        self.view.with(read)
    }

    fn subscribe(&self, listener: impl FnMut(&F) + 'static) -> Subscription {
        self.view.subscribe(listener)
    }
}
