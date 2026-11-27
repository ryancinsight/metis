//! The bounded permission probe: one sequential pass over four browser
//! capabilities, one row per outcome, one typed completion notice.

use super::bridge;
use crate::{logic::error_line, message::PostedMessage};
use moirai_pal::wasm::{
    WebNotificationPermission, WebPermissionKind, WebPermissionOutcome, WebPermissions,
    WebViewBridge, spawn_local_with_handle,
};
use std::{cell::Cell, io, rc::Rc};
use web_sys::{Document, Element, Window};

const PERMISSIONS: [(WebPermissionKind, &str); 4] = [
    (WebPermissionKind::Geolocation, "geolocation"),
    (WebPermissionKind::Camera, "camera"),
    (WebPermissionKind::Microphone, "microphone"),
    (WebPermissionKind::Notifications, "notifications"),
];

/// One frontend element handle bundle for the permission probe.
#[derive(Clone)]
struct ProbeElements {
    /// The host connection status line.
    status: Element,
    /// The probe progress and result line.
    result: Element,
    /// One row per bounded capability, in the fixed probe order.
    rows: Vec<Element>,
    host_denials: Rc<Cell<[bool; PERMISSIONS.len()]>>,
}

fn collect_probe(document: &Document) -> Option<ProbeElements> {
    let rows = PERMISSIONS
        .iter()
        .map(|(_, permission)| {
            let id = format!("permission-{permission}");
            document.get_element_by_id(&id)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ProbeElements {
        status: document.get_element_by_id("host-status")?,
        result: document.get_element_by_id("result")?,
        rows,
        host_denials: Rc::default(),
    })
}

/// Runs the bounded capability probes: one sequential pass over four browser
/// capabilities, one row per outcome, one typed completion notice.
pub(super) fn run_permission_probe(window: &Window, document: &Document) {
    let Some(elements) = collect_probe(document) else {
        return;
    };
    let denied_elements = elements.clone();
    let connection = bridge::connect(move |message| {
        let message = match message {
            Ok(message) => message,
            Err(error) => {
                let diagnostic = bridge::reply_diagnostic(&error);
                denied_elements.result.set_text_content(Some(&diagnostic));
                return;
            }
        };
        match message {
            PostedMessage::PermissionDenied {
                status,
                error_code,
                permission,
                message: text,
                user_initiated,
            } => {
                if let Some(index) = PERMISSIONS
                    .iter()
                    .position(|(_, admitted)| *admitted == permission)
                {
                    let note = if user_initiated {
                        "host denied after user action"
                    } else {
                        "host denied"
                    };
                    let mut host_denials = denied_elements.host_denials.get();
                    host_denials[index] = true;
                    denied_elements.host_denials.set(host_denials);
                    denied_elements.rows[index]
                        .set_text_content(Some(&format!("{permission}: {note}")));
                    let line = error_line(&status, &text, error_code);
                    denied_elements.result.set_text_content(Some(&line));
                }
            }
            PostedMessage::Error {
                status,
                error_code,
                message: text,
            } => {
                let line = error_line(&status, &text, error_code);
                denied_elements.result.set_text_content(Some(&line));
            }
            PostedMessage::Result { .. } => {}
        }
    });
    let Ok((bridge, listener)) = connection else {
        elements
            .status
            .set_text_content(Some("Host bridge unavailable"));
        elements
            .result
            .set_text_content(Some("Permission probe could not reach the host bridge."));
        return;
    };
    let Ok(permissions) = WebPermissions::current() else {
        elements
            .result
            .set_text_content(Some("Browser permission APIs are unavailable."));
        return;
    };
    elements.status.set_text_content(Some(
        "Host bridge connected; every capability request is denied by policy.",
    ));
    let result = elements.result.clone();
    let task = spawn_local_with_handle(async move {
        if let Err(error) = run_probe_sequence(&elements, &permissions, &bridge).await {
            elements
                .result
                .set_text_content(Some(&format!("Permission probe failed: {error}")));
        }
    });
    if bridge::own_session(window, listener, Some(task)).is_err() {
        result.set_text_content(Some(
            "The permission probe listener could not be installed.",
        ));
    }
}

async fn run_probe_sequence(
    elements: &ProbeElements,
    permissions: &WebPermissions,
    bridge: &WebViewBridge,
) -> io::Result<()> {
    for (index, (kind, name)) in PERMISSIONS.into_iter().enumerate() {
        elements
            .result
            .set_text_content(Some(&format!("Requesting {name} permission.")));
        let outcome = permissions
            .request(kind, WebViewBridge::operation_wait_limit())?
            .await?;
        if !elements.host_denials.get()[index] {
            let text = match outcome {
                WebPermissionOutcome::Unavailable(_) => "API unavailable",
                WebPermissionOutcome::Rejected { .. } => "browser rejected the request",
                WebPermissionOutcome::GeolocationGranted
                | WebPermissionOutcome::MediaGranted(_) => "unexpected success",
                WebPermissionOutcome::Notification(value) => match value {
                    WebNotificationPermission::Granted => "browser result: granted",
                    WebNotificationPermission::Denied => "browser result: denied",
                    WebNotificationPermission::Default => "browser result: default",
                },
            };
            elements.rows[index].set_text_content(Some(&format!("{name}: {text}")));
        }
    }
    elements.result.set_text_content(Some(&format!(
        "Completed {} bounded capability probes.",
        PERMISSIONS.len()
    )));
    bridge.post_json(r#"{"action":"permission_probe_complete"}"#)
}
