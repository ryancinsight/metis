//! One headless form submission over inherited pipes; stdout is wire bytes only.
use crate::invocation::WebViewTheme;
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::{MemoryTransport, transport::StreamTransport};
use metis_ui_lang::{SemanticAction, SemanticNode, SemanticRole, SemanticTree};
use serde::Serialize;
use std::{
    fs,
    io::{stdin, stdout},
    path::Path,
};

#[cfg(windows)]
mod native;
#[cfg(windows)]
mod native_accessibility;
#[cfg(windows)]
mod webview;

/// Runs the visible native host on supported desktop targets.
pub(crate) fn run_native(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        native::run(inputs)
    }
    #[cfg(not(windows))]
    {
        drop(inputs);
        Err("the native frontend role requires Windows".into())
    }
}

/// Runs the visible Windows `WebView2` host over the supervised pipe.
pub(crate) fn run_webview(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        webview::run(inputs)
    }
    #[cfg(not(windows))]
    {
        drop(inputs);
        Err("the WebView2 frontend role requires Windows".into())
    }
}

/// Runs the visible Windows `WebView2` permission-denial probe.
pub(crate) fn run_webview_permission_probe(
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        webview::run_permission_probe(inputs)
    }
    #[cfg(not(windows))]
    {
        drop(inputs);
        Err("the WebView2 permission-probe frontend requires Windows".into())
    }
}

/// Runs the visible Windows `WebView2` permission probe and saves its page.
pub(crate) fn run_webview_permission_probe_capture(
    output: &Path,
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        webview::run_permission_probe_capture(inputs, output)
    }
    #[cfg(not(windows))]
    {
        let _ = (output, inputs);
        Err("the WebView2 permission-probe capture frontend requires Windows".into())
    }
}

/// Runs the visible `WebView2` form and captures one bounded presentation mode.
pub(crate) fn run_webview_theme_capture(
    output: &Path,
    theme: WebViewTheme,
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        webview::run_theme_capture(inputs, output, theme)
    }
    #[cfg(not(windows))]
    {
        let _ = (output, theme, inputs);
        Err("the WebView2 theme capture frontend requires Windows".into())
    }
}

/// Writes the bounded host-neutral semantic projection for the authored form.
///
/// The capture is a native executable artifact for accessibility consumers;
/// it does not claim a screen-reader or operating-system bridge. A memory
/// transport keeps this probe independent of a backend process while the
/// presentation still runs through the production `FrontendApp` renderer.
pub(crate) fn run_semantic_capture(
    output: &Path,
    inputs: [String; 3],
) -> Result<(), Box<dyn std::error::Error>> {
    let [weight, concentration, dose] = inputs;
    let (transport, _peer) = MemoryTransport::pair();
    let mut app = FrontendApp::new(transport, 800, 600)?;
    app.set_inputs(
        "demo",
        weight.parse()?,
        concentration.parse()?,
        dose.parse()?,
    )?;
    let tree = app.semantic_tree()?;
    let capture = SemanticCapture::try_from(&tree)?;
    let bytes = serde_json::to_vec_pretty(&capture)?;
    fs::write(output, &bytes)?;
    eprintln!(
        "semantic_capture_path={} semantic_capture_bytes={} semantic_elements={}",
        output.display(),
        bytes.len(),
        tree.element_count
    );
    Ok(())
}

const SEMANTIC_CAPTURE_SCHEMA: u8 = 1;

#[derive(Debug, Serialize)]
struct SemanticCapture {
    schema: u8,
    element_count: usize,
    root: SemanticCaptureNode,
}

impl TryFrom<&SemanticTree> for SemanticCapture {
    type Error = std::io::Error;

    fn try_from(tree: &SemanticTree) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: SEMANTIC_CAPTURE_SCHEMA,
            element_count: tree.element_count,
            root: SemanticCaptureNode::try_from(&tree.root)?,
        })
    }
}

#[derive(Debug, Serialize)]
struct SemanticCaptureNode {
    id: Option<String>,
    role: &'static str,
    name: String,
    description: Option<String>,
    value: Option<String>,
    disabled: bool,
    hidden: bool,
    expanded: Option<bool>,
    selected: Option<bool>,
    checked: Option<bool>,
    focusable: bool,
    actions: Vec<&'static str>,
    children: Vec<Self>,
}

impl TryFrom<&SemanticNode> for SemanticCaptureNode {
    type Error = std::io::Error;

    fn try_from(node: &SemanticNode) -> Result<Self, Self::Error> {
        let mut actions = Vec::new();
        actions
            .try_reserve(node.actions.len())
            .map_err(|_| std::io::Error::other("semantic action capture allocation failed"))?;
        actions.extend(node.actions.iter().copied().map(semantic_action_name));

        let mut children = Vec::new();
        children
            .try_reserve(node.children.len())
            .map_err(|_| std::io::Error::other("semantic child capture allocation failed"))?;
        for child in &node.children {
            children.push(Self::try_from(child)?);
        }

        Ok(Self {
            id: node.id.clone(),
            role: semantic_role_name(node.role),
            name: node.name.clone(),
            description: node.description.clone(),
            value: node.value.clone(),
            disabled: node.disabled,
            hidden: node.hidden,
            expanded: node.expanded,
            selected: node.selected,
            checked: node.checked,
            focusable: node.focusable,
            actions,
            children,
        })
    }
}

fn semantic_role_name(role: SemanticRole) -> &'static str {
    match role {
        SemanticRole::Application => "application",
        SemanticRole::Main => "main",
        SemanticRole::Navigation => "navigation",
        SemanticRole::Complementary => "complementary",
        SemanticRole::Group => "group",
        SemanticRole::Toolbar => "toolbar",
        SemanticRole::Menu => "menu",
        SemanticRole::MenuItem => "menuitem",
        SemanticRole::Button => "button",
        SemanticRole::Text => "text",
        SemanticRole::TextBox => "textbox",
        SemanticRole::CheckBox => "checkbox",
        SemanticRole::Radio => "radio",
        SemanticRole::Slider => "slider",
        SemanticRole::ComboBox => "combobox",
        SemanticRole::Dialog => "dialog",
        SemanticRole::Status => "status",
        SemanticRole::Table => "table",
        _ => "unknown",
    }
}

fn semantic_action_name(action: SemanticAction) -> &'static str {
    match action {
        SemanticAction::Activate => "activate",
        SemanticAction::SetValue => "set_value",
        SemanticAction::Toggle => "toggle",
        SemanticAction::AdjustValue => "adjust_value",
        SemanticAction::Open => "open",
        _ => "unknown",
    }
}

pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    // Binary error reporter is the non-hot type-erasure boundary.
    let [weight, concentration, dose] = inputs;
    let transport = StreamTransport::new(stdin(), stdout());
    let mut app = FrontendApp::new(transport, 800, 600)?;
    let pid = std::process::id();
    let mut principal_id = [0; 16];
    principal_id[..4].copy_from_slice(&pid.to_be_bytes());
    app.init(pid, principal_id)?;
    app.set_inputs(
        "demo",
        weight.parse()?,
        concentration.parse()?,
        dose.parse()?,
    )?;
    app.submit_calculation()?;
    match app.state() {
        FormState::Success(response) => eprintln!(
            "frontend_pid={pid} rate_ml_hr={} drug_rate_mg_hr={} audit_sequence={}",
            response.rate_ml_hr, response.drug_rate_mg_hr, response.audit_sequence_id
        ),
        FormState::Rejected(error) => {
            return Err(format!(
                "Backend rejected request [0x{:04X}]: {}",
                error.error_code, error.message
            )
            .into());
        }
        state => return Err(format!("Submission completed without a result: {state:?}").into()),
    }
    Ok(())
}
