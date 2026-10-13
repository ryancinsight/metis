//! Sequential real requests retain native denials independently of browser results.

use super::{
    application::State,
    coordinator::{Coordinator, invalid},
    resources::{self, TaskOwner},
    view,
};
use crate::embedded::{
    BrowserPermissionResult, CapabilityObservation, PageCapability, PageEvent, PageRole,
};
use moirai_pal::wasm::{
    WebNotificationPermission, WebPermissionKind, WebPermissionOutcome, WebViewBridge,
    spawn_local_with_handle,
};
use std::{cell::RefCell, io, rc::Rc};

pub(super) fn start_if_probe(page: &Rc<Coordinator>) -> io::Result<()> {
    if page.identity().role() != PageRole::PermissionProbe {
        return Ok(());
    }
    let state = Rc::clone(
        &page
            .application
            .borrow()
            .as_ref()
            .ok_or_else(|| invalid("Permission probe has no mounted application"))?
            .state,
    );
    let weak = Rc::downgrade(page);
    let permissions = page.permissions.clone();
    let identity = page.identity();
    let owner = TaskOwner::acquire(&page.tasks)?;
    let task = spawn_local_with_handle(async move {
        let result = async {
            for kind in PageCapability::ALL {
                if let Some(page) = weak.upgrade() {
                    page.require_ready(identity)?;
                    view::element(&page.document, "result")?
                        .set_text(&format!("Requesting {} permission.", kind.as_str()));
                } else {
                    return Err(invalid("Permission coordinator is released"));
                }
                let outcome = permissions
                    .request(native_kind(kind), WebViewBridge::operation_wait_limit())?
                    .await?;
                let browser = browser_result(outcome);
                let mut values = state.borrow_mut();
                values.observations[kind.index()] = Some(CapabilityObservation {
                    kind,
                    browser,
                    host_denial: values.denials[kind.index()],
                });
                drop(values);
                if let Some(page) = weak.upgrade() {
                    render(&page, &state, kind)?;
                }
            }
            Ok(())
        }
        .await;
        // The completion event observes actual released future/provider owners.
        drop(owner);
        if let Some(page) = weak.upgrade() {
            let result = result.and_then(|()| completed(&page, &state));
            if let Err(error) = result {
                page.failure(&error);
            }
        }
    });
    page.application
        .borrow_mut()
        .as_mut()
        .ok_or_else(|| invalid("Permission task loses its mounted owner"))?
        .task = Some(task);
    Ok(())
}

fn native_kind(kind: PageCapability) -> WebPermissionKind {
    match kind {
        PageCapability::Geolocation => WebPermissionKind::Geolocation,
        PageCapability::Camera => WebPermissionKind::Camera,
        PageCapability::Microphone => WebPermissionKind::Microphone,
        PageCapability::Notifications => WebPermissionKind::Notifications,
    }
}

fn browser_result(outcome: WebPermissionOutcome) -> BrowserPermissionResult {
    match outcome {
        WebPermissionOutcome::Unavailable(kind) => BrowserPermissionResult::Unavailable {
            message: format!("Browser interface for {kind:?} is unavailable"),
        },
        WebPermissionOutcome::Rejected { error, .. } => BrowserPermissionResult::Rejected {
            name: error.name,
            code: error.code,
            message: error.message,
        },
        WebPermissionOutcome::GeolocationGranted
        | WebPermissionOutcome::MediaGranted(_)
        | WebPermissionOutcome::Notification(WebNotificationPermission::Granted) => {
            BrowserPermissionResult::Granted
        }
        WebPermissionOutcome::Notification(WebNotificationPermission::Denied) => {
            BrowserPermissionResult::Denied
        }
        WebPermissionOutcome::Notification(WebNotificationPermission::Default) => {
            BrowserPermissionResult::Default
        }
    }
}

pub(super) fn denied(
    page: &Coordinator,
    kind: PageCapability,
    user_initiated: bool,
) -> io::Result<()> {
    if page.identity().role() != PageRole::PermissionProbe {
        return Err(invalid("Permission observation is outside the probe role"));
    }
    let state = Rc::clone(
        &page
            .application
            .borrow()
            .as_ref()
            .ok_or_else(|| invalid("Permission denial has no mounted owner"))?
            .state,
    );
    {
        let mut state = state.borrow_mut();
        state.denials[kind.index()] = Some(user_initiated);
        if let Some(observation) = state.observations[kind.index()].as_mut() {
            observation.host_denial = Some(user_initiated);
        }
    }
    render(page, &state, kind)?;
    let reconciled = {
        let state = state.borrow();
        state.probe_completed
            && !state.final_completion_published
            && state.denials.iter().all(Option::is_some)
    };
    if reconciled {
        completed(page, &state)?;
    }
    Ok(())
}

fn render(page: &Coordinator, state: &RefCell<State>, kind: PageCapability) -> io::Result<()> {
    let state = state.borrow();
    let detail = if let Some(user_initiated) = state.denials[kind.index()] {
        if user_initiated {
            "host denied after user action".to_owned()
        } else {
            "host denied".to_owned()
        }
    } else if let Some(observation) = &state.observations[kind.index()] {
        match &observation.browser {
            BrowserPermissionResult::Granted => "unexpected success".to_owned(),
            BrowserPermissionResult::Rejected { message, .. } => {
                format!("browser rejected the request: {message}")
            }
            BrowserPermissionResult::Denied => "browser result: denied".to_owned(),
            BrowserPermissionResult::Default => "browser result: default".to_owned(),
            BrowserPermissionResult::Unavailable { message } => message.clone(),
        }
    } else {
        "pending".to_owned()
    };
    view::element(&page.document, &format!("permission-{}", kind.as_str()))?
        .set_text(&format!("{}: {detail}", kind.as_str()));
    Ok(())
}

fn completed(page: &Coordinator, state: &RefCell<State>) -> io::Result<()> {
    page.require_ready(page.identity())?;
    let observations: [CapabilityObservation; 4] = state
        .borrow()
        .observations
        .iter()
        .cloned()
        .map(|item| {
            item.ok_or_else(|| invalid("Permission probe completion precedes a real outcome"))
        })
        .collect::<io::Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| invalid("Permission outcome count differs from its closed domain"))?;
    view::element(&page.document, "result")?.set_text("Completed 4 bounded capability probes.");
    page.post(&PageEvent::ProbeCompleted {
        identity: page.identity(),
        observations: Box::new(observations),
        resources: resources::snapshot(page)?,
    })?;
    let mut state = state.borrow_mut();
    state.probe_completed = true;
    state.final_completion_published = state.denials.iter().all(Option::is_some);
    Ok(())
}
