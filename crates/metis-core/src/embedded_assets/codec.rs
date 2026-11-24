//! Canonical binary encoding for [`EmbeddedModuleInventory`](super::EmbeddedModuleInventory).
//!
//! [`encode`] always emits canonical bytes; [`decode`] accepts only canonical
//! bytes. See the parent module for the field order and the rejected classes.

use super::{
    AssetKind, AssetRecord, BuildConfiguration, EmbeddedModuleInventory, INVENTORY_MAGIC,
    INVENTORY_SCHEMA, InputRecord, InventoryError, MAX_MANIFEST_BYTES, MAX_MODULE_BYTES,
    MAX_RECORDS, ProviderRecord, ProviderSource, Result, ToolRecord, check_assets,
    check_input_order, check_inputs, check_path, check_string,
};

/// Encode `inventory` to canonical bytes.
///
/// Sorting is applied (providers by package id, tools by name, features
/// sorted and deduplicated, environment by key then value, inputs by owner
/// then path, assets by destination), so equal inventories always produce
/// equal bytes. Every other canonical rule is enforced: exactly three tool
/// records, exactly one JavaScript plus one WASM asset, bounded counts,
/// valid portable paths, no duplicates or case collisions, asset and
/// manifest budgets.
///
/// # Errors
///
/// Returns the violated rule; nothing is written on failure.
pub fn encode(inventory: &EmbeddedModuleInventory) -> Result<Vec<u8>> {
    if inventory.tools.len() != 3 {
        return Err(InventoryError::BadToolSet);
    }
    if inventory.inputs.len() > MAX_RECORDS
        || inventory.providers.len() > MAX_RECORDS
        || inventory.tools.len() > MAX_RECORDS
        || inventory.assets.len() > MAX_RECORDS
        || inventory.config.features.len() > MAX_RECORDS
        || inventory.config.rustflags.len() > MAX_RECORDS
        || inventory.config.environment.len() > MAX_RECORDS
    {
        return Err(InventoryError::TooManyRecords);
    }
    let mut providers = inventory.providers.clone();
    providers.sort_by(|a, b| a.package_id.cmp(&b.package_id));
    let mut tools = inventory.tools.clone();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    let mut features = inventory.config.features.clone();
    features.sort();
    features.dedup();
    let mut environment = inventory.config.environment.clone();
    environment.sort();
    let mut inputs = inventory.inputs.clone();
    inputs.sort_by(|a, b| (&a.owner, &a.path).cmp(&(&b.owner, &b.path)));
    let mut assets = inventory.assets.clone();
    assets.sort_by(|a, b| a.destination.cmp(&b.destination));
    check_inputs(&inputs)?;
    check_assets(&assets)?;

    let mut out = Vec::new();
    push_identity(&mut out, inventory, &providers, &tools)?;
    push_config(&mut out, &inventory.config, &features, &environment)?;
    push_inputs(&mut out, &inputs)?;
    push_assets(&mut out, &assets)?;
    if out.len() > MAX_MANIFEST_BYTES {
        return Err(InventoryError::OversizeManifest);
    }
    Ok(out)
}

fn push_identity(
    out: &mut Vec<u8>,
    inventory: &EmbeddedModuleInventory,
    providers: &[ProviderRecord],
    tools: &[ToolRecord],
) -> Result<()> {
    out.extend_from_slice(INVENTORY_MAGIC);
    push_u16(out, INVENTORY_SCHEMA);
    push_string(out, &inventory.package)?;
    push_string(out, &inventory.package_id)?;
    push_string(out, &inventory.cdylib_target)?;
    push_string(out, &inventory.wasm_target)?;
    push_string(out, &inventory.export_name)?;
    push_string(out, &inventory.platform_tag)?;
    push_u16(out, inventory.page_schema);
    out.extend_from_slice(&inventory.lock_digest);
    for provider in providers {
        check_string(&provider.package_id)?;
        check_string(&provider.version)?;
        match &provider.source {
            ProviderSource::Git { url, revision } => {
                check_string(url)?;
                check_string(revision)?;
            }
            ProviderSource::Registry { .. } => {}
            ProviderSource::Path { member } => check_string(member)?,
        }
    }
    for tool in tools {
        check_string(&tool.name)?;
        check_string(&tool.version_output)?;
    }
    push_u32(out, u32_len(providers.len())?);
    for provider in providers {
        push_string(out, &provider.package_id)?;
        push_string(out, &provider.version)?;
        match &provider.source {
            ProviderSource::Git { url, revision } => {
                out.push(0);
                push_string(out, url)?;
                push_string(out, revision)?;
            }
            ProviderSource::Registry { checksum } => {
                out.push(1);
                out.extend_from_slice(checksum);
            }
            ProviderSource::Path { member } => {
                out.push(2);
                push_string(out, member)?;
            }
        }
    }
    push_u32(out, u32_len(tools.len())?);
    for tool in tools {
        push_string(out, &tool.name)?;
        push_string(out, &tool.version_output)?;
        out.extend_from_slice(&tool.executable_digest);
    }
    Ok(())
}

fn push_config(
    out: &mut Vec<u8>,
    config: &BuildConfiguration,
    features: &[String],
    environment: &[(String, String)],
) -> Result<()> {
    check_string(&config.profile)?;
    for feature in features {
        check_string(feature)?;
    }
    for flag in &config.rustflags {
        check_string(flag)?;
    }
    for (key, value) in environment {
        check_string(key)?;
        check_string(value)?;
    }
    push_string(out, &config.profile)?;
    push_u32(out, u32_len(features.len())?);
    for feature in features {
        push_string(out, feature)?;
    }
    push_u32(out, u32_len(config.rustflags.len())?);
    for flag in &config.rustflags {
        push_string(out, flag)?;
    }
    push_u32(out, u32_len(environment.len())?);
    for (key, value) in environment {
        push_string(out, key)?;
        push_string(out, value)?;
    }
    Ok(())
}

fn push_inputs(out: &mut Vec<u8>, inputs: &[InputRecord]) -> Result<()> {
    push_u32(out, u32_len(inputs.len())?);
    for input in inputs {
        push_string(out, &input.owner)?;
        push_string(out, &input.path)?;
        push_u64(out, input.length);
        out.extend_from_slice(&input.digest);
    }
    Ok(())
}

fn push_assets(out: &mut Vec<u8>, assets: &[AssetRecord]) -> Result<()> {
    push_u32(out, u32_len(assets.len())?);
    for asset in assets {
        out.push(match asset.kind {
            AssetKind::JavaScript => 0,
            AssetKind::Wasm => 1,
        });
        push_string(out, &asset.destination)?;
        push_u64(out, asset.length);
        out.extend_from_slice(&asset.digest);
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
    let mut cursor = Cursor::new(bytes);
    let magic = cursor.take(INVENTORY_MAGIC.len())?;
    if magic != INVENTORY_MAGIC {
        return Err(InventoryError::UnknownSchema(0));
    }
    let schema = cursor.u16_be()?;
    if schema != INVENTORY_SCHEMA {
        return Err(InventoryError::UnknownSchema(schema));
    }
    let package = cursor.string()?;
    let package_id = cursor.string()?;
    let cdylib_target = cursor.string()?;
    let wasm_target = cursor.string()?;
    let export_name = cursor.string()?;
    let platform_tag = cursor.string()?;
    let page_schema = cursor.u16_be()?;
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
    if providers
        .windows(2)
        .any(|pair| pair[0].package_id >= pair[1].package_id)
    {
        return Err(InventoryError::UnsortedRecords);
    }
    Ok(providers)
}

fn pull_tools(cursor: &mut Cursor<'_>) -> Result<Vec<ToolRecord>> {
    let mut tools = Vec::new();
    for _ in 0..cursor.count()? {
        tools.push(ToolRecord {
            name: cursor.string()?,
            version_output: cursor.string()?,
            executable_digest: cursor.digest()?,
        });
    }
    if tools.len() != 3 {
        return Err(InventoryError::BadToolSet);
    }
    if tools.windows(2).any(|pair| pair[0].name >= pair[1].name) {
        return Err(InventoryError::UnsortedRecords);
    }
    Ok(tools)
}

fn pull_config(cursor: &mut Cursor<'_>) -> Result<BuildConfiguration> {
    let profile = cursor.string()?;
    let mut features = Vec::new();
    for _ in 0..cursor.count()? {
        features.push(cursor.string()?);
    }
    if features.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(InventoryError::UnsortedRecords);
    }
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
    if environment.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(InventoryError::UnsortedRecords);
    }
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
        let length = cursor.u64_be()?;
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
    for _ in 0..cursor.count()? {
        let kind = match cursor.tag()? {
            0 => AssetKind::JavaScript,
            1 => AssetKind::Wasm,
            other => return Err(InventoryError::UnknownTag(other)),
        };
        let destination = cursor.string()?;
        check_path(&destination)?;
        let length = cursor.u64_be()?;
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

/// Convert a length to `u32`, rejecting values no manifest can hold.
fn u32_len(len: usize) -> Result<u32> {
    u32::try_from(len).map_err(|_| InventoryError::OversizeManifest)
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_string(out: &mut Vec<u8>, value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    push_u32(out, u32_len(bytes.len())?);
    out.extend_from_slice(bytes);
    Ok(())
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

    fn u16_be(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn u32_be(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64_be(&mut self) -> Result<u64> {
        let bytes = self.take(8)?;
        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn count(&mut self) -> Result<usize> {
        let value = self.u32_be()?;
        if usize::try_from(value).map_err(|_| InventoryError::TooManyRecords)? > MAX_RECORDS {
            return Err(InventoryError::TooManyRecords);
        }
        // The bound above proves the conversion is exact on every platform.
        Ok(usize::try_from(value).map_err(|_| InventoryError::TooManyRecords)?)
    }

    fn string(&mut self) -> Result<String> {
        let len = self.u32_be()?;
        if u64::from(len) > self.remaining() as u64 {
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
        let bytes = self.take(32)?;
        let mut out = [0u8; 32];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn tag(&mut self) -> Result<u8> {
        let bytes = self.take(1)?;
        Ok(bytes[0])
    }
}
