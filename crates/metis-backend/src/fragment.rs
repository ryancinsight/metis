//! Permission-scoped browser fragment actions.

use crate::plugins::PluginExecutor;
use metis_core::capability::CapabilityScope;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    FragmentAction, FragmentPatch, FragmentPatchSet, Plugin, PluginDescriptor, PluginOperation,
};

static COMMANDS: [PluginOperation; 1] =
    [PluginOperation::new("action", CapabilityScope::UI_RENDER)];

/// Built-in UI plugin for generation-bound, text-safe browser actions.
///
/// The plugin owns action semantics and returns typed patch data. The browser
/// host remains responsible for its target and attribute allowlists.
pub struct UiFragmentPlugin;

impl Plugin for UiFragmentPlugin {
    const DESCRIPTOR: PluginDescriptor = PluginDescriptor::new("ui", 1, &COMMANDS, &[]);
}

impl PluginExecutor for UiFragmentPlugin {
    fn invoke(&mut self, operation_name: &str, body: &[u8]) -> Result<Vec<u8>> {
        if operation_name != "action" {
            return Err(MetisError::capability(
                ErrorCode::PluginOperationNotFound,
                "UI plugin received an undeclared operation",
            ));
        }
        let action = FragmentAction::decode(body)?;
        if action.action() != "status.describe" {
            return Err(MetisError::protocol(
                ErrorCode::MalformedPayload,
                "UI plugin received an unsupported action",
            ));
        }
        let value = format!(
            "Fragment action {} accepted: {}",
            action.action(),
            action.input()
        );
        FragmentPatchSet::new(
            action.generation(),
            vec![FragmentPatch::set_text(action.target(), value)?],
        )?
        .encode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_plugin_returns_input_sensitive_generation_bound_patch() {
        let action =
            FragmentAction::new(8, "status.describe", "metis-events", "session").expect("action");
        let body = UiFragmentPlugin
            .invoke("action", &action.encode().expect("encode"))
            .expect("response");
        let response = FragmentPatchSet::decode(&body).expect("patch set");
        assert_eq!(response.generation(), 8);
        assert_eq!(response.patches().len(), 1);
        assert_eq!(
            response.patches()[0],
            FragmentPatch::set_text(
                "metis-events",
                "Fragment action status.describe accepted: session"
            )
            .expect("patch")
        );
    }

    #[test]
    fn ui_plugin_rejects_unknown_action_without_markup_execution() {
        let action =
            FragmentAction::new(1, "status.other", "metis-events", "input").expect("action");
        let error = UiFragmentPlugin
            .invoke("action", &action.encode().expect("encode"))
            .expect_err("unknown action");
        assert_eq!(error.code, ErrorCode::MalformedPayload);
    }
}
