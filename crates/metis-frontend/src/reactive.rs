//! Svelte-style reactive stores for Rust-owned application state.
//!
//! A [`Writable`] holds a value and notifies its subscribers when the value
//! changes; [`derived`] computes a [`Readable`] from another store and
//! recomputes it whenever the source changes. Subscribing calls the listener
//! once with the current value, as Svelte's `subscribe` does, and the
//! returned [`Subscription`] unsubscribes when dropped, so a component that
//! drops its handles cannot leave a listener behind. [`derived2`] computes
//! from two sources, [`Writable::project`] gives a writable view of one
//! field that notifies only when that field changes, and [`resource`] holds
//! the outcome of asynchronous work started for each source value, ignoring
//! results that arrive after the source moved on.
//!
//! Stores are single-threaded (`Rc`), matching the one presentation thread
//! each host runs. A listener may set other stores, or the store notifying
//! it; each change is delivered after the current round, in order, and a
//! cascade of more than [`MAX_CASCADE`] rounds is cut off and reported, so
//! two stores that keep setting each other cannot hang the frame.

mod combine;
mod derived;
mod projection;
mod resource;
mod store;
mod subscription;

pub use combine::derived2;
pub use derived::{Readable, derived};
pub use projection::{Field, Projection};
pub use resource::{Completion, Resource, ResourceState, resource};
pub use store::{CascadeLimit, MAX_CASCADE, MAX_SUBSCRIBERS, Writable};
pub use subscription::Subscription;

/// A store a component can read and subscribe to.
pub trait Store<T> {
    /// Calls `read` with the current value without copying it.
    fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R;

    /// Calls `listener` with the current value now and after every change,
    /// until the returned subscription is dropped. At [`MAX_SUBSCRIBERS`]
    /// the listener is refused and the subscription is inactive.
    fn subscribe(&self, listener: impl FnMut(&T) + 'static) -> Subscription;
}

#[cfg(test)]
mod tests;
