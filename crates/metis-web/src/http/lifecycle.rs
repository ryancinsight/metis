//! Publication requires equality of mount and request epochs.
use crate::epoch::{Epoch, Generation};
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Lease {
    mount: Generation,
    request: Generation,
}
impl Lease {
    pub(super) const fn mount(self) -> u64 {
        self.mount.value()
    }
}

pub(super) struct Lifecycle {
    mount: Epoch,
    request: Epoch,
    active: Option<Lease>,
}
impl Lifecycle {
    pub(super) const fn new() -> Self {
        Self {
            mount: Epoch::new(),
            request: Epoch::new(),
            active: None,
        }
    }
    #[cfg(test)]
    pub(super) fn at(mount: u64, request: u64) -> Self {
        Self {
            mount: Epoch::at(mount),
            request: Epoch::at(request),
            active: None,
        }
    }
    pub(super) const fn mount(&self) -> u64 {
        self.mount.current().value()
    }
    pub(super) fn begin(&mut self) -> io::Result<Lease> {
        self.active = None;
        let lease = Lease {
            mount: self.mount.current(),
            request: self.request.advance()?,
        };
        self.active = Some(lease);
        Ok(lease)
    }
    pub(super) fn reset(&mut self) -> io::Result<u64> {
        self.active = None;
        self.mount.advance().map(Generation::value)
    }
    pub(super) fn accepts(&self, lease: Lease) -> bool {
        self.active == Some(lease)
            && self.mount.accepts(lease.mount)
            && self.request.accepts(lease.request)
    }
    pub(super) fn finish(&mut self, lease: Lease) {
        if self.accepts(lease) {
            self.active = None;
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn stop(&mut self) {
        self.active = None;
    }
}
