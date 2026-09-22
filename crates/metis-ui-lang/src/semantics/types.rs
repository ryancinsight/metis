//! Public semantic roles, actions and tree values.

/// Maximum UTF-8 bytes retained for one semantic name, description or value.
pub const MAX_SEMANTIC_TEXT_BYTES: usize = 4_096;

/// Maximum UTF-8 bytes retained for one semantic identifier.
pub const MAX_SEMANTIC_ID_BYTES: usize = 256;

/// Role exposed by the custom renderer's semantic tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SemanticRole {
    /// The application root.
    Application,
    /// A primary document region.
    Main,
    /// A navigation landmark containing links or commands.
    Navigation,
    /// A complementary region such as a collapsible sidebar.
    Complementary,
    /// A generic grouping container.
    Group,
    /// A command toolbar or title bar.
    Toolbar,
    /// A popup command menu.
    Menu,
    /// An actionable command inside a menu.
    MenuItem,
    /// An actionable button.
    Button,
    /// Static text or a text label.
    Text,
    /// An editable text control.
    TextBox,
    /// A binary choice control.
    CheckBox,
    /// A mutually exclusive choice control.
    Radio,
    /// A bounded numeric value control.
    Slider,
    /// A list or combobox selection control.
    ComboBox,
    /// A modal or non-modal dialog surface.
    Dialog,
    /// A status or live-region message.
    Status,
    /// A table or grid surface.
    Table,
}

impl SemanticRole {
    pub(super) const fn is_interactive(self) -> bool {
        matches!(
            self,
            Self::MenuItem
                | Self::Button
                | Self::TextBox
                | Self::CheckBox
                | Self::Radio
                | Self::Slider
                | Self::ComboBox
        )
    }
}

/// Action a host may expose for one semantic node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SemanticAction {
    /// Activate a button or command.
    Activate,
    /// Replace the value of an editable control.
    SetValue,
    /// Toggle a checkbox or radio control.
    Toggle,
    /// Adjust a bounded numeric value.
    AdjustValue,
    /// Open a selection control.
    Open,
}

/// One node in the bounded semantic tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticNode {
    /// Explicit application identity, when the source element has an `id`.
    pub id: Option<String>,
    /// Role selected from the admitted renderer vocabulary.
    pub role: SemanticRole,
    /// Accessible name after `aria-label`, `aria-labelledby` or text fallback.
    pub name: String,
    /// Optional description resolved from `aria-description` or references.
    pub description: Option<String>,
    /// Current value for a value-bearing control.
    pub value: Option<String>,
    /// Whether the node is disabled for host actions.
    pub disabled: bool,
    /// Whether the node is hidden from host interaction.
    pub hidden: bool,
    /// Whether a disclosure or dialog is expanded.
    pub expanded: Option<bool>,
    /// Whether the node is selected.
    pub selected: Option<bool>,
    /// Whether a checkable node is checked.
    pub checked: Option<bool>,
    /// Whether keyboard focus may land on this node.
    pub focusable: bool,
    /// Actions admitted for this role and state.
    pub actions: Vec<SemanticAction>,
    /// Ordered child elements.
    pub children: Vec<SemanticNode>,
}

/// The complete semantic projection of one declarative document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTree {
    /// Semantic root corresponding to the source document's root element.
    pub root: SemanticNode,
    /// Number of element nodes represented by the tree.
    pub element_count: usize,
}
