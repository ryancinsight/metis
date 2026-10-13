//! Bounded local-only roots; binding paths do not enter the semantic inventory.
use super::super::{
    FILE_LIMIT, ToolKind,
    codec::{Reader, Writer},
    inventory::invalid,
};
use std::{io, path::PathBuf};

/// One admitted namespace's local canonical root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerBinding {
    /// Corresponding provider/workspace/generated namespace.
    pub owner_id: String,
    /// Local absolute canonical source directory.
    pub root: PathBuf,
}
/// One local executable path for its semantic tool record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolBinding {
    /// Corresponding fixed tool role.
    pub kind: ToolKind,
    /// Local absolute canonical executable file.
    pub executable: PathBuf,
}
/// Host-only binding file accompanying a semantic inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalBindings {
    /// Sorted unique provider/generated owner roots.
    pub owners: Vec<OwnerBinding>,
    /// Fixed Cargo/rustc/wasm-bindgen local executables.
    pub tools: [ToolBinding; 3],
}
impl LocalBindings {
    /// Canonical sidecar filename relative to the inventory directory.
    pub const FILE_NAME: &str = "bindings.bin";
    /// Encodes bounded local bindings, rejecting unsorted or duplicate owner IDs.
    ///
    /// # Errors
    /// Rejects ambiguous IDs, nonabsolute/non-UTF-8 paths and excessive counts.
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        validate(self)?;
        let mut output = Writer::new();
        output.put(b"METISLOC")?;
        output.put(&1_u16.to_be_bytes())?;
        output.count(self.owners.len())?;
        for owner in &self.owners {
            output.text(&owner.owner_id)?;
            output.text(
                owner
                    .root
                    .to_str()
                    .ok_or_else(|| invalid("owner root is not UTF-8"))?,
            )?;
        }
        for tool in &self.tools {
            output.byte(match tool.kind {
                ToolKind::Cargo => 1,
                ToolKind::Rustc => 2,
                ToolKind::WasmBindgen => 3,
            })?;
            output.text(
                tool.executable
                    .to_str()
                    .ok_or_else(|| invalid("tool path is not UTF-8"))?,
            )?;
        }
        Ok(output.finish())
    }
    /// Decodes the complete bounded local binding value.
    ///
    /// # Errors
    /// Rejects unknown schema/tags, truncated/trailing data and invalid bindings.
    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut input = Reader::new(bytes)?;
        if input.take(8)? != b"METISLOC" || u16::from_be_bytes(input.array()?) != 1 {
            return Err(invalid("unsupported local bindings magic/schema"));
        }
        let count = input.count()?;
        let mut owners = Vec::new();
        owners
            .try_reserve(count)
            .map_err(|error| invalid(format!("binding allocation: {error}")))?;
        for _ in 0..count {
            owners.push(OwnerBinding {
                owner_id: input.text()?,
                root: PathBuf::from(input.text()?),
            });
        }
        let mut tools = Vec::with_capacity(3);
        for _ in 0..3 {
            let kind = match input.byte()? {
                1 => ToolKind::Cargo,
                2 => ToolKind::Rustc,
                3 => ToolKind::WasmBindgen,
                _ => return Err(invalid("unknown local tool tag")),
            };
            tools.push(ToolBinding {
                kind,
                executable: PathBuf::from(input.text()?),
            });
        }
        input.finish()?;
        let value = Self {
            owners,
            tools: tools
                .try_into()
                .map_err(|_| invalid("local tool count differs from three"))?,
        };
        validate(&value)?;
        Ok(value)
    }
}
fn validate(value: &LocalBindings) -> io::Result<()> {
    if value.owners.is_empty() || value.owners.len() > FILE_LIMIT {
        return Err(invalid("local binding owner count is invalid"));
    }
    let mut previous = None;
    for owner in &value.owners {
        if owner.owner_id.is_empty()
            || owner.owner_id.chars().any(char::is_control)
            || previous.is_some_and(|prior| prior >= owner.owner_id.as_str())
        {
            return Err(invalid(
                "local owner IDs must be sorted, unique and unformatted",
            ));
        }
        previous = Some(owner.owner_id.as_str());
        absolute(&owner.root)?;
    }
    for (tool, kind) in
        value
            .tools
            .iter()
            .zip([ToolKind::Cargo, ToolKind::Rustc, ToolKind::WasmBindgen])
    {
        if tool.kind != kind {
            return Err(invalid("local tool roles are not canonical"));
        }
        absolute(&tool.executable)?;
    }
    Ok(())
}
fn absolute(path: &std::path::Path) -> io::Result<()> {
    if !path.is_absolute()
        || path
            .to_str()
            .is_none_or(|value| value.chars().any(char::is_control))
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(invalid(
            "local path must be an absolute canonical UTF-8 path",
        ));
    }
    Ok(())
}
