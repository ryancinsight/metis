//! Real child pipes and Cargo diagnostics exercise the public capture boundary.

use super::*;
use crate::embedded_assets::{ToolKind, resolve_tool, tests::traced};
use std::{error::Error, fs, io::Write, path::PathBuf};

const CHILD_ROLE: &str = "METIS_TOOL_CAPTURE_CHILD_ROLE";
const CHILD_TEST: &str = "embedded_assets::fs::process::tests::actual_child_role";
const UTF_OUTPUT: &[u8] = "\ncompiler observation Δ 日本語\n".as_bytes();

#[test]
fn actual_child_role() {
    let bytes: Vec<u8> = (u8::MIN..=u8::MAX).collect();
    let Ok(role) = std::env::var(CHILD_ROLE) else {
        traced(|| {
            let arrange = tracing::info_span!("arrange").entered();
            let expected_sum = 255 * 256 / 2;
            drop(arrange);
            let observed = {
                let _act = tracing::info_span!("act").entered();
                bytes.iter().copied().map(u32::from).sum::<u32>()
            };
            let _assert = tracing::info_span!("assert").entered();
            assert_eq!(bytes.len(), 256);
            assert_eq!(observed, expected_sum);
            assert_eq!(
                std::str::from_utf8(UTF_OUTPUT).expect("Unicode fixture"),
                "\ncompiler observation Δ 日本語\n"
            );
        });
        return;
    };
    let (output, code): (&[u8], i32) = match role.as_str() {
        "unicode-success" => (UTF_OUTPUT, 0),
        "unicode-failure" => (UTF_OUTPUT, 23),
        "bytes-success" => (&bytes, 0),
        "bytes-failure" => (&bytes, 23),
        _ => panic!("unknown real child role: {role}"),
    };
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(output).expect("actual child stdout");
    stdout.flush().expect("actual child pipe flush");
    std::process::exit(code);
}

fn actual_child(role: &str) -> (PathBuf, io::Result<Vec<u8>>) {
    let program = std::env::current_exe().expect("actual installed test executable");
    let result = capture_command(
        &program,
        &[
            "--exact".into(),
            CHILD_TEST.into(),
            "--nocapture".into(),
            "--test-threads=1".into(),
            "--color=never".into(),
        ],
        TOOL_IDENTITY_DEADLINE,
        &[(CHILD_ROLE, role.into())],
        None,
    );
    (program, result)
}

#[test]
fn actual_nonzero_child_retains_unicode_stdout_and_program_context() {
    traced(|| {
        let expected = {
            let _phase = tracing::info_span!("arrange").entered();
            let (_, result) = actual_child("unicode-success");
            let output = result.expect("actual successful child bytes");
            assert!(output.ends_with(UTF_OUTPUT));
            output
        };
        let (program, error) = {
            let _phase = tracing::info_span!("act").entered();
            let (program, result) = actual_child("unicode-failure");
            (program, result.expect_err("actual nonzero child must fail"))
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        let failure = error
            .get_ref()
            .expect("owned failure")
            .downcast_ref::<failure::ToolFailure>()
            .expect("actual tool failure payload");
        assert_eq!(failure.program, program);
        assert_eq!(failure.status.code(), Some(23));
        assert_eq!(failure.stdout, expected);
        let message = error.to_string();
        assert!(message.contains(&format!("{program:?}")), "{message}");
        assert!(message.contains("23"), "{message}");
        let expected = std::str::from_utf8(&expected).expect("actual Unicode pipe bytes");
        assert!(message.contains(expected), "{message}");
        assert!(
            format!("{error:?}").contains(expected),
            "CLI Debug must retain stdout"
        );
    });
}

#[test]
fn actual_nonzero_child_preserves_every_byte_without_lossy_decoding() {
    traced(|| {
        let expected = {
            let _phase = tracing::info_span!("arrange").entered();
            let (_, result) = actual_child("bytes-success");
            let output = result.expect("actual successful binary stdout");
            let all_bytes: Vec<u8> = (u8::MIN..=u8::MAX).collect();
            assert!(output.ends_with(&all_bytes));
            assert_eq!(output[output.len() - 256..], all_bytes);
            output
        };
        let (program, error) = {
            let _phase = tracing::info_span!("act").entered();
            let (program, result) = actual_child("bytes-failure");
            (program, result.expect_err("actual binary failure"))
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        let failure = error
            .get_ref()
            .expect("owned failure")
            .downcast_ref::<failure::ToolFailure>()
            .expect("actual binary failure payload");
        assert_eq!(failure.program, program);
        assert_eq!(failure.status.code(), Some(23));
        assert_eq!(failure.stdout, expected);
        let message = error.to_string();
        assert!(message.contains(&format!("{program:?}")), "{message}");
        assert!(message.contains("escaped bytes"), "{message}");
        assert!(
            message.contains(&expected.escape_ascii().to_string()),
            "{message}"
        );
        assert!(!message.contains('�'));
    });
}

#[test]
fn actual_invalid_rust_crate_keeps_cargo_compiler_stdout() {
    traced(|| {
        let (owner, cargo, compiler) = {
            let _phase = tracing::info_span!("arrange").entered();
            let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .find(|path| path.join("rust-toolchain.toml").is_file())
                .expect("actual pinned workspace");
            let cargo = resolve_tool(ToolKind::Cargo, Path::new("cargo"), workspace)
                .expect("actual selected Cargo");
            let compiler = resolve_tool(ToolKind::Rustc, Path::new("rustc"), workspace)
                .expect("actual selected Rust compiler");
            let root = std::env::temp_dir()
                .canonicalize()
                .expect("actual temporary root")
                .join(format!("metis-invalid-compiler-{}", std::process::id()));
            fs::create_dir(&root).expect("exclusive real compiler input owner");
            let owner = CompilerFiles(root);
            fs::create_dir(owner.0.join("src")).expect("actual crate source directory");
            fs::write(owner.0.join("Cargo.toml"),
                "[package]\nname = 'capture-diagnostic'\nversion = '0.0.0'\nedition = '2024'\n[workspace]\n")
                .expect("actual dependency-free Cargo manifest");
            // Rust's static type law rejects bool where usize is required: E0308.
            fs::write(
                owner.0.join("src/main.rs"),
                "fn main() { let _: usize = true; }\n",
            )
            .expect("actual invalid Rust source");
            (owner, cargo, compiler)
        };
        let error = {
            let _phase = tracing::info_span!("act").entered();
            capture_command(
                &cargo,
                &[
                    "check".into(),
                    "--offline".into(),
                    // Cargo's JSON mode emits compiler messages on stdout.
                    // json-render-diagnostics instead renders them to stderr.
                    "--message-format=json".into(),
                    "--manifest-path".into(),
                    owner.0.join("Cargo.toml").into_os_string(),
                ],
                TOOL_IDENTITY_DEADLINE,
                &[
                    ("RUSTC", compiler.into_os_string()),
                    ("CARGO_TARGET_DIR", owner.0.join("target").into_os_string()),
                ],
                Some(&owner.0),
            )
            .expect_err("actual Rust type error must not succeed")
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        let failure = error
            .get_ref()
            .expect("owned failure")
            .downcast_ref::<failure::ToolFailure>()
            .expect("actual Cargo failure payload");
        assert_eq!(failure.program, cargo);
        assert_eq!(failure.status.code(), Some(101));
        let stdout = std::str::from_utf8(&failure.stdout).expect("actual Cargo JSON UTF-8");
        tracing::info!(program = ?failure.program, status = %failure.status,
            stdout_bytes = failure.stdout.len(), %stdout, "actual Cargo failure stdout");
        assert!(
            stdout.contains("\"reason\":\"compiler-message\""),
            "{error}"
        );
        assert!(stdout.contains("\"code\":\"E0308\""), "{error}");
        let message = error.to_string();
        assert!(message.contains(&format!("{cargo:?}")), "{message}");
        assert!(message.contains("101"), "{message}");
        assert!(
            message.contains("\"reason\":\"compiler-message\""),
            "{message}"
        );
        assert!(message.contains("\"code\":\"E0308\""), "{message}");
        assert!(message.contains("\"level\":\"error\""), "{message}");
        assert!(message.contains("main.rs"), "{message}");
        assert!(
            format!("{error:?}").contains("E0308"),
            "CLI Debug must retain compiler output"
        );
    });
}

#[test]
fn actual_primary_bytes_survive_contextual_cleanup_composition() {
    traced(|| {
        let (program, primary, cleanup, bytes) = {
            let _phase = tracing::info_span!("arrange").entered();
            let (program, success) = actual_child("unicode-success");
            let bytes = success.expect("actual successful stdout oracle");
            let (_, failed) = actual_child("unicode-failure");
            let primary = failed.expect_err("actual failed child");
            // This validates diagnostic composition, not an executor shutdown
            // fault: the secondary error comes from an actual filesystem call.
            let cleanup = fs::metadata(program.join("not-a-directory"))
                .expect_err("an executable is a file, not a directory");
            (program, primary, cleanup, bytes)
        };
        let (combined, first, second, secondary_kind) = {
            let _phase = tracing::info_span!("act").entered();
            let first = primary.to_string();
            let second = cleanup.to_string();
            let secondary_kind = cleanup.kind();
            (
                io::Error::other(failure::CleanupFailure { primary, cleanup }),
                first,
                second,
                secondary_kind,
            )
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(combined.kind(), io::ErrorKind::Other);
        assert_eq!(
            combined.to_string(),
            format!("{first}; executor shutdown also failed: {second}")
        );
        let retained = combined
            .get_ref()
            .expect("owned combined error")
            .downcast_ref::<failure::CleanupFailure>()
            .expect("combined diagnostic owner");
        assert_eq!(retained.cleanup.kind(), secondary_kind);
        let original = retained
            .source()
            .expect("primary source retained")
            .downcast_ref::<io::Error>()
            .expect("original io error")
            .get_ref()
            .expect("original tool failure")
            .downcast_ref::<failure::ToolFailure>()
            .expect("original byte owner");
        assert_eq!(original.program, program);
        assert_eq!(original.status.code(), Some(23));
        assert_eq!(original.stdout, bytes);
        assert!(format!("{combined:?}").contains("compiler observation Δ 日本語"));
    });
}

struct CompilerFiles(PathBuf);
impl Drop for CompilerFiles {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            tracing::warn!(%error, path = %self.0.display(), "actual compiler fixture cleanup failed");
        }
    }
}
