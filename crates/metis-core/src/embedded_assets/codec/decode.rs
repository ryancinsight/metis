//! Decode checks wire lengths and remaining bytes before allocation.
use super::{MAGIC, SCHEMA};
use crate::embedded_assets::inventory::invalid;
use crate::embedded_assets::{
    AssetKind, AssetRecord, BuildConfiguration, Digest, DirectoryRecord, FILE_LIMIT, FileIdentity,
    InputRecord, InventorySpec, MANIFEST_LIMIT, ModuleIdentity, NativePlatform, ProviderIdentity,
    ProviderSource, ToolIdentity, ToolKind,
};
use std::io;

pub(crate) struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}
impl<'bytes> Reader<'bytes> {
    pub(crate) fn new(bytes: &'bytes [u8]) -> io::Result<Self> {
        if u64::try_from(bytes.len()).map_err(|_| invalid("inventory length overflow"))?
            > MANIFEST_LIMIT
        {
            return Err(invalid("inventory exceeds 1 MiB"));
        }
        Ok(Self { bytes, offset: 0 })
    }
    pub(crate) fn take(&mut self, count: usize) -> io::Result<&'bytes [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| invalid("wire offset overflow"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| invalid("truncated inventory field"))?;
        self.offset = end;
        Ok(bytes)
    }
    pub(crate) fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        self.take(N)?
            .try_into()
            .map_err(|_| invalid("fixed field width mismatch"))
    }
    pub(crate) fn byte(&mut self) -> io::Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    pub(crate) fn count(&mut self) -> io::Result<usize> {
        let count = usize::try_from(u32::from_be_bytes(self.array()?))
            .map_err(|_| invalid("wire count overflow"))?;
        if count > FILE_LIMIT || count > self.bytes.len().saturating_sub(self.offset) {
            return Err(invalid(
                "wire sequence count exceeds bound or remaining bytes",
            ));
        }
        Ok(count)
    }
    pub(crate) fn text(&mut self) -> io::Result<String> {
        let length = usize::try_from(u32::from_be_bytes(self.array()?))
            .map_err(|_| invalid("wire string length overflow"))?;
        let text = std::str::from_utf8(self.take(length)?)
            .map_err(|error| invalid(format!("inventory UTF-8: {error}")))?;
        let mut value = String::new();
        value
            .try_reserve(length)
            .map_err(|error| invalid(format!("inventory string allocation: {error}")))?;
        value.push_str(text);
        Ok(value)
    }
    pub(crate) fn digest(&mut self) -> io::Result<Digest> {
        Ok(Digest::from_bytes(self.array()?))
    }
    pub(crate) fn identity(&mut self) -> io::Result<FileIdentity> {
        Ok(FileIdentity {
            bytes: u64::from_be_bytes(self.array()?),
            digest: self.digest()?,
        })
    }
    pub(crate) fn finish(self) -> io::Result<()> {
        if self.offset != self.bytes.len() {
            return Err(invalid("trailing inventory bytes"));
        }
        Ok(())
    }
    fn sequence<T>(
        &mut self,
        mut read: impl FnMut(&mut Self) -> io::Result<T>,
    ) -> io::Result<Vec<T>> {
        let count = self.count()?;
        let mut values = Vec::new();
        values
            .try_reserve(count)
            .map_err(|error| invalid(format!("inventory sequence allocation: {error}")))?;
        for _ in 0..count {
            values.push(read(self)?);
        }
        Ok(values)
    }
}
pub(in super::super) fn decode(bytes: &[u8]) -> io::Result<InventorySpec> {
    let mut input = Reader::new(bytes)?;
    if input.take(8)? != MAGIC || u16::from_be_bytes(input.array()?) != SCHEMA {
        return Err(invalid("unsupported inventory magic/schema"));
    }
    let module = module(&mut input)?;
    let lock_digest = input.digest()?;
    let providers = input.sequence(provider)?;
    let tools = tools(&mut input)?;
    let configuration = BuildConfiguration {
        profile: input.text()?,
        features: input.sequence(Reader::text)?,
        compiler_args: input.sequence(Reader::text)?,
        environment: input.sequence(|input| {
            let key = input.text()?;
            let value = match input.byte()? {
                0 => None,
                1 => Some(input.text()?),
                _ => return Err(invalid("unknown environment presence tag")),
            };
            Ok((key, value))
        })?,
    };
    let directories = input.sequence(|input| {
        let owner_id = input.text()?;
        let path = match input.byte()? {
            0 => None,
            1 => Some(input.text()?),
            _ => return Err(invalid("unknown directory scope tag")),
        };
        Ok(DirectoryRecord {
            owner_id,
            path,
            policy: match input.byte()? {
                1 => crate::embedded_assets::DirectoryPolicy::Declared,
                2 => crate::embedded_assets::DirectoryPolicy::CargoPackage,
                _ => return Err(invalid("unknown directory policy tag")),
            },
            digest: input.digest()?,
        })
    })?;
    let inputs = input.sequence(|input| {
        Ok(InputRecord {
            owner_id: input.text()?,
            path: input.text()?,
            identity: input.identity()?,
        })
    })?;
    let assets = input
        .sequence(|input| {
            let kind = match input.byte()? {
                1 => AssetKind::JavaScript,
                2 => AssetKind::WebAssembly,
                _ => return Err(invalid("unknown asset tag")),
            };
            Ok(AssetRecord {
                kind,
                destination: input.text()?,
                identity: input.identity()?,
            })
        })?
        .try_into()
        .map_err(|_| invalid("inventory requires exactly two assets"))?;
    input.finish()?;
    Ok(InventorySpec {
        module,
        lock_digest,
        providers,
        tools,
        configuration,
        inputs,
        directories,
        assets,
    })
}

fn module(input: &mut Reader<'_>) -> io::Result<ModuleIdentity> {
    let package = input.text()?;
    let package_id = input.text()?;
    let target = input.text()?;
    let wasm_target = input.text()?;
    let export = input.text()?;
    let native_platform = match input.byte()? {
        1 => NativePlatform::Windows,
        _ => return Err(invalid("unknown native platform tag")),
    };
    let page_schema = u16::from_be_bytes(input.array()?);
    Ok(ModuleIdentity {
        package,
        package_id,
        target,
        wasm_target,
        export,
        native_platform,
        page_schema,
    })
}

fn provider(input: &mut Reader<'_>) -> io::Result<ProviderIdentity> {
    let package_id = input.text()?;
    let package = input.text()?;
    let version = input.text()?;
    let source = match input.byte()? {
        1 => ProviderSource::Workspace,
        2 => ProviderSource::Git {
            url: input.text()?,
            revision: input.text()?,
        },
        3 => ProviderSource::Registry {
            url: input.text()?,
            checksum: input.digest()?,
        },
        _ => return Err(invalid("unknown provider source tag")),
    };
    Ok(ProviderIdentity {
        package_id,
        package,
        version,
        source,
    })
}

fn tools(input: &mut Reader<'_>) -> io::Result<[ToolIdentity; 3]> {
    let mut tools = Vec::with_capacity(3);
    for _ in 0..3 {
        let kind = match input.byte()? {
            1 => ToolKind::Cargo,
            2 => ToolKind::Rustc,
            3 => ToolKind::WasmBindgen,
            _ => return Err(invalid("unknown tool tag")),
        };
        tools.push(ToolIdentity {
            kind,
            version: input.text()?,
            executable: input.identity()?,
        });
    }
    tools
        .try_into()
        .map_err(|_| invalid("tool count differs from three"))
}
