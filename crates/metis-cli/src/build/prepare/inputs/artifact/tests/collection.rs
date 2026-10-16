//! The production event collector consumes actual Cargo units and raw output.
use super::{fixture::Fixture, *};
use crate::build::{
    artifacts::ModuleTarget,
    prepare::{
        inputs::{InputSet, events},
        metadata::Graph,
    },
};
use metis_core::embedded_assets::{FileIdentity, OwnerBinding, PAYLOAD_LIMIT};
use std::{collections::BTreeMap, ffi::OsString, path::Path};

#[test]
fn real_cargo_collector_retains_file_environment_union_and_mutation_sensitivity() {
    traced(|| {
        let fixture = Fixture::new(&["rlib", "cdylib"]);
        let exported = ordinary_exported_files(&fixture);
        let (raw, arguments) = {
            let _phase = tracing::info_span!("arrange_explicit_root_output").entered();
            let raw = fixture.root.join("actual-root-dependencies.d");
            let mut emit = OsString::from("--emit=dep-info=");
            emit.push(raw.as_os_str());
            let arguments = [
                "rustc".into(),
                "--package".into(),
                "artifact_roles".into(),
                "--lib".into(),
                "--".into(),
                emit,
            ];
            (raw, arguments)
        };
        let first = fixture.run(&arguments, "3");
        let Baseline {
            input,
            target,
            graph,
            generated,
            first_raw,
            first_generated,
            first_label,
        } = initial_inputs(&fixture, &first, &raw, &exported);
        environment_mutation(
            &fixture,
            &arguments,
            &raw,
            &input,
            &generated,
            &first_raw,
            first_generated,
        );
        let third = source_mutation(&fixture, &arguments, &raw, &input, &generated, first_label);
        let root_event = assert_root_identity(&third, &target, &graph, &raw);
        assert_graph_coverage(&target, &graph, &raw);
        assert_root_cardinality(&third, root_event, &target, &graph, &raw);
    });
}

fn ordinary_exported_files(fixture: &Fixture) -> BTreeSet<PathBuf> {
    let ordinary = fixture.run(
        &[
            "build".into(),
            "--package".into(),
            "artifact_roles".into(),
            "--lib".into(),
        ],
        "3",
    );
    let _phase = tracing::info_span!("assert_exported_dependency_file_set").entered();
    let ordinary_root = ordinary
        .iter()
        .find(|message| {
            message["reason"] == "compiler-artifact"
                && message["target"]["name"] == "artifact_roles"
        })
        .expect("actual ordinary root unit");
    let ordinary_unit = Unit::parse(ordinary_root).expect("actual ordinary root roles");
    let mut exported = BTreeSet::new();
    for primary in &ordinary_unit.primary {
        // Cargo output_depinfo.rs uses the uplifted output's extension,
        // preserving its representation prefix rather than raw naming.
        let path = primary.path.with_extension("d");
        let bytes = std::fs::read(&path).expect("actual Cargo exported declaration");
        assert!(
            !std::str::from_utf8(&bytes)
                .expect("actual Cargo declaration UTF-8")
                .lines()
                .any(|line| line.starts_with("# env-dep:"))
        );
        exported.extend(
            crate::build::prepare::dep_info::paths(&bytes, &fixture.root)
                .expect("actual exported file dependencies"),
        );
    }
    exported
}

struct Baseline {
    input: InputSet,
    target: ModuleTarget,
    graph: Graph,
    generated: PathBuf,
    first_raw: String,
    first_generated: FileIdentity,
    first_label: FileIdentity,
}

fn initial_inputs(
    fixture: &Fixture,
    first: &[Value],
    raw: &Path,
    exported: &BTreeSet<PathBuf>,
) -> Baseline {
    let (input, target, graph) = collected(fixture, first, Some(raw));
    let _phase = tracing::info_span!("assert_initial_compiler_input_union").entered();
    let generated = generated(first);
    let expected_files = [
        fixture.root.join("library/src/lib.rs"),
        fixture.root.join("library/label.txt"),
        fixture.root.join("library/build.rs"),
        fixture.root.join("macros/src/lib.rs"),
        generated.clone(),
    ];
    for path in expected_files {
        let path =
            metis_core::embedded_assets::canonical_file(&path).expect("actual expected source");
        assert!(
            input.files.contains(&path),
            "actual compiler/build input {}",
            path.display()
        );
    }
    assert!(
        exported.is_subset(&input.files),
        "actual Cargo exported files all belong to complete raw/compiler/script union: {:?}",
        exported.difference(&input.files).collect::<Vec<_>>()
    );
    assert_eq!(
        input.environment,
        [
            "METIS_ARTIFACT_ROLE_VALUE".into(),
            "METIS_ARTIFACT_COMPILER_VALUE".into()
        ]
        .into_iter()
        .collect()
    );
    let first_raw = std::fs::read_to_string(raw).expect("actual explicit raw record");
    assert!(
        first_raw
            .lines()
            .any(|line| line == "# env-dep:METIS_ARTIFACT_COMPILER_VALUE=3")
    );
    let first_generated =
        FileIdentity::read(&generated, PAYLOAD_LIMIT).expect("actual generated input identity");
    let first_label = FileIdentity::read(&fixture.root.join("library/label.txt"), PAYLOAD_LIMIT)
        .expect("actual include identity");
    Baseline {
        input,
        target,
        graph,
        generated,
        first_raw,
        first_generated,
        first_label,
    }
}

fn environment_mutation(
    fixture: &Fixture,
    arguments: &[OsString],
    raw: &Path,
    input: &InputSet,
    generated: &Path,
    first_raw: &str,
    first_generated: FileIdentity,
) {
    let second = fixture.run(arguments, "5");
    let (changed, _, _) = collected(fixture, &second, Some(raw));
    let _phase = tracing::info_span!("assert_environment_mutation").entered();
    assert_eq!(changed.environment, input.environment);
    assert_eq!(changed.files, input.files);
    let second_raw = std::fs::read_to_string(raw).expect("fresh raw environment record");
    assert!(
        second_raw
            .lines()
            .any(|line| line == "# env-dep:METIS_ARTIFACT_COMPILER_VALUE=5")
    );
    assert_ne!(second_raw, first_raw);
    assert_ne!(
        FileIdentity::read(generated, PAYLOAD_LIMIT).expect("changed generated identity"),
        first_generated
    );
    assert_eq!(
        std::fs::read_to_string(generated).expect("actual environment-sensitive computation"),
        "pub const GENERATED_VALUE: usize = 1470;\npub const GENERATED_ENV: &str = \"5\";\n"
    );
}

fn source_mutation(
    fixture: &Fixture,
    arguments: &[OsString],
    raw: &Path,
    input: &InputSet,
    generated: &Path,
    first_label: FileIdentity,
) -> Vec<Value> {
    {
        let _phase = tracing::info_span!("arrange_include_mutation").entered();
        std::fs::write(fixture.root.join("library/label.txt"), b"abd")
            .expect("actual include mutation");
    }
    let third = fixture.run(arguments, "5");
    let (changed_source, _, _) = collected(fixture, &third, Some(raw));
    let _phase = tracing::info_span!("assert_include_mutation").entered();
    assert_eq!(changed_source.files, input.files);
    assert_ne!(
        FileIdentity::read(&fixture.root.join("library/label.txt"), PAYLOAD_LIMIT)
            .expect("changed include identity"),
        first_label
    );
    assert_eq!(
        std::fs::read_to_string(generated).expect("actual source-sensitive computation"),
        "pub const GENERATED_VALUE: usize = 1475;\npub const GENERATED_ENV: &str = \"5\";\n"
    );
    third
}

fn assert_root_identity<'event>(
    third: &'event [Value],
    target: &ModuleTarget,
    graph: &Graph,
    raw: &Path,
) -> &'event Value {
    let phase = tracing::info_span!("arrange_wrong_root_identity").entered();
    let root_event = third
        .iter()
        .find(|message| {
            message["reason"] == "compiler-artifact"
                && message["target"]["name"] == "artifact_roles"
        })
        .expect("actual selected compiler unit");
    let root_unit = Unit::parse(root_event).expect("actual selected primary role");
    let wrong_target = ModuleTarget {
        target: "another_target".into(),
        package_id: target.package_id.clone(),
        bindgen_version: target.bindgen_version.clone(),
    };
    drop(phase);
    let phase = tracing::info_span!("act_admit_wrong_root_identity").entered();
    let error = events::collect(
        &encoded(third),
        &wrong_target,
        Some(raw),
        graph,
        &mut empty(graph),
    )
    .expect_err("wrong root identity cannot borrow the raw receipt");
    drop(phase);
    let phase = tracing::info_span!("assert_wrong_root_identity").entered();
    assert!(error.to_string().contains("admit raw dep-info"));
    assert!(
        error
            .to_string()
            .contains(&format!("{:?}", root_unit.primary[0].dependency))
    );
    drop(phase);
    root_event
}

fn assert_root_cardinality(
    third: &[Value],
    root_event: &Value,
    target: &ModuleTarget,
    graph: &Graph,
    raw: &Path,
) {
    let phase = tracing::info_span!("arrange_duplicate_root_event").entered();
    let mut duplicate = third.to_vec();
    duplicate.push(root_event.clone());
    drop(phase);
    let phase = tracing::info_span!("act_admit_duplicate_root").entered();
    let error = events::collect(
        &encoded(&duplicate),
        target,
        Some(raw),
        graph,
        &mut empty(graph),
    )
    .expect_err("explicit raw root ownership is unique");
    drop(phase);
    let phase = tracing::info_span!("assert_duplicate_root").entered();
    assert_eq!(
        error.to_string(),
        "prepared root raw dep-info requires exactly one matching compiler unit"
    );
    drop(phase);
    let phase = tracing::info_span!("arrange_missing_root_event").entered();
    let missing = third
        .iter()
        .filter(|message| *message != root_event)
        .cloned()
        .collect::<Vec<_>>();
    drop(phase);
    let phase = tracing::info_span!("act_admit_missing_root").entered();
    let error = events::collect(
        &encoded(&missing),
        target,
        Some(raw),
        graph,
        &mut empty(graph),
    )
    .expect_err("explicit raw root requires its actual compiler unit");
    drop(phase);
    let phase = tracing::info_span!("assert_missing_root").entered();
    assert_eq!(
        error.to_string(),
        "prepared root raw dep-info requires exactly one matching compiler unit"
    );
    drop(phase);
}

fn assert_graph_coverage(target: &ModuleTarget, graph: &Graph, raw: &Path) {
    for (reason, package, expected) in [
        (
            "compiler-artifact",
            json!("outside-admitted-graph"),
            "compiler/build-script package is absent from the admitted graph: outside-admitted-graph",
        ),
        (
            "build-script-executed",
            json!("outside-admitted-graph"),
            "compiler/build-script package is absent from the admitted graph: outside-admitted-graph",
        ),
        (
            "compiler-artifact",
            Value::Null,
            "compiler/build-script event omits its package identity",
        ),
        (
            "build-script-executed",
            Value::Null,
            "compiler/build-script event omits its package identity",
        ),
    ] {
        let hostile = {
            let _phase = tracing::info_span!("arrange_graph_owner_rejection", reason).entered();
            json!({"reason":reason,"package_id":package})
        };
        let phase = tracing::info_span!("act_admit_graph_owner", reason).entered();
        let error = events::collect(
            &encoded(&[hostile]),
            target,
            Some(raw),
            graph,
            &mut empty(graph),
        )
        .expect_err("total compilation coverage rejects unknown owners");
        drop(phase);
        let _phase = tracing::info_span!("assert_graph_owner_rejection", reason).entered();
        assert_eq!(error.to_string(), expected);
    }
}

fn generated(messages: &[Value]) -> PathBuf {
    PathBuf::from(
        messages
            .iter()
            .find(|message| message["reason"] == "build-script-executed")
            .expect("actual build-script message")["out_dir"]
            .as_str()
            .expect("actual generated directory"),
    )
    .join("generated.rs")
}

fn encoded(messages: &[Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for message in messages {
        serde_json::to_writer(&mut bytes, message).expect("unchanged actual Cargo event encoding");
        bytes.push(b'\n');
    }
    bytes
}

fn empty(graph: &Graph) -> InputSet {
    InputSet {
        files: BTreeSet::new(),
        declared_directories: BTreeSet::new(),
        owners: graph.owners.clone(),
        environment: BTreeSet::new(),
        features: BTreeSet::new(),
    }
}

fn collected(
    fixture: &Fixture,
    messages: &[Value],
    raw: Option<&Path>,
) -> (InputSet, ModuleTarget, Graph) {
    let _phase = tracing::info_span!("act_collect_actual_compiler_events").entered();
    let mut packages = BTreeMap::new();
    for message in messages
        .iter()
        .filter(|message| message["reason"] == "compiler-artifact")
    {
        let id = message["package_id"]
            .as_str()
            .expect("actual package identity");
        let macro_unit = message["target"]["name"] == "lib_prefix_macros";
        let (name, directory) = if macro_unit {
            ("lib_prefix_macros", "macros")
        } else {
            ("artifact_roles", "library")
        };
        packages.insert(
            id.to_owned(),
            (name.to_owned(), fixture.root.join(directory)),
        );
    }
    let package_id = packages
        .iter()
        .find(|(_, (name, _))| name == "artifact_roles")
        .expect("actual root package")
        .0
        .clone();
    let graph = Graph {
        workspace: fixture.root.clone(),
        providers: Vec::new(),
        owners: vec![OwnerBinding {
            owner_id: "workspace".into(),
            root: fixture.root.clone(),
        }],
        packages: packages
            .into_iter()
            .map(|(id, (name, root))| (id, name, root))
            .collect(),
    };
    let target = ModuleTarget {
        package_id,
        target: "artifact_roles".into(),
        bindgen_version: String::new(),
    };
    let mut input = empty(&graph);
    events::collect(&encoded(messages), &target, raw, &graph, &mut input)
        .expect("actual complete compiler input union");
    (input, target, graph)
}
