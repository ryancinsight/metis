//! The handle that keeps one listener subscribed.

/// A live subscription; dropping it removes the listener.
#[must_use = "dropping a subscription unsubscribes its listener immediately"]
pub struct Subscription {
    unsubscribe: Option<Box<dyn FnOnce()>>,
}

impl Subscription {
    pub(super) fn new(unsubscribe: impl FnOnce() + 'static) -> Self {
        Self {
            unsubscribe: Some(Box::new(unsubscribe)),
        }
    }

    /// A subscription with nothing to remove, for a listener that was
    /// refused.
    pub(super) fn inert() -> Self {
        Self { unsubscribe: None }
    }

    /// Whether dropping this handle removes a listener.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.unsubscribe.is_some()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(unsubscribe) = self.unsubscribe.take() {
            unsubscribe();
        }
    }
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Subscription")
            .field("active", &self.is_active())
            .finish()
    }
}
