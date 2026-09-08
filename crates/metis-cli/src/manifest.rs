//! Validated application identity and explicit payload ownership.
mod icon;
mod svg;

use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

/// Committed tool resource budgets: metadata is small; payload fits one cabinet.
pub(crate) const MANIFEST_LIMIT: u64 = 1024 * 1024;
pub(crate) const FILE_LIMIT: usize = 4096;
pub(crate) const PAYLOAD_LIMIT: u64 = 1024 * 1024 * 1024;
pub(crate) use icon::{ICON_LIMIT, source as icon_source, validate_file as validate_icon_file};

pub(crate) fn validate_resource_file(path: &Path) -> Result<()> {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
    {
        svg::validate_file(path)?;
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Application {
    pub(crate) schema: u32,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) manufacturer: String,
    pub(crate) upgrade_code: String,
    pub(crate) cargo_manifest: String,
    pub(crate) entry: String,
    #[serde(default)]
    pub(crate) icon: Option<String>,
    #[serde(default)]
    pub(crate) arguments: Vec<String>,
    pub(crate) binaries: Vec<Binary>,
    #[serde(default)]
    pub(crate) resources: Vec<Resource>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binary {
    pub(crate) package: String,
    pub(crate) bin: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Resource {
    pub(crate) source: String,
    pub(crate) destination: String,
}

impl Application {
    pub(crate) fn read(path: &Path) -> Result<(Self, PathBuf)> {
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(MANIFEST_LIMIT + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MANIFEST_LIMIT {
            return Err("application manifest exceeds the 1 MiB budget".into());
        }
        let value: Self = serde_json::from_slice(&bytes)?;
        value.validate()?;
        let absolute = path.canonicalize()?;
        let root = absolute
            .parent()
            .ok_or("manifest has no parent")?
            .to_path_buf();
        Ok((value, root))
    }

    fn validate(&self) -> Result<()> {
        if self.schema != 1 {
            return Err("unsupported application manifest schema".into());
        }
        identifier(&self.id)?;
        for field in [&self.name, &self.manufacturer] {
            if field.is_empty()
                || field.len() > 100
                || field
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '[' | ']' | '"' | ';'))
            {
                return Err(
                    "application label is empty, too long or contains installer syntax".into(),
                );
            }
        }
        let version: Vec<_> = self
            .version
            .split('.')
            .map(str::parse::<u32>)
            .collect::<std::result::Result<_, _>>()?;
        if !matches!(version.as_slice(), [major, minor, patch] if *major <= 255 && *minor <= 255 && *patch <= 65535)
            || self.version.split('.').any(|part| {
                part.is_empty()
                    || !part.bytes().all(|byte| byte.is_ascii_digit())
                    || (part.len() > 1 && part.starts_with('0'))
            })
        {
            return Err(
                "version must be canonical MSI major.minor.patch (255.255.65535 maximum)".into(),
            );
        }
        let guid = self.upgrade_code.as_bytes();
        if guid.len() != 38
            || guid.first() != Some(&b'{')
            || guid.last() != Some(&b'}')
            || !guid.iter().enumerate().all(|(i, b)| match i {
                0 | 37 => true,
                9 | 14 | 19 | 24 => *b == b'-',
                _ => b.is_ascii_hexdigit() && !b.is_ascii_lowercase(),
            })
        {
            return Err("upgrade_code must be an uppercase braced GUID, retained for this application identity".into());
        }
        relative(&self.cargo_manifest)?;
        identifier(&self.entry)?;
        if let Some(icon) = &self.icon {
            relative(icon)?;
        }
        if self.arguments.iter().map(String::len).sum::<usize>() > 4096
            || self.arguments.iter().any(|argument| {
                argument
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '[' | ']'))
            })
        {
            return Err(
                "launch arguments exceed the installer budget or contain formatted syntax".into(),
            );
        }
        self.validate_inventory()
    }

    fn validate_inventory(&self) -> Result<()> {
        if self.binaries.is_empty()
            || self.binaries.len().saturating_add(self.resources.len()) > FILE_LIMIT
        {
            return Err(
                "application must contain binaries and at most 4096 payload entries".into(),
            );
        }
        let mut names = BTreeSet::new();
        for binary in &self.binaries {
            identifier(&binary.package)?;
            identifier(&binary.bin)?;
            let name = format!("{}{}", binary.bin, std::env::consts::EXE_SUFFIX);
            relative(&name)?;
            if !names.insert(name.to_lowercase()) {
                return Err("duplicate executable destination".into());
            }
        }
        if !self.binaries.iter().any(|binary| binary.bin == self.entry) {
            return Err("entry must identify a declared binary".into());
        }
        let mut directories = BTreeMap::new();
        for resource in &self.resources {
            relative(&resource.source)?;
            relative(&resource.destination)?;
            for (index, _) in resource.destination.match_indices('/') {
                let directory = &resource.destination[..index];
                if directories
                    .insert(directory.to_lowercase(), directory)
                    .is_some_and(|previous| previous != directory)
                {
                    return Err("payload directory has inconsistent casing".into());
                }
            }
            if !names.insert(resource.destination.to_lowercase()) {
                return Err("case-insensitive payload destination collision".into());
            }
        }
        for name in &names {
            for (i, _) in name.match_indices('/') {
                if names.contains(&name[..i]) {
                    return Err("payload file also used as a directory".into());
                }
            }
        }
        Ok(())
    }
}

fn identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 100
        || !value.starts_with(|c: char| c.is_ascii_alphanumeric())
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    {
        return Err("identity/target must be a bounded ASCII identifier".into());
    }
    relative(value)
}

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

pub(crate) fn source(root: &Path, relative_path: &str) -> Result<PathBuf> {
    relative(relative_path)?;
    let mut current = root.to_path_buf();
    for part in Path::new(relative_path).components() {
        current.push(part);
        let metadata = fs::symlink_metadata(&current)?;
        if linked(&metadata) {
            return Err("linked application input is not admitted".into());
        }
    }
    if !current.is_file() {
        return Err("application input must be a regular file".into());
    }
    Ok(current)
}

pub(crate) fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT covers junctions as well as symlinks.
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(test)]
pub(crate) mod tests;
