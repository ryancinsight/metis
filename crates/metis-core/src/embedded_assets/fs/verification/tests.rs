//! Rebuild watches do not confer configuration-input authority.
use super::*;
use crate::embedded_assets::{MANIFEST_LIMIT, tests::traced};
use std::fs;

#[test]
fn configuration_watches_cannot_admit_undeclared_workspace_or_home_inputs() {
    traced(|| {
        let (root, workspace, home) = {
            let _phase = tracing::info_span!("arrange").entered();
            let root = std::env::temp_dir()
                .canonicalize()
                .expect("real temp root")
                .join(format!("metis-config-authority-{}", std::process::id()));
            let workspace = root.join("workspace");
            let home = root.join("cargo-home");
            fs::create_dir_all(workspace.join(".cargo")).expect("workspace config owner");
            fs::create_dir(&home).expect("Cargo home owner");
            (root, workspace, home)
        };
        for path in [
            workspace.join(".cargo/config.toml"),
            home.join("config.toml"),
        ] {
            let (verified, mut watches) = {
                let _phase = tracing::info_span!("arrange_case", path = %path.display()).entered();
                fs::write(&path, "[build]\nincremental = false\n")
                    .expect("actual configuration value");
                (BTreeMap::new(), vec![path.clone()])
            };
            let error = {
                let _phase = tracing::info_span!("act_undeclared").entered();
                configuration_inputs(&workspace, &home, &verified, &mut watches)
                    .expect_err("watch registration cannot admit new compiler configuration")
            };
            {
                let _phase = tracing::info_span!("assert_undeclared").entered();
                assert_eq!(error.kind(), io::ErrorKind::InvalidData);
                assert_eq!(
                    error.to_string(),
                    format!(
                        "current build configuration is not a declared input: {}",
                        path.display()
                    )
                );
            }
            let (verified, mut watches) = {
                let _phase = tracing::info_span!("arrange_declared").entered();
                (
                    BTreeMap::from([(
                        path.clone(),
                        FileIdentity::read(&path, MANIFEST_LIMIT)
                            .expect("hash actual declared config bytes"),
                    )]),
                    Vec::new(),
                )
            };
            {
                let _phase = tracing::info_span!("act_declared").entered();
                configuration_inputs(&workspace, &home, &verified, &mut watches)
                    .expect("actual recorded configuration remains admissible");
            }
            {
                let _phase = tracing::info_span!("assert_declared").entered();
                assert_eq!(
                    verified[&path],
                    FileIdentity::from_bytes(b"[build]\nincremental = false\n")
                        .expect("analytical complete config value")
                );
                assert_eq!(
                    watches,
                    [
                        workspace.join(".cargo/config"),
                        workspace.join(".cargo/config.toml"),
                        home.join("config"),
                        home.join("config.toml"),
                        workspace.join("rust-toolchain.toml"),
                        path.clone(),
                    ]
                );
            }
            fs::remove_file(path).expect("owned configuration cleanup");
        }
        fs::remove_dir_all(root).expect("owned tree cleanup");
    });
}
