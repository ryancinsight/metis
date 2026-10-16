//! Browser outcomes and separately observed native denials.

/// One of the four actual browser request domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PageCapability {
    /// Geolocation observation.
    Geolocation,
    /// Video capture.
    Camera,
    /// Audio capture.
    Microphone,
    /// Notification permission.
    Notifications,
}

impl PageCapability {
    /// The exact sequential probe order.
    pub const ALL: [Self; 4] = [
        Self::Geolocation,
        Self::Camera,
        Self::Microphone,
        Self::Notifications,
    ];
    /// Returns the stable native/probe spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Geolocation => "geolocation",
            Self::Camera => "camera",
            Self::Microphone => "microphone",
            Self::Notifications => "notifications",
        }
    }
    /// Returns the ordinal in the closed probe sequence.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Geolocation => 0,
            Self::Camera => 1,
            Self::Microphone => 2,
            Self::Notifications => 3,
        }
    }
    /// Resolves a browser/native capability label.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// The real browser result, independent of host event delivery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserPermissionResult {
    /// The browser grants the requested capability; the owned lease is released.
    Granted,
    /// A browser request rejects with its native context.
    Rejected {
        /// Browser error name when available.
        name: Option<String>,
        /// Native error code when available.
        code: Option<u32>,
        /// Actual browser error message.
        message: String,
    },
    /// The notification API returns denied.
    Denied,
    /// The notification API returns default rather than a denial.
    Default,
    /// The requested API is absent.
    Unavailable {
        /// Context describing the unavailable API.
        message: String,
    },
}

/// One settled browser result and any independently delivered host denial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityObservation {
    /// The requested browser mechanism.
    pub kind: PageCapability,
    /// The actual browser outcome.
    pub browser: BrowserPermissionResult,
    /// A host denial's observed user-initiated flag; absence is not denial.
    pub host_denial: Option<bool>,
}

impl CapabilityObservation {
    /// Validates the browser-result domain for the requested mechanism.
    /// Notification permission values cannot represent another `WebIDL` request.
    /// # Errors
    /// Rejects notification-only outcomes assigned to another capability.
    pub fn validate(&self) -> std::io::Result<()> {
        if matches!(
            self.browser,
            BrowserPermissionResult::Denied | BrowserPermissionResult::Default
        ) && self.kind != PageCapability::Notifications
        {
            return Err(super::identity::invalid(
                "Browser notification outcome belongs to another capability",
            ));
        }
        Ok(())
    }

    /// Returns whether the browser outcome permits native-denial reconciliation.
    /// This does not establish that the host delivers a denial event.
    /// Geolocation's permission-denied code is 1; media permission failure uses
    /// `NotAllowedError`. Notification denied/default are ungranted outcomes.
    /// Hardware absence, acquisition failure and timeouts are not denials.
    ///
    /// Specifications: <https://www.w3.org/TR/geolocation/#position_error_interface>
    /// and <https://www.w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia>.
    #[must_use]
    #[inline]
    pub fn permits_denial_reconciliation(&self) -> bool {
        match (&self.kind, &self.browser) {
            (
                PageCapability::Geolocation,
                BrowserPermissionResult::Rejected { code: Some(1), .. },
            )
            | (
                PageCapability::Notifications,
                BrowserPermissionResult::Denied | BrowserPermissionResult::Default,
            ) => true,
            (
                PageCapability::Camera | PageCapability::Microphone,
                BrowserPermissionResult::Rejected { name, .. },
            ) => name.as_deref() == Some("NotAllowedError"),
            _ => false,
        }
    }

    /// Requires a compatible browser outcome and independently observed denial.
    #[must_use]
    #[inline]
    pub fn confirms_host_denial(&self) -> bool {
        self.host_denial.is_some() && self.permits_denial_reconciliation()
    }
}
