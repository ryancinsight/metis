use super::*;

pub(crate) fn fixture() -> Application {
    serde_json::from_str(include_str!("../../../../metis.json")).expect("fixture manifest")
}

#[test]
fn unsafe_or_ambiguous_destinations_reject() {
    for path in [
        "../escape",
        "/absolute",
        "C:/file",
        "file:stream",
        "dir\\file",
        "NUL.txt",
        "con",
        "file.",
        "dir//file",
        "dir/../file",
        "COM1",
        "CONIN$",
        "CONOUT$.txt",
        "COM¹",
        "LPT².txt",
        "COM³",
        "x\nfile",
    ] {
        assert!(relative(path).is_err(), "accepted {path:?}");
    }
    for path in [
        "assets/form.html",
        "metis-app.exe",
        "Patient Images/scan.bin",
    ] {
        relative(path).expect("admitted path");
    }
}

#[test]
fn identity_inventory_version_and_schema_are_validated() {
    let mut app = fixture();
    app.validate().expect("valid manifest");
    app.resources.push(Resource {
        source: "README.md".into(),
        destination: format!("METIS-APP{}", std::env::consts::EXE_SUFFIX),
    });
    assert!(app.validate().is_err());
    app.resources.clear();
    app.entry = "missing".into();
    assert!(app.validate().is_err());
    app.entry = "metis-app".into();
    for version in [
        "1.2",
        "1.2.3.4",
        "256.0.0",
        "0.256.0",
        "0.0.65536",
        "01.0.0",
        "-1.0.0",
        "+1.0.0",
    ] {
        app.version = version.into();
        assert!(app.validate().is_err(), "accepted {version}");
    }
    app.version = "255.255.65535".into();
    app.validate().expect("MSI boundary version");
    app.schema = 2;
    assert!(app.validate().is_err());
}

#[test]
fn duplicate_and_unknown_manifest_fields_reject() {
    let original = include_str!("../../../../metis.json");
    for field in ["\"schema\":1,", "\"unexpected\":true,"] {
        let text = original.replacen('{', &format!("{{{field}"), 1);
        assert!(serde_json::from_str::<Application>(&text).is_err());
    }
}

#[test]
fn file_directory_collisions_reject() {
    let mut app = fixture();
    app.resources.push(Resource {
        source: "README.md".into(),
        destination: "assets".into(),
    });
    assert!(app.validate().is_err());
}

#[test]
fn shared_directory_casing_is_unambiguous() {
    let mut app = fixture();
    app.resources.push(Resource {
        source: "README.md".into(),
        destination: "Assets/readme.md".into(),
    });
    assert_eq!(
        app.validate()
            .expect_err("directory casing mismatch")
            .to_string(),
        "payload directory has inconsistent casing"
    );
}
