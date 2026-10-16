//! Actual pinned compiler output owns the filename encoding contract.
use super::*;
use crate::test_support::traced;
use metis_core::embedded_assets::{
    DirectoryPolicy, DirectorySnapshot, FileIdentity, MANIFEST_LIMIT, ToolKind,
};
use std::{ffi::OsString, fmt::Write, fs, time::Duration};

#[test]
fn pinned_compiler_dependency_paths_preserve_native_spelling_spaces_dollars_and_unicode() {
    traced(|| {
        let (owner, root, output, expected) = {
            let _phase = tracing::info_span!("arrange").entered();
            let owner = std::env::temp_dir()
                .canonicalize()
                .expect("real temporary root")
                .join(format!(
                    "metis-compiler-dependencies-{}",
                    std::process::id()
                ));
            fs::create_dir(&owner).expect("owned compiler fixture");
            let owner = CompilerFiles(owner);
            let root = owner.0.join("source tree $$ \u{00a0} 日本");
            fs::create_dir_all(root.join("source folder")).expect("actual source owner");
            let names = [
                "source folder/input file.rs",
                "cost$$value.rs",
                "unicode\u{00a0}space_日本.rs",
                "#hash file.rs",
            ];
            let mut main = String::new();
            let mut expected = BTreeSet::new();
            for (index, name) in names.into_iter().enumerate() {
                fs::write(
                    root.join(name),
                    format!("pub const SOURCE_FILE: &str = {name:?};\n"),
                )
                .expect("actual distinct compiler source");
                writeln!(main, "#[path={name:?}] pub mod source_{index};")
                    .expect("owned Rust source formatting");
                expected.insert(root.join(name));
            }
            fs::write(root.join("lib.rs"), main).expect("actual crate source");
            expected.insert(root.join("lib.rs"));
            let output = owner.0.join(if cfg!(unix) {
                "target: output $$ \u{00a0} 日本.d"
            } else {
                "target output $$ \u{00a0} 日本.d"
            });
            (owner, root, output, expected)
        };
        let (observed, target, stdout) = {
            let _phase = tracing::info_span!("act").entered();
            let bytes = compile_dep_info(&root, &output);
            let stdout = bytes.0;
            let input = bytes.1;
            let text = std::str::from_utf8(&input).expect("actual compiler UTF-8");
            let target = text
                .lines()
                .next()
                .expect("actual dependency rule")
                .rsplit_once(": ")
                .expect("actual rule separator")
                .0;
            (
                paths(&input, &root).expect("actual compiler path admission"),
                target.to_owned(),
                stdout,
            )
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            let expected = expected
                .into_iter()
                .map(|path| path.canonicalize().expect("actual source path"))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            assert_eq!(observed, expected);
            assert_eq!(target, output.to_string_lossy());
            assert_eq!(stdout, b"");
            let admitted = DirectorySnapshot::read(&root, None, DirectoryPolicy::CargoPackage)
                .expect("actual portable source admission");
            for path in &expected {
                let relative =
                    crate::tree::relative_name(&root, path).expect("actual relative source");
                let identity = admitted
                    .files()
                    .iter()
                    .find(|(name, _)| *name == relative)
                    .expect("every actual source is in the admitted domain")
                    .1;
                assert_eq!(
                    identity,
                    FileIdentity::read(path, MANIFEST_LIMIT).expect("actual source identity")
                );
            }
            for path in observed {
                let bytes = fs::read(&path).expect("complete actual source");
                assert_eq!(
                    FileIdentity::read(&path, MANIFEST_LIMIT).expect("actual streamed identity"),
                    FileIdentity::from_bytes(&bytes).expect("complete-byte identity")
                );
            }
        }
        drop(owner);
    });
}

#[cfg(unix)]
#[test]
fn actual_backslash_then_following_source_rejects_through_canonical_portable_admission() {
    traced(|| {
        let (owner, root, output) = {
            let _phase = tracing::info_span!("arrange").entered();
            let directory = std::env::temp_dir()
                .canonicalize()
                .expect("actual temp owner")
                .join(format!("metis-dep-domain-{}", std::process::id()));
            fs::create_dir(&directory).expect("actual owner");
            let owner = CompilerFiles(directory.clone());
            fs::write(
                directory.join("a_trailing\\"),
                "pub const LABEL: &str = \"terminal backslash\";\n",
            )
            .expect("actual unsupported Unix source");
            fs::write(
                directory.join("z_after.rs"),
                "pub const LABEL: &str = \"following source\";\n",
            )
            .expect("actual following source");
            fs::write(directory.join("lib.rs"), "#[path=\"a_trailing\\\\\"] pub mod first;\n#[path=\"z_after.rs\"] pub mod following;\n")
                .expect("actual ordered crate");
            let output = directory.join("actual.d");
            (owner, directory, output)
        };
        let (stdout, input, rejection) = {
            let _phase = tracing::info_span!("act").entered();
            let (stdout, input) = compile_dep_info(&root, &output);
            let rejection = DirectorySnapshot::read(&root, None, DirectoryPolicy::CargoPackage)
                .expect_err("literal Unix filename backslashes are outside the producer domain");
            (stdout, input, rejection)
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(stdout, b"");
            let text = std::str::from_utf8(&input).expect("actual compiler report");
            let source_list = text
                .lines()
                .next()
                .expect("actual primary rule")
                .rsplit_once(": ")
                .expect("actual rule boundary")
                .1;
            let expected = [
                root.join("lib.rs"),
                root.join("a_trailing\\"),
                root.join("z_after.rs"),
            ]
            .map(|path| path.to_string_lossy().replace(' ', "\\ "))
            .join(" ");
            assert_eq!(source_list, expected);
            assert_eq!(rejection.kind(), std::io::ErrorKind::InvalidInput);
            assert_eq!(
                rejection.to_string(),
                "payload path must be a bounded relative path without platform syntax"
            );
        }
        drop(owner);
    });
}

struct CompilerFiles(PathBuf);
impl Drop for CompilerFiles {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            tracing::warn!(%error, path = %self.0.display(), "compiler fixture cleanup failed");
        }
    }
}

fn compile_dep_info(root: &Path, output: &Path) -> (Vec<u8>, Vec<u8>) {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|directory| directory.join("rust-toolchain.toml").is_file())
        .expect("owning toolchain pin");
    let compiler =
        metis_core::embedded_assets::resolve_tool(ToolKind::Rustc, Path::new("rustc"), workspace)
            .expect("actual pinned compiler");
    let version = metis_core::embedded_assets::capture_command(
        &compiler,
        &["--version".into()],
        Duration::from_secs(30),
        &[],
        Some(workspace),
    )
    .expect("actual compiler identity");
    metis_core::embedded_assets::verify_compiler_pin(
        std::str::from_utf8(&version)
            .expect("compiler version UTF-8")
            .trim(),
        workspace,
    )
    .expect("repository compiler release");
    let mut emit = OsString::from("--emit=dep-info=");
    emit.push(output);
    let stdout = metis_core::embedded_assets::capture_command(
        &compiler,
        &[
            "--edition=2024".into(),
            "--crate-type=lib".into(),
            "--crate-name=dependency_path_contract".into(),
            emit,
            root.join("lib.rs").into_os_string(),
        ],
        Duration::from_secs(30),
        &[],
        Some(root),
    )
    .expect("real compiler-produced dependency file");
    let input = crate::bounded_read::read_file(
        output,
        MANIFEST_LIMIT,
        "actual compiler dependency report exceeds the manifest bound",
    )
    .expect("complete bounded compiler report");
    (stdout, input)
}

#[test]
fn malformed_empty_and_non_utf8_dependency_reports_reject_with_the_actual_cause() {
    traced(|| {
        let root = {
            let _phase = tracing::info_span!("arrange").entered();
            std::env::temp_dir()
                .canonicalize()
                .expect("actual temp owner")
        };
        for (bytes, cause) in [
            (
                b"invalid line".as_slice(),
                "rustc dep-info omits target separator",
            ),
            (
                b"# env-dep:ONLY=3\n".as_slice(),
                "rustc dep-info contains no file inputs",
            ),
        ] {
            let error = {
                let _phase = tracing::info_span!("act_invalid").entered();
                paths(bytes, &root).expect_err("non-rule report must reject")
            };
            {
                let _phase = tracing::info_span!("assert_invalid").entered();
                assert_eq!(error.to_string(), cause);
            }
        }
        for (bytes, valid, length) in [
            (&[0xff][..], 0, Some(1)),
            (&[b'a', 0xff][..], 1, Some(1)),
            (&[0xe2, 0x82][..], 0, None),
        ] {
            let error = {
                let _phase = tracing::info_span!("act_encoding").entered();
                paths(bytes, &root).expect_err("invalid UTF-8 must reject")
            };
            {
                let _phase = tracing::info_span!("assert_encoding").entered();
                let error = error
                    .downcast_ref::<std::str::Utf8Error>()
                    .expect("preserved UTF-8 cause");
                assert_eq!(error.valid_up_to(), valid);
                assert_eq!(error.error_len(), length);
            }
        }
    });
}
