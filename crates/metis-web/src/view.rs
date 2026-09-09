use super::{BridgeStatus, BrowserState};
use crate::controls::{DisplayUnit, ResultDetail};
use crate::text_policy::CompositionState;
use metis_core::CapabilityScope;
use metis_core::protocol::{
    CapabilityCatalogPayload, MAX_PLUGINS, Plugin, PluginDescriptor, PluginOperation,
    PluginRegistry, TargetCapabilityPayload,
};
use metis_frontend::{ExplorerStatus, FormState, RESULT_PAGE_SIZE, VisibleEntry};
use metis_ipc::client::HandshakeError;
use moirai_pal::wasm::{WebDocument, WebElement};
use std::io;

pub(super) fn render(document: &WebDocument, state: &BrowserState) -> io::Result<()> {
    let inputs = &state.inputs;
    document
        .body()?
        .set_attribute("data-metis-theme", state.controls.theme().css_value())?;
    element(document, "metis-app")?
        .set_attribute("data-metis-theme", state.controls.theme().css_value())?;
    set_text(document, "result-patient", &inputs.patient_id)?;
    set_text(
        document,
        "result-weight",
        &format!("{:.2} kg", inputs.weight_kg),
    )?;
    set_text(
        document,
        "result-concentration",
        &format!("{:.2} mg/mL", inputs.concentration_mg_ml),
    )?;
    set_text(
        document,
        "result-dose",
        &format!("{:.3} mcg/kg/min", inputs.target_dose_mcg_kg_min),
    )?;
    let message = match &state.state {
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
    };
    let request_busy = matches!(state.state, FormState::Pending);
    let busy_value = if request_busy { "true" } else { "false" };
    for id in [
        "metis-status",
        "session-dialog-status",
        "metis-form",
        "result-state",
    ] {
        element(document, id)?.set_attribute("aria-busy", busy_value)?;
    }
    set_text(document, "metis-status", &message)?;
    set_text(document, "session-dialog-status", &message)?;
    set_text(document, "metis-capabilities", &state.capabilities)?;
    set_text(document, "session-dialog-capabilities", &state.capabilities)?;
    set_text(document, "metis-plugins", &state.plugins)?;
    let event_status = if state.controls.show_events() {
        state.event_status.as_str()
    } else {
        "Remote events: hidden by preference"
    };
    set_text(document, "metis-events", event_status)?;
    set_text(document, "options-state", &state.controls.summary())?;
    render_drop(document, state)?;
    render_text(document, state)?;
    let submit_disabled =
        !matches!(state.bridge, BridgeStatus::Ready) || matches!(state.state, FormState::Pending);
    element(document, "submit-calculation")?.set_disabled(submit_disabled)?;
    let metrics = match &state.state {
        FormState::Success(response) => match state.controls.display_unit() {
            DisplayUnit::Volume => format!("Volume rate: {:.6} mL/hr", response.rate_ml_hr),
            DisplayUnit::DrugMass => {
                format!("Drug mass rate: {:.6} mg/hr", response.drug_rate_mg_hr)
            }
        },
        _ => match state.controls.display_unit() {
            DisplayUnit::Volume => "Volume rate: unavailable",
            DisplayUnit::DrugMass => "Drug mass rate: unavailable",
        }
        .to_owned(),
    };
    set_text(document, "result-metrics", &metrics)?;
    let detail = match (&state.state, state.controls.result_detail()) {
        (FormState::Success(response), ResultDetail::Summary) => {
            format!(
                "Clinical summary for response {}",
                response.audit_sequence_id
            )
        }
        (FormState::Success(response), ResultDetail::Audit) => {
            format!("Audit detail: sequence {}", response.audit_sequence_id)
        }
        (_, ResultDetail::Summary) => "Clinical summary awaiting backend response".to_owned(),
        (_, ResultDetail::Audit) => "Audit detail unavailable until backend response".to_owned(),
    };
    set_text(document, "result-detail", &detail)?;
    element(document, "view-options")?.set_attribute(
        "data-result-scale-percent",
        &state.controls.scale().value().to_string(),
    )?;
    set_text(document, "result-state", &message)?;
    render_explorer(document, state)
}

fn render_explorer(document: &WebDocument, state: &BrowserState) -> io::Result<()> {
    let explorer = &state.result_explorer;
    render_explorer_summary(document, explorer)?;
    render_explorer_entries(document, explorer)?;
    render_explorer_pagination(document, explorer)
}

fn render_explorer_summary(
    document: &WebDocument,
    explorer: &metis_frontend::ResultExplorer,
) -> io::Result<()> {
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
    set_text(document, "explorer-status", &status)?;
    set_text(
        document,
        "explorer-caption",
        &format!(
            "Retained results grouped by patient; filter `{}`; order {} {}",
            explorer.filter(),
            explorer.sort().key().label(),
            explorer.sort().direction().value(),
        ),
    )?;
    let table = element(document, "explorer-table")?;
    table.set_attribute(
        "data-result-status",
        explorer_status_name(explorer.status()),
    )?;
    table.set_attribute(
        "aria-busy",
        if matches!(explorer.status(), ExplorerStatus::Loading) {
            "true"
        } else {
            "false"
        },
    )?;
    table.set_attribute("data-window-start", &explorer.window_start().to_string())?;
    Ok(())
}

fn render_explorer_entries(
    document: &WebDocument,
    explorer: &metis_frontend::ResultExplorer,
) -> io::Result<()> {
    for slot in 0..RESULT_PAGE_SIZE {
        let button = element(document, &format!("explorer-entry-{slot}"))?;
        let entry = explorer.visible_entry(slot);
        render_explorer_entry(&button, entry.as_ref(), explorer.selected_id())?;
    }
    Ok(())
}

fn render_explorer_entry(
    button: &WebElement,
    entry: Option<&VisibleEntry<'_>>,
    selected_id: Option<metis_frontend::ResultId>,
) -> io::Result<()> {
    match entry {
        Some(VisibleEntry::Group {
            id,
            label,
            expanded,
            row_count,
        }) => {
            let disclosure = if *expanded { "expanded" } else { "collapsed" };
            let text = format!("{label} — {row_count} result(s) — {disclosure}");
            button.set_text(&text);
            button.set_attribute("class", "explorer-entry explorer-group")?;
            button.set_attribute("aria-label", &text)?;
            button.set_attribute("aria-hidden", "false")?;
            button.set_attribute("aria-expanded", if *expanded { "true" } else { "false" })?;
            button.set_attribute("aria-level", "1")?;
            button.set_attribute("data-entry-kind", "group")?;
            button.set_attribute("data-group-id", &id.get().to_string())?;
            button.set_disabled(false)?;
        }
        Some(VisibleEntry::Row(row)) => {
            let selected = selected_id == Some(row.id());
            let selection = if selected { " — selected" } else { "" };
            let text = format!(
                "{} — sequence {} — {:.3} mL/hr — {:.3} mg/hr{selection}",
                row.patient_id(),
                row.id().get(),
                row.rate_ml_hr(),
                row.drug_rate_mg_hr(),
            );
            button.set_text(&text);
            button.set_attribute(
                "class",
                if selected {
                    "explorer-entry explorer-row explorer-row-selected"
                } else {
                    "explorer-entry explorer-row"
                },
            )?;
            button.set_attribute("aria-label", &text)?;
            button.set_attribute("aria-hidden", "false")?;
            button.set_attribute("aria-expanded", "false")?;
            button.set_attribute("aria-level", "2")?;
            button.set_attribute("aria-pressed", if selected { "true" } else { "false" })?;
            button.set_attribute("data-entry-kind", "row")?;
            button.set_attribute("data-result-id", &row.id().get().to_string())?;
            button.set_disabled(false)?;
        }
        None => {
            button.set_text("No visible result");
            button.set_attribute("class", "explorer-entry explorer-entry-empty")?;
            button.set_attribute("aria-label", "No visible result")?;
            button.set_attribute("aria-hidden", "true")?;
            button.set_attribute("aria-expanded", "false")?;
            button.set_attribute("aria-level", "1")?;
            button.set_attribute("aria-pressed", "false")?;
            button.set_attribute("data-entry-kind", "empty")?;
            button.set_attribute("data-group-id", "")?;
            button.set_attribute("data-result-id", "")?;
            button.set_disabled(true)?;
        }
    }
    Ok(())
}

fn render_explorer_pagination(
    document: &WebDocument,
    explorer: &metis_frontend::ResultExplorer,
) -> io::Result<()> {
    let window_status = if explorer.visible_count() == 0 {
        "Entries 0 of 0".to_owned()
    } else {
        let first = explorer.window_start() + 1;
        let last = (explorer.window_start() + RESULT_PAGE_SIZE).min(explorer.visible_count());
        format!(
            "Entries {first}–{last} of {}; {} retained",
            explorer.visible_count(),
            explorer.row_count()
        )
    };
    set_text(document, "explorer-window-status", &window_status)?;
    element(document, "explorer-previous")?.set_disabled(!explorer.can_previous())?;
    element(document, "explorer-next")?.set_disabled(!explorer.can_next())?;
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

fn render_drop(document: &WebDocument, state: &BrowserState) -> io::Result<()> {
    set_text(document, "drop-status", &state.drop_state.status_message())?;
    set_text(
        document,
        "drop-byte-status",
        &state.drop_read_state.status_message(),
    )?;
    let drop_zone = element(document, "drop-zone")?;
    drop_zone.set_attribute("data-drop-state", state.drop_state.state_name())?;
    drop_zone.set_attribute(
        "data-drop-count",
        &state.drop_state.file_count().to_string(),
    )?;
    drop_zone.set_attribute("data-byte-state", state.drop_read_state.state_name())
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

fn render_text(document: &WebDocument, state: &BrowserState) -> io::Result<()> {
    let text = &state.text_state;
    set_text(document, "text-status", &text.text_status())?;
    set_text(
        document,
        "text-preview",
        &format!("Text value preview: {}", text.value_display()),
    )?;
    set_text(document, "composition-status", &text.composition_status())?;
    set_text(document, "selection-status", &text.selection_status())?;
    let control = element(document, "text-specimen")?;
    let selection = text.selection();
    control.set_attribute("data-text-state", text.state_name())?;
    control.set_attribute("data-selection-start", &selection.start().to_string())?;
    control.set_attribute("data-selection-end", &selection.end().to_string())?;
    control.set_attribute("data-selection-direction", selection.direction().label())?;
    control.set_attribute(
        "data-composing",
        if matches!(text.composition(), CompositionState::Active) {
            "true"
        } else {
            "false"
        },
    )?;
    control.set_attribute("data-input-type", text.input_type())
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

pub(super) fn set_text(document: &WebDocument, id: &str, text: &str) -> io::Result<()> {
    element(document, id)?.set_text(text);
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
