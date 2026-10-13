//! Computed ownership observations at a page stop acknowledgement.

/// Live application resources, distinct from the coordinator and settlement guards.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceCounts {
    /// Registered application DOM listeners.
    pub dom_listeners: u32,
    /// Application futures whose child ownership is still live.
    pub application_tasks: u32,
    /// Retained coordinator bridge listeners.
    pub bridge_listeners: u32,
    /// Failed bridge cleanup guards retained until realm termination.
    pub retained_bridge_callbacks: u32,
    /// PAL capability requests still owned by a live request future.
    pub active_requests: u32,
    /// Real registered browser geolocation watches.
    pub geolocation_watches: u32,
    /// Realm-scoped dispatchers that keep queued geolocation callbacks valid.
    pub geolocation_dispatchers: u32,
    /// Real owned media stream leases.
    pub media_leases: u32,
    /// Actual owned media tracks not yet stopped.
    pub media_tracks: u32,
    /// Native settlement owners; at most one per each of the four request kinds.
    pub unsettled_guards: u32,
}

impl ResourceCounts {
    /// Tests the precise acknowledgement law without hiding settlement ownership.
    #[must_use]
    pub const fn application_released(self) -> bool {
        self.dom_listeners == 0
            && self.application_tasks == 0
            && self.bridge_listeners == 1
            && self.retained_bridge_callbacks == 0
            && self.active_requests == 0
            && self.geolocation_watches == 0
            && matches!(self.geolocation_dispatchers, 0 | 2)
            && self.media_leases == 0
            && self.media_tracks == 0
            && self.unsettled_guards <= 4
    }
}
