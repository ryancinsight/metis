//! A refreshed projection must equal the fresh one.
//!
//! Every case here edits a document, refreshes the projection built before the
//! edit, and compares it with a projection built from scratch afterwards. The
//! fresh projection is the oracle: reuse is an optimization, so any difference
//! is a defect in it.

use super::SemanticTree;
use crate::dom::{DomElement, DomNode};
use crate::parse_markup;

const FORM: &str = "<screen id='root'><text id='title'>Clinical study</text><text id='patient'>Patient: PT-1</text><button id='save' aria-labelledby='title'>Save</button><input id='note' aria-describedby='patient' value='none' tabindex='0'/></screen>";

fn projected() -> (crate::dom::DomDocument, SemanticTree) {
    let document = parse_markup(FORM).expect("authored markup");
    let tree = SemanticTree::from_document(&document).expect("semantic tree");
    (document, tree)
}

/// Refreshes after `edit` and asserts the result equals a fresh projection.
fn assert_refresh_matches(
    document: &mut crate::dom::DomDocument,
    tree: SemanticTree,
    changed: &[&str],
) -> SemanticTree {
    let changed: Vec<String> = changed.iter().map(|id| (*id).to_owned()).collect();
    let refreshed = tree
        .refresh(document, &changed)
        .expect("refreshed projection");
    let fresh = SemanticTree::from_document(document).expect("fresh projection");
    assert_eq!(
        refreshed, fresh,
        "refresh diverged from the fresh projection"
    );
    refreshed
}

#[test]
fn an_edited_label_renames_its_ancestors() {
    let (mut document, tree) = projected();
    assert_eq!(
        document.set_text_content("patient", "Patient: PT-2"),
        crate::Edit::Changed
    );
    let tree = assert_refresh_matches(&mut document, tree, &["patient"]);
    assert_eq!(tree.root.children[1].name, "Patient: PT-2");
    // The root aggregates every descendant's content, so its name moved too.
    assert_eq!(tree.root.name, "Clinical study Patient: PT-2 Save");
}

#[test]
fn an_edit_a_reference_resolves_through_renames_the_referrer() {
    let (mut document, tree) = projected();
    assert_eq!(
        document.set_text_content("title", "Study A"),
        crate::Edit::Changed
    );
    let tree = assert_refresh_matches(&mut document, tree, &["title"]);
    assert_eq!(tree.root.children[2].name, "Study A");
    assert_eq!(
        tree.root.children[3].description.as_deref(),
        Some("Patient: PT-1")
    );
}

#[test]
fn hiding_a_container_hides_its_subtree() {
    let (mut document, tree) = projected();
    assert_eq!(
        document.set_attribute("root", "aria-hidden", "true"),
        crate::Edit::Changed
    );
    let tree = assert_refresh_matches(&mut document, tree, &["root"]);
    assert!(tree.root.hidden);
    assert!(
        !tree.root.children[3].focusable,
        "a hidden subtree is not focusable"
    );
}

#[test]
fn an_edit_that_changes_no_content_leaves_the_projection() {
    let (mut document, tree) = projected();
    assert_eq!(
        document.set_text_content("patient", "Patient: PT-1"),
        crate::Edit::Unchanged
    );
    let before = tree.clone();
    let tree = assert_refresh_matches(&mut document, tree, &[]);
    assert_eq!(tree, before);
}

#[test]
fn an_edit_of_an_identifier_the_document_does_not_hold_leaves_the_projection() {
    let (mut document, tree) = projected();
    let before = tree.clone();
    let tree = assert_refresh_matches(&mut document, tree, &["absent"]);
    assert_eq!(tree, before);
}

#[test]
fn a_duplicate_identity_an_edit_introduces_is_rejected() {
    let (mut document, tree) = projected();
    assert_eq!(
        document.set_attribute("patient", "id", "title"),
        crate::Edit::Changed
    );
    let changed = vec!["patient".to_owned()];
    assert!(tree.refresh(&document, &changed).is_err());
    assert!(SemanticTree::from_document(&document).is_err());
}

#[test]
fn an_unresolved_reference_an_edit_introduces_is_rejected() {
    let (mut document, tree) = projected();
    assert_eq!(
        document.set_attribute("save", "aria-labelledby", "absent"),
        crate::Edit::Changed
    );
    let changed = vec!["save".to_owned()];
    assert!(tree.refresh(&document, &changed).is_err());
    assert!(SemanticTree::from_document(&document).is_err());
}

#[test]
fn a_role_outside_the_vocabulary_an_edit_introduces_is_rejected() {
    let (mut document, tree) = projected();
    assert_eq!(
        document.set_attribute("save", "role", "marquee"),
        crate::Edit::Changed
    );
    let changed = vec!["save".to_owned()];
    assert!(tree.refresh(&document, &changed).is_err());
    assert!(SemanticTree::from_document(&document).is_err());
}

/// The tracked document cannot change the tree's shape, but the DOM is public,
/// so a shape change must not be served from the previous projection.
#[test]
fn a_document_that_changed_shape_is_projected_from_scratch() {
    let (mut document, tree) = projected();
    document
        .root
        .children
        .push(DomNode::Element(Box::new(DomElement::new("text"))));
    let tree = assert_refresh_matches(&mut document, tree, &["patient"]);
    assert_eq!(tree.element_count, 6);
}
