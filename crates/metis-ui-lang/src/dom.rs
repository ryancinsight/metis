//! DOM Tree representation for declarative UI documents.

use crate::style::ComputedStyle;
use std::collections::HashMap;

/// An element node in the DOM.
#[derive(Debug, Clone, PartialEq)]
pub struct DomElement {
    /// Case-sensitive markup tag.
    pub tag: String,
    /// Attribute names and unescaped values.
    pub attributes: HashMap<String, String>,
    /// Ordered element and text children.
    pub children: Vec<DomNode>,
    /// Parsed inline style declarations.
    pub computed_style: ComputedStyle,
}

impl DomElement {
    /// Creates an empty element with default style.
    #[must_use]
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            attributes: HashMap::new(),
            children: Vec::new(),
            computed_style: ComputedStyle::default(),
        }
    }

    /// Returns the element's ID if present.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.attributes.get("id").map(String::as_str)
    }

    /// Returns the element's text content (aggregating child text nodes).
    #[must_use]
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        for child in &self.children {
            match child {
                DomNode::Text(t) => out.push_str(t),
                DomNode::Element(el) => out.push_str(&el.text_content()),
            }
        }
        out
    }
}

/// A node in the DOM tree.
#[derive(Debug, Clone, PartialEq)]
pub enum DomNode {
    /// Nested element, boxed so a text node does not reserve an element's
    /// computed style.
    Element(Box<DomElement>),
    /// Literal text content.
    Text(String),
}

/// What an edit addressed by element ID did to a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Edit {
    /// No element carries the ID; the document is unchanged.
    Missing,
    /// The element already held the value; the document is unchanged.
    Unchanged,
    /// The element now holds a different value.
    Changed,
}

/// A complete declarative UI document.
#[derive(Debug, Clone, PartialEq)]
pub struct DomDocument {
    /// Root element.
    pub root: DomElement,
}

impl DomDocument {
    /// Creates a document from an application-owned root.
    #[must_use]
    pub fn new(root: DomElement) -> Self {
        Self { root }
    }

    /// Finds an immutable reference to an element by ID.
    #[must_use]
    pub fn find_element_by_id(&self, id: &str) -> Option<&DomElement> {
        Self::find_rec_elem(&self.root, id)
    }

    /// Finds a mutable reference to an element by ID.
    pub fn find_element_by_id_mut(&mut self, id: &str) -> Option<&mut DomElement> {
        Self::find_rec_elem_mut(&mut self.root, id)
    }

    /// Sets the text content of the element with the specified ID.
    ///
    /// The element's children become one text node holding `text`; an
    /// element that already holds exactly that node is left untouched.
    pub fn set_text_content(&mut self, id: &str, text: &str) -> Edit {
        let Some(element) = self.find_element_by_id_mut(id) else {
            return Edit::Missing;
        };
        if let [DomNode::Text(current)] = element.children.as_mut_slice() {
            if current == text {
                return Edit::Unchanged;
            }
            current.clear();
            current.push_str(text);
        } else {
            element.children.clear();
            element.children.push(DomNode::Text(text.to_owned()));
        }
        Edit::Changed
    }

    /// Sets an attribute on the element with the specified ID.
    ///
    /// An attribute that already holds `value` is left untouched.
    pub fn set_attribute(&mut self, id: &str, key: &str, value: &str) -> Edit {
        let Some(element) = self.find_element_by_id_mut(id) else {
            return Edit::Missing;
        };
        match element.attributes.get_mut(key) {
            Some(current) if current == value => return Edit::Unchanged,
            Some(current) => {
                current.clear();
                current.push_str(value);
            }
            None => {
                element.attributes.insert(key.to_owned(), value.to_owned());
            }
        }
        Edit::Changed
    }

    fn find_rec_elem<'a>(el: &'a DomElement, id: &str) -> Option<&'a DomElement> {
        if el.id() == Some(id) {
            return Some(el);
        }
        for child in &el.children {
            if let DomNode::Element(child_el) = child
                && let Some(found) = Self::find_rec_elem(child_el, id)
            {
                return Some(found);
            }
        }
        None
    }

    fn find_rec_elem_mut<'a>(el: &'a mut DomElement, id: &str) -> Option<&'a mut DomElement> {
        if el.id() == Some(id) {
            return Some(el);
        }
        for child in &mut el.children {
            if let DomNode::Element(child_el) = child
                && let Some(found) = Self::find_rec_elem_mut(child_el, id)
            {
                return Some(found);
            }
        }
        None
    }
}

#[cfg(test)]
#[path = "dom_tests.rs"]
mod tests;
