//! Private numeric resource observations; all counts fit exact JSON integers.

use super::super::ResourceCounts;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireResources {
    dom_listeners: u32,
    application_tasks: u32,
    bridge_listeners: u32,
    retained_bridge_callbacks: u32,
    active_requests: u32,
    geolocation_watches: u32,
    geolocation_dispatchers: u32,
    media_leases: u32,
    media_tracks: u32,
    unsettled_guards: u32,
}

impl From<ResourceCounts> for WireResources {
    fn from(value: ResourceCounts) -> Self {
        Self {
            dom_listeners: value.dom_listeners,
            application_tasks: value.application_tasks,
            bridge_listeners: value.bridge_listeners,
            retained_bridge_callbacks: value.retained_bridge_callbacks,
            active_requests: value.active_requests,
            geolocation_watches: value.geolocation_watches,
            geolocation_dispatchers: value.geolocation_dispatchers,
            media_leases: value.media_leases,
            media_tracks: value.media_tracks,
            unsettled_guards: value.unsettled_guards,
        }
    }
}

impl From<WireResources> for ResourceCounts {
    fn from(value: WireResources) -> Self {
        Self {
            dom_listeners: value.dom_listeners,
            application_tasks: value.application_tasks,
            bridge_listeners: value.bridge_listeners,
            retained_bridge_callbacks: value.retained_bridge_callbacks,
            active_requests: value.active_requests,
            geolocation_watches: value.geolocation_watches,
            geolocation_dispatchers: value.geolocation_dispatchers,
            media_leases: value.media_leases,
            media_tracks: value.media_tracks,
            unsettled_guards: value.unsettled_guards,
        }
    }
}
