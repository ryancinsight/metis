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
