use super::view_panels::{render_clipboard, render_drop, render_result, render_text};
use super::{BridgeStatus, BrowserState};
use crate::dom_cache::{DOCUMENT_BODY_TARGET, DomWriteCache};
use metis_core::CapabilityScope;
use metis_core::protocol::{
    CapabilityCatalogPayload, MAX_PLUGINS, Plugin, PluginDescriptor, PluginOperation,
    PluginRegistry, TargetCapabilityPayload,
};
use metis_frontend::{ExplorerStatus, FormState};
use metis_ipc::client::HandshakeError;
use moirai_pal::wasm::{WebDocument, WebElement};
use std::io;

/// Renders the current state, writing only DOM state that changed.
///
/// Every text, attribute, and disabled write passes through the state's
/// [`DomWriteCache`](crate::dom_cache::DomWriteCache): a repeated render with
/// unchanged state performs no provider writes, so idle keystrokes produce no
/// mutations. A failed pass discards the cache because error reporting writes
/// outside it and the recorded values may no longer describe the document.
pub(super) fn render(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let result = render_dom(document, state);
    if result.is_err() {
        state.dom_cache.invalidate();
    }
    result
}

fn render_dom(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let message = status_message(state);
    render_theme_and_inputs(document, state)?;
    render_commands(document, state)?;
    render_status(document, state, &message)?;
    render_result(document, state, &message)
}

/// Records the Rust-owned browser listener count and lifecycle generation.
///
/// The values are bounded DOM attributes so a browser conformance runner can
/// verify teardown and remount without relying on provider-private inspection
/// APIs. The count covers the listener guards retained by this application;
/// provider internals remain outside this contract.
pub(super) fn render_lifecycle(
    document: &WebDocument,
    listener_count: usize,
    generation: u64,
    message: &str,
) -> io::Result<()> {
    let root = element(document, "metis-app")?;
    root.set_attribute("data-metis-listener-count", &listener_count.to_string())?;
    root.set_attribute("data-metis-generation", &generation.to_string())?;
    let lifecycle = element(document, "metis-lifecycle")?;
    let lifecycle_message =
        format!("{message} ({listener_count} listener handles; generation {generation})");
    lifecycle.set_text(&lifecycle_message);
    lifecycle.set_attribute("data-listener-count", &listener_count.to_string())?;
    lifecycle.set_attribute("data-generation", &generation.to_string())?;
    Ok(())
}

fn render_theme_and_inputs(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let BrowserState {
        inputs,
        controls,
        dom_cache,
        ..
    } = &mut *state;
    let theme = controls.theme().css_value();
    if dom_cache.write_attribute(DOCUMENT_BODY_TARGET, "data-metis-theme", theme) {
        document.body()?.set_attribute("data-metis-theme", theme)?;
    }
    set_attribute(dom_cache, document, "metis-app", "data-metis-theme", theme)?;
    set_text(dom_cache, document, "result-patient", &inputs.patient_id)?;
    set_text(
        dom_cache,
        document,
        "result-weight",
        &format!("{:.2} kg", inputs.weight_kg),
    )?;
    set_text(
        dom_cache,
        document,
        "result-concentration",
        &format!("{:.2} mg/mL", inputs.concentration_mg_ml),
    )?;
    set_text(
        dom_cache,
        document,
        "result-dose",
        &format!("{:.3} mcg/kg/min", inputs.target_dose_mcg_kg_min),
    )?;
    Ok(())
}

fn status_message(state: &BrowserState) -> String {
    match &state.state {
        FormState::Idle => match state.bridge {
            BridgeStatus::Disabled => "Controls active; no authorized backend bridge configured",
            BridgeStatus::Connecting => "Connecting to authorized backend",
            BridgeStatus::Ready => "Authorized backend session ready",
        }
        .to_owned(),
        FormState::Failed(error) => format!("Input rejected [{}]", error.code.as_str()),
        FormState::Disconnected(error) => {
            format!("Backend unavailable [{}]", error.code.as_str())
        }
        FormState::Pending => "Request in progress".to_owned(),
        FormState::Success(_) => "Backend result received".to_owned(),
        FormState::Rejected(error) => {
            format!("Backend rejected request [0x{:04X}]", error.error_code)
        }
        FormState::SessionFailed(error) => match error {
            HandshakeError::Local(error) => {
                format!("Backend session failed [{}]", error.code.as_str())
            }
            HandshakeError::Remote(error) => {
                format!("Backend session rejected [0x{:04X}]", error.error_code)
            }
            _ => "Backend session failed".to_owned(),
        },
        _ => "Unsupported form state".to_owned(),
    }
}

fn render_status(
    document: &WebDocument,
    state: &mut BrowserState,
    message: &str,
) -> io::Result<()> {
    set_request_busy_attributes(document, state)?;
    let BrowserState {
        controls,
        event_status,
        capabilities,
        plugins,
        dom_cache,
        ..
    } = &mut *state;
    set_text(dom_cache, document, "metis-status", message)?;
    set_text(dom_cache, document, "session-dialog-status", message)?;
    set_text(dom_cache, document, "metis-capabilities", capabilities)?;
    set_text(
        dom_cache,
        document,
        "session-dialog-capabilities",
        capabilities,
    )?;
    set_text(dom_cache, document, "metis-plugins", plugins)?;
    let event_status = if controls.show_events() {
        event_status.as_str()
    } else {
        "Remote events: hidden by preference"
    };
    set_text(dom_cache, document, "metis-events", event_status)?;
    set_text(dom_cache, document, "options-state", &controls.summary())?;
    render_drop(document, state)?;
    render_text(document, state)?;
    render_clipboard(document, state)?;
    let BrowserState {
        bridge,
        state: form_state,
        dom_cache,
        ..
    } = &mut *state;
    let submit_disabled =
        !matches!(bridge, BridgeStatus::Ready) || matches!(form_state, FormState::Pending);
    set_disabled(dom_cache, document, "submit-calculation", submit_disabled)?;
    Ok(())
}

fn render_commands(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let BrowserState {
        commands,
        dom_cache,
        ..
    } = &mut *state;
    let expanded = commands.menu_open;
    let expanded_value = if expanded { "true" } else { "false" };
    let hidden_value = if expanded { "false" } else { "true" };
    set_attribute(
        dom_cache,
        document,
        "command-menu-toggle",
        "aria-expanded",
        expanded_value,
    )?;
    set_attribute(
        dom_cache,
        document,
        "command-menu",
        "aria-hidden",
        hidden_value,
    )?;
    set_attribute(
        dom_cache,
        document,
        "command-menu",
        "data-command-menu-open",
        expanded_value,
    )?;
    set_text(dom_cache, document, "command-status", &commands.status)
}

fn set_request_busy_attributes(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let request_busy = matches!(state.state, FormState::Pending);
    let busy_value = if request_busy { "true" } else { "false" };
    let BrowserState { dom_cache, .. } = &mut *state;
    for id in [
        "metis-status",
        "session-dialog-status",
        "metis-form",
        "result-state",
    ] {
        // The provider call keeps the historical method-call form pinned by
        // the accessibility presentation contract; the cache guard around it
        // is what elides the redundant write.
        if dom_cache.write_attribute(id, "aria-busy", busy_value) {
            element(document, id)?.set_attribute("aria-busy", busy_value)?;
        }
    }
    Ok(())
}

/// Renders the explorer status surface: status text, caption, and table state.
///
/// The row widget itself lives in [`view_explorer`](super::view_explorer);
/// this summary stays beside the other status surfaces so the accessibility
/// presentation contract keeps one home for status derivations.
pub(super) fn render_explorer_summary(
    document: &WebDocument,
    state: &mut BrowserState,
) -> io::Result<()> {
    let BrowserState {
        result_explorer: explorer,
        dom_cache,
        ..
    } = &mut *state;
    let status = match explorer.status() {
        ExplorerStatus::Empty => "Explorer: no backend results".to_owned(),
        ExplorerStatus::Loading => "Explorer: loading backend result".to_owned(),
        ExplorerStatus::Ready => format!(
            "Explorer: {} result{} retained",
            explorer.row_count(),
            if explorer.row_count() == 1 { "" } else { "s" }
        ),
        ExplorerStatus::Error(error) => {
            format!("Explorer error [{}]", error.code.as_str())
        }
        _ => "Explorer: unsupported state".to_owned(),
    };
    set_text(dom_cache, document, "explorer-status", &status)?;
    set_text(
        dom_cache,
        document,
        "explorer-caption",
        &format!(
            "Retained results grouped by patient; filter `{}`; order {} {}",
            explorer.filter(),
            explorer.sort().key().label(),
            explorer.sort().direction().value(),
        ),
    )?;
    set_attribute(
        dom_cache,
        document,
        "explorer-table",
        "data-result-status",
        explorer_status_name(explorer.status()),
    )?;
    set_attribute(
        dom_cache,
        document,
        "explorer-table",
        "aria-busy",
        if matches!(explorer.status(), ExplorerStatus::Loading) {
            "true"
        } else {
            "false"
        },
    )?;
    set_attribute(
        dom_cache,
        document,
        "explorer-table",
        "data-window-start",
        &explorer.window_start().to_string(),
    )?;
    Ok(())
}

fn explorer_status_name(status: &ExplorerStatus) -> &'static str {
    match status {
        ExplorerStatus::Empty => "empty",
        ExplorerStatus::Loading => "loading",
        ExplorerStatus::Ready => "ready",
        ExplorerStatus::Error(_) => "error",
        _ => "unknown",
    }
}

pub(super) fn capability_summary(
    catalog: &CapabilityCatalogPayload,
    target: &TargetCapabilityPayload,
) -> String {
    let command_names = catalog
        .commands()
        .iter()
        .filter_map(|command| {
            command
                .descriptor()
                .map(metis_core::CommandDescriptor::name)
        })
        .collect::<Vec<_>>();
    let host_surfaces = target
        .capabilities()
        .iter()
        .map(|capability| capability.name())
        .collect::<Vec<_>>();
    let browser = TargetCapabilityPayload::browser_application();
    let browser_surfaces = browser
        .capabilities()
        .iter()
        .map(|capability| capability.name())
        .collect::<Vec<_>>();
    format!(
        "Host capabilities: target={} surfaces=[{}] commands=[{}]; browser target={} surfaces=[{}]",
        target.platform().name(),
        host_surfaces.join(", "),
        command_names.join(", "),
        browser.platform().name(),
        browser_surfaces.join(", "),
    )
}

static WORKBENCH_EVENTS: [PluginOperation; 1] = [PluginOperation::new(
    "form.state",
    CapabilityScope::UI_RENDER,
)];

struct WorkbenchPlugin;

impl Plugin for WorkbenchPlugin {
    const DESCRIPTOR: PluginDescriptor =
        PluginDescriptor::new("workbench", 1, &[], &WORKBENCH_EVENTS);
}

pub(super) fn plugin_summary() -> String {
    let mut registry = PluginRegistry::<MAX_PLUGINS>::new()
        .expect("invariant: the browser plugin registry has a positive bounded capacity");
    registry
        .register::<WorkbenchPlugin>()
        .expect("invariant: the static browser plugin manifest is valid");
    let entries = registry
        .plugins()
        .iter()
        .map(|plugin| format!("{} v{}", plugin.name(), plugin.version()))
        .collect::<Vec<_>>();
    format!("Registered frontend extensions: {}", entries.join(", "))
}

/// Writes `text` to the element `id` unless the cache already holds it.
///
/// A cache hit skips both the element lookup and the provider write, so
/// repeated renders with unchanged state produce no mutations. A miss records
/// the value before writing; if the write then fails, [`render`] discards the
/// whole cache.
pub(super) fn set_text(
    cache: &mut DomWriteCache,
    document: &WebDocument,
    id: &str,
    text: &str,
) -> io::Result<()> {
    if cache.write_text(id, text) {
        element(document, id)?.set_text(text);
    }
    Ok(())
}

/// Writes attribute `name` on the element `id` unless the cache already holds it.
pub(super) fn set_attribute(
    cache: &mut DomWriteCache,
    document: &WebDocument,
    id: &str,
    name: &str,
    value: &str,
) -> io::Result<()> {
    if cache.write_attribute(id, name, value) {
        element(document, id)?.set_attribute(name, value)?;
    }
    Ok(())
}

/// Writes the disabled state of the element `id` unless the cache already holds it.
pub(super) fn set_disabled(
    cache: &mut DomWriteCache,
    document: &WebDocument,
    id: &str,
    disabled: bool,
) -> io::Result<()> {
    if cache.write_disabled(id, disabled) {
        element(document, id)?.set_disabled(disabled)?;
    }
    Ok(())
}

pub(super) fn element(document: &WebDocument, id: &str) -> io::Result<WebElement> {
    document.get_element_by_id(id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Metis DOM element #{id} is absent"),
        )
    })
}

pub(super) fn set_mount_error(document: &WebDocument, error: &io::Error) {
    if let Ok(status) = element(document, "metis-status") {
        status.set_text(&format!("Browser host error: {error}"));
    }
}

pub(super) fn set_status_error(
    document: &WebDocument,
    status: &WebElement,
    label: &str,
    message: &str,
) {
    status.set_text(&format!("{label}: error ({message})"));
    if let Ok(mount_status) = element(document, "metis-status") {
        mount_status.set_text(&format!("Browser host error: {message}"));
    }
}
