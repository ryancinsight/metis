//! Writable stores and change delivery.

use super::{Store, Subscription};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// Most listeners one store keeps; further subscriptions are refused.
pub const MAX_SUBSCRIBERS: usize = 256;
/// Most notification rounds one change may cascade through.
pub const MAX_CASCADE: usize = 64;

/// A change cascaded through more than [`MAX_CASCADE`] rounds; the store
/// holds the latest value, and later rounds were not delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CascadeLimit;

impl std::fmt::Display for CascadeLimit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("reactive update cascaded past its round limit")
    }
}

impl std::error::Error for CascadeLimit {}

type Listener<T> = Box<dyn FnMut(&T)>;

struct Slot<T> {
    id: u64,
    /// `None` while the listener is running, so it can be called without
    /// holding a borrow of the slot list.
    listener: Option<Listener<T>>,
}

pub(super) struct Shared<T> {
    value: RefCell<T>,
    slots: RefCell<Vec<Slot<T>>>,
    next_id: Cell<u64>,
    notifying: Cell<bool>,
    pending: Cell<bool>,
}

/// A store whose value the owner sets.
pub struct Writable<T> {
    shared: Rc<Shared<T>>,
}

impl<T> Clone for Writable<T> {
    fn clone(&self) -> Self {
        Self {
            shared: Rc::clone(&self.shared),
        }
    }
}

impl<T: Clone + PartialEq + 'static> Writable<T> {
    /// A store holding `value`.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            shared: Rc::new(Shared {
                value: RefCell::new(value),
                slots: RefCell::new(Vec::new()),
                next_id: Cell::new(0),
                notifying: Cell::new(false),
                pending: Cell::new(false),
            }),
        }
    }

    /// A copy of the current value.
    #[must_use]
    pub fn get(&self) -> T {
        self.shared.value.borrow().clone()
    }

    /// Replaces the value, notifying subscribers when it changed. Returns
    /// whether it changed.
    ///
    /// # Errors
    /// Returns [`CascadeLimit`] when listeners keep changing stores past
    /// [`MAX_CASCADE`] rounds; the value is still set.
    pub fn set(&self, value: T) -> Result<bool, CascadeLimit> {
        if *self.shared.value.borrow() == value {
            return Ok(false);
        }
        *self.shared.value.borrow_mut() = value;
        self.shared.notify().map(|()| true)
    }

    /// Changes the value in place with `change`, notifying subscribers when
    /// the result differs. Returns whether it changed.
    ///
    /// # Errors
    /// Returns [`CascadeLimit`] as [`Self::set`] does.
    pub fn update(&self, change: impl FnOnce(&mut T)) -> Result<bool, CascadeLimit> {
        let mut next = self.get();
        change(&mut next);
        self.set(next)
    }

    /// Number of live subscriptions.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.shared.slots.borrow().len()
    }

    pub(super) fn downgrade(&self) -> Weak<Shared<T>> {
        Rc::downgrade(&self.shared)
    }

    pub(super) fn from_shared(shared: Rc<Shared<T>>) -> Self {
        Self { shared }
    }
}

impl<T: Clone + PartialEq + 'static> Store<T> for Writable<T> {
    fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        read(&self.shared.value.borrow())
    }

    fn subscribe(&self, listener: impl FnMut(&T) + 'static) -> Subscription {
        self.shared.subscribe(Box::new(listener))
    }
}

impl<T: Clone + PartialEq + 'static> Shared<T> {
    fn subscribe(self: &Rc<Self>, mut listener: Listener<T>) -> Subscription {
        if self.slots.borrow().len() >= MAX_SUBSCRIBERS {
            return Subscription::inert();
        }
        let current = self.value.borrow().clone();
        listener(&current);
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        self.slots.borrow_mut().push(Slot {
            id,
            listener: Some(listener),
        });
        let weak = Rc::downgrade(self);
        Subscription::new(move || {
            if let Some(shared) = weak.upgrade() {
                shared.slots.borrow_mut().retain(|slot| slot.id != id);
            }
        })
    }

    /// Delivers the current value to every listener, repeating while a
    /// listener changed this store during delivery.
    fn notify(&self) -> Result<(), CascadeLimit> {
        if self.notifying.get() {
            // The running delivery loop sees the change after its round.
            self.pending.set(true);
            return Ok(());
        }
        self.notifying.set(true);
        let mut rounds = 0;
        let outcome = loop {
            self.pending.set(false);
            rounds += 1;
            let snapshot = self.value.borrow().clone();
            let ids: Vec<u64> = self.slots.borrow().iter().map(|slot| slot.id).collect();
            for id in ids {
                self.call(id, &snapshot);
            }
            if !self.pending.get() {
                break Ok(());
            }
            if rounds == MAX_CASCADE {
                break Err(CascadeLimit);
            }
        };
        self.notifying.set(false);
        self.pending.set(false);
        outcome
    }

    /// Runs listener `id` without holding the slot list, then puts it back
    /// unless it was unsubscribed while running.
    fn call(&self, id: u64, value: &T) {
        let taken = self
            .slots
            .borrow_mut()
            .iter_mut()
            .find(|slot| slot.id == id)
            .and_then(|slot| slot.listener.take());
        let Some(mut listener) = taken else {
            return;
        };
        listener(value);
        if let Some(slot) = self
            .slots
            .borrow_mut()
            .iter_mut()
            .find(|slot| slot.id == id)
        {
            slot.listener = Some(listener);
        }
    }
}
