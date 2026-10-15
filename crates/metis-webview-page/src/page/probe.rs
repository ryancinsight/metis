//! The bounded permission probe: one sequential pass over four browser
//! capabilities, one row per outcome, one typed completion notice.

use super::bridge::{find_bridge, listen_for_messages, message_data, post_text};
use super::{PostedMessage, retain};
use crate::logic::error_line;
use js_sys::{Function, Object, Promise, Reflect as JsReflect};
use serde::Serialize;
use std::{cell::RefCell, vec::Vec};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::{Document, Element, Window};

/// The permission-row element ids, in the fixed probe order.
const PROBE_PERMISSIONS: [&str; 4] = ["geolocation", "camera", "microphone", "notifications"];

// Records which probe row already carries a typed outcome; the sequential
// probe and the host-denial listener both run through it, so one capability
// always renders at most one row line, like the retired recorded set.
thread_local! {
    static RECORDED: RefCell<[bool; 4]> = const { RefCell::new([false; 4]) };
}

/// The outbound probe-completion notice.
#[derive(Serialize)]
struct ProbeComplete {
    /// The only admitted action.
    action: &'static str,
}

/// One frontend element handle bundle for the permission probe.
#[derive(Clone)]
struct ProbeElements {
    /// The host connection status line.
    status: Element,
    /// The probe progress and result line.
    result: Element,
    /// One row per bounded capability, in the fixed probe order.
    rows: Vec<Element>,
}

fn collect_probe(document: &Document) -> Option<ProbeElements> {
    let rows = PROBE_PERMISSIONS
        .iter()
        .map(|permission| {
            let id = format!("permission-{permission}");
            document.get_element_by_id(&id)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ProbeElements {
        status: document.get_element_by_id("host-status")?,
        result: document.get_element_by_id("result")?,
        rows,
    })
}

/// Writes one row's typed outcome at most once, like the retired recorded set.
fn record(elements: &ProbeElements, index: usize, permission: &str, text: &str) {
    RECORDED.with(|flags| {
        let mut flags = flags.borrow_mut();
        if flags[index] {
            return;
        }
        flags[index] = true;
        if let Some(row) = elements.rows.get(index) {
            let line = format!("{permission}: {text}");
            row.set_text_content(Some(&line));
        }
    });
}

/// Runs the bounded capability probes: one sequential pass over four browser
/// capabilities, one row per outcome, one typed completion notice.
pub(super) fn run_permission_probe(window: &Window, document: &Document) {
    let Some(elements) = collect_probe(document) else {
        return;
    };
    let Some(bridge) = find_bridge(window) else {
        elements
            .status
            .set_text_content(Some("Host bridge unavailable"));
        elements
            .result
            .set_text_content(Some("Permission probe could not reach the host bridge."));
        return;
    };
    elements.status.set_text_content(Some(
        "Host bridge connected; every capability request is denied by policy.",
    ));

    let denied_elements = elements.clone();
    let on_denied = Closure::<dyn FnMut(JsValue)>::new(move |value: JsValue| {
        (|| {
            let Some(data) = message_data(&value) else {
                return;
            };
            let Ok(message) = serde_json::from_str::<PostedMessage>(&data) else {
                return;
            };
            match message {
                PostedMessage::PermissionDenied {
                    status,
                    error_code,
                    permission,
                    message: text,
                    user_initiated,
                } => {
                    if let Some(index) = PROBE_PERMISSIONS
                        .iter()
                        .position(|admitted| *admitted == permission)
                    {
                        let note = if user_initiated {
                            "host denied after user action"
                        } else {
                            "host denied"
                        };
                        record(&denied_elements, index, &permission, note);
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
        })();
    });
    listen_for_messages(&bridge, &on_denied);
    retain(on_denied);

    wasm_bindgen_futures::spawn_local(run_probe_sequence(window.clone(), elements, bridge));
}

/// Runs the sequential probe chain: request, record, next, then announce the
/// bounded completion count and notify the host exactly once.
async fn run_probe_sequence(window: Window, elements: ProbeElements, bridge: JsValue) {
    for (index, permission) in PROBE_PERMISSIONS.iter().enumerate() {
        let pending = format!("Requesting {permission} permission.");
        elements.result.set_text_content(Some(&pending));
        let text = invoke_probe(&window, permission).await;
        record(&elements, index, permission, &text);
    }
    let completed = RECORDED.with(|flags| flags.borrow().iter().filter(|flag| **flag).count());
    let line = format!("Completed {completed} bounded capability probes.");
    elements.result.set_text_content(Some(&line));
    if let Ok(payload) = serde_json::to_string(&ProbeComplete {
        action: "permission_probe_complete",
    }) {
        post_text(&bridge, &payload);
    }
}

/// Runs one bounded capability request and renders its typed outcome text.
async fn invoke_probe(window: &Window, permission: &str) -> String {
    match permission {
        "geolocation" => probe_geolocation(window).await,
        "camera" => probe_media(window, true).await,
        "microphone" => probe_media(window, false).await,
        "notifications" => probe_notifications(window).await,
        _ => "browser rejected the request".to_owned(),
    }
}

/// The retired geolocation probe: an unavailable API is typed as such and any
/// granted or denied outcome is typed without claiming a success.
async fn probe_geolocation(window: &Window) -> String {
    let navigator = window.navigator();
    if !JsReflect::has(&navigator.clone().into(), &JsValue::from_str("geolocation"))
        .unwrap_or(false)
    {
        return "API unavailable".to_owned();
    }
    let Ok(geolocation) = navigator.geolocation() else {
        return "browser rejected the request".to_owned();
    };
    let promise = Promise::new(&mut |resolve: Function, _reject: Function| {
        let granted = resolve.clone();
        let denied = resolve;
        let success = Box::leak(Box::new(Closure::<dyn FnMut(Object)>::new(
            move |_: Object| {
                let _ignored =
                    granted.call1(&JsValue::NULL, &JsValue::from_str("unexpected success"));
            },
        )));
        let failure = Box::leak(Box::new(Closure::<dyn FnMut(Object)>::new(
            move |_: Object| {
                let _ignored = denied.call1(
                    &JsValue::NULL,
                    &JsValue::from_str("browser rejected the request"),
                );
            },
        )));
        let _ignored = geolocation.get_current_position_with_error_callback(
            success.as_ref().unchecked_ref(),
            Some(failure.as_ref().unchecked_ref()),
        );
    });
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(value) => value
            .as_string()
            .unwrap_or_else(|| "unexpected success".to_owned()),
        Err(_) => "browser rejected the request".to_owned(),
    }
}

/// The retired camera or microphone probe: request one track, stop every
/// granted track immediately and type the bounded outcome.
async fn probe_media(window: &Window, video: bool) -> String {
    let navigator = window.navigator();
    if !JsReflect::has(
        &navigator.clone().into(),
        &JsValue::from_str("mediaDevices"),
    )
    .unwrap_or(false)
    {
        return "API unavailable".to_owned();
    }
    let Ok(devices) = navigator.media_devices() else {
        return "browser rejected the request".to_owned();
    };
    let constraints = {
        let constraints = web_sys::MediaStreamConstraints::new();
        if video {
            constraints.set_video(&JsValue::TRUE);
        } else {
            constraints.set_audio(&JsValue::TRUE);
        }
        constraints
    };
    let Ok(promise) = devices.get_user_media_with_constraints(&constraints) else {
        return "browser rejected the request".to_owned();
    };
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(value) => {
            if let Ok(stream) = value.dyn_into::<web_sys::MediaStream>() {
                for track in stream.get_tracks().iter() {
                    if let Ok(track) = track.dyn_into::<web_sys::MediaStreamTrack>() {
                        track.stop();
                    }
                }
            }
            "unexpected success".to_owned()
        }
        Err(_) => "browser rejected the request".to_owned(),
    }
}

/// The retired notification probe: the permission prompt resolves to a typed
/// browser decision without claiming a success.
async fn probe_notifications(window: &Window) -> String {
    if !JsReflect::has(&window.clone().into(), &JsValue::from_str("Notification")).unwrap_or(false)
    {
        return "API unavailable".to_owned();
    }
    let Ok(promise) = web_sys::Notification::request_permission() else {
        return "browser rejected the request".to_owned();
    };
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(value) => match value.as_string() {
            Some(permission) => format!("browser result: {permission}"),
            None => "unexpected success".to_owned(),
        },
        Err(_) => "browser rejected the request".to_owned(),
    }
}
