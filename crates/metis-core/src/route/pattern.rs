//! Parsed route patterns.

use super::RouteError;
use crate::uri_path;
use std::cmp::Ordering;

const MAX_PATTERN_PARTS: usize = super::MAX_ROUTE_SEGMENTS;
const MAX_NAME_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Literal(String),
    Parameter(String),
    Rest(String),
}

impl Part {
    /// Lower is more specific.
    const fn rank(&self) -> u8 {
        match self {
            Self::Literal(_) => 0,
            Self::Parameter(_) => 1,
            Self::Rest(_) => 2,
        }
    }
}

/// A validated route pattern: `/`-separated literal segments, `:name`
/// parameters matching one segment and an optional final `*name` matching
/// the remaining segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePattern {
    parts: Vec<Part>,
}

impl RoutePattern {
    /// Parses a pattern such as `/study/:id/series/:series`.
    ///
    /// Literal segments are percent-decoded like paths; names are lowercase
    /// ASCII letters, digits and `_`, starting with a letter, and distinct.
    ///
    /// # Errors
    /// Returns [`RouteError::InvalidPattern`] for any other form.
    pub fn parse(pattern: &str) -> Result<Self, RouteError> {
        let path = pattern
            .strip_prefix('/')
            .ok_or(RouteError::InvalidPattern)?;
        let raw: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        if raw.len() > MAX_PATTERN_PARTS {
            return Err(RouteError::InvalidPattern);
        }
        let mut parts = Vec::with_capacity(raw.len());
        for (index, text) in raw.iter().enumerate() {
            let part = if let Some(name) = text.strip_prefix(':') {
                Part::Parameter(name_of(name)?)
            } else if let Some(name) = text.strip_prefix('*') {
                if index + 1 != raw.len() {
                    return Err(RouteError::InvalidPattern);
                }
                Part::Rest(name_of(name)?)
            } else {
                let mut decoded =
                    uri_path::segments(text, 1).map_err(|_| RouteError::InvalidPattern)?;
                Part::Literal(decoded.pop().ok_or(RouteError::InvalidPattern)?)
            };
            let name = match &part {
                Part::Parameter(name) | Part::Rest(name) => Some(name),
                Part::Literal(_) => None,
            };
            if name.is_some_and(|name| {
                parts.iter().any(|earlier: &Part| {
                    matches!(earlier, Part::Parameter(n) | Part::Rest(n) if n == name)
                })
            }) {
                return Err(RouteError::InvalidPattern);
            }
            parts.push(part);
        }
        Ok(Self { parts })
    }

    /// The path for `parameters`, percent-encoding each value; a rest value
    /// is split on `/` into segments.
    ///
    /// # Errors
    /// Returns [`RouteError::Malformed`] when a parameter is missing or a
    /// value would not route back to this pattern: an empty segment, `.`,
    /// `..`, or a `/` inside a single-segment parameter.
    pub fn href(&self, parameters: &[(&str, &str)]) -> Result<String, RouteError> {
        let value_of = |name: &str| {
            parameters
                .iter()
                .find_map(|(key, value)| (*key == name).then_some(*value))
                .ok_or(RouteError::Malformed)
        };
        let segment = |value: &str| {
            if value.is_empty() || value == "." || value == ".." || value.contains('/') {
                Err(RouteError::Malformed)
            } else {
                Ok(uri_path::encode_segment(value))
            }
        };
        let mut path = String::new();
        for part in &self.parts {
            match part {
                Part::Literal(text) => {
                    path.push('/');
                    path.push_str(&uri_path::encode_segment(text));
                }
                Part::Parameter(name) => {
                    path.push('/');
                    path.push_str(&segment(value_of(name)?)?);
                }
                Part::Rest(name) => {
                    for piece in value_of(name)?.split('/') {
                        path.push('/');
                        path.push_str(&segment(piece)?);
                    }
                }
            }
        }
        if path.is_empty() {
            path.push('/');
        }
        Ok(path)
    }

    pub(super) fn same_shape(&self, other: &Self) -> bool {
        self.parts.len() == other.parts.len()
            && self.parts.iter().zip(&other.parts).all(|pair| match pair {
                (Part::Literal(a), Part::Literal(b)) => a == b,
                (Part::Parameter(_), Part::Parameter(_)) | (Part::Rest(_), Part::Rest(_)) => true,
                _ => false,
            })
    }

    /// Orders two matching patterns so the more specific comes first.
    pub(super) fn precedence(&self, other: &Self) -> Ordering {
        self.parts
            .iter()
            .zip(&other.parts)
            .map(|(a, b)| a.rank().cmp(&b.rank()))
            .find(|order| order.is_ne())
            .unwrap_or_else(|| other.parts.len().cmp(&self.parts.len()))
    }

    /// The parameters when `segments` match this pattern.
    pub(super) fn capture<'a, S: AsRef<str>>(
        &'a self,
        segments: &[S],
    ) -> Option<Vec<(&'a str, String)>> {
        let mut parameters = Vec::new();
        for (index, part) in self.parts.iter().enumerate() {
            match part {
                Part::Literal(text) => {
                    if segments.get(index)?.as_ref() != text {
                        return None;
                    }
                }
                Part::Parameter(name) => {
                    parameters.push((name.as_str(), segments.get(index)?.as_ref().to_owned()));
                }
                Part::Rest(name) => {
                    let rest = segments.get(index..)?;
                    if rest.is_empty() {
                        return None;
                    }
                    let joined = rest.iter().map(AsRef::as_ref).collect::<Vec<_>>().join("/");
                    parameters.push((name.as_str(), joined));
                    return Some(parameters);
                }
            }
        }
        (segments.len() == self.parts.len()).then_some(parameters)
    }
}

fn name_of(name: &str) -> Result<String, RouteError> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    if valid {
        Ok(name.to_owned())
    } else {
        Err(RouteError::InvalidPattern)
    }
}
