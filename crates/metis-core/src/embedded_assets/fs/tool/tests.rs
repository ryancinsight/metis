use super::*;
use crate::embedded_assets::{tests::traced, verify_compiler_pin};

#[cfg(unix)]
mod multicall;

#[test]
fn actual_rustup_short_and_host_qualified_selectors_have_one_compiler_identity() {
    traced(|| {
        let workspace = {
            let _phase = tracing::info_span!("arrange").entered();
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .find(|directory| directory.join("rust-toolchain.toml").is_file())
                .expect("actual owning repository pin")
                .to_path_buf()
        };
        let (compiler, short, qualified) = {
            let _phase = tracing::info_span!("act").entered();
            let compiler = resolve_tool(ToolKind::Rustc, Path::new("rustc"), &workspace)
                .expect("actual Rustup-selected compiler");
            let detailed = capture_command(
                &compiler,
                &["-vV".into()],
                TOOL_IDENTITY_DEADLINE,
                &[],
                Some(&workspace),
            )
            .expect("actual compiler host/release metadata");
            let detailed = std::str::from_utf8(&detailed).expect("Rust compiler UTF-8 metadata");
            let host = detailed
                .lines()
                .find_map(|line| line.strip_prefix("host: "))
                .expect("actual compiler host");
            let release = detailed
                .lines()
                .find_map(|line| line.strip_prefix("release: "))
                .expect("actual compiler release");
            let selectors = [release.to_owned(), format!("{release}-{host}")];
            let outputs = selectors.map(|selector| {
                capture_command(
                    Path::new("rustup"),
                    &[
                        "run".into(),
                        selector.into(),
                        "rustc".into(),
                        "--version".into(),
                    ],
                    TOOL_IDENTITY_DEADLINE,
                    &[],
                    Some(&workspace),
                )
                .expect("actual installed pinned selector")
            });
            (compiler, outputs[0].clone(), outputs[1].clone())
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(short, qualified);
            let version = std::str::from_utf8(&short)
                .expect("real version text")
                .trim();
            verify_compiler_pin(version, &workspace)
                .expect("actual compiler release equals repository pin");
            let direct = capture_command(
                &compiler,
                &["--version".into()],
                TOOL_IDENTITY_DEADLINE,
                &[],
                Some(&workspace),
            )
            .expect("actual selected executable");
            assert_eq!(direct, qualified);
            assert!(compiler.is_absolute());
            assert_ne!(
                FileIdentity::read(&compiler, PAYLOAD_LIMIT)
                    .expect("actual compiler digest")
                    .bytes,
                0
            );
        }
    });
}
