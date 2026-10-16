use super::*;
use crate::test_support::traced;
use serde_json::json;

mod collection;
mod fixture;

#[test]
fn actual_cargo_units_partition_every_generated_output_and_compute_inputs() {
    traced(|| {
        let fixture = fixture::Fixture::new(&["rlib", "cdylib"]);
        let messages = fixture.run(
            &[
                "build".into(),
                "--package".into(),
                "artifact_roles".into(),
                "--lib".into(),
            ],
            "3",
        );
        let _phase = tracing::info_span!("assert_actual_compiler_units").entered();
        let mut units = Vec::new();
        for message in messages.iter().filter(|message| {
            message.get("reason").and_then(Value::as_str) == Some("compiler-artifact")
        }) {
            let unit = Unit::parse(message).expect("actual compiler output roles");
            let reported = message["filenames"]
                .as_array()
                .expect("actual filenames")
                .iter()
                .map(|value| PathBuf::from(value.as_str().expect("actual native path")))
                .collect::<BTreeSet<_>>();
            let partition = unit
                .primary
                .iter()
                .map(|primary| &primary.path)
                .chain(&unit.companions)
                .cloned()
                .collect::<BTreeSet<_>>();
            assert_eq!(partition, reported);
            assert_eq!(unit.primary.len() + unit.companions.len(), reported.len());
            for path in partition {
                let metadata = std::fs::metadata(&path).expect("real generated output");
                assert!(metadata.is_file() || metadata.is_dir());
            }
            units.push(unit);
        }
        assert_eq!(units.len(), 3);
        let macro_unit = units
            .iter()
            .find(|unit| unit.target == "lib_prefix_macros")
            .expect("actual macro unit");
        assert_eq!(macro_unit.primary.len(), 1);
        assert!(!macro_unit.custom_build);
        assert_eq!(units.iter().filter(|unit| unit.custom_build).count(), 1);
        let library = units
            .iter()
            .find(|unit| unit.target == "artifact_roles")
            .expect("actual library unit");
        assert!(library.primary.len() >= 2);
        if cfg!(any(windows, target_os = "macos")) {
            assert!(!library.companions.is_empty());
        }
        let generated = messages
            .iter()
            .find(|message| {
                message.get("reason").and_then(Value::as_str) == Some("build-script-executed")
            })
            .expect("actual build-script execution");
        let path = PathBuf::from(generated["out_dir"].as_str().expect("actual output owner"))
            .join("generated.rs");
        assert_eq!(
            std::fs::read_to_string(path).expect("actual computed source"),
            "pub const GENERATED_VALUE: usize = 882;\npub const GENERATED_ENV: &str = \"3\";\n"
        );
        assert_eq!(
            std::fs::read(fixture.root.join("library/label.txt")).expect("real declared source"),
            b"abc"
        );
    });
}

fn event(target: &str, types: &[&str], names: &[&str]) -> Value {
    let root = std::env::temp_dir();
    json!({
        "target": {"name":target,"kind":types,"crate_types":types},
        "filenames":names.iter().map(|name|root.join(name)).collect::<Vec<_>>()
    })
}

#[test]
fn declared_library_roles_partition_primary_and_companion_representations() {
    traced(|| {
        let cases = [
            ("rlib", "liblib_owner-0123456789abcdef.rlib", None),
            ("rlib", "liblib_owner-0123456789abcdef.rmeta", None),
            ("cdylib", "lib_owner.dll", Some("lib_owner.dll.lib")),
            ("cdylib", "lib_owner.dll", Some("lib_owner.dll.exp")),
            ("cdylib", "lib_owner.dll", Some("lib_owner.pdb")),
            ("dylib", "liblib_owner.so", Some("liblib_owner.so.dwp")),
            (
                "proc-macro",
                "liblib_owner.dylib",
                Some("liblib_owner.dylib.dSYM"),
            ),
            ("proc-macro", "liblib_owner.dll", Some("liblib_owner.dll.a")),
            ("staticlib", "liblib_owner.a", None),
            ("staticlib", "lib_owner.lib", None),
            ("cdylib", "lib_owner.wasm", None),
        ];
        for (kind, primary, companion) in cases {
            let _case = tracing::info_span!("representation", kind, primary, companion).entered();
            let (message, expected_primary, expected_companions) = {
                let _phase = tracing::info_span!("arrange").entered();
                let names = companion.map_or_else(|| vec![primary], |name| vec![name, primary]);
                let root = std::env::temp_dir();
                (
                    event("lib_owner", &[kind], &names),
                    vec![root.join(primary)],
                    companion
                        .into_iter()
                        .map(|name| root.join(name))
                        .collect::<Vec<_>>(),
                )
            };
            let unit = {
                let _phase = tracing::info_span!("act").entered();
                Unit::parse(&message).expect("declared representation roles")
            };
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(unit.target, "lib_owner");
            assert!(!unit.custom_build);
            assert_eq!(
                unit.primary
                    .iter()
                    .map(|primary| &primary.path)
                    .collect::<Vec<_>>(),
                expected_primary.iter().collect::<Vec<_>>()
            );
            for primary in &unit.primary {
                let name = primary
                    .dependency
                    .file_stem()
                    .expect("admitted dependency stem")
                    .to_str()
                    .expect("logical name");
                assert!(name == "lib_owner" || name == "lib_owner-0123456789abcdef");
            }
            assert_eq!(unit.companions, expected_companions);
        }
    });
}

#[test]
fn custom_build_role_keeps_hyphenated_primary_and_underscored_debug_name() {
    traced(|| {
        for (primary, companion) in [
            ("build-script-build.exe", "build_script_build.pdb"),
            ("build-script-build", "build-script-build.dSYM"),
            (
                "build_script_build-0123456789abcdef",
                "build_script_build-0123456789abcdef.dwp",
            ),
        ] {
            let message = {
                let _phase = tracing::info_span!("arrange").entered();
                let mut message = event("build-script-build", &["bin"], &[companion, primary]);
                message["target"]["kind"] = json!(["custom-build"]);
                message
            };
            let unit = {
                let _phase = tracing::info_span!("act").entered();
                Unit::parse(&message).expect("actual custom-build spellings")
            };
            let _phase = tracing::info_span!("assert").entered();
            assert!(unit.custom_build);
            assert_eq!(unit.target, "build-script-build");
            assert_eq!(
                unit.primary
                    .iter()
                    .map(|primary| &primary.path)
                    .collect::<Vec<_>>(),
                vec![&std::env::temp_dir().join(primary)]
            );
            assert_eq!(unit.companions, vec![std::env::temp_dir().join(companion)]);
        }
    });
}

#[test]
fn malformed_or_unowned_compiler_outputs_reject_with_exact_cause() {
    traced(|| {
        let phase = tracing::info_span!("arrange_malformed_compiler_events").entered();
        let mut cases = vec![
            (
                event("owner", &["cdylib"], &["owner.pdb"]),
                "compiler event reports companions without a primary output",
            ),
            (
                event("owner", &["cdylib"], &["other.dll"]),
                "compiler output basename differs from its logical target",
            ),
            (
                event("owner", &["cdylib"], &["owner_extra.dll"]),
                "compiler output basename differs from its logical target",
            ),
            (
                event("owner", &["cdylib"], &["owner-.dll"]),
                "compiler output basename differs from its logical target",
            ),
            (
                event("owner", &["cdylib"], &["owner.dll.bak"]),
                "compiler output has an unsupported representation",
            ),
            (
                event("owner", &["staticlib"], &["owner.dll.lib"]),
                "compiler companion contradicts its target role",
            ),
            (
                event("owner", &["rlib"], &["owner.dll"]),
                "compiler primary output contradicts its target role",
            ),
            (
                event("owner", &["bin"], &["owner.exe"]),
                "embedded compiler event has an unsupported target role",
            ),
            (
                event("owner", &["rlib"], &["libowner.rlib", "libowner.rlib"]),
                "compiler output is relative or duplicated",
            ),
        ];
        for (field, value, expected) in [
            ("name", json!(""), "compiler target omits its logical name"),
            (
                "kind",
                json!([1]),
                "compiler target kind contains a non-name",
            ),
            (
                "crate_types",
                json!([]),
                "compiler target omits crate_types",
            ),
        ] {
            let mut message = event("owner", &["rlib"], &["libowner.rlib"]);
            message["target"][field] = value;
            cases.push((message, expected));
        }
        for (value, expected) in [
            (json!([]), "compiler event omits output filenames"),
            (json!([7]), "compiler output filename is not text"),
            (
                json!(["relative.rlib"]),
                "compiler output is relative or duplicated",
            ),
        ] {
            let mut message = event("owner", &["rlib"], &["libowner.rlib"]);
            message["filenames"] = value;
            cases.push((message, expected));
        }
        drop(phase);
        for (message, expected) in cases {
            let phase = tracing::info_span!("act_parse_malformed_event", expected).entered();
            let error = Unit::parse(&message).expect_err("malformed event rejects");
            drop(phase);
            let _phase = tracing::info_span!("assert_rejection_cause", expected).entered();
            assert_eq!(error.to_string(), expected);
        }
    });
}
