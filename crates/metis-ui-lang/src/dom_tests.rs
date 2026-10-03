use super::{DomDocument, Edit};
use crate::parse_markup;

fn document() -> DomDocument {
    parse_markup("<a id='root'><b id='label' tone='calm'>old <i>text</i></b></a>")
        .expect("authored markup")
}

#[test]
fn text_replaces_the_children_and_reports_the_change() {
    let mut doc = document();
    assert_eq!(doc.set_text_content("label", "new"), Edit::Changed);
    let label = doc.find_element_by_id("label").expect("label");
    assert_eq!(label.children.len(), 1);
    assert_eq!(label.text_content(), "new");
}

#[test]
fn text_that_is_already_present_leaves_the_document_equal() {
    let mut doc = document();
    assert_eq!(doc.set_text_content("label", "new"), Edit::Changed);
    let before = doc.clone();
    assert_eq!(doc.set_text_content("label", "new"), Edit::Unchanged);
    assert_eq!(doc, before);
    assert_eq!(doc.set_text_content("label", "newer"), Edit::Changed);
    assert_eq!(
        doc.find_element_by_id("label")
            .expect("label")
            .text_content(),
        "newer"
    );
}

#[test]
fn an_unknown_id_is_missing_for_both_edits() {
    let mut doc = document();
    let before = doc.clone();
    assert_eq!(doc.set_text_content("absent", "x"), Edit::Missing);
    assert_eq!(doc.set_attribute("absent", "tone", "x"), Edit::Missing);
    assert_eq!(doc, before);
}

#[test]
fn attributes_are_inserted_replaced_and_left_alone_when_equal() {
    let mut doc = document();
    assert_eq!(doc.set_attribute("label", "tone", "calm"), Edit::Unchanged);
    assert_eq!(doc.set_attribute("label", "tone", "loud"), Edit::Changed);
    assert_eq!(doc.set_attribute("label", "mood", "x"), Edit::Changed);
    let label = doc.find_element_by_id("label").expect("label");
    assert_eq!(
        label.attributes.get("tone").map(String::as_str),
        Some("loud")
    );
    assert_eq!(label.attributes.get("mood").map(String::as_str), Some("x"));
    assert_eq!(doc.set_attribute("label", "mood", "x"), Edit::Unchanged);
}
