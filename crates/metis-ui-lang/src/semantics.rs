//! Bounded semantic projection for the format-neutral UI document.
//!
//! The projection gives native and software-rendered hosts one deterministic
//! description of roles, names, states and actions. It is not an operating
//! system accessibility bridge; a host still has to translate this tree to
//! UIA, `NSAccessibility`, AT-SPI or another platform contract.

use crate::dom::{DomDocument, DomElement, DomNode};
use crate::parser::{MAX_DEPTH, MAX_NODES};
use metis_core::error::{ErrorCode, MetisError, Result};
use std::collections::HashMap;

mod types;

pub use types::{
    MAX_SEMANTIC_ID_BYTES, MAX_SEMANTIC_TEXT_BYTES, SemanticAction, SemanticNode, SemanticRole,
    SemanticTree,
};

impl SemanticTree {
    /// Builds a deterministic semantic projection from a bounded DOM.
    ///
    /// IDs are checked for uniqueness before references are resolved. The
    /// parser's depth and node limits are retained for application-built DOMs
    /// so a host cannot turn semantic extraction into an unbounded traversal.
    ///
    /// # Errors
    /// Returns a UI error for duplicate IDs, unresolved ARIA references,
    /// unknown roles, malformed state values, or a semantic text/depth bound.
    pub fn from_document(document: &DomDocument) -> Result<Self> {
        let mut ids = HashMap::new();
        let mut source_nodes = 0;
        index_element(&document.root, 1, &mut source_nodes, &mut ids)?;
        let mut elements = 0;
        let root = build_element(&document.root, 1, &ids, &mut elements, false)?;
        Ok(Self {
            root,
            element_count: elements,
        })
    }
}

fn index_element<'a>(
    element: &'a DomElement,
    depth: usize,
    source_nodes: &mut usize,
    ids: &mut HashMap<&'a str, &'a DomElement>,
) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(limit_error("Semantic tree depth limit exceeded"));
    }
    count_node(source_nodes)?;
    if let Some(id) = element.id() {
        bounded_id(id)?;
        if ids.insert(id, element).is_some() {
            return Err(markup_error("Semantic element IDs must be unique"));
        }
    }
    for child in &element.children {
        match child {
            DomNode::Element(child) => index_element(child, depth + 1, source_nodes, ids)?,
            DomNode::Text(text) => {
                if text.len() > MAX_SEMANTIC_TEXT_BYTES {
                    return Err(limit_error("Semantic text node exceeds its byte limit"));
                }
                count_node(source_nodes)?;
            }
        }
    }
    Ok(())
}

fn build_element(
    element: &DomElement,
    depth: usize,
    ids: &HashMap<&str, &DomElement>,
    elements: &mut usize,
    inherited_hidden: bool,
) -> Result<SemanticNode> {
    if depth > MAX_DEPTH {
        return Err(limit_error("Semantic tree depth limit exceeded"));
    }
    *elements = elements
        .checked_add(1)
        .ok_or_else(|| limit_error("Semantic element count overflow"))?;
    let role = role_for(element)?;
    let local_hidden = element.attributes.contains_key("hidden")
        || boolean_attribute(element, "aria-hidden")?.unwrap_or(false);
    // Visibility is inherited through the semantic subtree. An explicit
    // `aria-hidden="false"` on a descendant cannot reactivate a hidden
    // ancestor, because exposing that control would make keyboard and host
    // actions reach content the application declared unavailable.
    let hidden = inherited_hidden || local_hidden;
    let disabled = element.attributes.contains_key("disabled")
        || boolean_attribute(element, "aria-disabled")?.unwrap_or(false);
    let name = accessible_name(element, ids, depth)?;
    let description = referenced_or_literal(element, "aria-description", "aria-describedby", ids)?;
    let value = element
        .attributes
        .get("value")
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
    let mut children = Vec::new();
    children
        .try_reserve(element.children.len())
        .map_err(|_| limit_error("Semantic child allocation failed"))?;
    for child in &element.children {
        if let DomNode::Element(child) = child {
            children.push(build_element(child, depth + 1, ids, elements, hidden)?);
        }
    }
    Ok(SemanticNode {
        id: element.id().map(str::to_owned),
        role,
        name,
        description,
        value,
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

fn role_for(element: &DomElement) -> Result<SemanticRole> {
    if let Some(role) = element.attributes.get("role") {
        return role_from_name(role);
    }
    let tag = element.tag.to_ascii_lowercase();
    Ok(match tag.as_str() {
        "screen" | "application" => SemanticRole::Application,
        "main" => SemanticRole::Main,
        "nav" => SemanticRole::Navigation,
        "aside" => SemanticRole::Complementary,
        "toolbar" => SemanticRole::Toolbar,
        "menu" => SemanticRole::Menu,
        "menuitem" => SemanticRole::MenuItem,
        "button" => SemanticRole::Button,
        "input" | "textarea" | "text-input" => SemanticRole::TextBox,
        "checkbox" => SemanticRole::CheckBox,
        "radio" => SemanticRole::Radio,
        "range" | "slider" => SemanticRole::Slider,
        "select" | "combobox" => SemanticRole::ComboBox,
        "dialog" => SemanticRole::Dialog,
        "status" | "output" => SemanticRole::Status,
        "table" | "grid" => SemanticRole::Table,
        "text" | "label" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => SemanticRole::Text,
        _ => SemanticRole::Group,
    })
}

fn role_from_name(value: &str) -> Result<SemanticRole> {
    match value.trim().to_ascii_lowercase().as_str() {
        "application" => Ok(SemanticRole::Application),
        "main" => Ok(SemanticRole::Main),
        "navigation" => Ok(SemanticRole::Navigation),
        "complementary" => Ok(SemanticRole::Complementary),
        "group" => Ok(SemanticRole::Group),
        "toolbar" => Ok(SemanticRole::Toolbar),
        "menu" => Ok(SemanticRole::Menu),
        "menuitem" => Ok(SemanticRole::MenuItem),
        "button" => Ok(SemanticRole::Button),
        "text" | "label" => Ok(SemanticRole::Text),
        "textbox" => Ok(SemanticRole::TextBox),
        "checkbox" => Ok(SemanticRole::CheckBox),
        "radio" => Ok(SemanticRole::Radio),
        "slider" => Ok(SemanticRole::Slider),
        "combobox" => Ok(SemanticRole::ComboBox),
        "dialog" => Ok(SemanticRole::Dialog),
        "status" => Ok(SemanticRole::Status),
        "table" | "grid" => Ok(SemanticRole::Table),
        _ => Err(markup_error(
            "Semantic role is outside the admitted vocabulary",
        )),
    }
}

fn actions_for(role: SemanticRole) -> Vec<SemanticAction> {
    match role {
        SemanticRole::Button | SemanticRole::MenuItem => vec![SemanticAction::Activate],
        SemanticRole::TextBox => vec![SemanticAction::SetValue],
        SemanticRole::CheckBox | SemanticRole::Radio => vec![SemanticAction::Toggle],
        SemanticRole::Slider => vec![SemanticAction::AdjustValue],
        SemanticRole::ComboBox => vec![SemanticAction::Open],
        _ => Vec::new(),
    }
}

fn accessible_name(
    element: &DomElement,
    ids: &HashMap<&str, &DomElement>,
    depth: usize,
) -> Result<String> {
    if let Some(value) = element.attributes.get("aria-label") {
        return normalize_text(value);
    }
    if let Some(value) = element.attributes.get("aria-labelledby") {
        return referenced_text(value, ids);
    }
    text_content(element, depth)
}

fn referenced_or_literal(
    element: &DomElement,
    literal: &str,
    references: &str,
    ids: &HashMap<&str, &DomElement>,
) -> Result<Option<String>> {
    if let Some(value) = element.attributes.get(literal) {
        return Ok(Some(normalize_text(value)?));
    }
    element
        .attributes
        .get(references)
        .map(|value| referenced_text(value, ids))
        .transpose()
}

fn referenced_text(value: &str, ids: &HashMap<&str, &DomElement>) -> Result<String> {
    let mut result = String::new();
    for id in value.split_whitespace() {
        let target = ids
            .get(id)
            .copied()
            .ok_or_else(|| markup_error("Semantic ARIA reference does not resolve to an ID"))?;
        append_text(&mut result, &text_content(target, 1)?)?;
    }
    Ok(result)
}

fn text_content(element: &DomElement, depth: usize) -> Result<String> {
    if depth > MAX_DEPTH {
        return Err(limit_error("Semantic text traversal depth exceeded"));
    }
    let mut result = String::new();
    for child in &element.children {
        match child {
            DomNode::Text(text) => append_text(&mut result, text)?,
            DomNode::Element(child) => append_text(&mut result, &text_content(child, depth + 1)?)?,
        }
    }
    Ok(result)
}

fn append_text(output: &mut String, text: &str) -> Result<()> {
    let normalized = normalize_text(text)?;
    if normalized.is_empty() {
        return Ok(());
    }
    let separator = usize::from(!output.is_empty());
    if output
        .len()
        .checked_add(separator)
        .and_then(|length| length.checked_add(normalized.len()))
        .is_none_or(|length| length > MAX_SEMANTIC_TEXT_BYTES)
    {
        return Err(limit_error("Semantic text exceeds its byte limit"));
    }
    if separator != 0 {
        output.push(' ');
    }
    output.push_str(&normalized);
    Ok(())
}

fn normalize_text(value: &str) -> Result<String> {
    let mut output = String::new();
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        if output.len() + character.len_utf8() > MAX_SEMANTIC_TEXT_BYTES {
            return Err(limit_error("Semantic text exceeds its byte limit"));
        }
        output.push(character);
    }
    Ok(output)
}

fn boolean_attribute(element: &DomElement, name: &str) -> Result<Option<bool>> {
    element
        .attributes
        .get(name)
        .map(|value| {
            if value.is_empty() {
                return Ok(true);
            }
            match value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => Ok(true),
                "false" | "0" => Ok(false),
                _ => Err(markup_error("Semantic boolean attribute is malformed")),
            }
        })
        .transpose()
}

fn optional_boolean(element: &DomElement, name: &str) -> Result<Option<bool>> {
    boolean_attribute(element, name)
}

fn bounded_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > MAX_SEMANTIC_ID_BYTES || id.chars().any(char::is_whitespace) {
        return Err(markup_error(
            "Semantic IDs must be non-empty bounded tokens",
        ));
    }
    Ok(())
}

fn count_node(count: &mut usize) -> Result<()> {
    *count = count
        .checked_add(1)
        .ok_or_else(|| limit_error("Semantic source node count overflow"))?;
    if *count > MAX_NODES {
        return Err(limit_error("Semantic source node limit exceeded"));
    }
    Ok(())
}

fn markup_error(message: &'static str) -> MetisError {
    MetisError::ui(ErrorCode::MalformedMarkup, message)
}

fn limit_error(message: &'static str) -> MetisError {
    MetisError::ui(ErrorCode::LayoutOverflow, message)
}

#[cfg(test)]
mod tests;
