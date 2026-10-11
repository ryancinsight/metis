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

mod composition {
    use super::super::{Completion, ResourceState, Store, Writable, derived2, resource};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn derived2_recomputes_from_either_source_and_releases_both() {
        let width = Writable::new(3_u32);
        let height = Writable::new(4_u32);
        let area = derived2(&width, &height, |w, h| w * h);
        assert_eq!(area.get(), 12);
        width.set(5).expect("set width");
        assert_eq!(area.get(), 20);
        height.set(2).expect("set height");
        assert_eq!(area.get(), 10);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&seen);
        let watch = area.subscribe(move |value| log.borrow_mut().push(*value));
        width.set(10).expect("set width");
        height.set(1).expect("set height");
        // 10 x 1 equals the previous 5 x 2 only after the intermediate 10 x 2.
        assert_eq!(*seen.borrow(), [10, 20, 10]);
        height.set(1).expect("unchanged height");
        width.set(10).expect("unchanged width");
        assert_eq!(seen.borrow().len(), 3, "unchanged sources notify nothing");
        drop(watch);
        drop(area);
        assert_eq!(width.subscriber_count(), 0);
        assert_eq!(height.subscriber_count(), 0);
    }

    type Started = Rc<RefCell<Vec<(u32, Completion<String, String>)>>>;

    #[derive(Clone, PartialEq, Debug)]
    struct Patient {
        name: String,
        weight: u32,
    }

    #[test]
    fn projections_notify_only_for_their_field_and_write_through() {
        let patient = Writable::new(Patient {
            name: "A".into(),
            weight: 60,
        });
        let weight = patient.project((|p| &p.weight, |p| &mut p.weight));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&seen);
        let _watch = weight.subscribe(move |value| log.borrow_mut().push(*value));
        patient.update(|p| p.name = "B".into()).expect("rename");
        assert_eq!(*seen.borrow(), [60], "a name change is not a weight change");
        assert!(weight.set(72).expect("set weight"));
        assert_eq!(patient.get().weight, 72);
        weight.update(|w| *w += 1).expect("update weight");
        assert_eq!(*seen.borrow(), [60, 72, 73]);
        assert_eq!(weight.get(), 73);
    }

    #[test]
    fn resources_ignore_completions_for_superseded_values() {
        let query = Writable::new(1_u32);
        let started: Started = Rc::new(RefCell::new(Vec::new()));
        let pending = Rc::clone(&started);
        let result = resource(&query, move |value: &u32, completion| {
            pending.borrow_mut().push((*value, completion));
        });
        assert_eq!(result.get(), ResourceState::Pending);
        query.set(2).expect("new query");
        let mut calls = std::mem::take(&mut *started.borrow_mut());
        assert_eq!(calls.len(), 2);
        let (second_value, second) = calls.pop().expect("second call");
        let (_, first) = calls.pop().expect("first call");
        assert!(!first.is_current());
        assert!(second.is_current());
        assert!(second.complete(Ok(format!("study {second_value}"))));
        assert!(!first.complete(Ok("stale".into())), "stale work is dropped");
        assert_eq!(result.get(), ResourceState::Ready("study 2".into()));

        query.set(3).expect("third query");
        assert_eq!(result.get(), ResourceState::Pending);
        let (_, third) = started.borrow_mut().pop().expect("third call");
        assert!(third.complete(Err("offline".into())));
        assert_eq!(result.get(), ResourceState::Failed("offline".into()));

        query.set(4).expect("fourth query");
        let (_, orphan) = started.borrow_mut().pop().expect("fourth call");
        drop(result);
        assert!(
            !orphan.complete(Ok("late".into())),
            "a dropped resource ignores work"
        );
        assert_eq!(query.subscriber_count(), 0);
    }
}

mod list_model {
    use super::super::{
        CascadeLimit, ListModel, MAX_CASCADE, MAX_MODEL_ROWS, ModelError, RowChange,
    };
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[test]
    fn peers_hear_each_typed_row_change_in_order() {
        let model = ListModel::from_rows(vec!["a", "b"]).expect("bounded rows");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let subscription = model.subscribe(move |change| sink.borrow_mut().push(*change));
        model.push_row("c").expect("push");
        model.insert_row(0, "d").expect("insert");
        assert_eq!(model.set_row_data(2, "e"), Ok(true));
        assert_eq!(model.remove_row(0).expect("remove"), "d");
        assert_eq!(
            *seen.borrow(),
            [
                RowChange::Reset,
                RowChange::Added { index: 2, count: 1 },
                RowChange::Added { index: 0, count: 1 },
                RowChange::Changed(2),
                RowChange::Removed { index: 0, count: 1 },
            ],
            "subscribe begins with Reset so loading and reloading share one path"
        );
        assert_eq!(model.row_count(), 3);
        assert_eq!(model.row_data(0), Some("a"));
        assert_eq!(model.row_data(2), Some("c"));
        assert_eq!(model.row_data(3), None);
        drop(subscription);
    }

    #[test]
    fn equal_replacements_and_out_of_bounds_rows_deliver_nothing() {
        let model = ListModel::from_rows(vec!["a", "b"]).expect("bounded rows");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let subscription = model.subscribe(move |change| sink.borrow_mut().push(*change));
        assert_eq!(
            model.set_row_data(1, "b"),
            Ok(false),
            "an equal value is not a change"
        );
        assert_eq!(model.set_row_data(2, "c"), Err(ModelError::OutOfBounds));
        assert_eq!(model.remove_row(2), Err(ModelError::OutOfBounds));
        assert_eq!(model.insert_row(3, "c"), Err(ModelError::OutOfBounds));
        assert_eq!(
            *seen.borrow(),
            [RowChange::Reset],
            "refused and equal mutations deliver nothing"
        );
        drop(subscription);
    }

    #[test]
    fn the_row_bound_refuses_the_next_row() {
        let model = ListModel::from_rows(vec![0_u8; MAX_MODEL_ROWS]).expect("bound rows");
        assert_eq!(model.push_row(1), Err(ModelError::RowBound));
        assert_eq!(model.insert_row(0, 1), Err(ModelError::RowBound));
        assert!(matches!(
            ListModel::from_rows(vec![0_u8; MAX_MODEL_ROWS + 1]),
            Err(ModelError::RowBound)
        ));
    }

    #[test]
    fn replace_rows_reloads_every_peer() {
        let model = ListModel::from_rows(vec![1_u32, 2]).expect("bounded rows");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let subscription = model.subscribe(move |change| sink.borrow_mut().push(*change));
        model.replace_rows(vec![3]).expect("replace");
        assert_eq!(*seen.borrow(), [RowChange::Reset, RowChange::Reset]);
        assert_eq!(model.row_data(0), Some(3));
        assert_eq!(model.row_count(), 1);
        drop(subscription);
    }

    #[test]
    fn peers_may_change_rows_while_notified() {
        let model = ListModel::new();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let mutator = model.clone();
        let grown = Rc::new(Cell::new(false));
        let flag = Rc::clone(&grown);
        let _subscription = model.subscribe(move |change| {
            sink.borrow_mut().push(*change);
            if *change == (RowChange::Added { index: 0, count: 1 }) && !flag.replace(true) {
                mutator.push_row(2).expect("nested push");
            }
        });
        model.push_row(1).expect("push");
        assert_eq!(
            *seen.borrow(),
            [
                RowChange::Reset,
                RowChange::Added { index: 0, count: 1 },
                RowChange::Added { index: 1, count: 1 },
            ],
            "a change raised during delivery arrives in a later round"
        );
        assert_eq!(model.row_count(), 2);
    }

    #[test]
    fn a_runaway_row_cascade_is_cut_off_and_reported() {
        let model = ListModel::new();
        let calls = Rc::new(Cell::new(0_usize));
        let counter = Rc::clone(&calls);
        let mutator = model.clone();
        let _subscription = model.subscribe(move |change| {
            counter.set(counter.get() + 1);
            if *change != RowChange::Reset {
                let _ = mutator.push_row(1);
            }
        });
        calls.set(0);
        assert_eq!(model.push_row(0), Err(ModelError::Cascade(CascadeLimit)));
        assert_eq!(
            calls.get(),
            MAX_CASCADE,
            "one delivery round per cascade step"
        );
        assert_eq!(
            model.row_count(),
            1 + MAX_CASCADE,
            "rows already changed stay"
        );
    }

    #[test]
    fn dropped_peers_hear_nothing() {
        let model = ListModel::from_rows(vec![1_u32]).expect("bounded rows");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let subscription = model.subscribe(move |change| sink.borrow_mut().push(*change));
        assert_eq!(model.subscriber_count(), 1);
        drop(subscription);
        assert_eq!(model.subscriber_count(), 0);
        model.push_row(2).expect("push");
        assert_eq!(
            *seen.borrow(),
            [RowChange::Reset],
            "a dropped peer hears nothing"
        );
    }
}
