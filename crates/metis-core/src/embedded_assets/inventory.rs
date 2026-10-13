//! Semantic validation precedes encoding and follows decoding.
use super::{
    AssetKind, Digest, FILE_LIMIT, InventorySpec, MANIFEST_LIMIT, MODULE_LIMIT, PAYLOAD_LIMIT,
    ToolKind, relative,
};
use std::{collections::BTreeSet, io};

pub(super) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Validated immutable semantic identity of one generated module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedModuleInventory {
    spec: InventorySpec,
}
impl EmbeddedModuleInventory {
    /// Admits a complete, sorted, bounded specification.
    ///
    /// # Errors
    /// Rejects invalid schema identities, sorting, paths, counts and byte bounds.
    pub fn new(spec: InventorySpec) -> io::Result<Self> {
        validate(&spec)?;
        let inventory = Self { spec };
        if u64::try_from(inventory.encode()?.len())
            .map_err(|_| invalid("inventory length overflow"))?
            > MANIFEST_LIMIT
        {
            return Err(invalid("inventory exceeds 1 MiB"));
        }
        Ok(inventory)
    }
    /// Borrows the admitted fields.
    #[must_use]
    pub const fn spec(&self) -> &InventorySpec {
        &self.spec
    }
    /// Produces the unique METISAS1 binary encoding.
    ///
    /// # Errors
    /// Returns an error when encoded lengths exceed the committed budget.
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        super::codec::encode(&self.spec)
    }
    /// Decodes and validates the entire bounded binary value.
    ///
    /// # Errors
    /// Rejects unknown, truncated, trailing, unsorted or invalid values.
    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        Self::new(super::codec::decode(bytes)?)
    }
    /// SHA-256 of the canonical inventory encoding.
    ///
    /// # Errors
    /// Propagates an encoding bound failure.
    pub fn digest(&self) -> io::Result<Digest> {
        Ok(Digest::hash(&self.encode()?))
    }
}

fn text(value: &str) -> io::Result<()> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(invalid("inventory identity is empty or contains controls"));
    }
    Ok(())
}
fn identifier(value: &str) -> io::Result<()> {
    text(value)?;
    if value.contains("file:") || value.starts_with('/') || value.contains('\\') {
        return Err(invalid(
            "semantic owner identity contains a local absolute path",
        ));
    }
    Ok(())
}
fn sorted<'a>(items: impl Iterator<Item = &'a str>) -> io::Result<()> {
    let mut prior = None;
    for item in items {
        text(item)?;
        if prior.is_some_and(|previous| previous >= item) {
            return Err(invalid("inventory records must be sorted and unique"));
        }
        prior = Some(item);
    }
    Ok(())
}
fn paths<'a>(items: impl Iterator<Item = (&'a str, &'a str)>) -> io::Result<()> {
    let mut previous = None;
    let mut folded = BTreeSet::new();
    let mut directories = std::collections::BTreeMap::new();
    for (owner, path) in items {
        identifier(owner)?;
        relative(path)?;
        if previous.is_some_and(|pair| pair >= (owner, path)) {
            return Err(invalid("input paths must be sorted and unique"));
        }
        previous = Some((owner, path));
        if !folded.insert((owner.to_owned(), path.to_lowercase())) {
            return Err(invalid("case-insensitive input path collision"));
        }
        for (index, _) in path.match_indices('/') {
            let directory = &path[..index];
            if directories
                .insert((owner.to_owned(), directory.to_lowercase()), directory)
                .is_some_and(|prior| prior != directory)
            {
                return Err(invalid("input directory casing collision"));
            }
        }
    }
    for (owner, path) in &folded {
        for (index, _) in path.match_indices('/') {
            if folded.contains(&(owner.clone(), path[..index].to_owned())) {
                return Err(invalid("input file is also a directory"));
            }
        }
    }
    Ok(())
}
fn validate(spec: &InventorySpec) -> io::Result<()> {
    for value in [
        &spec.module.package,
        &spec.module.package_id,
        &spec.module.target,
        &spec.module.export,
    ] {
        identifier(value)?;
    }
    if spec.module.wasm_target != "wasm32-unknown-unknown"
        || spec.module.export != "metis_embedded_start"
        || spec.module.page_schema != 1
    {
        return Err(invalid(
            "unsupported embedded module target, export or page schema",
        ));
    }
    if spec.providers.is_empty()
        || spec.inputs.is_empty()
        || spec
            .providers
            .len()
            .checked_add(spec.inputs.len())
            .and_then(|n| n.checked_add(spec.directories.len()))
            .and_then(|n| n.checked_add(2))
            .is_none_or(|n| n > FILE_LIMIT)
    {
        return Err(invalid(
            "inventory requires bounded nonempty provider/input records",
        ));
    }
    providers(spec)?;
    configuration(spec)?;
    let mut prior_directory = None;
    for directory in &spec.directories {
        identifier(&directory.owner_id)?;
        if directory.policy == super::DirectoryPolicy::CargoPackage && directory.path.is_some() {
            return Err(invalid(
                "package-tree directory policy requires owner-root scope",
            ));
        }
        if let Some(path) = &directory.path {
            relative(path)?;
        }
        let key = (&directory.owner_id, &directory.path);
        if prior_directory.is_some_and(|previous| previous >= key) {
            return Err(invalid("declared directories must be sorted and unique"));
        }
        prior_directory = Some(key);
    }
    paths(
        spec.inputs
            .iter()
            .map(|input| (input.owner_id.as_str(), input.path.as_str())),
    )?;
    paths(
        spec.assets
            .iter()
            .map(|asset| ("assets", asset.destination.as_str())),
    )?;
    payload(spec)?;
    Ok(())
}

fn providers(spec: &InventorySpec) -> io::Result<()> {
    sorted(spec.providers.iter().map(|p| p.package_id.as_str()))?;
    for provider in &spec.providers {
        identifier(&provider.package_id)?;
        text(&provider.package)?;
        text(&provider.version)?;
        if provider.package_id != provider.canonical_id() {
            return Err(invalid(
                "provider identity differs from canonical locked source fields",
            ));
        }
        match &provider.source {
            super::ProviderSource::Workspace => {
                if provider.package_id
                    != format!("workspace:{}@{}", provider.package, provider.version)
                {
                    return Err(invalid("workspace provider identity is not canonical"));
                }
            }
            super::ProviderSource::Git { url, revision } => {
                identifier(url)?;
                if revision.len() != 40
                    || !revision
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(invalid("Git revision requires exact lowercase commit"));
                }
            }
            super::ProviderSource::Registry { url, .. } => identifier(url)?,
        }
    }
    if !spec.providers.iter().any(|p| {
        p.package_id == spec.module.package_id
            && p.package == spec.module.package
            && matches!(p.source, super::ProviderSource::Workspace)
    }) {
        return Err(invalid("selected module is not a workspace provider"));
    }
    Ok(())
}

fn configuration(spec: &InventorySpec) -> io::Result<()> {
    for (tool, expected) in
        spec.tools
            .iter()
            .zip([ToolKind::Cargo, ToolKind::Rustc, ToolKind::WasmBindgen])
    {
        if tool.kind != expected || tool.executable.bytes == 0 {
            return Err(invalid("tool roles or executable lengths are invalid"));
        }
        text(&tool.version)?;
    }
    if spec.configuration.profile != "release" {
        return Err(invalid("embedded WASM requires release profile"));
    }
    sorted(spec.configuration.features.iter().map(String::as_str))?;
    sorted(
        spec.configuration
            .environment
            .iter()
            .map(|(key, _)| key.as_str()),
    )?;
    for (key, value) in &spec.configuration.environment {
        if !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || value.as_ref().is_some_and(|value| value.contains('\0'))
        {
            return Err(invalid("invalid build environment entry"));
        }
    }
    for arg in &spec.configuration.compiler_args {
        if arg.contains('\0') {
            return Err(invalid("compiler argument contains a zero byte"));
        }
    }
    Ok(())
}

fn payload(spec: &InventorySpec) -> io::Result<()> {
    if spec
        .assets
        .iter()
        .filter(|a| a.kind == AssetKind::JavaScript)
        .count()
        != 1
        || spec
            .assets
            .iter()
            .filter(|a| a.kind == AssetKind::WebAssembly)
            .count()
            != 1
    {
        return Err(invalid("inventory requires exactly JavaScript and WASM"));
    }
    let mut payload = 0_u64;
    for asset in &spec.assets {
        if asset.identity.bytes == 0 || asset.identity.bytes > MODULE_LIMIT {
            return Err(invalid("module exceeds 64 MiB or is empty"));
        }
        let extension = match asset.kind {
            AssetKind::JavaScript => "js",
            AssetKind::WebAssembly => "wasm",
        };
        if !asset.destination.ends_with(&format!(".{extension}")) {
            return Err(invalid("generated asset extension differs from role"));
        }
        payload = payload
            .checked_add(asset.identity.bytes)
            .ok_or_else(|| invalid("module size overflow"))?;
    }
    for input in &spec.inputs {
        payload = payload
            .checked_add(input.identity.bytes)
            .ok_or_else(|| invalid("input size overflow"))?;
    }
    if payload > PAYLOAD_LIMIT {
        return Err(invalid("prepared inputs and modules exceed 1 GiB"));
    }
    Ok(())
}
