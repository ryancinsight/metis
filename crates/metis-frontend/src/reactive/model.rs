//! Row models with typed change delivery.
//!
//! A [`ListModel`] holds rows and tells its peers exactly which rows moved,
//! in the vocabulary of Slint's `ModelNotify`: one row replaced
//! ([`RowChange::Changed`]), rows inserted ([`RowChange::Added`]), rows
//! removed ([`RowChange::Removed`]), or a wholesale reload
//! ([`RowChange::Reset`]). Peers — a virtualized window, a result table, a
//! browser list — apply the change to their own row-indexed state instead of
//! re-reading every row on every edit.
//!
//! The contract is Slint's shape with Metis's bounds: rows are limited by
//! [`MAX_MODEL_ROWS`], peers by [`MAX_SUBSCRIBERS`], and a change cascade is
//! cut off and reported at [`MAX_CASCADE`] rounds. Unlike Slint's
//! `VecModel`, an equal replacement is not a change and delivers nothing,
//! matching [`Writable`](super::Writable). Dependencies stay explicit: a
//! peer subscribes and receives [`RowChange::Reset`] first, so its initial
//! load is the same code path as a reload; nothing tracks which row a
//! listener happened to read.

use super::{CascadeLimit, MAX_CASCADE, MAX_SUBSCRIBERS, Subscription};
use metis_ui_lang::layout::MAX_VIRTUAL_ITEMS;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

/// Most rows one [`ListModel`] holds: the same bound as the `VirtualList`
/// windowing primitive, so every modeled list can be virtualized.
pub const MAX_MODEL_ROWS: usize = MAX_VIRTUAL_ITEMS;

/// A refused model mutation (Slint's `ModelError` counterpart).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelError {
    /// The row is outside the model (Slint's `ModelError::out_of_bounds`).
    OutOfBounds,
    /// The mutation would move the row count past [`MAX_MODEL_ROWS`].
    RowBound,
    /// Listeners kept changing rows past [`MAX_CASCADE`] rounds
    /// ([`CascadeLimit`]); undelivered changes were dropped, so peers must
    /// reload through [`RowChange::Reset`] semantics.
    Cascade(CascadeLimit),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfBounds => formatter.write_str("model row is out of range"),
            Self::RowBound => formatter.write_str("model row count exceeds its bound"),
            Self::Cascade(limit) => limit.fmt(formatter),
        }
    }
}

impl std::error::Error for ModelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cascade(limit) => Some(limit),
            Self::OutOfBounds | Self::RowBound => None,
        }
    }
}

impl From<CascadeLimit> for ModelError {
    fn from(limit: CascadeLimit) -> Self {
        Self::Cascade(limit)
    }
}

/// One row-level change (Slint's `ModelNotify` vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowChange {
    /// `row_changed`: the row at the index was replaced in place.
    Changed(usize),
    /// `row_added`: `count` rows appeared at `index`, moving later rows right.
    Added {
        /// First inserted row.
        index: usize,
        /// Number of inserted rows.
        count: usize,
    },
    /// `row_removed`: `count` rows disappeared at `index`, moving later rows
    /// left.
    Removed {
        /// First removed row.
        index: usize,
        /// Number of removed rows.
        count: usize,
    },
    /// `reset`: the rows changed in a way peers must reload completely.
    Reset,
}

type Listener = Box<dyn FnMut(&RowChange)>;

struct Slot {
    id: u64,
    /// `None` while the listener runs, so it can be called without holding a
    /// borrow of the slot list.
    listener: Option<Listener>,
}

struct Shared<T> {
    rows: RefCell<Vec<T>>,
    slots: RefCell<Vec<Slot>>,
    next_id: Cell<u64>,
    /// Changes awaiting the next delivery round; listeners may mutate the
    /// model mid-delivery, and their changes arrive in a later round.
    pending: RefCell<VecDeque<RowChange>>,
    delivering: Cell<bool>,
}

/// A row model whose peers hear typed changes (Slint's `VecModel` and
/// `ModelNotify` counterparts). Clones share the rows and the peer list.
pub struct ListModel<T> {
    shared: Rc<Shared<T>>,
}

impl<T> Clone for ListModel<T> {
    fn clone(&self) -> Self {
        Self {
            shared: Rc::clone(&self.shared),
        }
    }
}

impl<T: Clone + PartialEq + 'static> ListModel<T> {
    /// A model with no rows.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Rc::new(Shared {
                rows: RefCell::new(Vec::new()),
                slots: RefCell::new(Vec::new()),
                next_id: Cell::new(0),
                pending: RefCell::new(VecDeque::new()),
                delivering: Cell::new(false),
            }),
        }
    }

    /// A model holding `rows`.
    ///
    /// # Errors
    /// Returns [`ModelError::RowBound`] when `rows` exceeds
    /// [`MAX_MODEL_ROWS`].
    pub fn from_rows(rows: Vec<T>) -> Result<Self, ModelError> {
        if rows.len() > MAX_MODEL_ROWS {
            return Err(ModelError::RowBound);
        }
        Ok(Self {
            shared: Rc::new(Shared {
                rows: RefCell::new(rows),
                slots: RefCell::new(Vec::new()),
                next_id: Cell::new(0),
                pending: RefCell::new(VecDeque::new()),
                delivering: Cell::new(false),
            }),
        })
    }

    /// Number of rows.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.shared.rows.borrow().len()
    }

    /// Whether the model has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.row_count() == 0
    }

    /// A copy of the row at `row`, or `None` outside the model.
    #[must_use]
    pub fn row_data(&self, row: usize) -> Option<T> {
        self.shared.rows.borrow().get(row).cloned()
    }

    /// Replaces the row at `row`, delivering [`RowChange::Changed`]. Returns
    /// whether it changed; an equal value delivers nothing.
    ///
    /// # Errors
    /// Returns [`ModelError::OutOfBounds`] for a row outside the model and
    /// [`ModelError::Cascade`] as [`Self::push_row`] does.
    pub fn set_row_data(&self, row: usize, data: T) -> Result<bool, ModelError> {
        {
            let mut rows = self.shared.rows.borrow_mut();
            let Some(current) = rows.get_mut(row) else {
                return Err(ModelError::OutOfBounds);
            };
            if *current == data {
                return Ok(false);
            }
            *current = data;
        }
        self.shared.queue(RowChange::Changed(row))?;
        Ok(true)
    }

    /// Appends `data`, delivering [`RowChange::Added`] for one row.
    ///
    /// # Errors
    /// Returns [`ModelError::RowBound`] at [`MAX_MODEL_ROWS`] and
    /// [`ModelError::Cascade`] when listeners keep changing rows past
    /// [`MAX_CASCADE`] rounds.
    pub fn push_row(&self, data: T) -> Result<(), ModelError> {
        self.insert_row(self.row_count(), data)
    }

    /// Inserts `data` at `index`, moving later rows right, and delivers
    /// [`RowChange::Added`] for one row. An index at the row count appends.
    ///
    /// # Errors
    /// Returns [`ModelError::OutOfBounds`] past the row count,
    /// [`ModelError::RowBound`] at [`MAX_MODEL_ROWS`] and
    /// [`ModelError::Cascade`] as [`Self::push_row`] does.
    pub fn insert_row(&self, index: usize, data: T) -> Result<(), ModelError> {
        {
            let mut rows = self.shared.rows.borrow_mut();
            if rows.len() >= MAX_MODEL_ROWS {
                return Err(ModelError::RowBound);
            }
            if index > rows.len() {
                return Err(ModelError::OutOfBounds);
            }
            rows.insert(index, data);
        }
        self.shared.queue(RowChange::Added { index, count: 1 })?;
        Ok(())
    }

    /// Removes and returns the row at `row`, delivering
    /// [`RowChange::Removed`] for one row.
    ///
    /// # Errors
    /// Returns [`ModelError::OutOfBounds`] for a row outside the model and
    /// [`ModelError::Cascade`] as [`Self::push_row`] does.
    pub fn remove_row(&self, row: usize) -> Result<T, ModelError> {
        let removed = {
            let mut rows = self.shared.rows.borrow_mut();
            if row >= rows.len() {
                return Err(ModelError::OutOfBounds);
            }
            rows.remove(row)
        };
        self.shared.queue(RowChange::Removed {
            index: row,
            count: 1,
        })?;
        Ok(removed)
    }

    /// Replaces every row with `rows`, delivering [`RowChange::Reset`] so
    /// peers reload. This is the escape hatch for batched or reordered
    /// changes that have no row-level description.
    ///
    /// # Errors
    /// Returns [`ModelError::RowBound`] when `rows` exceeds
    /// [`MAX_MODEL_ROWS`] and [`ModelError::Cascade`] as [`Self::push_row`]
    /// does.
    pub fn replace_rows(&self, rows: Vec<T>) -> Result<(), ModelError> {
        if rows.len() > MAX_MODEL_ROWS {
            return Err(ModelError::RowBound);
        }
        *self.shared.rows.borrow_mut() = rows;
        self.shared.queue(RowChange::Reset)?;
        Ok(())
    }

    /// Calls `listener` with [`RowChange::Reset`] now and with every change
    /// until the returned [`Subscription`] is dropped. At
    /// [`MAX_SUBSCRIBERS`] the listener is refused and the subscription is
    /// inactive.
    pub fn subscribe(&self, mut listener: impl FnMut(&RowChange) + 'static) -> Subscription {
        if self.shared.slots.borrow().len() >= MAX_SUBSCRIBERS {
            return Subscription::inert();
        }
        listener(&RowChange::Reset);
        let id = self.shared.next_id.get();
        self.shared.next_id.set(id + 1);
        self.shared.slots.borrow_mut().push(Slot {
            id,
            listener: Some(Box::new(listener)),
        });
        let weak = Rc::downgrade(&self.shared);
        Subscription::new(move || {
            if let Some(shared) = weak.upgrade() {
                shared.slots.borrow_mut().retain(|slot| slot.id != id);
            }
        })
    }

    /// Number of live peer subscriptions.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.shared.slots.borrow().len()
    }
}

impl<T: Clone + PartialEq + 'static> Default for ListModel<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + PartialEq + 'static> Shared<T> {
    /// Appends one change and delivers it, or leaves it for the running
    /// delivery loop.
    fn queue(&self, change: RowChange) -> Result<(), CascadeLimit> {
        self.pending.borrow_mut().push_back(change);
        if self.delivering.get() {
            // The running delivery loop sees the change in its next round.
            return Ok(());
        }
        self.delivering.set(true);
        let mut rounds = 0;
        let outcome = loop {
            rounds += 1;
            let batch: Vec<RowChange> = self.pending.borrow_mut().drain(..).collect();
            for change in &batch {
                let ids: Vec<u64> = self.slots.borrow().iter().map(|slot| slot.id).collect();
                for id in ids {
                    self.call(id, change);
                }
            }
            if self.pending.borrow().is_empty() {
                break Ok(());
            }
            if rounds == MAX_CASCADE {
                // Undelivered changes are dropped with the report; peers
                // reload through `RowChange::Reset` semantics.
                self.pending.borrow_mut().clear();
                break Err(CascadeLimit);
            }
        };
        self.delivering.set(false);
        outcome
    }

    /// Runs listener `id` without holding the slot list, then puts it back
    /// unless it was unsubscribed while running.
    fn call(&self, id: u64, change: &RowChange) {
        let taken = self
            .slots
            .borrow_mut()
            .iter_mut()
            .find(|slot| slot.id == id)
            .and_then(|slot| slot.listener.take());
        let Some(mut listener) = taken else {
            return;
        };
        listener(change);
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
