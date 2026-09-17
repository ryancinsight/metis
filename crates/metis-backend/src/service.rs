//! Session authorization and audited dispatch for one connected frontend.
//!
//! A service belongs to one supervisor-created private transport. A supplied
//! principal is a session label, not OS identity proof. The supervisor owns peer
//! authentication through private pipe transfer. The launcher supplies a fresh
//! OS-generated symmetric session MAC key which never crosses that transport and
//! is unrelated to registry or signing credentials.

use crate::audit::AuditLedger;
#[cfg(not(target_arch = "wasm32"))]
use crate::audit::FileAuditStore;
use crate::clinical::SafetyEnvelope;
use crate::plugins::{PluginExecutor, PluginRouter};
use metis_core::capability::CapabilityToken;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::host::{HostContext, HostPolicy};
use metis_core::protocol::{Plugin, RemoteEventPayload, TargetCapability, TargetCapabilityPayload};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Session lifetime policy: one hour; changes require expiry-policy review.
pub const SESSION_LIFETIME: Duration = Duration::from_hours(1);

/// Paired wall and monotonic observations from one clock boundary.
#[derive(Debug, Clone, Copy)]
pub struct ClockReading {
    /// UTC duration since the Unix epoch, used for audit and token claims.
    pub unix_time: Duration,
    /// Elapsed duration from a fixed origin, used for session expiry.
    pub monotonic: Duration,
}

/// Clock boundary with a fixed monotonic origin for each service.
pub trait Clock {
    /// Samples UTC and monotonic time.
    ///
    /// # Errors
    /// Reports unavailable or unrepresentable clock observations.
    fn now(&self) -> Result<ClockReading>;
}

/// Production clock using `SystemTime` for UTC and `Instant` for elapsed time.
pub struct SystemClock {
    origin: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Result<ClockReading> {
        let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
            MetisError::capability(
                ErrorCode::CapabilityExpired,
                "System clock precedes Unix epoch",
            )
        })?;
        Ok(ClockReading {
            unix_time,
            monotonic: self.origin.elapsed(),
        })
    }
}

struct Session {
    token: CapabilityToken,
    context: HostContext,
    started: Duration,
}

/// Backend state belonging exclusively to one private transport session.
pub struct BackendService<C = SystemClock> {
    master_key: [u8; 32],
    envelope: SafetyEnvelope,
    host_policy: HostPolicy,
    trusted_context: Option<HostContext>,
    ledger: AuditLedger,
    #[cfg(not(target_arch = "wasm32"))]
    audit_store: Option<FileAuditStore>,
    clock: C,
    session: Option<Session>,
    last_sequence: u64,
    last_reading: Option<ClockReading>,
    clock_failed: bool,
    pending_event: Option<RemoteEventPayload>,
    plugins: PluginRouter,
    target_capabilities: TargetCapabilityPayload,
}

impl BackendService {
    /// Creates a service with real UTC and monotonic clocks.
    ///
    /// The launcher must supply an unpredictable key unique to this session.
    /// Example envelope limits do not constitute clinical validation.
    #[must_use]
    pub fn new(master_key: [u8; 32], envelope: SafetyEnvelope) -> Self {
        Self::with_clock_and_policy(
            master_key,
            envelope,
            SystemClock::default(),
            HostPolicy::native(),
        )
    }
}

impl<C> BackendService<C> {
    /// Selects a clock implementation for deterministic temporal verification.
    #[must_use]
    pub fn with_clock(master_key: [u8; 32], envelope: SafetyEnvelope, clock: C) -> Self {
        Self::with_clock_and_policy(master_key, envelope, clock, HostPolicy::native())
    }

    /// Selects a clock and exact host policy for deterministic verification.
    ///
    /// # Panics
    /// Panics only if the compile-time default plugin-router capacity violates
    /// its invariant; the default capacity is fixed below the host bound.
    #[must_use]
    pub fn with_clock_and_policy(
        master_key: [u8; 32],
        envelope: SafetyEnvelope,
        clock: C,
        host_policy: HostPolicy,
    ) -> Self {
        Self {
            master_key,
            envelope,
            host_policy,
            trusted_context: None,
            ledger: AuditLedger::new(),
            #[cfg(not(target_arch = "wasm32"))]
            audit_store: None,
            clock,
            session: None,
            last_sequence: 0,
            last_reading: None,
            clock_failed: false,
            pending_event: None,
            plugins: PluginRouter::new()
                .expect("invariant: the default plugin router capacity is valid"),
            target_capabilities: TargetCapabilityPayload::native_service(),
        }
    }

    /// Selects a clock, exact host policy, and already-observed session context.
    ///
    /// A native or service acceptor uses this constructor after validating the
    /// browser `Origin` header and generating the session principal. The
    /// handshake may repeat that principal as a routing label, but it cannot
    /// replace the trusted context with values supplied by the browser.
    ///
    /// # Errors
    /// Returns [`ErrorCode::NavigationDenied`] or [`ErrorCode::InvalidWindow`]
    /// when `context` does not satisfy `host_policy`.
    pub fn with_trusted_context(
        master_key: [u8; 32],
        envelope: SafetyEnvelope,
        clock: C,
        host_policy: HostPolicy,
        context: HostContext,
    ) -> Result<Self> {
        host_policy.check_context(&context)?;
        Ok(Self {
            master_key,
            envelope,
            host_policy,
            trusted_context: Some(context),
            ledger: AuditLedger::new(),
            #[cfg(not(target_arch = "wasm32"))]
            audit_store: None,
            clock,
            session: None,
            last_sequence: 0,
            last_reading: None,
            clock_failed: false,
            pending_event: None,
            plugins: PluginRouter::new()?,
            target_capabilities: TargetCapabilityPayload::native_service(),
        })
    }

    /// Restores a service from authenticated native audit snapshots.
    ///
    /// The checkpoint key is supplied by the host and remains outside the
    /// persisted records. A malformed or unauthenticated snapshot prevents
    /// service construction.
    ///
    /// # Errors
    /// Returns an audit recovery error when the store cannot be loaded.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_persistent_audit(
        master_key: [u8; 32],
        envelope: SafetyEnvelope,
        clock: C,
        store: FileAuditStore,
    ) -> Result<Self> {
        Self::with_persistent_audit_and_policy(
            master_key,
            envelope,
            clock,
            HostPolicy::native(),
            store,
        )
    }

    /// Restores a service from authenticated snapshots under an exact policy.
    ///
    /// # Errors
    /// Returns a host-policy or audit-recovery error.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_persistent_audit_and_policy(
        master_key: [u8; 32],
        envelope: SafetyEnvelope,
        clock: C,
        host_policy: HostPolicy,
        mut store: FileAuditStore,
    ) -> Result<Self> {
        let ledger = store.load()?;
        let mut service = Self::with_clock_and_policy(master_key, envelope, clock, host_policy);
        service.ledger = ledger;
        service.audit_store = Some(store);
        Ok(service)
    }

    /// Restores a service with a trusted context and authenticated snapshots.
    ///
    /// # Errors
    /// Returns a host-policy or audit-recovery error.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_trusted_context_and_persistent_audit(
        master_key: [u8; 32],
        envelope: SafetyEnvelope,
        clock: C,
        host_policy: HostPolicy,
        context: HostContext,
        mut store: FileAuditStore,
    ) -> Result<Self> {
        host_policy.check_context(&context)?;
        let ledger = store.load()?;
        let mut service = Self::with_clock_and_policy(master_key, envelope, clock, host_policy);
        service.trusted_context = Some(context);
        service.ledger = ledger;
        service.audit_store = Some(store);
        Ok(service)
    }

    /// Retained request outcomes, including rejected requests.
    #[must_use]
    pub const fn ledger(&self) -> &AuditLedger {
        &self.ledger
    }

    /// Registers a permission-scoped plugin executor for this host.
    ///
    /// Registration validates the plugin's static descriptor and keeps the
    /// executor behind the bounded extension router. It does not grant the
    /// plugin operating-system authority; each invocation still verifies the
    /// declared capability scope against the trusted session token.
    ///
    /// # Errors
    /// Returns a typed descriptor, duplicate, capacity, or allocation error.
    pub fn register_plugin<P>(&mut self, plugin: P) -> Result<()>
    where
        P: Plugin + PluginExecutor + 'static,
    {
        self.plugins.register(plugin)
    }

    pub(crate) fn browser_policy(&self) -> &HostPolicy {
        &self.host_policy
    }

    pub(crate) const fn has_trusted_context(&self) -> bool {
        self.trusted_context.is_some()
    }

    /// Returns the target surfaces exposed by this host boundary.
    #[must_use]
    pub const fn target_capabilities(&self) -> &TargetCapabilityPayload {
        &self.target_capabilities
    }

    /// Adds one implemented target surface to the host descriptor.
    ///
    /// Hosts call this before accepting a session so discovery reflects the
    /// transport and runtime surfaces that are actually installed.
    ///
    /// # Errors
    /// Returns [`ErrorCode::PayloadTooLarge`] when the descriptor is full.
    pub fn add_target_capability(&mut self, capability: TargetCapability) -> Result<()> {
        self.target_capabilities.add_capability(capability)
    }
}

#[path = "service/dispatch.rs"]
mod dispatch;

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;
