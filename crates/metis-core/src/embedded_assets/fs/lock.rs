//! Exact current Cargo.lock provider admission uses the complete TOML parser.
use super::super::{EmbeddedModuleInventory, ProviderSource, inventory::invalid};
use serde::Deserialize;
use std::{collections::BTreeMap, io};

#[derive(Deserialize)]
struct Lock {
    version: u16,
    package: Vec<Package>,
}
#[derive(Deserialize)]
struct Package {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

pub(super) fn providers(inventory: &EmbeddedModuleInventory, bytes: &[u8]) -> io::Result<()> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| invalid(format!("Cargo.lock UTF-8: {error}")))?;
    let lock: Lock =
        toml::from_str(text).map_err(|error| invalid(format!("Cargo.lock TOML: {error}")))?;
    if lock.version != 4 {
        return Err(invalid("unsupported Cargo.lock format"));
    }
    let mut packages = BTreeMap::new();
    for package in lock.package {
        let key = (
            package.name.clone(),
            package.version.clone(),
            package.source.clone(),
        );
        if packages.insert(key, package).is_some() {
            return Err(invalid("duplicate Cargo.lock package identity"));
        }
    }
    for provider in &inventory.spec().providers {
        let source = match &provider.source {
            ProviderSource::Workspace => None,
            ProviderSource::Git { url, revision } => Some(format!("{url}#{revision}")),
            ProviderSource::Registry { url, .. } => Some(url.clone()),
        };
        let package = packages
            .get(&(provider.package.clone(), provider.version.clone(), source))
            .ok_or_else(|| {
                invalid(format!(
                    "provider is not current locked identity: {}",
                    provider.package_id
                ))
            })?;
        match &provider.source {
            ProviderSource::Registry { checksum, .. } => {
                if package.checksum.as_deref() != Some(checksum.to_string().as_str()) {
                    return Err(invalid(
                        "registry provider checksum differs from current lock",
                    ));
                }
            }
            ProviderSource::Workspace | ProviderSource::Git { .. } => {
                if package.checksum.is_some() {
                    return Err(invalid("nonregistry provider has a checksum"));
                }
            }
        }
    }
    Ok(())
}
