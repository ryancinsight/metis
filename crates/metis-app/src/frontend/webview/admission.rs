//! Native origin/evidence admission around the shared page lifecycle owner.
//!
//! Native denials and browser observations remain independent. Capture admits
//! only the exact ready mount, four real native kinds and reconciled outcomes.

use metis_core::host::HostOrigin;
use metis_web::embedded::{
    CapabilityObservation, PageCapability, PageCommand, PageIdentity, PageRole, PageSession,
    PageStage, ResourceCounts,
};
use std::io;

pub(super) struct NativePage {
    pub(super) session: PageSession,
    source: String,
    permission_origin: String,
    denials: [Option<bool>; 4],
    observations: Option<[CapabilityObservation; 4]>,
}

impl NativePage {
    pub(super) fn new(identity: PageIdentity, source: String) -> io::Result<Self> {
        let origin = HostOrigin::from_url(&source)
            .map_err(|error| invalid(format!("Packaged document origin is invalid: {error}")))?;
        let permission_origin = format!("{origin}/");
        if !permission_origin.starts_with("https://") || !source.starts_with(&permission_origin) {
            return Err(invalid(
                "Packaged document is not a canonical mapped HTTPS URL",
            ));
        }
        Ok(Self {
            session: PageSession::new(identity),
            source,
            permission_origin,
            denials: [None; 4],
            observations: None,
        })
    }

    // PermissionRequestedEventArgs.Uri reports the requesting origin. Unlike
    // WebMessageReceived.Source it does not identify the document path.
    // The actual SDK origin spelling has a trailing slash; no other path,
    // query, fragment, host, scheme or port is admitted by this equality.
    pub(super) fn permission_source(&self, actual: &str) -> io::Result<()> {
        if actual != self.permission_origin {
            return Err(invalid(
                "Native permission source differs from the admitted origin",
            ));
        }
        Ok(())
    }

    pub(super) fn source(&self, actual: &str) -> io::Result<()> {
        if actual != self.source {
            return Err(invalid(
                "Page event source differs from the admitted packaged document",
            ));
        }
        Ok(())
    }

    pub(super) fn native_denial(
        &mut self,
        source: &str,
        kind: PageCapability,
        initiated: bool,
    ) -> io::Result<()> {
        self.permission_source(source)?;
        if self.session.identity().role() != PageRole::PermissionProbe
            || !matches!(self.session.stage(), PageStage::Loading | PageStage::Ready)
        {
            return Err(invalid(
                "Native permission observation is outside the current probe",
            ));
        }
        let slot = &mut self.denials[kind.index()];
        if slot.is_some() {
            return Err(invalid("Native probe permission kind is duplicated"));
        }
        *slot = Some(initiated);
        Ok(())
    }

    pub(super) fn denial_commands(&self) -> impl Iterator<Item = PageCommand> + '_ {
        PageCapability::ALL.into_iter().filter_map(|kind| {
            self.denials[kind.index()].map(|user_initiated| PageCommand::PermissionDenied {
                identity: self.session.identity(),
                kind,
                user_initiated,
            })
        })
    }

    pub(super) fn probe(
        &mut self,
        identity: PageIdentity,
        observations: [CapabilityObservation; 4],
        resources: ResourceCounts,
    ) -> io::Result<&[CapabilityObservation; 4]> {
        if self.session.identity() != identity
            || self.session.stage() != PageStage::Ready
            || identity.role() != PageRole::PermissionProbe
        {
            return Err(invalid(
                "Probe completion does not address the current ready probe",
            ));
        }
        if resources.application_tasks != 0
            || resources.active_requests != 0
            || resources.geolocation_watches != 0
            || resources.media_leases != 0
            || resources.media_tracks != 0
            || resources.unsettled_guards != 0
            || !matches!(resources.geolocation_dispatchers, 0 | 2)
            || resources.bridge_listeners != 1
            || resources.retained_bridge_callbacks != 0
        {
            return Err(invalid(
                "Completed probe still owns requests, tracks or failed cleanup",
            ));
        }
        for (observation, kind) in observations.iter().zip(PageCapability::ALL) {
            observation.validate()?;
            if observation.kind != kind {
                return Err(invalid(
                    "Probe outcomes differ from the declared capability sequence",
                ));
            }
            if !observation.permits_denial_reconciliation() {
                return Err(invalid(format!(
                    "Required native probe {} cannot reconcile browser outcome: {:?}",
                    kind.as_str(),
                    observation.browser
                )));
            }
            if observation.host_denial.is_some()
                && observation.host_denial != self.denials[kind.index()]
            {
                return Err(invalid(
                    "Page claims an unobserved or mismatched native denial",
                ));
            }
        }
        if let Some(previous) = &self.observations {
            if previous == &observations {
                return Err(invalid("Probe completion is duplicated"));
            }
            for (old, new) in previous.iter().zip(&observations) {
                if old.kind != new.kind
                    || old.browser != new.browser
                    || (old.host_denial.is_some() && old.host_denial != new.host_denial)
                {
                    return Err(invalid(
                        "Reconciled probe changes a settled browser outcome or native denial",
                    ));
                }
            }
        }
        Ok(self.observations.insert(observations))
    }

    pub(super) fn probe_capture_ready(&self) -> bool {
        self.session.stage() == PageStage::Ready
            && self.observations.as_ref().is_some_and(|observations| {
                observations.iter().all(|observation| {
                    observation.confirms_host_denial()
                        && observation.host_denial == self.denials[observation.kind.index()]
                })
            })
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
#[path = "admission/tests.rs"]
mod tests;
