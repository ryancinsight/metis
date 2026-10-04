//! Last-written DOM state for the web presentation path.
//!
//! [`DomWriteCache`] records the value carried by the most recent provider
//! write per element, so the render pass can skip both the element lookup and
//! the provider call when the computed value already matches the document.
//! The cache stores values only, never element handles: handles would dangle
//! across markup replacement, while a value mismatch simply rewrites.
//!
//! A remount constructs fresh browser state whose cache starts empty, so the
//! first pass after markup replacement rewrites every node. Any render pass
//! that fails discards its cache for the same reason, since out-of-cache
//! writes may have landed after the divergence.

use std::collections::HashMap;

/// Target identifying document-level attributes in [`DomWriteCache`].
///
/// Element entries are keyed by element id; the document body has no id, so
/// body attributes record under this sentinel. No element may carry this id.
pub(crate) const DOCUMENT_BODY_TARGET: &str = "body";

/// Records the last value written through each DOM mutation entry point.
///
/// Each `write_*` method reports whether the caller must perform the provider
/// write: true on the first write of a value and whenever the value differs
/// from the recorded one, false when the recorded value already matches. A
/// false return lets the caller skip the lookup and the write with no
/// observable change. [`invalidate`](Self::invalidate) drops every recorded
/// value so the next pass rewrites unconditionally.
#[derive(Debug, Default)]
pub(crate) struct DomWriteCache {
    text: HashMap<Box<str>, Box<str>>,
    attributes: HashMap<(Box<str>, Box<str>), Box<str>>,
    disabled: HashMap<Box<str>, bool>,
}

impl DomWriteCache {
    /// Reports whether `text` must be written to the element `id`.
    pub(crate) fn write_text(&mut self, id: &str, text: &str) -> bool {
        match self.text.get(id) {
            Some(current) if current.as_ref() == text => false,
            _ => {
                self.text.insert(id.into(), text.into());
                true
            }
        }
    }

    /// Reports whether `value` must be written to attribute `name` on `target`.
    pub(crate) fn write_attribute(&mut self, target: &str, name: &str, value: &str) -> bool {
        let key = (target.into(), name.into());
        match self.attributes.get(&key) {
            Some(current) if current.as_ref() == value => false,
            _ => {
                self.attributes.insert(key, value.into());
                true
            }
        }
    }

    /// Reports whether the disabled state of the element `id` must be written.
    pub(crate) fn write_disabled(&mut self, id: &str, disabled: bool) -> bool {
        match self.disabled.get(id) {
            Some(current) if *current == disabled => false,
            _ => {
                self.disabled.insert(id.into(), disabled);
                true
            }
        }
    }

    /// Drops every recorded value so the next pass rewrites unconditionally.
    pub(crate) fn invalidate(&mut self) {
        self.text.clear();
        self.attributes.clear();
        self.disabled.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{DOCUMENT_BODY_TARGET, DomWriteCache};

    #[test]
    fn redundant_text_write_elides_provider_call() {
        let mut cache = DomWriteCache::default();
        assert!(cache.write_text("metis-status", "Controls active"));
        assert!(!cache.write_text("metis-status", "Controls active"));
        assert!(!cache.write_text("metis-status", "Controls active"));
    }

    #[test]
    fn changed_text_write_passes_through() {
        let mut cache = DomWriteCache::default();
        assert!(cache.write_text("metis-status", "Request in progress"));
        assert!(cache.write_text("metis-status", "Backend result received"));
        assert!(!cache.write_text("metis-status", "Backend result received"));
    }

    #[test]
    fn text_entries_are_independent_per_element() {
        let mut cache = DomWriteCache::default();
        assert!(cache.write_text("metis-status", "Controls active"));
        assert!(cache.write_text("session-dialog-status", "Controls active"));
        assert!(!cache.write_text("metis-status", "Controls active"));
        assert!(cache.write_text("metis-status", "Request in progress"));
        assert!(!cache.write_text("session-dialog-status", "Controls active"));
    }

    #[test]
    fn redundant_attribute_write_elides_provider_call() {
        let mut cache = DomWriteCache::default();
        assert!(cache.write_attribute("metis-form", "aria-busy", "false"));
        assert!(!cache.write_attribute("metis-form", "aria-busy", "false"));
        assert!(cache.write_attribute("metis-form", "aria-busy", "true"));
        assert!(!cache.write_attribute("metis-form", "aria-busy", "true"));
    }

    #[test]
    fn attribute_entries_distinguish_name_and_target() {
        let mut cache = DomWriteCache::default();
        assert!(cache.write_attribute("metis-form", "aria-busy", "false"));
        assert!(cache.write_attribute("metis-form", "data-other", "false"));
        assert!(cache.write_attribute(DOCUMENT_BODY_TARGET, "data-metis-theme", "light"));
        assert!(!cache.write_attribute("metis-form", "aria-busy", "false"));
        assert!(!cache.write_attribute(DOCUMENT_BODY_TARGET, "data-metis-theme", "light"));
    }

    #[test]
    fn redundant_disabled_write_elides_provider_call() {
        let mut cache = DomWriteCache::default();
        assert!(cache.write_disabled("submit-calculation", false));
        assert!(!cache.write_disabled("submit-calculation", false));
        assert!(cache.write_disabled("submit-calculation", true));
        assert!(!cache.write_disabled("submit-calculation", true));
    }

    #[test]
    fn invalidation_restores_writes_after_remount() {
        let mut cache = DomWriteCache::default();
        assert!(cache.write_text("metis-status", "Controls active"));
        assert!(cache.write_attribute("metis-form", "aria-busy", "false"));
        assert!(cache.write_disabled("submit-calculation", false));
        cache.invalidate();
        assert!(cache.write_text("metis-status", "Controls active"));
        assert!(cache.write_attribute("metis-form", "aria-busy", "false"));
        assert!(cache.write_disabled("submit-calculation", false));
    }
}
