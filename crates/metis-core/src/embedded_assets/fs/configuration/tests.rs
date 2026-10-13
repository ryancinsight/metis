use super::*;
use crate::embedded_assets::{build_environment_value, tests::traced};
use std::fs;

#[test]
fn actual_config_include_order_retains_origin_and_outer_precedence() {
    traced(|| {
        let root = {
            let _phase = tracing::info_span!("arrange").entered();
            let root = env::temp_dir()
                .canonicalize()
                .expect("real temp root")
                .join(format!("metis-config-order-{}", std::process::id()));
            fs::create_dir(&root).expect("owned root");
            fs::create_dir(root.join(".cargo")).expect("config directory");
            fs::create_dir(root.join("shared")).expect("include directory");
            fs::write(
                root.join("shared/first.toml"),
                "[build]\ntarget-dir='first'\n",
            )
            .expect("first file");
            fs::write(
                root.join("shared/last.toml"),
                "[build]\ntarget-dir='last'\n",
            )
            .expect("last file");
            fs::write(
                root.join(".cargo/config.toml"),
                "include=['../shared/first.toml',{path='../shared/last.toml',optional=true}]\n",
            )
            .expect("actual config");
            root
        };
        let (inputs, selected) = {
            let _phase = tracing::info_span!("act").entered();
            let path = root.join(".cargo/config.toml");
            (
                configuration_inputs(&path).expect("real include closure"),
                target(&path).expect("actual target setting"),
            )
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(
                inputs,
                [
                    root.join("shared/first.toml"),
                    root.join("shared/last.toml"),
                    root.join(".cargo/config.toml")
                ]
            );
            assert_eq!(selected, Some(root.join("last")));
        }
        {
            let _phase = tracing::info_span!("act_outer_override").entered();
            fs::write(root.join(".cargo/config.toml"), "include=['../shared/first.toml','../shared/last.toml']\n[build]\ntarget-dir='outer'\n").expect("outer override");
        }
        {
            let _phase = tracing::info_span!("assert_outer_override").entered();
            assert_eq!(
                target(&root.join(".cargo/config.toml")).expect("real outer precedence"),
                Some(root.join("outer"))
            );
        }
        fs::remove_dir_all(root).expect("owned cleanup");
    });
}

#[test]
fn config_closure_rejects_cycle_unknown_fields_missing_and_oversized_inputs() {
    traced(|| {
        let root = {
            let _phase = tracing::info_span!("arrange").entered();
            let root = env::temp_dir()
                .canonicalize()
                .expect("real temp root")
                .join(format!("metis-config-rejection-{}", std::process::id()));
            fs::create_dir(&root).expect("owned root");
            root
        };
        for (document, kind, message) in [
            (
                "include=['config.toml']",
                io::ErrorKind::InvalidData,
                "cycle",
            ),
            (
                "include=[{path='config.toml',other=true}]",
                io::ErrorKind::InvalidData,
                "unknown fields",
            ),
            (
                "include=[{path='missing.toml',optional=true}]",
                io::ErrorKind::NotFound,
                "",
            ),
        ] {
            {
                let _phase = tracing::info_span!("arrange_case", document).entered();
                fs::write(root.join("config.toml"), document).expect("declared config");
            }
            let error = {
                let _phase = tracing::info_span!("act").entered();
                configuration_inputs(&root.join("config.toml")).expect_err("closed invalid context")
            };
            {
                let _phase = tracing::info_span!("assert").entered();
                assert_eq!(error.kind(), kind);
                assert!(error.to_string().contains(message));
            }
        }
        let oversized = {
            let _phase = tracing::info_span!("act_bound").entered();
            let bytes =
                vec![b' '; usize::try_from(MANIFEST_LIMIT + 1).expect("host manifest width")];
            fs::write(root.join("config.toml"), bytes).expect("oversized actual file");
            configuration_inputs(&root.join("config.toml")).expect_err("reject before parse")
        };
        {
            let _phase = tracing::info_span!("assert_bound").entered();
            assert_eq!(oversized.kind(), io::ErrorKind::InvalidData);
            assert!(oversized.to_string().contains("oversized"));
        }
        fs::remove_dir_all(root).expect("owned cleanup");
    });
}

#[test]
fn actual_cargo_child_environment_resolves_force_relative_and_include_override() {
    traced(|| {
        let root = {
            let _phase = tracing::info_span!("arrange").entered();
            let root = env::temp_dir()
                .canonicalize()
                .expect("real temp root")
                .join(format!("metis-config-env-{}", std::process::id()));
            fs::create_dir(&root).expect("owned root");
            for directory in [".cargo", "cargo-home", "shared"] {
                fs::create_dir(root.join(directory)).expect("real config owner");
            }
            fs::write(
                root.join("cargo-home/config.toml"),
                "[env]\nMETIS_CONTRACT_INPUT={value='home',force=true}\n",
            )
            .expect("Cargo home config");
            fs::write(
                root.join("shared/values.toml"),
                "[env]\nMETIS_CONTRACT_INPUT={value='leaf',force=true,relative=true}\n",
            )
            .expect("included config");
            fs::write(root.join(".cargo/config.toml"), "include=['../shared/values.toml']\n[env]\nMETIS_CONTRACT_FIXED={value='exact',force=true}\n").expect("workspace config");
            root
        };
        let observed = {
            let _phase = tracing::info_span!("act").entered();
            ["METIS_CONTRACT_INPUT", "METIS_CONTRACT_FIXED"].map(|key| {
                build_environment_value(key, &root, &root.join("cargo-home"))
                    .expect("effective child environment")
            })
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(
                observed,
                [
                    Some(root.join("leaf").to_str().expect("UTF-8 path").to_owned()),
                    Some("exact".into())
                ]
            );
        }
        fs::remove_dir_all(root).expect("owned cleanup");
    });
}
