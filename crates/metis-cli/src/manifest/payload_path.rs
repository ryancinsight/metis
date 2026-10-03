//! Payload budget and the relative-path rule every packaged path obeys.

use crate::Result;
use std::path::{Component, Path};

/// Committed tool resource budget: the payload fits one cabinet.
pub(crate) const PAYLOAD_LIMIT: u64 = 1024 * 1024 * 1024;

/// Admits `value` as a bounded relative path without platform syntax.
///
/// # Errors
/// Returns an error for an empty, overlong, absolute, reserved-device or
/// otherwise ambiguous path.
pub(crate) fn relative(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 240
        || value.contains('\\')
        || value.chars().any(|c| {
            c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|' | '[' | ']' | ';')
        })
    {
        return Err("payload path must be a bounded relative path without platform syntax".into());
    }
    for part in value.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem
                    .as_bytes()
                    .last()
                    .is_some_and(|last| b"123456789".contains(last)))
            || ["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix)
                    .is_some_and(|number| matches!(number, "¹" | "²" | "³"))
            })
        {
            return Err("payload path contains a reserved or ambiguous component".into());
        }
    }
    if Path::new(value)
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("payload path must remain relative".into());
    }
    Ok(())
}
