//! Native preparation precedes Cargo and owns its typed inventory and sidecar.
mod dep_info;
mod inputs;
mod metadata;
mod witness;
use super::module;
use crate::{Result, manifest::Application, tree};
use metis_core::embedded_assets::{
    AssetKind, AssetRecord, BuildConfiguration, EmbeddedModuleInventory, FileIdentity,
    InventorySpec, LocalBindings, MANIFEST_LIMIT, MODULE_LIMIT, ModuleIdentity, NativePlatform,
    PAYLOAD_LIMIT, PreparedModule, ProducerContext, ToolBinding, ToolIdentity, ToolKind,
};
use std::{
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(crate) const INVENTORY_ENVIRONMENT: &str = "METIS_WEBVIEW_ASSET_INVENTORY";
const INVENTORY_FILE: &str = "inventory.bin";

pub(crate) struct Preparation {
    pub(crate) inventory_path: PathBuf,
    workspace: PathBuf,
    cargo_home: PathBuf,
}
impl Preparation {
    pub(crate) fn verify(&self) -> Result<PreparedModule> {
        Ok(PreparedModule::open(
            &self.inventory_path,
            &self.workspace,
            &self.cargo_home,
            ProducerContext,
        )?)
    }
    pub(crate) fn environment(&self) -> [(&'static str, OsString); 1] {
        [(
            INVENTORY_ENVIRONMENT,
            self.inventory_path.as_os_str().to_owned(),
        )]
    }
}
pub(crate) fn application(
    application: &Application,
    root: &Path,
    output: Option<&Path>,
) -> Result<Option<Preparation>> {
    let Some(declaration) = &application.embedded_module else {
        return Ok(None);
    };
    let _span =
        tracing::info_span!("prepare_embedded_module", package = %declaration.package).entered();
    let manifest = metis_core::embedded_assets::source(root, &application.cargo_manifest)?;
    let resolved = module::resolve(&manifest, &declaration.package)?;
    let graph = metadata::graph(&resolved)?;
    let tools = tools(&resolved)?;
    let source_witness = witness::SourceWitness::capture(&graph, &resolved.context)?;
    let checked = resolved.check(&manifest)?;
    let cargo_home = module::cargo_home(&graph.workspace)?;
    let before = inputs::collect(&checked, &graph, &cargo_home)?;
    source_witness.verify(&before, &graph)?;
    let compilation = checked.compile(&manifest)?;
    let compiled_inputs = inputs::collect(&compilation, &graph, &cargo_home)?;
    if before.records != compiled_inputs.records
        || before.environment != compiled_inputs.environment
        || before.features != compiled_inputs.features
        || before.directories != compiled_inputs.directories
    {
        return Err("module inputs changed between real check and code generation".into());
    }
    source_witness.verify(&compiled_inputs, &graph)?;
    let lock = FileIdentity::read(&graph.workspace.join("Cargo.lock"), MANIFEST_LIMIT)?.digest;
    let directory = output_directory(output, &graph.workspace)?;
    let result = (|| {
        compilation.generate(&directory)?;
        let after = inputs::collect(&compilation, &graph, &cargo_home)?;
        if before.records != after.records
            || before.environment != after.environment
            || before.features != after.features
            || before.directories != after.directories
            || lock
                != FileIdentity::read(&graph.workspace.join("Cargo.lock"), MANIFEST_LIMIT)?.digest
        {
            return Err("module inputs changed during generation".into());
        }
        if tools != self::tools(&compilation)? {
            return Err("selected compiler/generator changed during preparation".into());
        }
        let preparation = produce(
            &compilation,
            graph,
            after,
            tools,
            lock,
            &directory,
            cargo_home,
        )?;
        // The producer uses the same independent consumer admission path.
        preparation.verify()?;
        Ok(Some(preparation))
    })();
    if let Err(primary) = &result
        && let Err(cleanup) = fs::remove_dir_all(&directory)
    {
        return Err(format!("{primary}; preparation cleanup failed: {cleanup}").into());
    }
    result
}
fn produce(
    compilation: &module::ModuleBuild<module::Compiled>,
    graph: metadata::Graph,
    after: inputs::Inputs,
    tools: [ToolIdentity; 3],
    lock: metis_core::embedded_assets::Digest,
    directory: &Path,
    cargo_home: PathBuf,
) -> Result<Preparation> {
    let package = compilation.package.as_str();
    let module_package = graph
        .providers
        .iter()
        .find(|p| {
            p.package == package
                && matches!(
                    p.source,
                    metis_core::embedded_assets::ProviderSource::Workspace
                )
        })
        .ok_or("declared embedded module has no workspace owner")?;
    let inventory = EmbeddedModuleInventory::new(InventorySpec {
        module: ModuleIdentity {
            package: package.to_owned(),
            package_id: module_package.package_id.clone(),
            target: compilation.target.target.clone(),
            wasm_target: module::WASM_TARGET.into(),
            export: "metis_embedded_start".into(),
            native_platform: NativePlatform::Windows,
            page_schema: 1,
        },
        lock_digest: lock,
        providers: graph.providers,
        tools,
        configuration: BuildConfiguration {
            profile: "release".into(),
            features: after.features,
            compiler_args: module::semantic_arguments(package),
            environment: after.environment,
        },
        inputs: after.records,
        directories: after.directories,
        assets: assets(directory)?,
    })?;
    let bindings = LocalBindings {
        owners: after.owners,
        tools: [
            (&compilation.cargo, ToolKind::Cargo),
            (&compilation.rustc, ToolKind::Rustc),
            (&compilation.bindgen, ToolKind::WasmBindgen),
        ]
        .map(|(program, kind)| ToolBinding {
            kind,
            executable: program.clone(),
        }),
    };
    write_new(
        &directory.join(LocalBindings::FILE_NAME),
        &bindings.encode()?,
    )?;
    let inventory_path = directory.join(INVENTORY_FILE);
    write_new(&inventory_path, &inventory.encode()?)?;
    Ok(Preparation {
        inventory_path,
        workspace: graph.workspace,
        cargo_home,
    })
}
fn tools<State>(compilation: &module::ModuleBuild<State>) -> Result<[ToolIdentity; 3]> {
    let [cargo, rustc, bindgen] = [
        (&compilation.cargo, ToolKind::Cargo),
        (&compilation.rustc, ToolKind::Rustc),
        (&compilation.bindgen, ToolKind::WasmBindgen),
    ]
    .map(|(program, kind)| tool(program, kind, &compilation.context));
    Ok([cargo?, rustc?, bindgen?])
}
fn assets(directory: &Path) -> Result<[AssetRecord; 2]> {
    let files = tree::regular_files(directory, |_| false)?;
    let mut assets = Vec::new();
    for file in files {
        let destination = tree::relative_name(directory, &file)?;
        let kind = match file.extension().and_then(|extension| extension.to_str()) {
            Some("js") => AssetKind::JavaScript,
            Some("wasm") => AssetKind::WebAssembly,
            _ => return Err("generated module contains an undeclared artifact role".into()),
        };
        let identity = FileIdentity::read(&file, MODULE_LIMIT)?;
        assets.push(AssetRecord {
            kind,
            destination,
            identity,
        });
    }
    assets.sort_by(|left, right| left.destination.cmp(&right.destination));
    assets
        .try_into()
        .map_err(|_| "generated module requires exactly two assets".into())
}
fn tool(program: &Path, kind: ToolKind, context: &module::Context) -> Result<ToolIdentity> {
    let version = metis_core::embedded_assets::capture_command(
        program,
        &["--version".into()],
        Duration::from_secs(30),
        &context.environment,
        Some(&context.directory),
    )?;
    let version = String::from_utf8(version)?.trim().to_owned();
    if kind == ToolKind::Rustc {
        metis_core::embedded_assets::verify_compiler_pin(&version, &context.workspace)?;
    }
    Ok(ToolIdentity {
        kind,
        version,
        executable: FileIdentity::read(program, PAYLOAD_LIMIT)?,
    })
}
fn output_directory(output: Option<&Path>, workspace: &Path) -> Result<PathBuf> {
    if let Some(output) = output {
        let absolute = if output.is_absolute() {
            output.to_path_buf()
        } else {
            std::env::current_dir()?.join(output)
        };
        let parent = absolute
            .parent()
            .ok_or("preparation output has no parent")?;
        fs::create_dir_all(parent)?;
        fs::create_dir(&absolute)?;
        return Ok(absolute.canonicalize()?);
    }
    let parent = metis_core::embedded_assets::CargoLocations::current(workspace)?
        .target_directory()
        .join("metis-embedded");
    fs::create_dir_all(&parent)?;
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    for attempt in 0..8 {
        let output = parent.join(format!("{}-{epoch}-{attempt}", std::process::id()));
        match fs::create_dir(&output) {
            Ok(()) => return Ok(output.canonicalize()?),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err("cannot acquire prepared-module directory after eight attempts".into())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::File::options()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
