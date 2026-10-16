//! A copy of the installed Rustup image reproduces its real symlink dispatch.
use super::*;
use std::{fs, os::unix::fs::symlink};

#[test]
fn actual_rustup_symlink_preserves_compiler_query_and_canonical_identity() {
    actual_alias(ToolKind::Rustc, "rustc");
}

#[test]
fn actual_rustup_symlink_preserves_cargo_query_and_canonical_identity() {
    actual_alias(ToolKind::Cargo, "cargo");
}

fn actual_alias(kind: ToolKind, name: &str) {
    traced(|| {
        let (owner, workspace, expected, rustup, selected) = {
            let _phase = tracing::info_span!("arrange").entered();
            let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .find(|directory| directory.join("rust-toolchain.toml").is_file())
                .expect("real repository compiler pin");
            let expected = resolve_tool(kind, Path::new(name), workspace)
                .expect("actual installed selected tool");
            let original =
                executable(Path::new("rustup")).expect("installed multicall Rustup image");
            let directory = std::env::temp_dir()
                .canonicalize()
                .expect("actual temporary root")
                .join(format!(
                    "metis-rustup-dispatch-{name}-{}",
                    std::process::id()
                ));
            fs::create_dir(&directory).expect("exclusive actual executable owner");
            let owner = NativeFiles(directory);
            let image = owner.0.join("rustup-init");
            fs::copy(&original.image, &image).expect("copy actual installed Rustup bytes");
            let rustup_alias = owner.0.join("rustup");
            let selected_alias = owner.0.join(name);
            symlink(&image, &rustup_alias).expect("actual Rustup manager symlink");
            symlink(&image, &selected_alias).expect("actual Rustup tool proxy symlink");
            let rustup = executable(&rustup_alias).expect("real manager alias admission");
            let selected = executable(&selected_alias).expect("real tool alias admission");
            assert_eq!(rustup.invocation, rustup_alias);
            assert_eq!(rustup.image, image);
            assert_ne!(rustup.invocation, rustup.image);
            assert_eq!(
                FileIdentity::read(&rustup.image, PAYLOAD_LIMIT).expect("actual copied image"),
                FileIdentity::read(&original.image, PAYLOAD_LIMIT).expect("installed image")
            );
            (owner, workspace, expected, rustup, selected)
        };
        let (resolved, observed_version, expected_version) = {
            let _phase = tracing::info_span!("act").entered();
            let resolved = resolve_rustup(kind, selected, &rustup, workspace)
                .expect("manager invocation preserves actual Rustup dispatch");
            let observed_version = capture_command(
                &resolved,
                &["--version".into()],
                TOOL_IDENTITY_DEADLINE,
                &[],
                Some(workspace),
            )
            .expect("actual resolved tool version");
            let expected_version = capture_command(
                &expected,
                &["--version".into()],
                TOOL_IDENTITY_DEADLINE,
                &[],
                Some(workspace),
            )
            .expect("actual original tool version");
            (resolved, observed_version, expected_version)
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(resolved, expected);
            assert_eq!(
                resolved.canonicalize().expect("canonical selected image"),
                resolved
            );
            assert_eq!(observed_version, expected_version);
            let version = std::str::from_utf8(&observed_version).expect("actual tool version");
            assert!(version.starts_with(&format!("{name} ")));
            if kind == ToolKind::Rustc {
                verify_compiler_pin(version.trim(), workspace)
                    .expect("actual selected compiler matches repository pin");
            }
            assert_eq!(
                FileIdentity::read(&resolved, PAYLOAD_LIMIT).expect("selected tool image"),
                FileIdentity::read(&expected, PAYLOAD_LIMIT).expect("original tool image")
            );
            tracing::info!(tool = name, path = %resolved.display(), %version, "actual multicall query preserves selected tool");
        }
        drop(owner);
    });
}

struct NativeFiles(PathBuf);
impl Drop for NativeFiles {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            tracing::warn!(%error, path = %self.0.display(), "actual Rustup fixture cleanup failed");
        }
    }
}
