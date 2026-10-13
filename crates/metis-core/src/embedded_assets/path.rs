//! The portable relative-path contract shared by manifests and inventories.

use super::PORTABLE_PATH_LIMIT;
use std::io;
use std::path::{Component, Path};

/// Admits `value` as a bounded relative path without platform syntax.
///
/// # Errors
/// Returns an error for an empty, overlong, absolute, reserved-device or
/// otherwise ambiguous path.
pub fn relative(value: &str) -> io::Result<()> {
    if value.is_empty()
        || value.len() > PORTABLE_PATH_LIMIT
        || value.contains('\\')
        || value.chars().any(|c| {
            c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|' | '[' | ']' | ';')
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "payload path must be a bounded relative path without platform syntax",
        ));
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
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "payload path contains a reserved or ambiguous component",
            ));
        }
    }
    if Path::new(value)
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "payload path must remain relative",
        ));
    }
    Ok(())
}
