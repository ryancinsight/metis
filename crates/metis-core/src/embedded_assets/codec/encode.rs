//! Bounded encoding with fallible allocation before every appended field.
use super::{MAGIC, SCHEMA};
use crate::embedded_assets::inventory::invalid;
use crate::embedded_assets::{
    AssetKind, Digest, FileIdentity, InventorySpec, MANIFEST_LIMIT, ProviderSource, ToolKind,
};
use std::io;

pub(crate) struct Writer {
    bytes: Vec<u8>,
}
impl Writer {
    pub(crate) fn new() -> Self {
        Self { bytes: Vec::new() }
    }
    pub(crate) fn put(&mut self, bytes: &[u8]) -> io::Result<()> {
        let total = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| invalid("encoded length overflow"))?;
        if u64::try_from(total).map_err(|_| invalid("encoded length overflow"))? > MANIFEST_LIMIT {
            return Err(invalid("encoded inventory exceeds 1 MiB"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|error| invalid(format!("inventory allocation: {error}")))?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub(crate) fn byte(&mut self, value: u8) -> io::Result<()> {
        self.put(&[value])
    }
    pub(crate) fn count(&mut self, value: usize) -> io::Result<()> {
        self.put(
            &u32::try_from(value)
                .map_err(|_| invalid("sequence count overflow"))?
                .to_be_bytes(),
        )
    }
    pub(crate) fn text(&mut self, value: &str) -> io::Result<()> {
        self.count(value.len())?;
        self.put(value.as_bytes())
    }
    pub(crate) fn digest(&mut self, value: Digest) -> io::Result<()> {
        self.put(value.as_bytes())
    }
    pub(crate) fn identity(&mut self, value: &FileIdentity) -> io::Result<()> {
        self.put(&value.bytes.to_be_bytes())?;
        self.digest(value.digest)
    }
    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
pub(in super::super) fn encode(spec: &InventorySpec) -> io::Result<Vec<u8>> {
    let mut output = Writer::new();
    output.put(MAGIC)?;
    output.put(&SCHEMA.to_be_bytes())?;
    let module = &spec.module;
    for text in [
        &module.package,
        &module.package_id,
        &module.target,
        &module.wasm_target,
        &module.export,
    ] {
        output.text(text)?;
    }
    output.byte(1)?; // Windows, the only admitted native consumer platform.
    output.put(&module.page_schema.to_be_bytes())?;
    output.digest(spec.lock_digest)?;
    output.count(spec.providers.len())?;
    for provider in &spec.providers {
        for text in [&provider.package_id, &provider.package, &provider.version] {
            output.text(text)?;
        }
        match &provider.source {
            ProviderSource::Workspace => output.byte(1)?,
            ProviderSource::Git { url, revision } => {
                output.byte(2)?;
                output.text(url)?;
                output.text(revision)?;
            }
            ProviderSource::Registry { url, checksum } => {
                output.byte(3)?;
                output.text(url)?;
                output.digest(*checksum)?;
            }
        }
    }
    for tool in &spec.tools {
        output.byte(match tool.kind {
            ToolKind::Cargo => 1,
            ToolKind::Rustc => 2,
            ToolKind::WasmBindgen => 3,
        })?;
        output.text(&tool.version)?;
        output.identity(&tool.executable)?;
    }
    output.text(&spec.configuration.profile)?;
    output.count(spec.configuration.features.len())?;
    for feature in &spec.configuration.features {
        output.text(feature)?;
    }
    output.count(spec.configuration.compiler_args.len())?;
    for arg in &spec.configuration.compiler_args {
        output.text(arg)?;
    }
    output.count(spec.configuration.environment.len())?;
    for (key, value) in &spec.configuration.environment {
        output.text(key)?;
        match value {
            Some(value) => {
                output.byte(1)?;
                output.text(value)?;
            }
            None => output.byte(0)?,
        }
    }
    output.count(spec.directories.len())?;
    for directory in &spec.directories {
        output.text(&directory.owner_id)?;
        match &directory.path {
            Some(path) => {
                output.byte(1)?;
                output.text(path)?;
            }
            None => output.byte(0)?,
        }
        output.byte(match directory.policy {
            crate::embedded_assets::DirectoryPolicy::Declared => 1,
            crate::embedded_assets::DirectoryPolicy::CargoPackage => 2,
        })?;
        output.digest(directory.digest)?;
    }
    output.count(spec.inputs.len())?;
    for input in &spec.inputs {
        output.text(&input.owner_id)?;
        output.text(&input.path)?;
        output.identity(&input.identity)?;
    }
    output.count(spec.assets.len())?;
    for asset in &spec.assets {
        output.byte(match asset.kind {
            AssetKind::JavaScript => 1,
            AssetKind::WebAssembly => 2,
        })?;
        output.text(&asset.destination)?;
        output.identity(&asset.identity)?;
    }
    Ok(output.finish())
}
