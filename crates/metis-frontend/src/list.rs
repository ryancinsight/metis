//! Virtualized list view over a [`ListModel`].
//!
//! [`VirtualListView`] measures one extent per row, asks [`VirtualList`] which
//! rows a scroll position shows, and projects only those rows as DOM between
//! two spacers that reserve the rest of the content. The host supplies the
//! scroll offset and viewport extent; rows outside the window are never built.
//! This is the counterpart of egui's `ScrollArea::show_rows`, GPUI's
//! `uniform_list` and the DOM spacer technique.
//!
//! A row change reaches the view as its index contract ([`RowChange`]). The
//! subscription records where rows were inserted, removed or changed and marks
//! the affected extents unknown; values are read from the model at the next
//! query. A row is therefore measured from the state that query sees, never
//! from a state a later change has superseded. A reset rereads every row. A
//! query rebuilds the [`VirtualList`] only after a change, which costs `O(n)`
//! in the row count; per-row Fenwick updates are a tightening step, not
//! assumed to matter.
//!
//! The measure function must be a pure function of the row. A change that
//! lands while rows are being measured is refused as
//! [`ErrorCode::RenderFailure`] rather than merged into stale positions.
//! Queries are valid outside a model delivery round; a query made from a row
//! listener can see a model ahead of the view, and the view refuses it when
//! the row counts disagree.

use crate::reactive::{ListModel, RowChange, Subscription};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_ui_lang::layout::{VirtualList, VisibleWindow};
use metis_ui_lang::{DomElement, DomNode, Size};
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(test)]
mod tests;

/// Row extents mirrored from a model's change notifications.
#[derive(Debug, Default)]
struct Mirror {
    /// One entry per row; `None` marks a row whose extent must be measured.
    extents: Vec<Option<u32>>,
    /// Set when a reset arrives or a change cannot be followed by index; the
    /// next query rereads the row count and every row.
    reload: bool,
    /// Set by any change and cleared when the list is rebuilt.
    stale: bool,
    /// Counts every change, so a measurement that overlaps one is refused.
    revision: u64,
}

impl Mirror {
    fn apply(&mut self, change: RowChange) {
        self.stale = true;
        self.revision = self.revision.wrapping_add(1);
        if matches!(change, RowChange::Reset) {
            self.extents.clear();
            self.reload = true;
            return;
        }
        if self.reload {
            return;
        }
        let len = self.extents.len();
        match change {
            RowChange::Changed(row) if row < len => self.extents[row] = None,
            RowChange::Added { index, count } if index <= len => {
                self.extents
                    .splice(index..index, std::iter::repeat_n(None, count));
            }
            RowChange::Removed { index, count }
                if index.checked_add(count).is_some_and(|end| end <= len) =>
            {
                self.extents.drain(index..index + count);
            }
            _ => self.reload = true,
        }
    }
}

/// Rows of a [`ListModel`] projected as the window one scroll position shows.
pub struct VirtualListView<T, F> {
    model: ListModel<T>,
    measure: F,
    mirror: Rc<RefCell<Mirror>>,
    list: Option<VirtualList>,
    overscan: usize,
    #[expect(
        dead_code,
        reason = "the subscription guard unsubscribes from the model on drop"
    )]
    subscription: Subscription,
}

impl<T: Clone + PartialEq + 'static, F: Fn(&T) -> u32> VirtualListView<T, F> {
    /// Subscribes to `model`, measuring each row with `measure`.
    ///
    /// # Errors
    /// Returns [`ErrorCode::QueueFull`] when the model has no free subscriber
    /// slot, since the view would never hear its changes.
    pub fn new(model: ListModel<T>, measure: F) -> Result<Self> {
        let mirror = Rc::new(RefCell::new(Mirror::default()));
        let listener = Rc::clone(&mirror);
        let subscription = model.subscribe(move |change| listener.borrow_mut().apply(*change));
        if !subscription.is_active() {
            return Err(MetisError::ui(
                ErrorCode::QueueFull,
                "the row model has no free subscriber slot",
            ));
        }
        Ok(Self {
            model,
            measure,
            mirror,
            list: None,
            overscan: 0,
            subscription,
        })
    }

    /// Builds `items` extra rows on each side of the window, so a small scroll
    /// shows content before the host builds the next window.
    #[must_use]
    pub fn with_overscan(mut self, items: usize) -> Self {
        self.overscan = items;
        self.list = None;
        self
    }

    /// The rows a scroll position builds, with the scroll offset clamped.
    ///
    /// # Errors
    /// Returns the refusals of [`Self::total_extent`].
    pub fn window(&mut self, offset: u64, viewport: u32) -> Result<VisibleWindow> {
        self.refresh()?;
        Ok(self.list_ref()?.window(offset, viewport))
    }

    /// The content extent of every row, in logical pixels.
    ///
    /// # Errors
    /// Returns [`ErrorCode::RenderFailure`] when the view is out of step with
    /// its model, and [`ErrorCode::LayoutOverflow`] when the rows exceed the
    /// measurement bounds of [`VirtualList`].
    pub fn total_extent(&mut self) -> Result<u64> {
        self.refresh()?;
        Ok(self.list_ref()?.total_extent())
    }

    /// Projects the window as DOM: a top spacer, one wrapper per built row
    /// pinned to that row's extent, and a bottom spacer, so the children sum to
    /// the total extent. `row` builds the content of each built row.
    ///
    /// # Errors
    /// Returns [`ErrorCode::LayoutOverflow`] when a spacer or row exceeds the
    /// layout's pixel range, and [`ErrorCode::RenderFailure`] when the view is
    /// out of step with its model.
    pub fn dom(
        &mut self,
        offset: u64,
        viewport: u32,
        mut row: impl FnMut(usize, &T) -> DomElement,
    ) -> Result<DomElement> {
        self.refresh()?;
        let list = self.list_ref()?;
        let window = list.window(offset, viewport);
        let total = list.total_extent();
        let bottom = total.checked_sub(window.end).ok_or_else(out_of_step)?;
        let mut root = DomElement::new("div");
        root.children.push(spacer(window.start)?);
        for index in window.items.clone() {
            let span = list.item_span(index).ok_or_else(out_of_step)?;
            let data = self.model.row_data(index).ok_or_else(out_of_step)?;
            let mut wrapper = DomElement::new("div");
            wrapper.computed_style.height = pixels(span.end - span.start)?;
            wrapper
                .children
                .push(DomNode::Element(Box::new(row(index, &data))));
            root.children.push(DomNode::Element(Box::new(wrapper)));
        }
        root.children.push(spacer(bottom)?);
        Ok(root)
    }

    /// Rebuilds the list when a change or a reset invalidated it.
    fn refresh(&mut self) -> Result<()> {
        if self.list.is_some() && !self.mirror.borrow().stale {
            return Ok(());
        }
        let (unknown, revision) = {
            let mut mirror = self.mirror.borrow_mut();
            if mirror.reload {
                mirror.extents = vec![None; self.model.row_count()];
                mirror.reload = false;
            }
            let unknown: Vec<usize> = mirror
                .extents
                .iter()
                .enumerate()
                .filter_map(|(row, extent)| extent.is_none().then_some(row))
                .collect();
            (unknown, mirror.revision)
        };
        let mut measured = Vec::with_capacity(unknown.len());
        for row in unknown {
            let data = self.model.row_data(row).ok_or_else(out_of_step)?;
            measured.push((row, (self.measure)(&data)));
        }
        let mut mirror = self.mirror.borrow_mut();
        if mirror.revision != revision {
            return Err(MetisError::ui(
                ErrorCode::RenderFailure,
                "the row model changed while its rows were measured",
            ));
        }
        for (row, extent) in measured {
            let slot = mirror.extents.get_mut(row).ok_or_else(out_of_step)?;
            *slot = Some(extent);
        }
        let extents = mirror
            .extents
            .iter()
            .copied()
            .collect::<Option<Vec<u32>>>()
            .ok_or_else(out_of_step)?;
        if extents.len() != self.model.row_count() {
            return Err(out_of_step());
        }
        self.list = Some(VirtualList::variable(&extents)?.with_overscan(self.overscan));
        mirror.stale = false;
        Ok(())
    }

    fn list_ref(&self) -> Result<&VirtualList> {
        self.list.as_ref().ok_or_else(out_of_step)
    }
}

fn out_of_step() -> MetisError {
    MetisError::ui(
        ErrorCode::RenderFailure,
        "list view is out of step with its row model",
    )
}

fn pixels(extent: u64) -> Result<Size> {
    i32::try_from(extent).map(Size::Px).map_err(|_| {
        MetisError::ui(
            ErrorCode::LayoutOverflow,
            "list content exceeds the layout pixel range",
        )
    })
}

fn spacer(extent: u64) -> Result<DomNode> {
    let mut element = DomElement::new("div");
    element.computed_style.height = pixels(extent)?;
    Ok(DomNode::Element(Box::new(element)))
}
