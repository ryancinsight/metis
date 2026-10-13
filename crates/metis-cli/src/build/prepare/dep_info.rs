//! Rustc dep-info escapes ASCII spaces in sources; target names stay raw.
//!
//! Pinned rustc's source filename encoder substitutes only ` ` with `\ `.
//! Therefore decoding preserves native backslashes, dollar pairs and Unicode
//! whitespace, and removes only the backslash directly escaping an ASCII space.
//! Rules use ASCII spaces between sources. A source cannot contain a literal
//! `: ` after encoding, so the last such pair separates the raw target from its
//! sources, even when the target itself contains `: `. No general Make expansion
//! or line continuation is applied to compiler-produced names.
use crate::Result;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
#[cfg(test)]
mod tests;

pub(super) fn paths(bytes: &[u8], workspace: &Path) -> Result<Vec<PathBuf>> {
    let text = std::str::from_utf8(bytes)?;
    let mut dependencies = BTreeSet::new();
    for line in text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
    {
        let Some((_, dependencies_text)) = line.rsplit_once(": ") else {
            if line.ends_with(':') {
                continue;
            }
            return Err("rustc dep-info omits target separator".into());
        };
        for word in words(dependencies_text) {
            let path = Path::new(&word);
            dependencies.insert(if path.is_absolute() {
                metis_core::embedded_assets::canonical_file(path)?
            } else {
                metis_core::embedded_assets::canonical_file(&workspace.join(path))?
            });
        }
    }
    if dependencies.is_empty() {
        return Err("rustc dep-info contains no file inputs".into());
    }
    Ok(dependencies.into_iter().collect())
}
fn words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\' && characters.peek() == Some(&' ') {
            word.push(' ');
            characters.next();
        } else if character == ' ' {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(character);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}
