//! Shared identity, readiness, request and stop state transitions.

use super::identity::invalid;
use super::{PageIdentity, PageRole, ResourceCounts, SequenceId};
use std::io;

/// An observed page/controller lifecycle stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageStage {
    /// The module has not acknowledged readiness.
    Loading,
    /// The exact admitted page is ready.
    Ready,
    /// Stop is requested; application owners are being released.
    Stopping,
    /// Stop acknowledgement is valid; controller destruction is not yet observed.
    Stopped,
    /// Actual native destruction is observed.
    Closed,
}

/// One authoritative state machine used at both ends of the page bridge.
pub struct PageSession {
    identity: PageIdentity,
    stage: PageStage,
    last_request: Option<SequenceId>,
    pending: Option<SequenceId>,
}

impl PageSession {
    /// Begins a mount whose real readiness is still unobserved.
    #[must_use]
    pub const fn new(identity: PageIdentity) -> Self {
        Self {
            identity,
            stage: PageStage::Loading,
            last_request: None,
            pending: None,
        }
    }
    /// Returns the current observed stage.
    #[must_use]
    pub const fn stage(&self) -> PageStage {
        self.stage
    }
    /// Returns the admitted identity.
    #[must_use]
    pub const fn identity(&self) -> PageIdentity {
        self.identity
    }
    /// Returns the single outstanding request, if one exists.
    #[must_use]
    pub const fn pending(&self) -> Option<SequenceId> {
        self.pending
    }

    /// Accepts the exact mount's first readiness acknowledgement.
    /// # Errors
    /// Rejects stale, mismatched or duplicate readiness.
    pub fn ready(&mut self, identity: PageIdentity) -> io::Result<()> {
        self.require(identity, PageStage::Loading)?;
        self.stage = PageStage::Ready;
        Ok(())
    }
    /// Admits a monotonic request from the current ready form.
    /// # Errors
    /// Rejects another outstanding request, duplicate sequence or non-form role.
    pub fn begin_request(&mut self, identity: PageIdentity, request: SequenceId) -> io::Result<()> {
        self.require(identity, PageStage::Ready)?;
        if identity.role() != PageRole::Form
            || self.pending.is_some()
            || self.last_request.is_some_and(|last| request <= last)
        {
            return Err(invalid(
                "Page request is pending, stale or outside the form role",
            ));
        }
        self.last_request = Some(request);
        self.pending = Some(request);
        Ok(())
    }
    /// Releases only the exact outstanding request after a terminal reply.
    /// # Errors
    /// Rejects mismatched, stale or unsolicited terminal replies.
    pub fn complete_request(
        &mut self,
        identity: PageIdentity,
        request: SequenceId,
    ) -> io::Result<()> {
        self.require(identity, PageStage::Ready)?;
        if self.pending != Some(request) {
            return Err(invalid("Page reply has no matching pending request"));
        }
        self.pending = None;
        Ok(())
    }
    /// Invalidates work before application cleanup begins.
    /// # Errors
    /// Rejects an unready or mismatched stop.
    pub fn begin_stop(&mut self, identity: PageIdentity) -> io::Result<()> {
        self.require(identity, PageStage::Ready)?;
        self.stage = PageStage::Stopping;
        self.pending = None;
        Ok(())
    }
    /// Accepts computed cleanup observations without claiming controller destruction.
    /// # Errors
    /// Rejects live application owners, failed cleanup guards or a stale acknowledgement.
    pub fn stopped(&mut self, identity: PageIdentity, resources: ResourceCounts) -> io::Result<()> {
        self.require(identity, PageStage::Stopping)?;
        if !resources.application_released() {
            return Err(invalid(
                "Page stop retains application resources or failed cleanup",
            ));
        }
        self.stage = PageStage::Stopped;
        Ok(())
    }
    /// Records actual native destruction after the valid stop acknowledgement.
    /// # Errors
    /// Rejects destruction presented as a successful handshake before stop completes.
    pub fn observe_destroyed(&mut self) -> io::Result<()> {
        if self.stage != PageStage::Stopped {
            return Err(invalid(
                "Native destruction precedes the page stop acknowledgement",
            ));
        }
        self.stage = PageStage::Closed;
        Ok(())
    }
    /// Starts a strictly newer mount after the previous page releases its owners.
    /// # Errors
    /// Rejects changed package/role, reused mount or unfinished teardown.
    pub fn remount(&mut self, identity: PageIdentity) -> io::Result<()> {
        if !matches!(self.stage, PageStage::Stopped | PageStage::Closed)
            || identity.mount() <= self.identity.mount()
            || identity.role() != self.identity.role()
            || identity.module_digest() != self.identity.module_digest()
        {
            return Err(invalid(
                "Page remount changes authority, reuses identity or precedes teardown",
            ));
        }
        *self = Self::new(identity);
        Ok(())
    }
    fn require(&self, identity: PageIdentity, stage: PageStage) -> io::Result<()> {
        if identity != self.identity || self.stage != stage {
            return Err(invalid("Page identity or lifecycle stage does not match"));
        }
        Ok(())
    }
}
