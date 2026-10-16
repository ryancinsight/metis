//! Actual dependency-free Cargo units retain their owner until assertions end.
//!
//! The caller selects the manifest's crate representations and Cargo operation;
//! one invocation owner always supplies the lock, isolated target, JSON format,
//! pinned tools and neutral Context. The child-only environment is an input to
//! both rustc's env! dependency and the real build-script computation. No event
//! or artifact path is synthesized here: run returns Cargo's actual messages.

use crate::build::module::Context;
use metis_core::embedded_assets::{ToolKind, capture_command, resolve_tool, verify_compiler_pin};
use serde_json::Value;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) struct Fixture {
    pub(super) root: PathBuf,
    context: Context,
    cargo: PathBuf,
    // Context releases its neutral directory before the source/target owner.
    _files: FixtureFiles,
}

impl Fixture {
    pub(super) fn new(crate_types: &[&str]) -> Self {
        let _phase = tracing::info_span!("arrange_actual_cargo_workspace", ?crate_types).entered();
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("actual system epoch")
            .as_nanos();
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("actual temporary owner")
            .join(format!(
                "metis-artifact-roles-{}-{epoch}",
                std::process::id()
            ));
        fs::create_dir(&root).expect("exclusive fixture owner");
        let files = FixtureFiles(root.clone());
        workspace_sources(&root);
        crate_sources(&root, crate_types);
        let (context, cargo) = pinned_context(&root);
        Self {
            root,
            context,
            cargo,
            _files: files,
        }
    }

    pub(super) fn run(&self, arguments: &[OsString], environment_value: &str) -> Vec<Value> {
        let _phase =
            tracing::info_span!("act_actual_cargo_units", ?arguments, environment_value).entered();
        let (operation, options) = arguments.split_first().expect("actual Cargo operation");
        assert!(matches!(
            operation.to_str(),
            Some("build" | "check" | "rustc")
        ));
        let mut arguments = vec![
            operation.clone(),
            "--locked".into(),
            "--offline".into(),
            "--message-format=json".into(),
            "--manifest-path".into(),
            self.root.join("Cargo.toml").into_os_string(),
            "--target-dir".into(),
            self.root.join("target").into_os_string(),
        ];
        arguments.extend_from_slice(options);
        let mut environment = self.context.environment.clone();
        environment.push(("METIS_ARTIFACT_ROLE_VALUE", environment_value.into()));
        environment.push(("METIS_ARTIFACT_COMPILER_VALUE", environment_value.into()));
        let output = capture_command(
            &self.cargo,
            &arguments,
            Duration::from_secs(30),
            &environment,
            Some(&self.context.directory),
        )
        .expect("actual bounded locked Cargo artifact production");
        output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).expect("actual Cargo JSON event"))
            .collect()
    }
}

fn workspace_sources(root: &Path) {
    let _phase = tracing::info_span!("arrange_workspace_sources").entered();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|directory| directory.join("rust-toolchain.toml").is_file())
        .expect("owning repository toolchain pin");
    fs::copy(
        repository.join("rust-toolchain.toml"),
        root.join("rust-toolchain.toml"),
    )
    .expect("actual repository pin in the fixture context");
    let split = if cfg!(target_os = "macos") {
        "split-debuginfo='packed'\n"
    } else {
        ""
    };
    write(
        root,
        "Cargo.toml",
        &format!(
            "[workspace]\nmembers=['macros','library']\nresolver='3'\n\
                 [profile.dev]\ndebug=2\n{split}"
        ),
    );
    write(
        root,
        "Cargo.lock",
        "version=4\n\
             [[package]]\nname='artifact_roles'\nversion='0.1.0'\n\
             dependencies=['lib_prefix_macros']\n\
             [[package]]\nname='lib_prefix_macros'\nversion='0.1.0'\n",
    );
}

fn crate_sources(root: &Path, crate_types: &[&str]) {
    let _phase = tracing::info_span!("arrange_crate_sources", ?crate_types).entered();
    write(
        root,
        "macros/Cargo.toml",
        "[package]\nname='lib_prefix_macros'\nversion='0.1.0'\nedition='2024'\n\
             [lib]\nproc-macro=true\n",
    );
    write(
        root,
        "macros/src/lib.rs",
        "extern crate proc_macro;\n\
             #[proc_macro]\n\
             pub fn count_tokens(input: proc_macro::TokenStream) -> proc_macro::TokenStream {\n\
                 input.into_iter().count().to_string().parse().expect(\"integer token stream\")\n\
             }\n",
    );
    write(
        root,
        "library/Cargo.toml",
        &format!(
            "[package]\nname='artifact_roles'\nversion='0.1.0'\nedition='2024'\n\
                 [lib]\ncrate-type={}\n\
                 [dependencies]\nlib_prefix_macros={{path='../macros'}}\n",
            serde_json::to_string(crate_types).expect("actual crate representation strings")
        ),
    );
    write(root, "library/label.txt", "abc");
    write(
        root,
        "library/build.rs",
        r#"fn main() {
    let input = std::fs::read("label.txt").expect("actual declared input");
    let text = std::env::var("METIS_ARTIFACT_ROLE_VALUE").expect("actual child environment");
    let factor: usize = text.parse().expect("integer environment input");
    let sum: usize = input.into_iter().map(usize::from).sum();
    let generated = format!("pub const GENERATED_VALUE: usize = {};\npub const GENERATED_ENV: &str = {:?};\n", sum * factor, text);
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("actual Cargo output"));
    std::fs::write(output.join("generated.rs"), generated).expect("actual generated Rust input");
    println!("cargo::rerun-if-changed=label.txt");
    println!("cargo::rerun-if-env-changed=METIS_ARTIFACT_ROLE_VALUE");
}
"#,
    );
    write(
        root,
        "library/src/lib.rs",
        r#"pub const LABEL: &str = include_str!("../label.txt");
pub const ENVIRONMENT: &str = env!("METIS_ARTIFACT_COMPILER_VALUE");
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
pub const TOKENS: usize = lib_prefix_macros::count_tokens!(one two three);
const _: () = assert!(TOKENS == 3 && LABEL.len() == 3 && !ENVIRONMENT.is_empty());
pub fn computed_value() -> usize {
    let factor: usize = ENVIRONMENT.parse().expect("actual compiler environment");
    assert_eq!(GENERATED_ENV, ENVIRONMENT);
    assert_eq!(GENERATED_VALUE, LABEL.bytes().map(usize::from).sum::<usize>() * factor);
    GENERATED_VALUE
}
"#,
    );
}

fn pinned_context(root: &Path) -> (Context, PathBuf) {
    let _phase = tracing::info_span!("act_pinned_tool_context").entered();
    let manifest = root.join("Cargo.toml");
    let mut context = Context::new(&manifest).expect("real neutral fixture context");
    let cargo_command = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let cargo = resolve_tool(ToolKind::Cargo, Path::new(&cargo_command), root)
        .expect("actual selected pinned Cargo");
    let cargo_home = crate::build::module::cargo_home(root).expect("actual Cargo home");
    let rustc_command = metis_core::embedded_assets::compiler_command(root, &cargo_home)
        .expect("actual selected compiler command");
    let rustc = resolve_tool(ToolKind::Rustc, Path::new(&rustc_command), root)
        .expect("actual selected pinned rustc");
    let version = capture_command(
        &rustc,
        &["--version".into()],
        Duration::from_secs(30),
        &context.environment,
        Some(&context.directory),
    )
    .expect("actual compiler identity");
    verify_compiler_pin(
        std::str::from_utf8(&version)
            .expect("actual compiler version UTF-8")
            .trim(),
        root,
    )
    .expect("fixture compiler matches the repository pin");
    context.environment.push(("RUSTC", rustc.into_os_string()));
    (context, cargo)
}

fn write(root: &Path, relative: &str, source: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("source fixture parent"))
        .expect("actual source fixture directories");
    fs::write(path, source).expect("actual source fixture bytes");
}

struct FixtureFiles(PathBuf);
impl Drop for FixtureFiles {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            tracing::warn!(%error, path = %self.0.display(), "artifact fixture cleanup failed");
        }
    }
}
