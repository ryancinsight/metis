//! Browser-side target policy for typed fragment responses.

use metis_core::protocol::{FragmentPatchSet, MAX_FRAGMENT_ATTRIBUTE_BYTES};

const TARGET_IDS: &[&str] = &[
    "metis-status",
    "metis-capabilities",
    "metis-plugins",
    "metis-events",
    "session-dialog-status",
    "session-dialog-capabilities",
    "options-state",
    "pointer-status",
    "wheel-status",
    "gesture-status",
    "drop-status",
    "drop-byte-status",
    "text-status",
    "text-preview",
    "composition-status",
    "selection-status",
    "result-state",
    "result-metrics",
    "result-detail",
    "result-patient",
    "result-weight",
    "result-concentration",
    "result-dose",
    "explorer-status",
    "explorer-caption",
    "explorer-window-status",
    "explorer-entry-0",
    "explorer-entry-1",
    "explorer-entry-2",
    "explorer-entry-3",
    "explorer-entry-4",
    "explorer-entry-5",
    "explorer-entry-6",
    "explorer-entry-7",
];

/// Returns whether a fragment may address a mounted Metis element.
#[must_use]
pub(crate) fn target_allowed(target: &str) -> bool {
    TARGET_IDS.contains(&target)
}

/// Returns whether a fragment may change one DOM attribute.
///
/// Dynamic updates are limited to accessibility/data attributes and the
/// existing class/value presentation fields. Navigation, event-handler,
/// style, source, and URL attributes are intentionally outside this policy.
#[must_use]
pub(crate) fn attribute_allowed(name: &str) -> bool {
    let valid_name = !name.is_empty()
        && name.len() <= MAX_FRAGMENT_ATTRIBUTE_BYTES
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b':' | b'_')
        });
    valid_name
        && (name == "class"
            || name == "value"
            || (name.starts_with("aria-") && name.len() > "aria-".len())
            || (name.starts_with("data-") && name.len() > "data-".len()))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn listeners(
    document: &moirai_pal::wasm::WebDocument,
    app: &std::rc::Rc<
        std::cell::RefCell<
            Option<metis_frontend::AsyncFrontendApp<metis_ipc::BrowserWebSocketTransport>>,
        >,
    >,
    generation: crate::epoch::Generation,
    task_slot: &std::rc::Rc<std::cell::RefCell<Option<moirai_pal::wasm::LocalTaskHandle>>>,
) -> std::io::Result<Vec<moirai_pal::wasm::WebEventListener>> {
    use moirai_pal::wasm::{LocalTaskHandle, WebDocument, spawn_local_with_handle};
    use std::cell::RefCell;
    use std::io::{Error, ErrorKind};
    use std::rc::Rc;

    let trigger = document
        .get_element_by_id("open-session-dialog")
        .ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                "Fragment action trigger is not mounted",
            )
        })?;
    let listener_document = document.clone();
    let listener_app = Rc::clone(app);
    let listener_task = Rc::clone(task_slot);
    let mut listeners = Vec::with_capacity(1);
    listeners.push(trigger.add_event_listener("click", move |_event| {
        if !crate::browser::generation_is_current(generation) || listener_task.borrow().is_some() {
            return;
        }
        let Some(mut frontend) = listener_app.borrow_mut().take() else {
            return;
        };
        let Ok(action) = metis_core::protocol::FragmentAction::new(
            generation.value(),
            "status.describe",
            "metis-events",
            "session-dialog",
        ) else {
            *listener_app.borrow_mut() = Some(frontend);
            return;
        };
        let task_cleanup: Rc<RefCell<Option<LocalTaskHandle>>> = Rc::clone(&listener_task);
        let result_app = Rc::clone(&listener_app);
        let result_document: WebDocument = listener_document.clone();
        let task = spawn_local_with_handle(async move {
            let result = frontend.dispatch_fragment_action(&action).await;
            if !crate::browser::generation_is_current(generation) {
                let _ = task_cleanup.borrow_mut().take();
                return;
            }
            match result {
                Ok(patch_set) => {
                    if apply_patch_set(&result_document, generation.value(), &patch_set).is_err()
                        && let Some(status) = result_document.get_element_by_id("metis-events")
                    {
                        status.set_text("Fragment response rejected by the browser target policy");
                    }
                }
                Err(_) => {
                    if let Some(status) = result_document.get_element_by_id("metis-events") {
                        status.set_text("Fragment action rejected by the authorized backend");
                    }
                }
            }
            *result_app.borrow_mut() = Some(frontend);
            let _ = task_cleanup.borrow_mut().take();
        });
        *listener_task.borrow_mut() = Some(task);
    })?);
    Ok(listeners)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn apply_patch_set(
    document: &moirai_pal::wasm::WebDocument,
    expected_generation: u64,
    patch_set: &FragmentPatchSet,
) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};

    if patch_set.generation() != expected_generation {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Fragment response generation is stale",
        ));
    }

    // Preflight every target and attribute before mutating the DOM. The
    // synchronous function cannot interleave browser script between passes,
    // so an invalid patch leaves the mounted tree unchanged.
    for patch in patch_set.patches() {
        if !target_allowed(patch.target()) {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Fragment target is outside the mounted target policy",
            ));
        }
        if document.get_element_by_id(patch.target()).is_none() {
            return Err(Error::new(
                ErrorKind::NotFound,
                "Fragment target is not mounted",
            ));
        }
        if let Some((name, _)) = patch.attribute()
            && !attribute_allowed(name)
        {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Fragment attribute is outside the presentation policy",
            ));
        }
    }

    for patch in patch_set.patches() {
        let target = document.get_element_by_id(patch.target()).ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                "Fragment target disappeared during apply",
            )
        })?;
        if let Some(value) = patch.text() {
            target.set_text(value);
        } else if let Some((name, value)) = patch.attribute() {
            target.set_attribute(name, value)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_core::protocol::FragmentPatch;

    #[test]
    fn target_policy_accepts_mounted_status_surfaces_only() {
        assert!(target_allowed("metis-events"));
        assert!(target_allowed("explorer-entry-7"));
        assert!(!target_allowed("patient-id"));
        assert!(!target_allowed("document-body"));
    }

    #[test]
    fn attribute_policy_excludes_authority_bearing_names() {
        assert!(attribute_allowed("aria-busy"));
        assert!(attribute_allowed("data-result-id"));
        assert!(attribute_allowed("class"));
        assert!(!attribute_allowed("aria-"));
        assert!(!attribute_allowed("data-"));
        assert!(!attribute_allowed("data-invalid name"));
        assert!(!attribute_allowed("DATA-result-id"));
        assert!(!attribute_allowed("data-!"));
        assert!(!attribute_allowed("onclick"));
        assert!(!attribute_allowed("style"));
        assert!(!attribute_allowed("href"));
        assert!(!attribute_allowed("src"));
    }

    #[test]
    fn policy_is_independent_of_patch_body_values() {
        let patch =
            FragmentPatch::set_text("metis-events", "<script>ignored</script>").expect("patch");
        let response = FragmentPatchSet::new(1, vec![patch]).expect("patch set");
        assert_eq!(
            response.patches()[0].text(),
            Some("<script>ignored</script>")
        );
        assert!(target_allowed(response.patches()[0].target()));
    }
}
