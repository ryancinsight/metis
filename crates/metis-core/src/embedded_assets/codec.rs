//! Canonical binary encoding for [`EmbeddedModuleInventory`](super::EmbeddedModuleInventory).
//!
//! [`encode`] always emits canonical bytes; [`decode`] accepts only canonical
//! bytes. See the parent module for the field order and the rejected classes.

use super::types::{
    AssetKind, AssetRecord, BuildConfiguration, EmbeddedModuleInventory, INVENTORY_MAGIC,
    INVENTORY_SCHEMA, InputRecord, InventoryError, MAX_MANIFEST_BYTES, MAX_MODULE_BYTES,
    MAX_RECORDS, ProviderRecord, ProviderSource, Result, ToolRecord, check_assets,
    check_input_order, check_inputs, check_path,
};

/// Encode `inventory` to canonical bytes.
///
/// Sorting is applied (providers by package id, tools by name, features
/// sorted and deduplicated, environment by key then value, inputs by owner
/// then path, assets by destination), so equal inventories always produce
/// equal bytes. The tool records identify `cargo`, `rustc`, and `wasm-bindgen`.
/// The codec requires exactly one JavaScript plus one WASM asset, bounded counts,
/// valid portable paths, no duplicates or case collisions, asset and
/// manifest budgets.
///
/// # Errors
///
/// Returns the violated rule; nothing is written on failure.
pub fn encode(inventory: &EmbeddedModuleInventory) -> Result<Vec<u8>> {
    if inventory.tools.len() != TOOL_NAMES.len() {
        return Err(InventoryError::BadToolSet);
    }
    if inventory.inputs.len() > MAX_RECORDS
        || inventory.providers.len() > MAX_RECORDS
        || inventory.assets.len() > MAX_RECORDS
        || inventory.config.features.len() > MAX_RECORDS
        || inventory.config.rustflags.len() > MAX_RECORDS
        || inventory.config.environment.len() > MAX_RECORDS
    {
        return Err(InventoryError::TooManyRecords);
    }
    let mut providers = inventory.providers.iter().collect::<Vec<_>>();
    providers.sort_unstable_by(|a, b| a.package_id.cmp(&b.package_id));
    let mut tools = inventory.tools.iter().collect::<Vec<_>>();
    tools.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    let mut features = inventory.config.features.iter().collect::<Vec<_>>();
    features.sort_unstable();
    features.dedup();
    let mut environment = inventory.config.environment.iter().collect::<Vec<_>>();
    environment.sort_unstable();
    let mut inputs = inventory.inputs.iter().collect::<Vec<_>>();
    inputs.sort_unstable_by(|a, b| (&a.owner, &a.path).cmp(&(&b.owner, &b.path)));
    let mut assets = inventory.assets.iter().collect::<Vec<_>>();
    assets.sort_unstable_by(|a, b| a.destination.cmp(&b.destination));
    check_providers(providers.iter().copied())?;
    check_tools(tools.iter().copied())?;
    check_strict_order(environment.iter().copied())?;
    check_assets(&assets)?;

    let mut out = Writer::default();
    push_identity(&mut out, inventory, &providers, &tools)?;
    push_config(&mut out, &inventory.config, &features, &environment)?;
    push_inputs(&mut out, &inputs)?;
    push_assets(&mut out, &assets)?;
    // The byte budget now bounds the owner/path data used for collision checks.
    check_inputs(&inventory.inputs)?;
    Ok(out.bytes)
}

fn push_identity(
    out: &mut Writer,
    inventory: &EmbeddedModuleInventory,
    providers: &[&ProviderRecord],
    tools: &[&ToolRecord],
) -> Result<()> {
    out.write(INVENTORY_MAGIC)?;
    out.write(&INVENTORY_SCHEMA.to_be_bytes())?;
    out.string(&inventory.package)?;
    out.string(&inventory.package_id)?;
    out.string(&inventory.cdylib_target)?;
    out.string(&inventory.wasm_target)?;
    out.string(&inventory.export_name)?;
    out.string(&inventory.platform_tag)?;
    out.write(&inventory.page_schema.to_be_bytes())?;
    out.write(&inventory.lock_digest)?;
    out.length(providers.len())?;
    for provider in providers {
        out.string(&provider.package_id)?;
        out.string(&provider.version)?;
        match &provider.source {
            ProviderSource::Git { url, revision } => {
                out.write(&[0])?;
                out.string(url)?;
                out.string(revision)?;
            }
            ProviderSource::Registry { checksum } => {
                out.write(&[1])?;
                out.write(checksum)?;
            }
            ProviderSource::Path { member } => {
                out.write(&[2])?;
                out.string(member)?;
            }
        }
    }
    out.length(tools.len())?;
    for tool in tools {
        out.string(&tool.name)?;
        out.string(&tool.version_output)?;
        out.write(&tool.executable_digest)?;
    }
    Ok(())
}

fn push_config(
    out: &mut Writer,
    config: &BuildConfiguration,
    features: &[&String],
    environment: &[&(String, String)],
) -> Result<()> {
    out.string(&config.profile)?;
    out.length(features.len())?;
    for feature in features {
        out.string(feature)?;
    }
    out.length(config.rustflags.len())?;
    for flag in &config.rustflags {
        out.string(flag)?;
    }
    out.length(environment.len())?;
    for (key, value) in environment {
        out.string(key)?;
        out.string(value)?;
    }
    Ok(())
}

fn push_inputs(out: &mut Writer, inputs: &[&InputRecord]) -> Result<()> {
    out.length(inputs.len())?;
    for input in inputs {
        out.string(&input.owner)?;
        out.string(&input.path)?;
        out.write(&input.length.to_be_bytes())?;
        out.write(&input.digest)?;
    }
    Ok(())
}

fn push_assets(out: &mut Writer, assets: &[&AssetRecord]) -> Result<()> {
    out.length(assets.len())?;
    for asset in assets {
        out.write(&[match asset.kind {
            AssetKind::JavaScript => 0,
            AssetKind::Wasm => 1,
        }])?;
        out.string(&asset.destination)?;
        out.write(&asset.length.to_be_bytes())?;
        out.write(&asset.digest)?;
    }
    Ok(())
}

/// Decode canonical `bytes` into an [`EmbeddedModuleInventory`].
///
/// Only canonical bytes are accepted; see the parent module for the rejected
/// classes.
///
/// # Errors
///
/// Returns the first violated rule encountered.
pub fn decode(bytes: &[u8]) -> Result<EmbeddedModuleInventory> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(InventoryError::OversizeManifest);
    }
    let mut cursor = Cursor::new(bytes);
    let magic = cursor.take(INVENTORY_MAGIC.len())?;
    if magic != INVENTORY_MAGIC {
        return Err(InventoryError::UnknownSchema(0));
    }
    let schema = u16::from_be_bytes(cursor.fixed()?);
    if schema != INVENTORY_SCHEMA {
        return Err(InventoryError::UnknownSchema(schema));
    }
    let package = cursor.string()?;
    let package_id = cursor.string()?;
    let cdylib_target = cursor.string()?;
    let wasm_target = cursor.string()?;
    let export_name = cursor.string()?;
    let platform_tag = cursor.string()?;
    let page_schema = u16::from_be_bytes(cursor.fixed()?);
    let lock_digest = cursor.digest()?;
    let providers = pull_providers(&mut cursor)?;
    let tools = pull_tools(&mut cursor)?;
    let config = pull_config(&mut cursor)?;
    let inputs = pull_inputs(&mut cursor)?;
    let assets = pull_assets(&mut cursor)?;
    if cursor.remaining() != 0 {
        return Err(InventoryError::TrailingBytes);
    }
    Ok(EmbeddedModuleInventory {
        package,
        package_id,
        cdylib_target,
        wasm_target,
        export_name,
        platform_tag,
        page_schema,
        lock_digest,
        providers,
        tools,
        config,
        inputs,
        assets,
    })
}

fn pull_providers(cursor: &mut Cursor<'_>) -> Result<Vec<ProviderRecord>> {
    let mut providers = Vec::new();
    for _ in 0..cursor.count()? {
        let package_id = cursor.string()?;
        let version = cursor.string()?;
        let tag = cursor.tag()?;
        let source = match tag {
            0 => ProviderSource::Git {
                url: cursor.string()?,
                revision: cursor.string()?,
            },
            1 => ProviderSource::Registry {
                checksum: cursor.digest()?,
            },
            2 => ProviderSource::Path {
                member: cursor.string()?,
            },
            other => return Err(InventoryError::UnknownTag(other)),
        };
        providers.push(ProviderRecord {
            package_id,
            version,
            source,
        });
    }
    check_providers(providers.iter())?;
    Ok(providers)
}

fn pull_tools(cursor: &mut Cursor<'_>) -> Result<Vec<ToolRecord>> {
    let mut tools = Vec::new();
    let count = cursor.count()?;
    if count != TOOL_NAMES.len() {
        return Err(InventoryError::BadToolSet);
    }
    for _ in 0..count {
        tools.push(ToolRecord {
            name: cursor.string()?,
            version_output: cursor.string()?,
            executable_digest: cursor.digest()?,
        });
    }
    check_tools(tools.iter())?;
    Ok(tools)
}

fn pull_config(cursor: &mut Cursor<'_>) -> Result<BuildConfiguration> {
    let profile = cursor.string()?;
    let mut features = Vec::new();
    for _ in 0..cursor.count()? {
        features.push(cursor.string()?);
    }
    check_strict_order(features.iter())?;
    let mut rustflags = Vec::new();
    for _ in 0..cursor.count()? {
        rustflags.push(cursor.string()?);
    }
    let mut environment = Vec::new();
    for _ in 0..cursor.count()? {
        let key = cursor.string()?;
        let value = cursor.string()?;
        environment.push((key, value));
    }
    check_strict_order(environment.iter())?;
    Ok(BuildConfiguration {
        profile,
        features,
        rustflags,
        environment,
    })
}

fn pull_inputs(cursor: &mut Cursor<'_>) -> Result<Vec<InputRecord>> {
    let mut inputs = Vec::new();
    for _ in 0..cursor.count()? {
        let owner = cursor.string()?;
        let path = cursor.string()?;
        check_path(&path)?;
        let length = u64::from_be_bytes(cursor.fixed()?);
        let digest = cursor.digest()?;
        inputs.push(InputRecord {
            owner,
            path,
            length,
            digest,
        });
    }
    check_inputs(&inputs)?;
    check_input_order(&inputs)?;
    Ok(inputs)
}

fn pull_assets(cursor: &mut Cursor<'_>) -> Result<Vec<AssetRecord>> {
    let mut assets = Vec::new();
    let count = cursor.count()?;
    if count != 2 {
        return Err(InventoryError::BadAssetSet);
    }
    for _ in 0..count {
        let kind = match cursor.tag()? {
            0 => AssetKind::JavaScript,
            1 => AssetKind::Wasm,
            other => return Err(InventoryError::UnknownTag(other)),
        };
        let destination = cursor.string()?;
        check_path(&destination)?;
        let length = u64::from_be_bytes(cursor.fixed()?);
        if length > MAX_MODULE_BYTES {
            return Err(InventoryError::OversizeAsset);
        }
        let digest = cursor.digest()?;
        assets.push(AssetRecord {
            kind,
            destination,
            length,
            digest,
        });
    }
    check_assets(&assets)?;
    Ok(assets)
}

const TOOL_NAMES: [&str; 3] = ["cargo", "rustc", "wasm-bindgen"];

fn check_strict_order<T: Ord>(keys: impl Iterator<Item = T>) -> Result<()> {
    if keys.is_sorted_by(|a, b| a < b) {
        Ok(())
    } else {
        Err(InventoryError::UnsortedRecords)
    }
}

fn check_providers<'a>(providers: impl Iterator<Item = &'a ProviderRecord>) -> Result<()> {
    check_strict_order(providers.map(|p| &p.package_id))
}

fn check_tools<'a>(tools: impl Iterator<Item = &'a ToolRecord> + Clone) -> Result<()> {
    let names = tools.map(|tool| tool.name.as_str());
    check_strict_order(names.clone())?;
    if names.eq(TOOL_NAMES) {
        Ok(())
    } else {
        Err(InventoryError::BadToolSet)
    }
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_MANIFEST_BYTES - self.bytes.len() {
            return Err(InventoryError::OversizeManifest);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn length(&mut self, length: usize) -> Result<()> {
        let length = u32::try_from(length).map_err(|_| InventoryError::OversizeManifest)?;
        self.write(&length.to_be_bytes())
    }

    fn string(&mut self, value: &str) -> Result<()> {
        self.length(value.len())?;
        self.write(value.as_bytes())
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(InventoryError::Truncated)?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(InventoryError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.take(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn count(&mut self) -> Result<usize> {
        let count = u32::from_be_bytes(self.fixed()?);
        let count = usize::try_from(count).map_err(|_| InventoryError::TooManyRecords)?;
        if count > MAX_RECORDS {
            return Err(InventoryError::TooManyRecords);
        }
        Ok(count)
    }

    fn string(&mut self) -> Result<String> {
        let len = u32::from_be_bytes(self.fixed()?);
        let remaining = u64::try_from(self.remaining()).map_err(|_| InventoryError::Truncated)?;
        if u64::from(len) > remaining {
            return Err(InventoryError::Truncated);
        }
        // Exact: len fits the remaining in-memory bytes, which fit in usize.
        let len = usize::try_from(len).map_err(|_| InventoryError::Truncated)?;
        let bytes = self.take(len)?;
        core::str::from_utf8(bytes)
            .map_err(|_| InventoryError::InvalidUtf8)
            .map(str::to_owned)
    }

    fn digest(&mut self) -> Result<[u8; 32]> {
        self.fixed()
    }

    fn tag(&mut self) -> Result<u8> {
        let bytes = self.take(1)?;
        Ok(bytes[0])
    }
}
