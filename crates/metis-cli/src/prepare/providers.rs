//! Provider closure: registry checksums from the workspace lock and the
//! resolved package rows `metis prepare` records as provenance.

use crate::{Result, build::artifacts::closure};
use metis_core::embedded_assets::{ProviderRecord, ProviderSource};
use std::collections::BTreeMap;

/// Locked registry checksums from `Cargo.lock`, keyed by
/// `(name, version, source)`: the provenance pin `cargo metadata` no longer
/// reports (its graph carries `checksum: null` for every registry package
/// while the lock records them), so the lock is the single source of truth.
pub(super) type LockedChecksums = BTreeMap<(String, String, String), [u8; 32]>;

/// Parses the workspace lock into its registry checksums. A registry entry
/// without a checksum is unpinned and rejected by name; other sources carry
/// no checksum by construction and are skipped.
pub(super) fn lock_checksums(lock: &str) -> Result<LockedChecksums> {
    let document: toml::Table = toml::from_str(lock)?;
    let packages = document
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("workspace lock omits its package table")?;
    let mut checksums = LockedChecksums::new();
    for package in packages {
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or("locked package omits its name")?;
        let version = package
            .get("version")
            .and_then(toml::Value::as_str)
            .ok_or("locked package omits its version")?;
        let source = package
            .get("source")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        if !source.starts_with("registry+") {
            continue;
        }
        let raw = package
            .get("checksum")
            .and_then(toml::Value::as_str)
            .ok_or(format!(
                "locked registry package omits its checksum: {name} {version}"
            ))?;
        checksums.insert(
            (name.to_owned(), version.to_owned(), source.to_owned()),
            hex_to_digest(raw)?,
        );
    }
    Ok(checksums)
}

/// Workspace packages reachable from `root_id` through the resolved graph,
/// as `(name, version, source)` tuples. Registry checksums are resolved
/// separately from the parsed lock (see [`lock_checksums`]).
type ProviderRow = (String, String, Option<String>);

fn provider_packages(metadata: &[u8], root_id: &str) -> Result<Vec<ProviderRow>> {
    let document: serde_json::Value = serde_json::from_slice(metadata)?;
    let packages = document
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("cargo metadata omitted packages")?;
    let mut records = Vec::new();
    for id in closure(&document, root_id)? {
        let package = packages
            .iter()
            .find(|candidate| candidate.get("id") == Some(id))
            .ok_or("resolved package omitted its manifest entry")?;
        let name = package
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or("package omitted its name")?;
        let version = package
            .get("version")
            .and_then(serde_json::Value::as_str)
            .ok_or("package omitted its version")?;
        records.push((
            name.to_owned(),
            version.to_owned(),
            package
                .get("source")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        ));
    }
    Ok(records)
}

pub(super) fn providers(
    metadata: &[u8],
    root_id: &str,
    checksums: &LockedChecksums,
) -> Result<Vec<ProviderRecord>> {
    let mut records = Vec::new();
    for (name, version, source) in provider_packages(metadata, root_id)? {
        let package_id = format!("{name} {version}");
        let source = match source.as_deref() {
            None => ProviderSource::Path { member: name },
            Some(source) if source.starts_with("registry+") => {
                let key = (name.clone(), version.clone(), source.to_owned());
                let checksum = checksums.get(&key).copied().ok_or(format!(
                    "locked registry checksum is missing for {package_id}"
                ))?;
                ProviderSource::Registry { checksum }
            }
            Some(source) if source.starts_with("git+") => {
                let (url, revision) = split_git_source(source)?;
                ProviderSource::Git { url, revision }
            }
            Some(source) => {
                return Err(
                    format!("unsupported provider source for {package_id}: {source}").into(),
                );
            }
        };
        records.push(ProviderRecord {
            package_id,
            version,
            source,
        });
    }
    Ok(records)
}

pub(super) fn split_git_source(source: &str) -> Result<(String, String)> {
    let rest = source
        .strip_prefix("git+")
        .ok_or("git source lacks its scheme")?;
    let (url, revision) = rest
        .rsplit_once('#')
        .ok_or("git source lacks its resolved revision")?;
    let url = url.split('?').next().unwrap_or(url).to_owned();
    if url.is_empty() || revision.is_empty() {
        return Err("git source lacks its URL or revision".into());
    }
    Ok((url, revision.to_owned()))
}

pub(super) fn hex_to_digest(hex: &str) -> Result<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("registry checksum is not 64 hexadecimal digits".into());
    }
    let mut out = [0u8; 32];
    for (index, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
        let pair = core::str::from_utf8(chunk).map_err(|_| "registry checksum is not ASCII")?;
        out[index] =
            u8::from_str_radix(pair, 16).map_err(|_| "registry checksum is not hexadecimal")?;
    }
    Ok(out)
}
