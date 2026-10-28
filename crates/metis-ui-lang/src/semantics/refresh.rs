//! Rebuilding the semantic projection after an edit.
//!
//! A projection is a pure function of the document, so the previous projection
//! answers most of it: an edit changes one element, the ancestors whose content
//! aggregates it, and any element whose name references it. Everything else is
//! reused, so an edit costs memory in proportion to what it touched rather than
//! to the form it did not.
//!
//! Reuse is only sound while the tree's shape is unchanged, because a node does
//! not record the structure it came from. [`SemanticTree::refresh`] therefore
//! counts the document's elements and compares the count with the projection's,
//! and it compares each reused node's child count with the element's. A document
//! that changed shape falls back to [`SemanticTree::from_document`].

use super::{
    MAX_SEMANTIC_TEXT_BYTES, SemanticNode, SemanticTree, actions_for, append_normalized,
    boolean_attribute, bounded_id, limit_error, markup_error, normalize_text, optional_boolean,
    role_for,
};
use crate::dom::{DomDocument, DomElement, DomNode};
use crate::parser::{MAX_DEPTH, MAX_NODES};
use metis_core::error::Result;
use std::collections::HashSet;

impl SemanticTree {
    /// Rebuilds the projection after `changed` elements were edited.
    ///
    /// `changed` holds the authored identifiers whose text or attributes the
    /// caller changed since this projection was built. The result is the
    /// projection [`Self::from_document`] returns for `document`; the previous
    /// tree supplies every node the edit could not have affected.
    ///
    /// # Errors
    /// Returns every error [`Self::from_document`] returns, including the
    /// duplicate-identity, unresolved-reference and bound errors an edit can
    /// introduce.
    #[must_use = "the refreshed projection replaces the one it was called on"]
    pub fn refresh(self, document: &DomDocument, changed: &[String]) -> Result<Self> {
        if changed.is_empty() {
            return Ok(self);
        }
        let mut scan = Scan::default();
        let mut path = Vec::new();
        scan.element(&document.root, 1, &mut path, changed)?;
        // A document that changed shape is not described by the previous
        // projection's structure, so the reuse below would attach nodes to
        // elements they did not come from.
        if scan.elements != self.element_count {
            return Self::from_document(document);
        }
        scan.references(&document.root, &mut path, changed)?;
        if scan.invalid.is_empty() {
            return Ok(self);
        }
        let Self {
            root,
            element_count,
        } = self;
        let root = rebuild(&document.root, Some(root), &scan, &document.root, 1, false)?;
        Ok(Self {
            root,
            element_count,
        })
    }
}

/// What one edit invalidates, and what the document now holds.
#[derive(Default)]
struct Scan<'a> {
    /// Elements whose projection the edit can change: the edited elements, the
    /// elements naming them, and the ancestors whose content aggregates both.
    invalid: HashSet<*const DomElement>,
    /// Every authored identifier, for uniqueness and reference resolution.
    ids: HashSet<&'a str>,
    /// Elements the document holds, compared with the projection's count.
    elements: usize,
    /// Elements and text nodes the document holds, bounded like a fresh
    /// projection bounds them.
    nodes: usize,
}

impl<'a> Scan<'a> {
    /// Indexes the identifiers and marks what `changed` invalidates.
    fn element(
        &mut self,
        element: &'a DomElement,
        depth: usize,
        path: &mut Vec<*const DomElement>,
        changed: &[String],
    ) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(limit_error("Semantic tree depth limit exceeded"));
        }
        self.count_node()?;
        let address = pointer(element);
        if let Some(id) = element.id() {
            bounded_id(id)?;
            if !self.ids.insert(id) {
                return Err(markup_error("Semantic element IDs must be unique"));
            }
            if changed.iter().any(|changed| changed == id) {
                // An ancestor's name aggregates its descendants' content, so
                // every element on the path to an edit is rebuilt.
                self.invalid.extend(path.iter().copied());
                self.invalid.insert(address);
            }
        }
        self.elements += 1;
        path.push(address);
        for child in &element.children {
            match child {
                DomNode::Element(child) => self.element(child, depth + 1, path, changed)?,
                DomNode::Text(text) => {
                    if text.len() > MAX_SEMANTIC_TEXT_BYTES {
                        return Err(limit_error("Semantic text node exceeds its byte limit"));
                    }
                    self.count_node()?;
                }
            }
        }
        path.pop();
        Ok(())
    }

    /// Resolves every reference, and marks the elements whose name or
    /// description the edit changes through one.
    fn references(
        &mut self,
        element: &'a DomElement,
        path: &mut Vec<*const DomElement>,
        changed: &[String],
    ) -> Result<()> {
        for attribute in ["aria-labelledby", "aria-describedby"] {
            let Some(value) = element.attributes.get(attribute) else {
                continue;
            };
            let mut touched = false;
            for id in value.split_whitespace() {
                if !self.ids.contains(id) {
                    return Err(markup_error(
                        "Semantic ARIA reference does not resolve to an ID",
                    ));
                }
                touched |= changed.iter().any(|changed| changed == id);
            }
            if touched {
                self.invalid.extend(path.iter().copied());
                self.invalid.insert(pointer(element));
            }
        }
        path.push(pointer(element));
        for child in &element.children {
            if let DomNode::Element(child) = child {
                self.references(child, path, changed)?;
            }
        }
        path.pop();
        Ok(())
    }

    fn count_node(&mut self) -> Result<()> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| limit_error("Semantic source node count overflow"))?;
        if self.nodes > MAX_NODES {
            return Err(limit_error("Semantic source node limit exceeded"));
        }
        Ok(())
    }
}

/// The projection of `element`, reusing `previous` when the edit left it alone.
fn rebuild(
    element: &DomElement,
    previous: Option<SemanticNode>,
    scan: &Scan<'_>,
    root: &DomElement,
    depth: usize,
    inherited_hidden: bool,
) -> Result<SemanticNode> {
    if depth > MAX_DEPTH {
        return Err(limit_error("Semantic tree depth limit exceeded"));
    }
    let previous = match previous {
        Some(previous)
            if !scan.invalid.contains(&pointer(element))
                && previous.id.as_deref() == element.id()
                && previous.children.len() == element_elements(element).count() =>
        {
            return Ok(previous);
        }
        other => other,
    };
    let role = role_for(element)?;
    let local_hidden = element.attributes.contains_key("hidden")
        || boolean_attribute(element, "aria-hidden")?.unwrap_or(false);
    // Visibility is inherited through the semantic subtree, so an edit that
    // changes it here changes every descendant.
    let hidden = inherited_hidden || local_hidden;
    let disabled = element.attributes.contains_key("disabled")
        || boolean_attribute(element, "aria-disabled")?.unwrap_or(false);
    let name = match element.attributes.get("aria-label") {
        Some(value) => normalize_text(value)?,
        None => match element.attributes.get("aria-labelledby") {
            Some(value) => referenced_text(value, root)?,
            None => subtree_content(element)?,
        },
    };
    let description = match element.attributes.get("aria-description") {
        Some(value) => Some(normalize_text(value)?),
        None => element
            .attributes
            .get("aria-describedby")
            .map(|value| referenced_text(value, root))
            .transpose()?,
    };
    let value = element
        .attributes
        .get("value")
        .map(|value| normalize_text(value))
        .transpose()?;
    let keyboard_shortcuts = element
        .attributes
        .get("aria-keyshortcuts")
        .map(|value| normalize_text(value))
        .transpose()?;
    let expanded = optional_boolean(element, "aria-expanded")?;
    let selected = optional_boolean(element, "aria-selected")?;
    let checked = optional_boolean(element, "aria-checked")?
        .or_else(|| element.attributes.contains_key("checked").then_some(true));
    let tabindex = element
        .attributes
        .get("tabindex")
        .map(|value| {
            value
                .parse::<i32>()
                .map_err(|_| markup_error("Semantic tabindex must be a signed decimal integer"))
        })
        .transpose()?;
    let focusable =
        !hidden && !disabled && (role.is_interactive() || tabindex.is_some_and(|value| value >= 0));
    let actions = if hidden || disabled {
        Vec::new()
    } else {
        actions_for(role)
    };
    // A descendant's projection is reusable only while the visibility it
    // inherits is the one it was built with.
    let mut previous_children = match previous {
        Some(previous) if previous.hidden == hidden => previous.children.into_iter(),
        _ => Vec::new().into_iter(),
    };
    let mut children = Vec::new();
    children
        .try_reserve(element_elements(element).count())
        .map_err(|_| limit_error("Semantic child allocation failed"))?;
    for child in element_elements(element) {
        children.push(rebuild(
            child,
            previous_children.next(),
            scan,
            root,
            depth + 1,
            hidden,
        )?);
    }
    Ok(SemanticNode {
        id: element.id().map(str::to_owned),
        role,
        name,
        description,
        value,
        keyboard_shortcuts,
        disabled,
        hidden,
        expanded,
        selected,
        checked,
        focusable,
        actions,
        children,
    })
}

/// The address of `element`, which identifies it while the document is read.
///
/// A projection is refreshed in one pass over an immutable document, so an
/// element's address is stable for the whole refresh and names it exactly.
fn pointer(element: &DomElement) -> *const DomElement {
    std::ptr::from_ref(element)
}

/// The child elements of `element`, in document order.
fn element_elements(element: &DomElement) -> impl Iterator<Item = &DomElement> {
    element.children.iter().filter_map(|child| match child {
        DomNode::Element(child) => Some(&**child),
        DomNode::Text(_) => None,
    })
}

/// The element's text content: its text nodes and its descendants' content,
/// each normalized and joined by single spaces.
fn subtree_content(element: &DomElement) -> Result<String> {
    // One buffer sized for the text the element holds: normalization shortens
    // each part and the join adds at most one separator per part, so this
    // capacity is never exceeded and the join requests nothing further.
    let (bytes, parts) = text_extent(element);
    let capacity = bytes.saturating_add(parts).min(MAX_SEMANTIC_TEXT_BYTES);
    let mut content = String::with_capacity(capacity);
    append_content(element, &mut content, &mut false)?;
    Ok(content)
}

/// The UTF-8 bytes of the text `element` holds, and how many text nodes hold
/// them.
fn text_extent(element: &DomElement) -> (usize, usize) {
    element
        .children
        .iter()
        .map(|child| match child {
            DomNode::Element(child) => text_extent(child),
            DomNode::Text(text) => (text.len(), 1),
        })
        .fold((0, 0), |(bytes, parts), (child_bytes, child_parts)| {
            (bytes.saturating_add(child_bytes), parts + child_parts)
        })
}

fn append_content(element: &DomElement, content: &mut String, space: &mut bool) -> Result<()> {
    for child in &element.children {
        match child {
            DomNode::Element(child) => {
                // A child's content is one part of this element's content, so
                // the parts are separated even when the child holds the text
                // deeper down.
                *space = true;
                append_content(child, content, space)?;
            }
            DomNode::Text(text) => append_text(content, text, space)?,
        }
    }
    Ok(())
}

/// Appends one text node's normalized content without materializing it.
///
/// This is [`normalize_text`] followed by [`append_normalized`], fused so a
/// rebuilt ancestor joins its descendants' text into one buffer instead of one
/// allocation per text node. `space` carries the pending separator: it is set
/// between parts, and materialized by the next character written over a
/// non-empty result, exactly as [`append_normalized`] separates its parts.
fn append_text(content: &mut String, text: &str, space: &mut bool) -> Result<()> {
    for character in text.chars() {
        if character.is_whitespace() {
            *space = !content.is_empty();
            continue;
        }
        if *space {
            if !content.is_empty() {
                content.push(' ');
            }
            *space = false;
        }
        if content.len() + character.len_utf8() > MAX_SEMANTIC_TEXT_BYTES {
            return Err(limit_error("Semantic text exceeds its byte limit"));
        }
        content.push(character);
    }
    Ok(())
}

/// The content of every element `value` references, joined by single spaces.
fn referenced_text(value: &str, root: &DomElement) -> Result<String> {
    let mut result = String::new();
    for id in value.split_whitespace() {
        let target = find_by_id(root, id)
            .ok_or_else(|| markup_error("Semantic ARIA reference does not resolve to an ID"))?;
        append_normalized(&mut result, &subtree_content(target)?)?;
    }
    Ok(result)
}

fn find_by_id<'a>(element: &'a DomElement, id: &str) -> Option<&'a DomElement> {
    if element.id() == Some(id) {
        return Some(element);
    }
    element_elements(element).find_map(|child| find_by_id(child, id))
}

#[cfg(test)]
mod tests;
