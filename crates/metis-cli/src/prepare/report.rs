//! Readable inventory report derived from the decoded canonical bytes,
//! so the report proves what the bytes say.

use metis_core::embedded_assets::{
    AssetKind, AssetRecord, BuildConfiguration, EmbeddedModuleInventory, INVENTORY_SCHEMA,
    InputRecord, ProviderRecord, ProviderSource, ToolRecord, hex_digest,
};

pub(super) fn report_json(
    inventory: &EmbeddedModuleInventory,
    digest: &[u8; 32],
) -> serde_json::Value {
    use serde_json::{Map, Value};
    let mut root = Map::new();
    root.insert("magic".into(), Value::String("METISAS1".into()));
    root.insert("schema".into(), Value::from(INVENTORY_SCHEMA));
    root.insert("package".into(), Value::String(inventory.package.clone()));
    root.insert(
        "package_id".into(),
        Value::String(inventory.package_id.clone()),
    );
    root.insert(
        "cdylib_target".into(),
        Value::String(inventory.cdylib_target.clone()),
    );
    root.insert(
        "wasm_target".into(),
        Value::String(inventory.wasm_target.clone()),
    );
    root.insert(
        "export".into(),
        Value::String(inventory.export_name.clone()),
    );
    root.insert(
        "platform_tag".into(),
        Value::String(inventory.platform_tag.clone()),
    );
    root.insert("page_schema".into(), Value::from(inventory.page_schema));
    root.insert(
        "lock_digest".into(),
        Value::String(hex_digest(&inventory.lock_digest)),
    );
    root.insert(
        "providers".into(),
        Value::Array(report_providers(&inventory.providers)),
    );
    root.insert("tools".into(), Value::Array(report_tools(&inventory.tools)));
    root.insert(
        "config".into(),
        Value::Object(report_config(&inventory.config)),
    );
    root.insert(
        "inputs".into(),
        Value::Array(report_inputs(&inventory.inputs)),
    );
    root.insert(
        "assets".into(),
        Value::Array(report_assets(&inventory.assets)),
    );
    root.insert("inventory_sha256".into(), Value::String(hex_digest(digest)));
    Value::Object(root)
}

fn report_providers(providers: &[ProviderRecord]) -> Vec<serde_json::Value> {
    use serde_json::{Map, Value};
    providers
        .iter()
        .map(|provider| {
            let mut entry = Map::new();
            entry.insert(
                "package_id".into(),
                Value::String(provider.package_id.clone()),
            );
            entry.insert("version".into(), Value::String(provider.version.clone()));
            entry.insert("source".into(), report_provider_source(&provider.source));
            Value::Object(entry)
        })
        .collect()
}

fn report_provider_source(source: &ProviderSource) -> serde_json::Value {
    use serde_json::{Map, Value};
    let mut entry = Map::new();
    match source {
        ProviderSource::Git { url, revision } => {
            entry.insert("git".into(), Value::String(url.clone()));
            entry.insert("revision".into(), Value::String(revision.clone()));
        }
        ProviderSource::Registry { checksum } => {
            entry.insert("registry".into(), Value::String(hex_digest(checksum)));
        }
        ProviderSource::Path { member } => {
            entry.insert("path".into(), Value::String(member.clone()));
        }
    }
    Value::Object(entry)
}

fn report_tools(tools: &[ToolRecord]) -> Vec<serde_json::Value> {
    use serde_json::{Map, Value};
    tools
        .iter()
        .map(|tool| {
            let mut entry = Map::new();
            entry.insert("name".into(), Value::String(tool.name.clone()));
            entry.insert("version".into(), Value::String(tool.version_output.clone()));
            entry.insert(
                "digest".into(),
                Value::String(hex_digest(&tool.executable_digest)),
            );
            Value::Object(entry)
        })
        .collect()
}

fn report_config(config: &BuildConfiguration) -> serde_json::Map<String, serde_json::Value> {
    use serde_json::{Map, Value};
    let mut entry = Map::new();
    entry.insert("profile".into(), Value::String(config.profile.clone()));
    entry.insert(
        "features".into(),
        Value::Array(
            config
                .features
                .iter()
                .map(|feature| Value::String(feature.clone()))
                .collect(),
        ),
    );
    entry.insert(
        "rustflags".into(),
        Value::Array(
            config
                .rustflags
                .iter()
                .map(|flag| Value::String(flag.clone()))
                .collect(),
        ),
    );
    entry.insert(
        "environment".into(),
        Value::Array(
            config
                .environment
                .iter()
                .map(|(key, value)| {
                    let mut pair = Map::new();
                    pair.insert("key".into(), Value::String(key.clone()));
                    pair.insert("value".into(), Value::String(value.clone()));
                    Value::Object(pair)
                })
                .collect(),
        ),
    );
    entry
}

fn report_inputs(inputs: &[InputRecord]) -> Vec<serde_json::Value> {
    use serde_json::{Map, Value};
    inputs
        .iter()
        .map(|input| {
            let mut entry = Map::new();
            entry.insert("owner".into(), Value::String(input.owner.clone()));
            entry.insert("path".into(), Value::String(input.path.clone()));
            entry.insert("length".into(), Value::from(input.length));
            entry.insert("digest".into(), Value::String(hex_digest(&input.digest)));
            Value::Object(entry)
        })
        .collect()
}

fn report_assets(assets: &[AssetRecord]) -> Vec<serde_json::Value> {
    use serde_json::{Map, Value};
    assets
        .iter()
        .map(|asset| {
            let mut entry = Map::new();
            entry.insert(
                "kind".into(),
                Value::String(
                    match asset.kind {
                        AssetKind::JavaScript => "javascript",
                        AssetKind::Wasm => "wasm",
                    }
                    .into(),
                ),
            );
            entry.insert(
                "destination".into(),
                Value::String(asset.destination.clone()),
            );
            entry.insert("length".into(), Value::from(asset.length));
            entry.insert("digest".into(), Value::String(hex_digest(&asset.digest)));
            Value::Object(entry)
        })
        .collect()
}
