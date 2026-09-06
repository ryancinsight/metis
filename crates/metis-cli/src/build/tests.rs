use super::*;

#[test]
fn artifact_inventory_requires_exact_declared_executables() {
    let selected = BTreeMap::from([
        ("metis-backend".to_owned(), "backend-owner".to_owned()),
        ("metis-frontend".to_owned(), "frontend-owner".to_owned()),
    ]);
    let root = std::env::current_dir().expect("working directory");
    let path = root.join("metis-backend.exe");
    let mut message = serde_json::Map::new();
    message.insert("reason".into(), "compiler-artifact".into());
    message.insert("package_id".into(), "backend-owner".into());
    message.insert(
        "executable".into(),
        path.to_string_lossy().into_owned().into(),
    );
    let mut target = serde_json::Map::new();
    target.insert("name".into(), "metis-backend".into());
    target.insert("kind".into(), vec![serde_json::Value::from("bin")].into());
    message.insert("target".into(), target.into());
    let first = serde_json::to_vec(&message).expect("artifact JSON");
    assert!(
        artifacts(&first, &selected).is_err(),
        "missing companion must fail"
    );
    message.get_mut("target").expect("target")["name"] = "metis-frontend".into();
    message.insert("package_id".into(), "frontend-owner".into());
    let companion = root.join("metis-frontend.exe");
    message.insert(
        "executable".into(),
        companion.to_string_lossy().into_owned().into(),
    );
    let second = serde_json::to_vec(&message).expect("artifact JSON");
    let mut stream = first.clone();
    stream.push(b'\n');
    stream.extend_from_slice(&second);
    let result = artifacts(&stream, &selected).expect("complete artifact stream");
    assert_eq!(result.get("metis-backend"), Some(&path));
    assert_eq!(result.get("metis-frontend"), Some(&companion));
    stream.push(b'\n');
    stream.extend(first);
    assert!(
        artifacts(&stream, &selected).is_err(),
        "duplicate artifact must fail"
    );
}

#[test]
fn package_target_pair_cannot_be_swapped() {
    let app = crate::manifest::tests::fixture();
    let metadata = br#"{"packages":[{"id":"backend-owner","name":"metis-backend","targets":[{"name":"metis-backend","kind":["bin"]}]},{"id":"frontend-owner","name":"metis-frontend","targets":[{"name":"metis-frontend","kind":["bin"]}]}],"workspace_members":["backend-owner","frontend-owner"]}"#;
    assert_eq!(
        targets(metadata, &app).expect("workspace targets"),
        BTreeMap::from([
            ("metis-backend".into(), "backend-owner".into()),
            ("metis-frontend".into(), "frontend-owner".into())
        ])
    );
    let mut app = app;
    app.binaries[0].package = "metis-frontend".into();
    assert_eq!(
        targets(metadata, &app)
            .expect_err("swapped owner")
            .to_string(),
        "declared binary does not belong to its selected workspace package"
    );
}
