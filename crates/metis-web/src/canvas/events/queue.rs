//! Bounded browser canvas event queue.

#[cfg(any(target_arch = "wasm32", test))]
use super::{CANVAS_EVENT_CAPACITY, CanvasEvent, CanvasEventError};
#[cfg(any(target_arch = "wasm32", test))]
use std::collections::VecDeque;

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) struct CanvasEventQueue {
    events: VecDeque<CanvasEvent>,
    error: Option<CanvasEventError>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl CanvasEventQueue {
    pub(crate) fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(CANVAS_EVENT_CAPACITY),
            error: None,
        }
    }

    pub(crate) fn push(&mut self, event: CanvasEvent) -> bool {
        if self.error.is_some() {
            return false;
        }
        if self.events.len() == CANVAS_EVENT_CAPACITY {
            self.events.clear();
            self.error = Some(CanvasEventError::QueueOverflow);
            return false;
        }
        self.events.push_back(event);
        true
    }

    pub(crate) fn fail(&mut self, error: CanvasEventError) {
        self.events.clear();
        self.error.get_or_insert(error);
    }

    pub(crate) fn take(&mut self) -> Result<Box<[CanvasEvent]>, CanvasEventError> {
        let events = self.events.drain(..).collect::<Vec<_>>().into_boxed_slice();
        match self.error.take() {
            Some(error) => {
                drop(events);
                Err(error)
            }
            None => Ok(events),
        }
    }
}
