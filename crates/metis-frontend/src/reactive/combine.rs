//! Stores computed from two sources.

use super::{Readable, Store, Subscription, Writable};
use std::rc::Rc;

/// A store holding `compute` of two sources' values, recomputed when either
/// changes and notifying only when the result changes, as a Dioxus memo
/// over two signals or Svelte's `derived([a, b], ...)` does.
///
/// Like [`super::derived`], the result holds both source subscriptions and
/// the sources hold only weak references back.
pub fn derived2<A, B, SA, SB, T>(
    first: &A,
    second: &B,
    compute: impl Fn(&SA, &SB) -> T + 'static,
) -> Readable<T>
where
    A: Store<SA> + Clone + 'static,
    B: Store<SB> + Clone + 'static,
    SA: 'static,
    SB: 'static,
    T: Clone + PartialEq + 'static,
{
    let compute = Rc::new(compute);
    let initial = first.with(|a| second.with(|b| compute(a, b)));
    let store = Writable::new(initial);

    let target = store.downgrade();
    let other = second.clone();
    let on_first = Rc::clone(&compute);
    let first_subscription = first.subscribe(move |a| {
        if let Some(shared) = target.upgrade() {
            let value = other.with(|b| on_first(a, b));
            let _ = Writable::<T>::from_shared(shared).set(value);
        }
    });

    let target = store.downgrade();
    let other = first.clone();
    let second_subscription = second.subscribe(move |b| {
        if let Some(shared) = target.upgrade() {
            let value = other.with(|a| compute(a, b));
            let _ = Writable::<T>::from_shared(shared).set(value);
        }
    });

    Readable::with_sources(
        store,
        Subscription::both(first_subscription, second_subscription),
    )
}
