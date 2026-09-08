//! Bounded SVG validation for same-origin application artwork.
//!
//! The browser can parse SVG more completely than the packaging tool, so the
//! packaging boundary admits only a deliberately small, scriptless subset:
//! one root with a fixed viewport and path elements with literal geometry and
//! colors. External references, event attributes and XML expansion features
//! are rejected before the asset enters a portable or installed payload.

use crate::Result;
use std::{fs, io::Read, path::Path};

/// Maximum encoded size of one packaged SVG resource.
pub(crate) const SVG_LIMIT: u64 = 256 * 1024;
/// Maximum viewport edge in CSS pixels admitted by the asset contract.
const MAX_DIMENSION: u32 = 4_096;
/// Maximum number of path elements in one admitted artwork.
const MAX_PATHS: usize = 64;
/// Maximum geometry string size for one path.
const MAX_PATH_DATA: usize = 64 * 1024;

pub(crate) fn validate_file(path: &Path) -> Result<()> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(SVG_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    validate(&bytes)
}

pub(crate) fn validate(bytes: &[u8]) -> Result<()> {
    if u64::try_from(bytes.len())? > SVG_LIMIT {
        return Err("SVG asset exceeds the 256 KiB budget".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "SVG asset must be UTF-8")?;
    if !text.is_ascii() {
        return Err("SVG asset must use ASCII markup".into());
    }
    Parser::new(text).document()
}

struct Parser<'a> {
    source: &'a str,
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
        }
    }

    fn document(mut self) -> Result<()> {
        self.skip_whitespace();
        let root = self.open_tag("svg")?;
        if root.self_closed {
            return Err("SVG root must contain path elements".into());
        }
        validate_root(&root.attributes)?;

        let mut path_count = 0_usize;
        loop {
            self.skip_whitespace();
            if self.consume("</svg>") {
                break;
            }
            let path = self.open_tag("path")?;
            if !path.self_closed {
                return Err("SVG path elements must be self-closing".into());
            }
            path_count = path_count
                .checked_add(1)
                .ok_or("SVG path count overflows")?;
            if path_count > MAX_PATHS {
                return Err("SVG contains too many path elements".into());
            }
            validate_path(&path.attributes)?;
        }
        if path_count == 0 {
            return Err("SVG must contain at least one path".into());
        }
        self.skip_whitespace();
        if self.position != self.source.len() {
            return Err("SVG contains trailing markup".into());
        }
        Ok(())
    }

    fn open_tag(&mut self, name: &str) -> Result<Tag<'a>> {
        if !self.consume("<") || !self.consume(name) {
            return Err(format!("SVG expected <{name}>").into());
        }
        if self
            .source
            .as_bytes()
            .get(self.position)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'/' && *byte != b'>')
        {
            return Err("SVG tag name must be delimited".into());
        }
        let attributes = self.attributes()?;
        let self_closed = if self.consume("/>") {
            true
        } else if self.consume(">") {
            false
        } else {
            return Err("SVG tag is not terminated".into());
        };
        Ok(Tag {
            attributes,
            self_closed,
        })
    }

    fn attributes(&mut self) -> Result<Vec<(&'a str, &'a str)>> {
        let mut attributes = Vec::new();
        loop {
            self.skip_whitespace();
            let Some(&byte) = self.source.as_bytes().get(self.position) else {
                return Err("SVG attribute list is truncated".into());
            };
            if byte == b'/' || byte == b'>' {
                return Ok(attributes);
            }
            let name_start = self.position;
            while self
                .source
                .as_bytes()
                .get(self.position)
                .is_some_and(|value| value.is_ascii_alphanumeric() || *value == b'-')
            {
                self.position += 1;
            }
            if self.position == name_start {
                return Err("SVG attribute name is malformed".into());
            }
            let name = &self.source[name_start..self.position];
            self.skip_whitespace();
            if !self.consume("=") {
                return Err("SVG attributes require an equals sign".into());
            }
            self.skip_whitespace();
            if !self.consume("\"") {
                return Err("SVG attributes require double-quoted values".into());
            }
            let value_start = self.position;
            let Some(relative_end) = self.source[value_start..].find('"') else {
                return Err("SVG attribute value is truncated".into());
            };
            self.position = value_start + relative_end;
            let value = &self.source[value_start..self.position];
            self.position += 1;
            if value
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte == b'&')
            {
                return Err("SVG attribute contains a control or entity character".into());
            }
            if attributes.iter().any(|(known, _)| *known == name) {
                return Err("SVG contains a duplicate attribute".into());
            }
            attributes.push((name, value));
            if attributes.len() > 16 {
                return Err("SVG tag contains too many attributes".into());
            }
        }
    }

    fn consume(&mut self, expected: &str) -> bool {
        if self.source[self.position..].starts_with(expected) {
            self.position += expected.len();
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while self
            .source
            .as_bytes()
            .get(self.position)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.position += 1;
        }
    }
}

struct Tag<'a> {
    attributes: Vec<(&'a str, &'a str)>,
    self_closed: bool,
}

fn validate_root(attributes: &[(&str, &str)]) -> Result<()> {
    let xmlns = attribute(attributes, "xmlns")?;
    if xmlns != "http://www.w3.org/2000/svg" {
        return Err("SVG namespace must be the standard SVG namespace".into());
    }
    let width = positive_dimension(attribute(attributes, "width")?)?;
    let height = positive_dimension(attribute(attributes, "height")?)?;
    let view_box = parse_view_box(attribute(attributes, "viewBox")?)?;
    if view_box[0] != 0 || view_box[1] != 0 || view_box[2] != width || view_box[3] != height {
        return Err("SVG viewBox must match its positive width and height".into());
    }
    if let Some(value) = optional_attribute(attributes, "preserveAspectRatio")
        && value != "xMidYMid meet"
    {
        return Err("SVG preserveAspectRatio must be xMidYMid meet".into());
    }
    for (name, _) in attributes {
        if !matches!(
            *name,
            "xmlns" | "width" | "height" | "viewBox" | "preserveAspectRatio"
        ) {
            return Err(format!("SVG root attribute {name} is not admitted").into());
        }
    }
    Ok(())
}

fn validate_path(attributes: &[(&str, &str)]) -> Result<()> {
    let data = attribute(attributes, "d")?;
    if data.is_empty() || data.len() > MAX_PATH_DATA {
        return Err("SVG path geometry is empty or exceeds its bound".into());
    }
    if !data.bytes().any(|byte| byte.is_ascii_alphabetic())
        || data.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b' ' | b'\t' | b'\r' | b'\n' | b',' | b'.' | b'+' | b'-'
                ))
        })
    {
        return Err("SVG path geometry contains an unsupported character".into());
    }
    for (name, value) in attributes {
        match *name {
            "d" => {}
            "fill" | "stroke" => validate_paint(value)?,
            "fill-opacity" | "stroke-opacity" => validate_opacity(value)?,
            "fill-rule" => {
                if !matches!(*value, "nonzero" | "evenodd") {
                    return Err("SVG fill-rule is not admitted".into());
                }
            }
            "stroke-width" => {
                positive_number(value)?;
            }
            _ => return Err(format!("SVG path attribute {name} is not admitted").into()),
        }
    }
    Ok(())
}

fn attribute<'a>(attributes: &[(&'a str, &'a str)], name: &str) -> Result<&'a str> {
    optional_attribute(attributes, name).ok_or_else(|| format!("SVG is missing {name}").into())
}

fn optional_attribute<'a>(attributes: &[(&'a str, &'a str)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find_map(|(known, value)| (*known == name).then_some(*value))
}

fn positive_dimension(value: &str) -> Result<u32> {
    let value = value
        .parse::<u32>()
        .map_err(|_| "SVG dimensions must be unsigned integer pixels")?;
    if !(1..=MAX_DIMENSION).contains(&value) {
        return Err("SVG dimensions exceed the 4096-pixel bound".into());
    }
    Ok(value)
}

fn parse_view_box(value: &str) -> Result<[u32; 4]> {
    let values = value
        .split_ascii_whitespace()
        .map(|part| part.trim_matches(','))
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if values.len() != 4 || values[2] == 0 || values[3] == 0 {
        return Err("SVG viewBox must contain four positive integer coordinates".into());
    }
    Ok([values[0], values[1], values[2], values[3]])
}

fn validate_paint(value: &str) -> Result<()> {
    if value == "none" {
        return Ok(());
    }
    let Some(hex) = value.strip_prefix('#') else {
        return Err("SVG paint must be none or a literal hexadecimal color".into());
    };
    if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("SVG paint color is malformed".into());
    }
    Ok(())
}

fn validate_opacity(value: &str) -> Result<()> {
    let value = positive_number(value)?;
    if value > 1.0 {
        return Err("SVG opacity must be between zero and one".into());
    }
    Ok(())
}

fn positive_number(value: &str) -> Result<f64> {
    let value = value
        .parse::<f64>()
        .map_err(|_| "SVG numeric attribute is malformed")?;
    if !value.is_finite() || value < 0.0 {
        return Err("SVG numeric attribute must be finite and nonnegative".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::validate;

    const PROJECT_MARK: &[u8] =
        include_bytes!("../../../../examples/browser/assets/metis-mark.svg");

    #[test]
    fn validates_project_vector_mark() {
        validate(PROJECT_MARK).expect("project SVG is admitted");
    }

    #[test]
    fn rejects_expansion_external_reference_and_geometry_mutations() {
        for replacement in [
            ("<svg", "<!DOCTYPE svg><svg"),
            ("<path", "<path href=\"https://example.invalid\""),
            ("fill=\"#0f766e\"", "fill=\"url(#remote)\""),
            ("viewBox=\"0 0 256 256\"", "viewBox=\"0 0 4097 256\""),
        ] {
            let source = String::from_utf8(PROJECT_MARK.to_vec())
                .expect("fixture is ASCII")
                .replacen(replacement.0, replacement.1, 1);
            assert!(
                validate(source.as_bytes()).is_err(),
                "accepted {replacement:?}"
            );
        }
    }

    #[test]
    fn rejects_missing_paths_and_trailing_markup() {
        assert!(validate(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1" viewBox="0 0 1 1"></svg>"#
        )
        .is_err());
        let mut trailing = PROJECT_MARK.to_vec();
        trailing.extend_from_slice(b"<path/>");
        assert!(validate(&trailing).is_err());
    }
}
