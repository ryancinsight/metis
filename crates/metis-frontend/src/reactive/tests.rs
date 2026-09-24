use super::{CascadeLimit, MAX_CASCADE, MAX_SUBSCRIBERS, Store, Writable, derived};
use std::cell::RefCell;
use std::rc::Rc;

fn recorder<T: Clone + 'static>() -> (Rc<RefCell<Vec<T>>>, impl FnMut(&T) + 'static) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&seen);
    (seen, move |value: &T| sink.borrow_mut().push(value.clone()))
}

#[test]
fn subscribers_see_the_current_value_then_each_change_once() {
    let count = Writable::new(1);
    let (seen, listener) = recorder();
    let subscription = count.subscribe(listener);
    assert_eq!(count.set(2), Ok(true));
    assert_eq!(count.set(2), Ok(false), "an equal value is not a change");
    assert_eq!(count.update(|value| *value += 1), Ok(true));
    assert_eq!(*seen.borrow(), [1, 2, 3]);
    drop(subscription);
    assert_eq!(count.subscriber_count(), 0);
    count.set(9).expect("set");
    assert_eq!(
        *seen.borrow(),
        [1, 2, 3],
        "a dropped subscription hears nothing"
    );
}

#[test]
fn derived_stores_recompute_only_on_real_changes_and_chain() {
    let weight = Writable::new(70_u32);
    let heavy = derived(&weight, |kg| *kg >= 100);
    let label = derived(&heavy, |heavy| if *heavy { "heavy" } else { "light" });
    let (seen, listener) = recorder();
    let _subscription = label.subscribe(listener);
    weight.set(80).expect("set");
    weight.set(120).expect("set");
    weight.set(130).expect("set");
    assert_eq!(*seen.borrow(), ["light", "heavy"]);
    assert_eq!(label.get(), "heavy");
    drop(label);
    drop(heavy);
    assert_eq!(
        weight.subscriber_count(),
        0,
        "dropping a derived store releases its source"
    );
}

#[test]
fn listeners_may_set_stores_and_unsubscribe_while_notified() {
    let source = Writable::new(0);
    let mirror = Writable::new(0);
    let mirror_setter = mirror.clone();
    let _link = source.subscribe(move |value| {
        mirror_setter.set(*value * 10).expect("mirror");
    });
    // A listener that clamps its own store re-enters delivery.
    let clamp = source.clone();
    let _clamp = source.subscribe(move |value| {
        if *value > 5 {
            clamp.set(5).expect("clamp");
        }
    });
    source.set(9).expect("set");
    assert_eq!(source.get(), 5);
    assert_eq!(mirror.get(), 50, "the final value reaches every listener");

    let holder: Rc<RefCell<Option<super::Subscription>>> = Rc::new(RefCell::new(None));
    let inner = Rc::clone(&holder);
    let once = Writable::new(0);
    let calls = Rc::new(RefCell::new(0));
    let counter = Rc::clone(&calls);
    *holder.borrow_mut() = Some(once.subscribe(move |_| {
        *counter.borrow_mut() += 1;
        if *counter.borrow() == 2 {
            inner.borrow_mut().take();
        }
    }));
    once.set(1).expect("set");
    once.set(2).expect("set");
    assert_eq!(
        *calls.borrow(),
        2,
        "a listener that unsubscribes itself is not called again"
    );
}

#[test]
fn runaway_cascades_and_subscriber_floods_are_bounded() {
    let store = Writable::new(0_usize);
    let echo = store.clone();
    let _loop = store.subscribe(move |value| {
        let _ = echo.set(value + 1);
    });
    assert_eq!(store.set(1_000), Err(CascadeLimit));
    assert!(store.get() >= 1_000 + MAX_CASCADE);

    let crowded = Writable::new(());
    let kept: Vec<_> = (0..MAX_SUBSCRIBERS)
        .map(|_| crowded.subscribe(|()| {}))
        .collect();
    assert!(kept.iter().all(super::Subscription::is_active));
    assert!(!crowded.subscribe(|()| {}).is_active());
}
