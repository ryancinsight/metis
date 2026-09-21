//! Native accessibility projection for the authored Metis form.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::FrontendApp;
use metis_ipc::IpcTransport;
use metis_platform::native::{
    AccessibilityAction, AccessibilityNode, AccessibilityRole, AccessibilityTree,
};
use metis_ui_lang::{SemanticAction, SemanticNode, SemanticRole};
use std::collections::HashSet;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0001_0000_01b3;

/// Projects one rendered Metis document into the native accessibility contract.
pub(crate) fn project<T: IpcTransport>(app: &FrontendApp<T>) -> Result<AccessibilityTree> {
    let source = app.semantic_tree()?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve(source.element_count)
        .map_err(|_| projection_error("native accessibility node storage reservation failed"))?;
    let mut identities = HashSet::new();
    identities.try_reserve(source.element_count).map_err(|_| {
        projection_error("native accessibility identity storage reservation failed")
    })?;
    let mut path = Vec::new();
    let mut focus = None;
    let root = append_node(
        &source.root,
        &mut path,
        &mut identities,
        &mut nodes,
        &mut focus,
    )?;
    let focus = focus.unwrap_or(root);
    AccessibilityTree::from_nodes(root, focus, nodes)
        .map_err(|_| projection_error("native accessibility projection produced an invalid tree"))
}

/// Returns the stable identity used for the authored submit control.
pub(crate) fn submit_button_identity() -> u64 {
    explicit_identity("btn-calc")
}

/// Returns the stable identity used for the authored patient input.
pub(crate) fn patient_input_identity() -> u64 {
    explicit_identity("patient-input")
}

fn append_node(
    source: &SemanticNode,
    path: &mut Vec<usize>,
    identities: &mut HashSet<u64>,
    nodes: &mut Vec<AccessibilityNode>,
    focus: &mut Option<u64>,
) -> Result<u64> {
    let identity = node_identity(source, path);
    if identity == 0 || !identities.insert(identity) {
        return Err(projection_error(
            "native accessibility node identities must be unique and nonzero",
        ));
    }
    if focus.is_none() && source.focusable && !source.hidden && !source.disabled {
        *focus = Some(identity);
    }

    let mut node = AccessibilityNode::new(identity, role(source.role)?, source.name.clone())
        .map_err(|_| projection_error("native accessibility node name exceeds its bound"))?;
    node.set_description(source.description.clone())
        .map_err(|_| projection_error("native accessibility description exceeds its bound"))?;
    node.set_value(source.value.clone())
        .map_err(|_| projection_error("native accessibility value exceeds its bound"))?;
    node.set_hidden(source.hidden);
    node.set_disabled(source.disabled);
    node.set_focusable(source.focusable);
    node.set_expanded(source.expanded);
    node.set_selected(source.selected);
    node.set_checked(source.checked);
    for action in &source.actions {
        node.add_action(action_for(*action)?);
    }

    let mut children = Vec::new();
    children
        .try_reserve(source.children.len())
        .map_err(|_| projection_error("native accessibility child storage reservation failed"))?;
    for (index, child) in source.children.iter().enumerate() {
        path.push(index);
        let child_identity = append_node(child, path, identities, nodes, focus)?;
        path.truncate(path.len().saturating_sub(1));
        children.push(child_identity);
    }
    node.set_children(children)
        .map_err(|_| projection_error("native accessibility child count exceeds its bound"))?;
    nodes.push(node);
    Ok(identity)
}

fn node_identity(source: &SemanticNode, path: &[usize]) -> u64 {
    source
        .id
        .as_deref()
        .map_or_else(|| path_identity(path), explicit_identity)
}

fn explicit_identity(label: &str) -> u64 {
    hash_bytes(FNV_OFFSET, 0x69, label.as_bytes())
}

fn path_identity(path: &[usize]) -> u64 {
    let mut hash = FNV_OFFSET;
    hash = hash_byte(hash, 0x70);
    for index in path {
        hash = hash_bytes(hash, 0x2f, &index.to_le_bytes());
    }
    nonzero(hash)
}

fn hash_bytes(mut hash: u64, namespace: u8, bytes: &[u8]) -> u64 {
    hash = hash_byte(hash, namespace);
    let mut index = 0;
    while index < bytes.len() {
        hash = hash_byte(hash, bytes[index]);
        index += 1;
    }
    nonzero(hash)
}

fn hash_byte(hash: u64, byte: u8) -> u64 {
    hash.wrapping_mul(FNV_PRIME) ^ u64::from(byte)
}

fn nonzero(hash: u64) -> u64 {
    if hash == 0 { 1 } else { hash }
}

fn role(role: SemanticRole) -> Result<AccessibilityRole> {
    match role {
        SemanticRole::Application => Ok(AccessibilityRole::Application),
        SemanticRole::Main => Ok(AccessibilityRole::Main),
        SemanticRole::Group => Ok(AccessibilityRole::Group),
        SemanticRole::Button => Ok(AccessibilityRole::Button),
        SemanticRole::Text => Ok(AccessibilityRole::Text),
        SemanticRole::TextBox => Ok(AccessibilityRole::TextInput),
        SemanticRole::CheckBox => Ok(AccessibilityRole::CheckBox),
        SemanticRole::Radio => Ok(AccessibilityRole::Radio),
        SemanticRole::Slider => Ok(AccessibilityRole::Slider),
        SemanticRole::ComboBox => Ok(AccessibilityRole::ComboBox),
        SemanticRole::Dialog => Ok(AccessibilityRole::Dialog),
        SemanticRole::Status => Ok(AccessibilityRole::Status),
        SemanticRole::Table => Ok(AccessibilityRole::Table),
        _ => Err(unsupported_projection(
            "semantic role is not supported by the native provider",
        )),
    }
}

fn action_for(action: SemanticAction) -> Result<AccessibilityAction> {
    match action {
        SemanticAction::Activate => Ok(AccessibilityAction::Activate),
        SemanticAction::SetValue => Ok(AccessibilityAction::SetValue),
        SemanticAction::Toggle => Ok(AccessibilityAction::Toggle),
        SemanticAction::AdjustValue => Ok(AccessibilityAction::AdjustValue),
        SemanticAction::Open => Ok(AccessibilityAction::Open),
        _ => Err(unsupported_projection(
            "semantic action is not supported by the native provider",
        )),
    }
}

fn projection_error(message: &'static str) -> MetisError {
    MetisError::ui(ErrorCode::MalformedMarkup, message)
}

fn unsupported_projection(message: &'static str) -> MetisError {
    MetisError::ui(ErrorCode::UnsupportedPlatformEvent, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_frontend::FrontendApp;
    use metis_ipc::MemoryTransport;

    #[test]
    fn authored_form_projects_to_a_stable_native_tree() {
        let (transport, _peer) = MemoryTransport::pair();
        let app = FrontendApp::new(transport, 800, 600).expect("form");
        let first = project(&app).expect("native accessibility tree");
        let second = project(&app).expect("native accessibility tree");
        assert_eq!(first, second);
        assert_ne!(submit_button_identity(), 0);
        assert_ne!(patient_input_identity(), 0);
    }

    #[test]
    fn explicit_and_path_identities_are_distinct_and_deterministic() {
        let (transport, _peer) = MemoryTransport::pair();
        let app = FrontendApp::new(transport, 800, 600).expect("form");
        let tree = app.semantic_tree().expect("semantic tree");
        let explicit =
            find_identity(&tree.root, &mut Vec::new(), "btn-calc").expect("submit button");
        assert_eq!(explicit, submit_button_identity());
        let patient =
            find_identity(&tree.root, &mut Vec::new(), "patient-input").expect("patient input");
        assert_eq!(patient, patient_input_identity());
        assert_ne!(explicit, patient);
        assert_ne!(path_identity(&[0]), path_identity(&[1]));
    }

    fn find_identity(node: &SemanticNode, path: &mut Vec<usize>, target: &str) -> Option<u64> {
        if node.id.as_deref() == Some(target) {
            return Some(node_identity(node, path));
        }
        for (index, child) in node.children.iter().enumerate() {
            path.push(index);
            let identity = find_identity(child, path, target);
            path.truncate(path.len().saturating_sub(1));
            if identity.is_some() {
                return identity;
            }
        }
        None
    }
}
