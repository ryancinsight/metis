use super::{BridgeStatus, BrowserState};
use crate::controls::DisplayUnit;
use metis_core::CapabilityScope;
use metis_core::protocol::{
    CapabilityCatalogPayload, MAX_PLUGINS, Plugin, PluginDescriptor, PluginOperation,
    PluginRegistry, TargetCapabilityPayload,
};
use metis_frontend::FormState;
use metis_ipc::client::HandshakeError;
use moirai_pal::wasm::{WebDocument, WebElement};
use std::io;

pub(super) fn render(document: &WebDocument, state: &BrowserState) -> io::Result<()> {
    let inputs = &state.inputs;
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
    set_text(document, "metis-status", &message)?;
    set_text(document, "metis-capabilities", &state.capabilities)?;
    set_text(document, "metis-plugins", &state.plugins)?;
    let event_status = if state.controls.show_events() {
        state.event_status.as_str()
    } else {
        "Remote events: hidden by preference"
    };
    set_text(document, "metis-events", event_status)?;
    set_text(document, "options-state", &state.controls.summary())?;
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
    element(document, "view-options")?.set_attribute(
        "data-result-scale-percent",
        &state.controls.scale().value().to_string(),
    )?;
    set_text(document, "result-state", &message)
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
