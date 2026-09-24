use super::*;

#[test]
fn artifact_inventory_requires_exact_declared_executables() {
    let selected = BTreeMap::from([
        ("metis-app".to_owned(), "application-owner".to_owned()),
        ("image-worker".to_owned(), "worker-owner".to_owned()),
    ]);
    let root = std::env::current_dir().expect("working directory");
    let path = root.join("metis-app.exe");
    let mut message = serde_json::Map::new();
    message.insert("reason".into(), "compiler-artifact".into());
    message.insert("package_id".into(), "application-owner".into());
    message.insert(
        "executable".into(),
        path.to_string_lossy().into_owned().into(),
    );
    let mut target = serde_json::Map::new();
    target.insert("name".into(), "metis-app".into());
    target.insert("kind".into(), vec![serde_json::Value::from("bin")].into());
    message.insert("target".into(), target.into());
    let first = serde_json::to_vec(&message).expect("artifact JSON");
    assert!(
        artifacts(&first, &selected).is_err(),
        "missing companion must fail"
    );
    message.get_mut("target").expect("target")["name"] = "image-worker".into();
    message.insert("package_id".into(), "worker-owner".into());
    let companion = root.join("image-worker.exe");
    message.insert(
        "executable".into(),
        companion.to_string_lossy().into_owned().into(),
    );
    let second = serde_json::to_vec(&message).expect("artifact JSON");
    let mut stream = first.clone();
    stream.push(b'\n');
    stream.extend_from_slice(&second);
    let result = artifacts(&stream, &selected).expect("complete artifact stream");
    assert_eq!(result.get("metis-app"), Some(&path));
    assert_eq!(result.get("image-worker"), Some(&companion));
    stream.push(b'\n');
    stream.extend(first);
    assert!(
        artifacts(&stream, &selected).is_err(),
        "duplicate artifact must fail"
    );
}

#[test]
fn package_target_pair_cannot_be_swapped() {
    let mut app = crate::manifest::tests::fixture();
    app.binaries.push(crate::manifest::Binary {
        package: "image-worker".into(),
        bin: "image-worker".into(),
    });
    let metadata = br#"{"packages":[{"id":"application-owner","name":"metis-app","targets":[{"name":"metis-app","kind":["bin"]}]},{"id":"worker-owner","name":"image-worker","targets":[{"name":"image-worker","kind":["bin"]}]}],"workspace_members":["application-owner","worker-owner"]}"#;
    assert_eq!(
        targets(metadata, &app).expect("workspace targets"),
        BTreeMap::from([
            ("metis-app".into(), "application-owner".into()),
            ("image-worker".into(), "worker-owner".into())
        ])
    );
    app.binaries[0].package = "image-worker".into();
    assert_eq!(
        targets(metadata, &app)
            .expect_err("swapped owner")
            .to_string(),
        "declared binary does not belong to its selected workspace package"
    );
}

#[test]
fn portable_build_target_is_host_native_except_for_windows_x64_msi() {
    assert_eq!(
        cargo_target("windows", "x86_64"),
        Some("x86_64-pc-windows-msvc")
    );
    for host in [
        ("linux", "x86_64"),
        ("macos", "aarch64"),
        ("windows", "aarch64"),
    ] {
        assert_eq!(cargo_target(host.0, host.1), None);
    }
}

#[test]
fn package_host_matches_the_platform_emitter() {
    assert!(supports_host_package());
}

/// Workspace metadata for a browser package whose closure reaches
/// `wasm-bindgen` through `bridge`, beside an unrelated member that locks
/// another `wasm-bindgen`.
fn module_metadata(bridge_bindgen: &str) -> serde_json::Value {
    serde_json::json!({
        "packages": [
            {"id": "starter", "name": "metis-starter", "version": "0.1.0",
             "targets": [{"name": "metis_starter", "kind": ["cdylib", "rlib"]}]},
            {"id": "bridge", "name": "moirai-pal", "version": "0.6.0", "targets": []},
            {"id": "bindgen-new", "name": "wasm-bindgen", "version": "0.2.128", "targets": []},
            {"id": "bindgen-old", "name": "wasm-bindgen", "version": "0.2.100", "targets": []},
            {"id": "other", "name": "other-member", "version": "0.1.0", "targets": []}
        ],
        "workspace_members": ["starter", "other"],
        "resolve": {"nodes": [
            {"id": "starter", "dependencies": ["bridge"]},
            {"id": "bridge", "dependencies": [bridge_bindgen]},
            {"id": "bindgen-new", "dependencies": []},
            {"id": "bindgen-old", "dependencies": []},
            {"id": "other", "dependencies": ["bindgen-old"]}
        ]}
    })
}

#[test]
fn module_target_takes_the_bindgen_version_from_the_package_closure() {
    let metadata = serde_json::to_vec(&module_metadata("bindgen-new")).expect("metadata");
    let target = artifacts::module_target(&metadata, "metis-starter").expect("module target");
    assert_eq!(target.package_id, "starter");
    assert_eq!(target.target, "metis_starter");
    assert_eq!(target.bindgen_version, "0.2.128");
    let metadata = serde_json::to_vec(&module_metadata("bindgen-old")).expect("metadata");
    let target = artifacts::module_target(&metadata, "metis-starter").expect("module target");
    assert_eq!(target.bindgen_version, "0.2.100");
}

#[test]
fn module_target_rejects_packages_it_cannot_bind() {
    let mut document = module_metadata("bindgen-new");
    document["resolve"]["nodes"][1]["dependencies"] =
        serde_json::json!(["bindgen-new", "bindgen-old"]);
    let two = serde_json::to_vec(&document).expect("metadata");
    let error = artifacts::module_target(&two, "metis-starter").expect_err("two versions");
    assert!(
        error.to_string().contains("more than one wasm-bindgen"),
        "{error}"
    );
    document["resolve"]["nodes"][1]["dependencies"] = serde_json::json!([]);
    let none = serde_json::to_vec(&document).expect("metadata");
    let error = artifacts::module_target(&none, "metis-starter").expect_err("no bindgen");
    assert!(
        error
            .to_string()
            .contains("does not depend on wasm-bindgen"),
        "{error}"
    );
    document["packages"][0]["targets"][0]["kind"] = serde_json::json!(["rlib"]);
    let library = serde_json::to_vec(&document).expect("metadata");
    let error = artifacts::module_target(&library, "metis-starter").expect_err("no cdylib");
    assert!(error.to_string().contains("no cdylib target"), "{error}");
    let error = artifacts::module_target(&library, "moirai-pal").expect_err("not a member");
    assert!(
        error.to_string().contains("not a workspace member"),
        "{error}"
    );
}

#[test]
fn module_artifact_is_the_reported_wasm_file() {
    let target = artifacts::ModuleTarget {
        package_id: "starter".into(),
        target: "metis_starter".into(),
        bindgen_version: "0.2.128".into(),
    };
    let root = std::env::current_dir().expect("working directory");
    let wasm = root.join("metis_starter.wasm");
    let message = |package: &str, files: Vec<std::path::PathBuf>| {
        serde_json::json!({
            "reason": "compiler-artifact",
            "package_id": package,
            "target": {"name": "metis_starter", "kind": ["cdylib", "rlib"]},
            "filenames": files,
        })
        .to_string()
    };
    let noise = serde_json::json!({"reason": "build-finished", "success": true}).to_string();
    let stream = [
        message("elsewhere", vec![root.join("other.wasm")]),
        message(
            "starter",
            vec![root.join("libmetis_starter.rlib"), wasm.clone()],
        ),
        noise,
    ]
    .join("\n");
    assert_eq!(
        artifacts::module(stream.as_bytes(), &target).expect("module"),
        wasm
    );
    let doubled = format!("{stream}\n{}", message("starter", vec![wasm.clone()]));
    assert!(
        artifacts::module(doubled.as_bytes(), &target).is_err(),
        "duplicate module"
    );
    let missing = message("elsewhere", vec![wasm]);
    assert!(
        artifacts::module(missing.as_bytes(), &target).is_err(),
        "no module reported"
    );
}
