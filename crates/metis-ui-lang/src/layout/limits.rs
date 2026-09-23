//! Resource validation before layout allocates display commands.

use crate::dom::{DomElement, DomNode};
use crate::parser::{MAX_DEPTH, MAX_INPUT_BYTES, MAX_NODES, limit_error};
use metis_core::error::Result;

pub(super) fn validate_layout_tree(element: &DomElement) -> Result<()> {
    let mut remaining_nodes = MAX_NODES;
    let mut remaining_bytes = MAX_INPUT_BYTES;
    validate(element, 1, &mut remaining_nodes, &mut remaining_bytes)
}

fn validate(
    element: &DomElement,
    depth: usize,
    nodes: &mut usize,
    bytes: &mut usize,
) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(limit_error("Layout nesting limit exceeded"));
    }
    *nodes = nodes
        .checked_sub(1)
        .ok_or_else(|| limit_error("Layout node limit exceeded"))?;
    for attribute in ["id", "popover-anchor"] {
        if let Some(value) = element.attributes.get(attribute) {
            *bytes = bytes
                .checked_sub(value.len())
                .ok_or_else(|| limit_error("Layout attribute byte limit exceeded"))?;
        }
    }
    for child in &element.children {
        match child {
            DomNode::Element(child) => validate(child, depth + 1, nodes, bytes)?,
            DomNode::Text(text) => {
                *nodes = nodes
                    .checked_sub(1)
                    .ok_or_else(|| limit_error("Layout node limit exceeded"))?;
                *bytes = bytes
                    .checked_sub(text.len())
                    .ok_or_else(|| limit_error("Layout text byte limit exceeded"))?;
            }
        }
    }
    Ok(())
}
