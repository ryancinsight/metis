use super::result::update_result_explorer;
use super::{BridgeStatus, BrowserState, events, generation_is_current, view};
use crate::epoch::Generation;
use metis_core::error::{ErrorCode, MetisError};
use metis_frontend::{AsyncFrontendApp, FormState};
use metis_ipc::BrowserWebSocketTransport;
use moirai_pal::wasm::{LocalTaskHandle, WebDocument, spawn_local_with_handle};
use std::{cell::RefCell, rc::Rc};

#[expect(
    clippy::too_many_lines,
    reason = "one browser submission owns the bounded request, event and render transition"
)]
pub(super) fn submit(
    document: &WebDocument,
    state: &Rc<RefCell<BrowserState>>,
    app_slot: &Rc<RefCell<Option<AsyncFrontendApp<BrowserWebSocketTransport>>>>,
    task_slot: &Rc<RefCell<Option<LocalTaskHandle>>>,
    fragment_task_slot: &Rc<RefCell<Option<LocalTaskHandle>>>,
    generation: Generation,
) {
    if !generation_is_current(generation) {
        return;
    }
    if !matches!(state.borrow().bridge, BridgeStatus::Ready) {
        return;
    }
    if task_slot.borrow().is_some() {
        let mut state = state.borrow_mut();
        state.state = FormState::Pending;
        state.result_explorer.begin_loading();
        if let Err(error) = view::render(document, &state) {
            view::set_mount_error(document, &error);
        }
        return;
    }
    // Fragment dispatch temporarily owns the same frontend slot. Ignore a
    // concurrent submit while that task holds it instead of translating the
    // temporary absence into a false disconnected bridge state.
    if fragment_task_slot.borrow().is_some() {
        return;
    }
    let Some(mut app) = app_slot.borrow_mut().take() else {
        let mut state = state.borrow_mut();
        state.state = FormState::Disconnected(MetisError::transport(
            ErrorCode::ConnectionClosed,
            "No authorized browser backend bridge is configured",
        ));
        if let Err(error) = view::render(document, &state) {
            view::set_mount_error(document, &error);
        }
        return;
    };
    let inputs = state.borrow().inputs.clone();
    app.set_inputs(
        &inputs.patient_id,
        inputs.weight_kg,
        inputs.concentration_mg_ml,
        inputs.target_dose_mcg_kg_min,
    );
    {
        let mut state = state.borrow_mut();
        state.state = FormState::Pending;
        state.result_explorer.begin_loading();
        if let Err(error) = view::render(document, &state) {
            view::set_mount_error(document, &error);
        }
    }
    let task_cleanup = Rc::clone(task_slot);
    let result_state = Rc::clone(state);
    let result_app = Rc::clone(app_slot);
    let result_document = document.clone();
    let task = spawn_local_with_handle(async move {
        let result = app.submit_calculation().await;
        let expected = match app.state() {
            FormState::Success(response) => Some(response.clone()),
            _ => None,
        };
        let event = match expected {
            Some(expected) if result.is_ok() => {
                events::receive_result_event(&mut app, expected).await
            }
            _ => Ok(None),
        };
        let event_error = event.as_ref().err().cloned();
        let outcome = event_error
            .clone()
            .map_or_else(|| app.state().clone(), FormState::Disconnected);
        if !generation_is_current(generation) {
            let _ = task_cleanup.borrow_mut().take();
            return;
        }
        let bridge = submission_bridge(&result, event_error.as_ref());
        *result_app.borrow_mut() = Some(app);
        {
            let mut state = result_state.borrow_mut();
            state.bridge = bridge;
            if result.is_err() || event_error.is_some() {
                "Host capabilities: unavailable".clone_into(&mut state.capabilities);
            }
            match (&event, &outcome) {
                (Ok(Some(summary)), _) => summary.clone_into(&mut state.event_status),
                (Ok(None), FormState::Rejected(_)) => {
                    "Remote events: none (request rejected)".clone_into(&mut state.event_status);
                }
                (Err(error), _) => {
                    state.event_status =
                        format!("Remote event unavailable [{}]", error.code.as_str());
                }
                _ => {}
            }
            state.state = outcome;
            update_result_explorer(
                &mut state,
                &inputs.patient_id,
                &result,
                event_error.as_ref(),
            );
            if let Err(error) = view::render(&result_document, &state) {
                view::set_mount_error(&result_document, &error);
            }
        }
        let _ = task_cleanup.borrow_mut().take();
    });
    *task_slot.borrow_mut() = Some(task);
}

pub(super) fn submission_bridge(
    result: &metis_core::error::Result<()>,
    event_error: Option<&MetisError>,
) -> BridgeStatus {
    if result.is_ok() && event_error.is_none() {
        BridgeStatus::Ready
    } else {
        BridgeStatus::Disabled
    }
}
