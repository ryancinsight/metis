//! Real source/configuration failures retain neutral-directory ownership.
use super::*;
use crate::test_support::traced;
use std::collections::BTreeSet;

#[test]
fn source_and_configuration_failures_and_success_drop_release_every_owned_directory() {
    traced(|| {
        let (root, manifest, before) = {
            let _phase = tracing::info_span!("arrange").entered();
            let root = std::env::temp_dir()
                .canonicalize()
                .expect("real temp owner")
                .join(format!("metis-context-cleanup-{}", std::process::id()));
            fs::create_dir(&root).expect("actual workspace tree");
            (root.clone(), root.join("Cargo.toml"), neutral_directories())
        };
        for (path, bytes, message) in [
            (&manifest, "[workspace\n", "unclosed table, expected `]`"),
            (
                &root.join("rust-toolchain.toml"),
                "[toolchain]\n",
                "Rust toolchain pin omits channel",
            ),
            (
                &root.join(".cargo/config.toml"),
                "include=7\n",
                "Cargo config include must be an array",
            ),
        ] {
            {
                let _phase =
                    tracing::info_span!("arrange_failure", path = %path.display()).entered();
                fs::write(&manifest, "[workspace]\nmembers=[]\n")
                    .expect("actual workspace manifest");
                fs::write(
                    root.join("rust-toolchain.toml"),
                    "[toolchain]\nchannel='1.97.0'\n",
                )
                .expect("actual exact pin");
                fs::create_dir_all(root.join(".cargo")).expect("actual configuration owner");
                fs::write(
                    root.join(".cargo/config.toml"),
                    "[build]\nincremental=false\n",
                )
                .expect("actual configuration");
                fs::write(path, bytes).expect("actual invalid input");
            }
            let (error, after) = {
                let _phase = tracing::info_span!("act_failure").entered();
                let error = Context::new(&manifest)
                    .err()
                    .expect("real malformed input must reject");
                (error.to_string(), neutral_directories())
            };
            {
                let _phase = tracing::info_span!("assert_failure").entered();
                assert!(error.contains(message), "actual cause: {error}");
                assert_eq!(after, before);
            }
        }
        {
            let _phase = tracing::info_span!("arrange_valid").entered();
            fs::write(
                root.join(".cargo/config.toml"),
                "[build]\nincremental=false\n",
            )
            .expect("actual valid configuration");
        }
        let (context, directory, during) = {
            let _phase = tracing::info_span!("act_owned").entered();
            let context = Context::new(&manifest).expect("actual admitted source/configuration");
            let directory = context.directory.clone();
            (context, directory, neutral_directories())
        };
        {
            let _phase = tracing::info_span!("assert_owned").entered();
            assert_eq!(context.workspace, root);
            assert_eq!(
                directory.canonicalize().expect("owned actual directory"),
                directory
            );
            assert_eq!(
                during.difference(&before).cloned().collect::<BTreeSet<_>>(),
                BTreeSet::from([directory.clone()])
            );
        }
        let after = {
            let _phase = tracing::info_span!("act_drop").entered();
            drop(context);
            neutral_directories()
        };
        {
            let _phase = tracing::info_span!("assert_drop").entered();
            assert_eq!(after, before);
            assert_eq!(
                fs::symlink_metadata(directory)
                    .expect_err("released neutral directory must be absent")
                    .kind(),
                std::io::ErrorKind::NotFound
            );
        }
        fs::remove_dir_all(root).expect("owned fixture cleanup");
    });
}

fn neutral_directories() -> BTreeSet<PathBuf> {
    let prefix = format!("metis-module-{}-", std::process::id());
    fs::read_dir(std::env::temp_dir().canonicalize().expect("real temp root"))
        .expect("actual directory inventory")
        .map(|entry| entry.expect("real entry"))
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .map(|entry| entry.path())
        .collect()
}
