//! Counts real child-future ownership, independently of cancellation handles.

use super::coordinator::Coordinator;
use crate::embedded::ResourceCounts;
use moirai_pal::wasm::{WebEventListener, WebViewBridge};
use std::{cell::Cell, io, rc::Rc};

pub(super) struct TaskOwner(Rc<Cell<u32>>);

impl TaskOwner {
    pub(super) fn acquire(count: &Rc<Cell<u32>>) -> io::Result<Self> {
        let next = count
            .get()
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Application task count exceeds its domain"))?;
        count.set(next);
        Ok(Self(Rc::clone(count)))
    }
}

impl Drop for TaskOwner {
    fn drop(&mut self) {
        // Construction increments once and ownership is neither copied nor aliased.
        self.0
            .set(self.0.get().checked_sub(1).expect("owned task decrement"));
    }
}

pub(super) fn snapshot(page: &Coordinator) -> io::Result<ResourceCounts> {
    let native = page.permissions.resource_snapshot();
    let listeners = WebEventListener::active_count();
    Ok(ResourceCounts {
        dom_listeners: count(
            listeners
                .checked_add(WebEventListener::retained_cleanup_count())
                .ok_or_else(|| io::Error::other("DOM listener count overflow"))?,
        )?,
        application_tasks: page.tasks.get(),
        bridge_listeners: u32::from(page.listener.borrow().is_some()),
        retained_bridge_callbacks: count(WebViewBridge::retained_cleanup_count())?,
        active_requests: count(native.active_requests)?,
        geolocation_watches: count(native.geolocation_watches)?,
        geolocation_dispatchers: count(native.geolocation_dispatchers)?,
        media_leases: count(native.media_leases)?,
        media_tracks: count(native.owned_media_tracks)?,
        unsettled_guards: count(native.unsettled_guards)?,
    })
}

fn count(value: usize) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| io::Error::other("Resource observation exceeds its domain"))
}
