//! Public page-domain messages; the serialized shapes remain private.

use super::{CapabilityObservation, PageCapability, PageIdentity, ResourceCounts, SequenceId};
use metis_core::protocol::ClinicalCalcResponsePayload;
use metis_frontend::FormInputs;

/// A page observation delivered to its native owner.
#[derive(Clone, Debug, PartialEq)]
pub enum PageEvent {
    /// All required elements, listeners, theme and real bridge are installed.
    Ready {
        /// Exact admitted mount.
        identity: PageIdentity,
    },
    /// The page requests native close after applying its local Escape policy.
    CloseRequested {
        /// Current ready mount.
        identity: PageIdentity,
    },
    /// Application resources are released; the coordinator reports retained guards.
    Stopped {
        /// Exact stopped mount.
        identity: PageIdentity,
        /// Computed resource observations.
        resources: ResourceCounts,
    },
    /// One captured form submission, still subject to native/backend admission.
    Submit {
        /// Current ready mount.
        identity: PageIdentity,
        /// Monotonic request within that mount.
        request: SequenceId,
        /// Captured inputs, including exact Unicode.
        inputs: FormInputs,
    },
    /// All four actual browser requests settle.
    ProbeCompleted {
        /// Current ready mount.
        identity: PageIdentity,
        /// Exact probe order and actual outcomes. One owned fixed array keeps
        /// every event envelope from reserving four error payloads inline.
        observations: Box<[CapabilityObservation; 4]>,
        /// Computed current owners.
        resources: ResourceCounts,
    },
    /// An explicit page/lifecycle failure.
    Failed {
        /// The mount whose operation fails.
        identity: PageIdentity,
        /// Context preserved from the failing operation.
        message: String,
    },
}

// A non-probe event must not reserve the full four-outcome payload inline.
const _: () = assert!(size_of::<PageEvent>() < size_of::<[CapabilityObservation; 4]>());

/// A native command addressed to one exact packaged page mount.
#[derive(Clone, Debug, PartialEq)]
pub enum PageCommand {
    /// Mount or remount the packaged role with a new identity.
    Mount {
        /// Identity of the admitted package and mount.
        identity: PageIdentity,
    },
    /// Invalidate a mount before releasing its application owners.
    Stop {
        /// The currently ready mount.
        identity: PageIdentity,
    },
    /// An actual supervised backend response.
    Result {
        /// Current mount.
        identity: PageIdentity,
        /// The single outstanding request.
        request: SequenceId,
        /// Exact backend computed values and audit identifier.
        response: ClinicalCalcResponsePayload,
    },
    /// A contextual native/backend error.
    Error {
        /// Current mount.
        identity: PageIdentity,
        /// Correlated submission, or a mount-level error.
        request: Option<SequenceId>,
        /// Stable native status.
        status: String,
        /// Original wire error discriminant.
        error_code: u16,
        /// Original error context.
        message: String,
    },
    /// A real native permission callback observes a denial.
    PermissionDenied {
        /// Current mount.
        identity: PageIdentity,
        /// Actual native permission kind.
        kind: PageCapability,
        /// Actual native user-initiated observation.
        user_initiated: bool,
    },
}

impl PageEvent {
    /// Returns the exact mount addressed by this event.
    #[must_use]
    pub const fn identity(&self) -> PageIdentity {
        match self {
            Self::Ready { identity }
            | Self::CloseRequested { identity }
            | Self::Stopped { identity, .. }
            | Self::Submit { identity, .. }
            | Self::ProbeCompleted { identity, .. }
            | Self::Failed { identity, .. } => *identity,
        }
    }
}

impl PageCommand {
    /// Returns the exact mount addressed by this command.
    #[must_use]
    pub const fn identity(&self) -> PageIdentity {
        match self {
            Self::Mount { identity }
            | Self::Stop { identity }
            | Self::Result { identity, .. }
            | Self::Error { identity, .. }
            | Self::PermissionDenied { identity, .. } => *identity,
        }
    }
}
