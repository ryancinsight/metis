//! Bounded parser for a case-sensitive XML-like markup subset.
//!
//! Supports one root, nested tags, boolean or quoted attributes, text, and comments.
//! Entity decoding, HTML void elements, scripts, and browser error recovery are absent.

use crate::dom::{DomDocument, DomElement, DomNode};
use crate::style::ComputedStyle;
use metis_core::error::{ErrorCode, MetisError, Result};
use std::collections::HashMap;

/// Maximum source bytes (one MiB), bounding copied text and attribute storage.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
/// Maximum element nesting, bounding recursive parser and DOM traversal stack use.
pub const MAX_DEPTH: usize = 64;
/// Maximum combined element and text node count.
pub const MAX_NODES: usize = 4096;
/// Maximum attributes on a single element.
pub const MAX_ATTRIBUTES: usize = 64;

/// Parses one complete document, trimming text-node boundary whitespace.
///
/// # Errors
/// Rejects malformed syntax, duplicate attributes, trailing content, mismatched or
/// unclosed tags, resource-limit violations, and storage reservation failures.
///
/// # Examples
/// ```
/// let document = metis_ui_lang::parse_markup("<label id='answer'>42</label>")?;
/// assert_eq!(document.root.text_content(), "42");
/// # Ok::<(), metis_core::error::MetisError>(())
/// ```
pub fn parse_markup(input: &str) -> Result<DomDocument> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(limit_error("Markup byte limit exceeded"));
    }
    let mut parser = Parser {
        input,
        pos: 0,
        nodes: 0,
    };
    parser.skip_whitespace_and_comments()?;
    let root = parser.parse_element(1)?;
    parser.skip_whitespace_and_comments()?;
    if parser.pos != input.len() {
        return Err(syntax_error("Trailing content after root element"));
    }
    Ok(DomDocument::new(root))
}

pub(crate) fn limit_error(message: &str) -> MetisError {
    MetisError::ui(ErrorCode::LayoutOverflow, message)
}
fn syntax_error(message: &str) -> MetisError {
    MetisError::ui(ErrorCode::MalformedMarkup, message)
}
pub(crate) fn copy_text(text: &str) -> Result<String> {
    let mut result = String::new();
    result
        .try_reserve_exact(text.len())
        .map_err(|_| limit_error("Text allocation failed"))?;
    result.push_str(text);
    Ok(result)
}

struct Parser<'input> {
    input: &'input str,
    pos: usize,
    nodes: usize,
}

impl Parser<'_> {
    fn add_node(&mut self) -> Result<()> {
        if self.nodes >= MAX_NODES {
            return Err(limit_error("Markup node limit exceeded"));
        }
        self.nodes += 1;
        Ok(())
    }

    fn parse_element(&mut self, depth: usize) -> Result<DomElement> {
        if depth > MAX_DEPTH {
            return Err(limit_error("Markup nesting limit exceeded"));
        }
        self.add_node()?;
        self.expect_char('<')?;
        let tag = self.read_identifier()?;
        if tag.is_empty() {
            return Err(syntax_error("Expected tag identifier"));
        }
        let (attributes, self_closing) = self.parse_attributes()?;
        let computed_style = attributes.get("style").map_or_else(
            || Ok(ComputedStyle::default()),
            |value| ComputedStyle::parse(value),
        )?;
        let mut element = DomElement {
            tag,
            attributes,
            children: Vec::new(),
            computed_style,
        };
        if self_closing {
            return Ok(element);
        }
        loop {
            self.skip_whitespace_and_comments()?;
            if self.pos == self.input.len() {
                return Err(MetisError::ui(
                    ErrorCode::UnclosedTag,
                    "End of input before closing tag",
                ));
            }
            if self.starts_with("</") {
                self.pos += 2;
                let closing = self.read_identifier()?;
                self.skip_whitespace();
                self.expect_char('>')?;
                if closing != element.tag {
                    return Err(MetisError::ui(
                        ErrorCode::TagMismatch,
                        "Closing tag does not match opening tag",
                    ));
                }
                return Ok(element);
            }
            let node = if self.starts_with("<") {
                DomNode::Element(self.parse_element(depth + 1)?)
            } else {
                let start = self.pos;
                self.advance_until('<');
                let text = self.input[start..self.pos].trim();
                if text.is_empty() {
                    continue;
                }
                self.add_node()?;
                DomNode::Text(copy_text(text)?)
            };
            element
                .children
                .try_reserve(1)
                .map_err(|_| limit_error("Node allocation failed"))?;
            element.children.push(node);
        }
    }

    fn parse_attributes(&mut self) -> Result<(HashMap<String, String>, bool)> {
        let mut attributes = HashMap::new();
        loop {
            let before_whitespace = self.pos;
            self.skip_whitespace();
            if self.starts_with("/>") {
                self.pos += 2;
                return Ok((attributes, true));
            }
            if self.starts_with(">") {
                self.pos += 1;
                return Ok((attributes, false));
            }
            if self.pos == before_whitespace {
                return Err(syntax_error("Attributes require separating whitespace"));
            }
            if attributes.len() >= MAX_ATTRIBUTES {
                return Err(limit_error("Attribute limit exceeded"));
            }
            let key = self.read_identifier()?;
            if key.is_empty() {
                return Err(syntax_error("Expected attribute name"));
            }
            let after_key = self.pos;
            self.skip_whitespace();
            let value = if self.starts_with("=") {
                self.pos += 1;
                self.skip_whitespace();
                self.read_quoted_value()?
            } else {
                self.pos = after_key;
                String::new()
            };
            attributes
                .try_reserve(1)
                .map_err(|_| limit_error("Attribute allocation failed"))?;
            if attributes.insert(key, value).is_some() {
                return Err(syntax_error("Duplicate attribute"));
            }
        }
    }

    fn read_identifier(&mut self) -> Result<String> {
        let start = self.pos;
        while let Some(c) = self.current_char() {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        copy_text(&self.input[start..self.pos])
    }

    fn read_quoted_value(&mut self) -> Result<String> {
        let Some(quote @ ('\'' | '"')) = self.current_char() else {
            return Err(syntax_error("Expected quoted attribute value"));
        };
        self.pos += 1;
        let start = self.pos;
        self.advance_until(quote);
        if self.current_char() != Some(quote) {
            return Err(MetisError::ui(
                ErrorCode::UnclosedTag,
                "Unterminated quoted attribute",
            ));
        }
        let value = copy_text(&self.input[start..self.pos])?;
        self.pos += 1;
        Ok(value)
    }

    fn advance_until(&mut self, delimiter: char) {
        while let Some(c) = self.current_char() {
            if c == delimiter {
                break;
            }
            self.pos += c.len_utf8();
        }
    }
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current_char() {
            if !c.is_whitespace() {
                break;
            }
            self.pos += c.len_utf8();
        }
    }
    fn skip_whitespace_and_comments(&mut self) -> Result<()> {
        loop {
            self.skip_whitespace();
            if !self.starts_with("<!--") {
                return Ok(());
            }
            self.pos += 4;
            let Some(end) = self.input[self.pos..].find("-->") else {
                return Err(MetisError::ui(
                    ErrorCode::UnclosedTag,
                    "Unclosed markup comment",
                ));
            };
            self.pos += end + 3;
        }
    }
    fn current_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }
    fn starts_with(&self, text: &str) -> bool {
        self.input[self.pos..].starts_with(text)
    }
    fn expect_char(&mut self, expected: char) -> Result<()> {
        if self.current_char() != Some(expected) {
            return Err(syntax_error("Unexpected markup delimiter"));
        }
        self.pos += expected.len_utf8();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn parses_both_quote_styles_and_unicode() {
        let doc = parse_markup("<!--before--><box id='root' title=\"éλ\"><label>42</label><!--middle--><value/>tail</box><!--after-->").expect("valid markup");
        assert_eq!(doc.root.tag, "box");
        assert_eq!(
            doc.root.attributes.get("title").map(String::as_str),
            Some("éλ")
        );
        assert_eq!(doc.root.children.len(), 3);
        assert_eq!(doc.root.text_content(), "42tail");
    }

    #[test]
    fn truncations_and_trailing_content_return_classified_errors() {
        for input in ["<a>", "<a>text", "<a><b/>", "<a><!--", "<a x='"] {
            assert_eq!(
                parse_markup(input).expect_err("truncated input").code,
                ErrorCode::UnclosedTag
            );
        }
        for input in [
            "<a/><b/>",
            "<a/>tail",
            "<a x='1'x='2'/>",
            "<a x='1' x='2'/>",
        ] {
            assert_eq!(
                parse_markup(input).expect_err("malformed input").code,
                ErrorCode::MalformedMarkup
            );
        }
        assert_eq!(
            parse_markup("<a></b>").expect_err("mismatch").code,
            ErrorCode::TagMismatch
        );
    }

    #[test]
    fn style_errors_propagate_from_element_attributes() {
        for input in [
            "<a style='unknown: value'/>",
            "<a style='gap'/>",
            "<a style='width: NaN%'/>",
        ] {
            assert_eq!(
                parse_markup(input).expect_err("invalid style").code,
                ErrorCode::InvalidCssStyle,
                "{input}"
            );
        }
    }

    #[test]
    fn limits_bound_depth_nodes_attributes_and_bytes() {
        let accepted = "<a>".repeat(MAX_DEPTH) + &"</a>".repeat(MAX_DEPTH);
        assert_eq!(
            parse_markup(&accepted).expect("depth boundary").root.tag,
            "a"
        );
        for input in [
            "<a>".repeat(MAX_DEPTH + 1),
            "<a>".to_owned() + &"<b/>".repeat(MAX_NODES),
            " ".repeat(MAX_INPUT_BYTES + 1),
        ] {
            assert_eq!(
                parse_markup(&input).expect_err("resource bound").code,
                ErrorCode::LayoutOverflow
            );
        }
        let mut attrs = String::new();
        for n in 0..=MAX_ATTRIBUTES {
            write!(attrs, " k{n}='v'").expect("test string");
        }
        assert_eq!(
            parse_markup(&format!("<a{attrs}/>"))
                .expect_err("attributes bound")
                .code,
            ErrorCode::LayoutOverflow
        );
    }

    #[test]
    fn every_prefix_of_unicode_document_terminates() {
        let input = "<a title='éλ'><b/>text<!--c--></a>";
        for end in input.char_indices().map(|(index, _)| index).skip(1) {
            assert!(
                parse_markup(&input[..end]).is_err(),
                "incomplete prefix at {end}"
            );
        }
    }

    #[test]
    fn boolean_attributes_preserve_the_next_separator() {
        let doc = parse_markup("<a disabled selected id='choice'/>").expect("boolean attributes");
        assert_eq!(
            doc.root.attributes.get("disabled").map(String::as_str),
            Some("")
        );
        assert_eq!(
            doc.root.attributes.get("selected").map(String::as_str),
            Some("")
        );
        assert_eq!(doc.root.id(), Some("choice"));
    }

    #[test]
    fn exhaustive_short_markup_corpus() {
        const ALPHABET: [char; 7] = ['<', '>', '/', 'a', '\'', '"', 'é'];
        let mut accepted = 0;
        // Enumerates every 0..=4 character string. In this alphabet only <a/> and <é/>
        // are complete documents, providing an independent grammar oracle.
        for length in 0..=4_u32 {
            for mut encoding in 0..ALPHABET.len().pow(length) {
                let mut input = String::new();
                for _ in 0..length {
                    input.push(ALPHABET[encoding % ALPHABET.len()]);
                    encoding /= ALPHABET.len();
                }
                match parse_markup(&input) {
                    Ok(document) => {
                        assert!(matches!(input.as_str(), "<a/>" | "<é/>"));
                        assert_eq!(
                            document.root.tag,
                            input.chars().nth(1).expect("accepted tag").to_string()
                        );
                        assert_eq!(document.root.children, []);
                        accepted += 1;
                    }
                    Err(error) => {
                        assert!(!matches!(input.as_str(), "<a/>" | "<é/>"));
                        assert!(matches!(
                            error.code,
                            ErrorCode::MalformedMarkup | ErrorCode::UnclosedTag
                        ));
                    }
                }
            }
        }
        assert_eq!(accepted, 2);
    }
}
