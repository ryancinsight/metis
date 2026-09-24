use super::{MAX_SEMANTIC_TEXT_BYTES, SemanticAction, SemanticRole, SemanticTree};
use crate::parse_markup;
use metis_core::ErrorCode;

#[test]
fn derives_roles_names_states_and_actions() {
    let document = parse_markup(
        "<screen id='root'><text id='title'>Clinical study</text><button id='save' aria-labelledby='title'>Save</button><input id='patient' aria-describedby='title' value='PT-1' tabindex='0'/><checkbox id='ready' aria-checked='true'/></screen>",
    )
    .expect("document");
    let tree = SemanticTree::from_document(&document).expect("semantic tree");
    assert_eq!(tree.element_count, 5);
    assert_eq!(tree.root.role, SemanticRole::Application);
    assert_eq!(tree.root.children[1].name, "Clinical study");
    assert_eq!(
        tree.root.children[1].actions,
        vec![SemanticAction::Activate]
    );
    assert_eq!(
        tree.root.children[2].description.as_deref(),
        Some("Clinical study")
    );
    assert_eq!(tree.root.children[2].value.as_deref(), Some("PT-1"));
    assert!(tree.root.children[2].focusable);
    assert_eq!(tree.root.children[3].checked, Some(true));
    assert_eq!(tree.root.children[3].actions, vec![SemanticAction::Toggle]);
}

#[test]
fn derives_command_surface_landmarks_and_menu_actions() {
    let document = parse_markup(
        "<screen><nav aria-label='Application navigation'/><aside aria-label='Study sidebar'/><toolbar aria-label='Study toolbar'><button>Open study</button></toolbar><menu aria-label='Study commands'><menuitem id='open'>Open study</menuitem><button role='menuitem' disabled='true'>Close study</button></menu></screen>",
    )
    .expect("document");
    let tree = SemanticTree::from_document(&document).expect("semantic tree");
    assert_eq!(tree.root.children[0].role, SemanticRole::Navigation);
    assert_eq!(tree.root.children[1].role, SemanticRole::Complementary);
    assert_eq!(tree.root.children[2].role, SemanticRole::Toolbar);
    assert_eq!(
        tree.root.children[2].children[0].actions,
        vec![SemanticAction::Activate]
    );

    let menu = &tree.root.children[3];
    assert_eq!(menu.role, SemanticRole::Menu);
    assert_eq!(menu.children[0].role, SemanticRole::MenuItem);
    assert!(menu.children[0].focusable);
    assert_eq!(menu.children[0].actions, vec![SemanticAction::Activate]);
    assert!(!menu.children[1].focusable);
    assert!(menu.children[1].actions.is_empty());
}

#[test]
fn accepts_explicit_command_surface_roles() {
    let document = parse_markup(
        "<screen><section role='navigation'>Sections</section><section role='complementary'>Inspector</section><section role='toolbar'><button role='menuitem'>Reset</button></section><section role='menu'><span role='menuitem'>Open</span></section></screen>",
    )
    .expect("document");
    let tree = SemanticTree::from_document(&document).expect("semantic tree");
    assert_eq!(tree.root.children[0].role, SemanticRole::Navigation);
    assert_eq!(tree.root.children[1].role, SemanticRole::Complementary);
    assert_eq!(tree.root.children[2].role, SemanticRole::Toolbar);
    assert_eq!(
        tree.root.children[2].children[0].actions,
        vec![SemanticAction::Activate]
    );
    assert_eq!(tree.root.children[3].role, SemanticRole::Menu);
    assert!(tree.root.children[3].children[0].focusable);
}

#[test]
fn rejects_duplicate_or_unresolved_identity_and_malformed_state() {
    for (markup, code) in [
        (
            "<screen><text id='same'/><button id='same'/></screen>",
            ErrorCode::MalformedMarkup,
        ),
        (
            "<screen><button aria-labelledby='missing'>x</button></screen>",
            ErrorCode::MalformedMarkup,
        ),
        (
            "<screen><checkbox aria-checked='maybe'/></screen>",
            ErrorCode::MalformedMarkup,
        ),
        (
            "<screen><button role='unsupported'>x</button></screen>",
            ErrorCode::MalformedMarkup,
        ),
    ] {
        let document = parse_markup(markup).expect("parser accepts source");
        assert_eq!(
            SemanticTree::from_document(&document)
                .expect_err("semantic validation must reject")
                .code,
            code
        );
    }
}

#[test]
fn hidden_and_disabled_nodes_have_no_actions_or_focus() {
    let document = parse_markup(
        "<screen><button id='hidden' hidden='false'>Hidden</button><button id='disabled' disabled='false'>Disabled</button></screen>",
    )
    .expect("document");
    let tree = SemanticTree::from_document(&document).expect("semantic tree");
    for node in &tree.root.children {
        assert!(!node.focusable);
        assert!(node.actions.is_empty());
    }
}

#[test]
fn hidden_ancestors_hide_descendant_actions_and_focusability() {
    let document = parse_markup(
        "<screen><group id='html-hidden' hidden='true'><button id='nested-html'>Hidden</button></group><group id='aria-hidden' aria-hidden='true'><button id='nested-aria' aria-hidden='false'>Hidden</button></group><button id='visible'>Visible</button></screen>",
    )
    .expect("document");
    let tree = SemanticTree::from_document(&document).expect("semantic tree");

    let html_group = &tree.root.children[0];
    let aria_group = &tree.root.children[1];
    let visible = &tree.root.children[2];
    for child in [&html_group.children[0], &aria_group.children[0]] {
        assert!(child.hidden);
        assert!(!child.focusable);
        assert!(child.actions.is_empty());
    }
    assert!(html_group.hidden);
    assert!(aria_group.hidden);
    assert!(visible.focusable);
    assert_eq!(visible.actions, vec![SemanticAction::Activate]);
}

#[test]
fn bounds_text_and_tabindex_before_host_projection() {
    let long_text = "x".repeat(MAX_SEMANTIC_TEXT_BYTES + 1);
    let document = parse_markup(&format!("<screen><button>{long_text}</button></screen>"))
        .expect("parser accepts bounded source");
    assert_eq!(
        SemanticTree::from_document(&document)
            .expect_err("oversized semantic name")
            .code,
        ErrorCode::LayoutOverflow
    );

    let document = parse_markup("<screen><button tabindex='later'>Open</button></screen>")
        .expect("parser accepts source");
    assert_eq!(
        SemanticTree::from_document(&document)
            .expect_err("malformed tabindex")
            .code,
        ErrorCode::MalformedMarkup
    );
}

#[test]
fn announces_normalized_keyboard_shortcuts() {
    let document = parse_markup(
        "<screen><button id='save' aria-keyshortcuts='  Control+S   Meta+S '>Save</button><button id='plain'>Plain</button></screen>",
    )
    .expect("document");
    let tree = SemanticTree::from_document(&document).expect("semantic tree");
    assert_eq!(
        tree.root.children[0].keyboard_shortcuts.as_deref(),
        Some("Control+S Meta+S")
    );
    assert_eq!(tree.root.children[1].keyboard_shortcuts, None);
}
