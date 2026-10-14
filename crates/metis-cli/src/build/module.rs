//! One locked compiler and generated-module pipeline for browser/native pages.
mod context;
mod receipt;
use super::{CARGO_BUILD_DEADLINE, artifacts};
use crate::{Result, process};
pub(crate) use context::{Context, cargo_home};
use receipt::Receipt;
pub(super) use receipt::{BrowserInputs, RootInputs};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) const WASM_TARGET: &str = "wasm32-unknown-unknown";
const METADATA_DEADLINE: Duration = Duration::from_mins(1);
const BINDGEN_DEADLINE: Duration = Duration::from_mins(5);
const VERSION_DEADLINE: Duration = Duration::from_secs(30);

pub(super) struct ModuleBuild<State> {
    pub(super) metadata: Vec<u8>,
    pub(super) target: artifacts::ModuleTarget,
    pub(super) cargo: PathBuf,
    pub(super) rustc: PathBuf,
    pub(super) context: Context,
    pub(super) state: State,
    pub(super) package: String,
    pub(super) bindgen: PathBuf,
}

pub(super) struct Resolved;
pub(super) struct Checked {
    pub(super) messages: Vec<u8>,
}
pub(super) struct Compiled<Inputs> {
    pub(super) messages: Vec<u8>,
    pub(super) module: PathBuf,
    inputs: Inputs,
}
/// Only resolved or checked source may enter final code generation.
pub(super) trait BeforeCodegen {
    type Inputs: Receipt;
}
impl BeforeCodegen for Resolved {
    type Inputs = BrowserInputs;
}
impl BeforeCodegen for Checked {
    type Inputs = RootInputs;
}
/// Compiler output exists after actual check or final code generation.
pub(super) trait CompilerOutput {
    fn messages(&self) -> &[u8];
    fn root_dep_info(&self) -> Option<&Path>;
}
impl CompilerOutput for Checked {
    fn messages(&self) -> &[u8] {
        &self.messages
    }
    fn root_dep_info(&self) -> Option<&Path> {
        None
    }
}
impl CompilerOutput for Compiled<RootInputs> {
    fn messages(&self) -> &[u8] {
        &self.messages
    }
    fn root_dep_info(&self) -> Option<&Path> {
        self.inputs.dep_info()
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Check,
    Build,
}

pub(super) fn resolve(cargo_manifest: &Path, package: &str) -> Result<ModuleBuild<Resolved>> {
    let mut context = Context::new(cargo_manifest)?;
    let cargo = metis_core::embedded_assets::resolve_tool(
        metis_core::embedded_assets::ToolKind::Cargo,
        Path::new(&std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())),
        &context.workspace,
    )?;
    let rustc = metis_core::embedded_assets::resolve_tool(
        metis_core::embedded_assets::ToolKind::Rustc,
        Path::new(&metis_core::embedded_assets::compiler_command(
            &context.workspace,
            &cargo_home(&context.workspace)?,
        )?),
        &context.workspace,
    )?;
    context
        .environment
        .push(("RUSTC", rustc.as_os_str().to_owned()));
    let manifest_path = cargo_manifest.as_os_str().to_owned();
    let metadata_arguments = arguments(
        &[
            "metadata",
            "--format-version=1",
            "--locked",
            "--filter-platform",
            WASM_TARGET,
        ],
        &manifest_path,
    );
    let metadata = metis_core::embedded_assets::capture_command(
        &cargo,
        &metadata_arguments,
        METADATA_DEADLINE,
        &context.environment,
        Some(&context.directory),
    )?;
    let target = artifacts::module_target(&metadata, package)?;
    let bindgen = wasm_bindgen(&target.bindgen_version, &context)?;
    Ok(ModuleBuild {
        metadata,
        target,
        cargo,
        rustc,
        bindgen,
        context,
        state: Resolved,
        package: package.into(),
    })
}

impl ModuleBuild<Resolved> {
    pub(super) fn check(self, cargo_manifest: &Path) -> Result<ModuleBuild<Checked>> {
        let messages = invoke(
            &self.cargo,
            &self.context,
            &self.package,
            cargo_manifest,
            Phase::Check,
            &BrowserInputs,
        )?;
        Ok(ModuleBuild {
            metadata: self.metadata,
            target: self.target,
            cargo: self.cargo,
            rustc: self.rustc,
            bindgen: self.bindgen,
            context: self.context,
            package: self.package,
            state: Checked { messages },
        })
    }
}
impl<State: BeforeCodegen> ModuleBuild<State> {
    pub(super) fn compile(
        self,
        cargo_manifest: &Path,
    ) -> Result<ModuleBuild<Compiled<State::Inputs>>> {
        let inputs = State::Inputs::new(&self.context);
        let messages = invoke(
            &self.cargo,
            &self.context,
            &self.package,
            cargo_manifest,
            Phase::Build,
            &inputs,
        )?;
        let module = artifacts::module(&messages, &self.target)?;
        Ok(ModuleBuild {
            metadata: self.metadata,
            target: self.target,
            cargo: self.cargo,
            rustc: self.rustc,
            bindgen: self.bindgen,
            context: self.context,
            package: self.package,
            state: Compiled {
                messages,
                module,
                inputs,
            },
        })
    }
}

impl<Inputs> ModuleBuild<Compiled<Inputs>> {
    pub(super) fn generate(&self, directory: &Path) -> Result<()> {
        let mut generate = vec![self.state.module.as_os_str().to_owned()];
        generate.extend(generator_options().into_iter().map(OsString::from));
        generate.push("--out-dir".into());
        generate.push(directory.as_os_str().to_owned());
        process::run(&self.bindgen, &generate, BINDGEN_DEADLINE)?;
        Ok(())
    }
}

/// Ordered tool options share the same owner as the actual command invocation.
/// Artifact/manifest/output paths are typed by their owners rather than storing
/// host paths inside the semantic compiler configuration.
pub(super) fn semantic_arguments(package: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    for phase in [Phase::Check, Phase::Build] {
        arguments.push("cargo".into());
        arguments.extend(compiler_options::<RootInputs>(package, phase));
        if matches!(phase, Phase::Build) {
            arguments.extend(RootInputs::semantic_arguments());
        }
    }
    arguments.push("wasm-bindgen".into());
    arguments.extend(generator_options());
    arguments
}
fn compiler_options<Inputs: Receipt>(package: &str, phase: Phase) -> Vec<String> {
    [
        match phase {
            Phase::Check => "check",
            Phase::Build => Inputs::command(),
        },
        "--release",
        "--locked",
        "--message-format=json-render-diagnostics",
        "--target",
        WASM_TARGET,
        "--package",
        package,
        "--lib",
    ]
    .map(String::from)
    .to_vec()
}
fn invoke<Inputs: Receipt>(
    cargo: &Path,
    context: &Context,
    package: &str,
    manifest: &Path,
    phase: Phase,
    inputs: &Inputs,
) -> Result<Vec<u8>> {
    let mut arguments = compiler_options::<Inputs>(package, phase)
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    arguments.extend(["--manifest-path".into(), manifest.as_os_str().to_owned()]);
    if matches!(phase, Phase::Build) {
        arguments.extend(inputs.arguments());
    }
    Ok(metis_core::embedded_assets::capture_command(
        cargo,
        &arguments,
        CARGO_BUILD_DEADLINE,
        &context.environment,
        Some(&context.directory),
    )?)
}
fn generator_options() -> Vec<String> {
    ["--target", "web", "--no-typescript"]
        .map(String::from)
        .to_vec()
}

fn arguments(leading: &[&str], manifest_path: &OsString) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = leading.iter().map(OsString::from).collect();
    arguments.push("--manifest-path".into());
    arguments.push(manifest_path.clone());
    arguments
}

/// The `wasm-bindgen` CLI: `WASM_BINDGEN` when set, else the one on `PATH`.
///
/// Its version must equal the locked `wasm-bindgen` crate's, because the
/// generated loader and the crate's embedded schema change together.
fn wasm_bindgen(version: &str, context: &Context) -> Result<PathBuf> {
    let program = metis_core::embedded_assets::resolve_tool(
        metis_core::embedded_assets::ToolKind::WasmBindgen,
        Path::new(&std::env::var_os("WASM_BINDGEN").unwrap_or_else(|| "wasm-bindgen".into())),
        &context.workspace,
    )?;
    let reported = metis_core::embedded_assets::capture_command(
        &program,
        &["--version".into()],
        VERSION_DEADLINE,
        &context.environment,
        Some(&context.directory),
    )?;
    let reported = String::from_utf8(reported)?;
    if reported.trim() != format!("wasm-bindgen {version}") {
        return Err(format!(
            "the locked wasm-bindgen crate needs wasm-bindgen {version}, but {} reports {:?}; \
             install it with `cargo install wasm-bindgen-cli --version {version} --locked` \
             or set WASM_BINDGEN to its path",
            program.display(),
            reported.trim()
        )
        .into());
    }
    Ok(program)
}
