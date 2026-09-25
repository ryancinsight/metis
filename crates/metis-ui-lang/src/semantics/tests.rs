use super::{MAX_SEMANTIC_TEXT_BYTES, SemanticAction, SemanticNode, SemanticRole, SemanticTree};
use crate::dom::{DomElement, DomNode};
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

/// A text node or element subtree as the name computation reads it: each
/// text node's whitespace runs collapsed and trimmed, parts joined by one
/// space. Written directly from that definition, per element, as the oracle
/// for the indexed contents.
fn reference_content(element: &DomElement) -> String {
    let parts = element.children.iter().map(|child| match child {
        DomNode::Text(text) => text.split_whitespace().collect::<Vec<_>>().join(" "),
        DomNode::Element(child) => reference_content(child),
    });
    parts
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn reference_references(value: &str, root: &DomElement) -> String {
    let parts = value.split_whitespace().map(|id| {
        let target = find(root, id).expect("reference resolves");
        reference_content(target)
    });
    parts
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn reference_name(element: &DomElement, root: &DomElement) -> String {
    if let Some(label) = element.attributes.get("aria-label") {
        return label.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    element.attributes.get("aria-labelledby").map_or_else(
        || reference_content(element),
        |references| reference_references(references, root),
    )
}

fn find<'a>(element: &'a DomElement, id: &str) -> Option<&'a DomElement> {
    if element.id() == Some(id) {
        return Some(element);
    }
    element.children.iter().find_map(|child| match child {
        DomNode::Element(child) => find(child, id),
        DomNode::Text(_) => None,
    })
}

/// Checks every node's name and referenced description against the oracle,
/// walking the document and the tree together.
fn assert_names(element: &DomElement, node: &SemanticNode, root: &DomElement) -> usize {
    assert_eq!(
        node.name,
        reference_name(element, root),
        "{:?}",
        element.id()
    );
    if let Some(references) = element.attributes.get("aria-describedby") {
        assert_eq!(
            node.description.as_deref(),
            Some(reference_references(references, root).as_str())
        );
    }
    let children = element.children.iter().filter_map(|child| match child {
        DomNode::Element(child) => Some(child),
        DomNode::Text(_) => None,
    });
    1 + children
        .zip(&node.children)
        .map(|(child, node)| assert_names(child, node, root))
        .sum::<usize>()
}

#[test]
fn names_equal_the_text_content_of_their_subtree_or_references() {
    let documents = [
        "<screen id='root'>  Lead \n\t text <group id='a'> one <text id='b'>two  <label id='c'>\u{2003}three\u{a0}four </label></text>  </group> tail </screen>",
        "<screen><text id='t1'>First</text><text id='t2'>  </text><text id='t3'>Third  part</text><button id='go' aria-labelledby='t3 t2  t1'>ignored</button><input id='in' aria-describedby='t1 t3' aria-label='  Entry   field '/></screen>",
        "<screen><group><group><group><text>deep</text></group> mid </group><text></text></group><group aria-label='labelled'><text>unused</text></group></screen>",
    ];
    for markup in documents {
        let document = parse_markup(markup).expect("document");
        let tree = SemanticTree::from_document(&document).expect("semantic tree");
        let checked = assert_names(&document.root, &tree.root, &document.root);
        assert_eq!(checked, tree.element_count);
    }
}

#[test]
fn over_long_content_fails_only_where_a_name_reads_it() {
    // Each text node is within its own limit; the two joined are not.
    let long = "word ".repeat(600);
    assert!(long.len() < MAX_SEMANTIC_TEXT_BYTES && 2 * long.len() > MAX_SEMANTIC_TEXT_BYTES);
    let named = format!(
        "<screen aria-label='Form'><group aria-label='Log'><text>{long}</text><text>{long}</text></group></screen>"
    );
    let tree = SemanticTree::from_document(&parse_markup(&named).expect("document"))
        .expect("labels replace the contents a name would read");
    assert_eq!(tree.root.children[0].name, "Log");
    let unnamed = format!(
        "<screen aria-label='Form'><group><text>{long}</text><text>{long}</text></group></screen>"
    );
    assert_eq!(
        SemanticTree::from_document(&parse_markup(&unnamed).expect("document"))
            .expect_err("the group's name reads both texts")
            .code,
        ErrorCode::LayoutOverflow
    );
}
