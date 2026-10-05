//! The presentation document and the revision that says when it changed.

use metis_ui_lang::{ComputedStyle, DomDocument, Edit};
use std::ops::Deref;

/// A document whose every mutation goes through a method that moves its
/// revision only when the content changes.
///
/// Everything derived from the document, such as its semantic projection
/// and its layout, is valid for the revision it was derived at. The document
/// is readable through [`Deref`] and has no mutable access, so a derived
/// value cannot outlive an edit unnoticed.
pub(crate) struct TrackedDocument {
    document: DomDocument,
    revision: u64,
}

impl TrackedDocument {
    /// Tracks `document` from revision zero.
    pub(crate) const fn new(document: DomDocument) -> Self {
        Self {
            document,
            revision: 0,
        }
    }

    /// The document, for callers that need the plain type.
    pub(crate) const fn document(&self) -> &DomDocument {
        &self.document
    }

    /// Identifies the content: two reads with the same revision saw the same
    /// document.
    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    /// Sets the text of the element `id`; see [`DomDocument::set_text_content`].
    pub(crate) fn set_text(&mut self, id: &str, text: &str) -> Edit {
        let edit = self.document.set_text_content(id, text);
        self.record(edit)
    }

    /// Sets an attribute of the element `id`; see [`DomDocument::set_attribute`].
    pub(crate) fn set_attribute(&mut self, id: &str, key: &str, value: &str) -> Edit {
        let edit = self.document.set_attribute(id, key, value);
        self.record(edit)
    }

    /// Applies `update` to the style of the element `id`, keeping the result
    /// only when it differs from the style the element holds.
    pub(crate) fn restyle(&mut self, id: &str, update: impl FnOnce(&mut ComputedStyle)) -> Edit {
        let Some(element) = self.document.find_element_by_id_mut(id) else {
            return Edit::Missing;
        };
        let mut style = element.computed_style.clone();
        update(&mut style);
        if style == element.computed_style {
            return Edit::Unchanged;
        }
        element.computed_style = style;
        self.record(Edit::Changed)
    }

    fn record(&mut self, edit: Edit) -> Edit {
        if edit == Edit::Changed {
            self.revision += 1;
        }
        edit
    }
}

impl Deref for TrackedDocument {
    type Target = DomDocument;

    fn deref(&self) -> &DomDocument {
        &self.document
    }
}

#[cfg(test)]
mod tests {
    use super::TrackedDocument;
    use metis_ui_lang::{Color, DomElement, Edit, parse_markup};

    fn tracked() -> TrackedDocument {
        TrackedDocument::new(
            parse_markup("<a id='root' style='color: #102030'><b id='label'>old</b></a>")
                .expect("authored markup"),
        )
    }

    #[test]
    fn the_revision_moves_exactly_when_an_edit_changes_the_content() {
        let mut doc = tracked();
        assert_eq!(doc.revision(), 0);
        assert_eq!(doc.set_text("label", "old"), Edit::Unchanged);
        assert_eq!(doc.set_attribute("label", "id", "label"), Edit::Unchanged);
        assert_eq!(doc.set_text("absent", "x"), Edit::Missing);
        assert_eq!(doc.revision(), 0);

        assert_eq!(doc.set_text("label", "new"), Edit::Changed);
        assert_eq!(doc.revision(), 1);
        assert_eq!(doc.set_attribute("label", "tone", "calm"), Edit::Changed);
        assert_eq!(doc.revision(), 2);
        assert_eq!(
            doc.find_element_by_id("label")
                .map(DomElement::text_content),
            Some("new".to_owned())
        );
    }

    #[test]
    fn a_restyle_that_writes_the_held_value_leaves_the_revision() {
        let mut doc = tracked();
        let held = doc
            .find_element_by_id("label")
            .expect("label")
            .computed_style
            .text_color;
        assert_eq!(
            doc.restyle("label", |style| style.text_color = held),
            Edit::Unchanged
        );
        assert_eq!(
            doc.restyle("absent", |style| style.text_color = held),
            Edit::Missing
        );
        assert_eq!(doc.revision(), 0);

        let other = Color::rgb(1, 2, 3);
        assert_ne!(other, held);
        assert_eq!(
            doc.restyle("label", |style| style.text_color = other),
            Edit::Changed
        );
        assert_eq!(doc.revision(), 1);
        assert_eq!(
            doc.find_element_by_id("label")
                .expect("label")
                .computed_style
                .text_color,
            other
        );
    }
}
