use super::*;

mod external_links;

#[cfg(target_os = "macos")]
mod icon;

fn application_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../metis-starter")
}

fn capture() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    )
}

fn test_directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary directory")
        .join(format!(
            "metis-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock")
                .as_nanos()
        ));
    std::fs::create_dir(&path).expect("exclusively owned test directory");
    path
}

#[test]
fn target_selection_matches_sdk_and_compiler_target() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_target_selection").entered();
    for (name, sdk, triple) in [
        ("macos", "macosx", "aarch64-apple-darwin"),
        ("ios", "iphoneos", "aarch64-apple-ios"),
        ("ios-simulator", "iphonesimulator", "aarch64-apple-ios-sim"),
    ] {
        let args = tracing::info_span!("arrange", name)
            .in_scope(|| ["build".into(), "--target".into(), name.into()]);
        let options =
            tracing::info_span!("act").in_scope(|| arguments(&args).expect("target arguments"));
        let _assert = tracing::info_span!("assert").entered();
        assert_eq!(options.target.label(), name);
        assert_eq!(options.target.sdk(), sdk);
        if std::env::consts::ARCH == "aarch64" {
            assert_eq!(options.target.triple().expect("compiler target"), triple);
        }
        assert!(!options.run);
        assert_eq!(options.manifest, Path::new(DEFAULT_MANIFEST));
    }
}

#[test]
fn rejects_ambiguous_or_unprovisioned_launches() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_argument_rejection").entered();
    for (args, expected) in [
        (
            vec!["run", "--target", "ios"],
            "device installation requires Apple provisioning; build the ios bundle first",
        ),
        (
            vec!["build", "--device", "booted"],
            "--device requires --target ios-simulator",
        ),
        (
            vec!["build", "--target", "macos", "--target", "ios"],
            "apple accepts one --target",
        ),
        (
            vec!["build", "one.json", "two.json"],
            "apple accepts one MANIFEST",
        ),
        (
            vec!["build", "--target", "android"],
            "unknown Apple target android",
        ),
        (vec!["build", "--unknown"], "unknown apple option --unknown"),
        (vec!["build", "--output"], "--output requires a directory"),
    ] {
        let args = tracing::info_span!("arrange")
            .in_scope(|| args.into_iter().map(OsString::from).collect::<Vec<_>>());
        let error = tracing::info_span!("act")
            .in_scope(|| arguments(&args).err().expect("invalid arguments"));
        let _assert = tracing::info_span!("assert").entered();
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn apple_plist_has_native_bundle_and_device_families() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_bundle_metadata").entered();
    let arrange = tracing::info_span!("arrange").entered();
    let application: WebApplication =
        serde_json::from_str(include_str!("../../../metis-starter/metis.json"))
            .expect("real starter manifest");
    drop(arrange);
    let act = tracing::info_span!("act").entered();
    let desktop = bundle::info(&application, Target::Desktop);
    let mobile = bundle::info(&application, Target::Simulator);
    drop(act);
    let _assert = tracing::info_span!("assert").entered();
    assert!(desktop.contains("<key>LSMinimumSystemVersion</key>"));
    assert_eq!(
        Target::Desktop.deployment(),
        ("MACOSX_DEPLOYMENT_TARGET", "13.0")
    );
    assert!(desktop.contains("<key>LSMinimumSystemVersion</key><string>13.0</string>"));
    assert!(desktop.contains("<string>com.metis.metis-starter</string>"));
    assert!(desktop.contains("<string>metis-host</string>"));
    assert!(desktop.contains("<key>CFBundleIconFile</key><string>application.icns</string>"));
    assert!(mobile.contains(
        "<key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>"
    ));
    assert!(mobile.contains("<key>UIApplicationSceneManifest</key>"));
    for target in [Target::Simulator, Target::Device] {
        assert_eq!(target.deployment(), ("IPHONEOS_DEPLOYMENT_TARGET", "16.0"));
        assert!(
            bundle::info(&application, target)
                .contains("<key>MinimumOSVersion</key><string>16.0</string>")
        );
    }
    assert!(!mobile.contains("LSMinimumSystemVersion"));
    assert!(!mobile.contains("CFBundleIconFile"));
}

#[test]
fn apple_bundle_preserves_asset_bytes_and_protects_modified_outputs() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_bundle_rebuild").entered();
    let arrange = tracing::info_span!("arrange").entered();
    let root = test_directory("apple");
    let served = root.join("served");
    std::fs::create_dir_all(&served).expect("asset directory");
    let asset = b"<!doctype html><title>packaged byte oracle</title>";
    std::fs::write(served.join("index.html"), asset).expect("HTML asset");
    let mut application: WebApplication =
        serde_json::from_str(include_str!("../../../metis-starter/metis.json"))
            .expect("starter manifest");
    // This portable payload oracle covers the admitted manifest without an
    // icon. Native icon decoding and inclusion use real macOS tests below.
    application.icon = None;
    let executable = std::env::current_exe().expect("real executable");
    drop(arrange);
    for target in [Target::Desktop, Target::Simulator, Target::Device] {
        let _case = tracing::info_span!("target", ?target).entered();
        let output = root.join(target.label());
        let app = tracing::info_span!("act", operation = "initial_bundle").in_scope(|| {
            bundle::create(
                &application,
                &application_root(),
                &served,
                &executable,
                &output,
                target,
            )
            .expect("initial bundle")
        });
        let assert_initial = tracing::info_span!("assert", operation = "initial_bundle").entered();
        let content = if target == Target::Desktop {
            app.join("Contents/Resources/app/index.html")
        } else {
            app.join("app/index.html")
        };
        assert_eq!(std::fs::read(&content).expect("packaged HTML"), asset);
        let plist = if target == Target::Desktop {
            app.join("Contents/Info.plist")
        } else {
            app.join("Info.plist")
        };
        assert_eq!(
            std::fs::read_to_string(plist).expect("bundle plist"),
            bundle::info(&application, target)
        );
        drop(assert_initial);
        let act_rebuild = tracing::info_span!("act", operation = "rebuild").entered();
        bundle::record(&application, &app, target).expect("final inventory");
        let rebuilt = bundle::create(
            &application,
            &application_root(),
            &served,
            &executable,
            &output,
            target,
        )
        .expect("owned rebuild");
        drop(act_rebuild);
        let assert_rebuild = tracing::info_span!("assert", operation = "rebuild").entered();
        assert_eq!(rebuilt, app);
        assert_eq!(std::fs::read(&content).expect("rebuilt bytes"), asset);
        drop(assert_rebuild);
        let act_modified = tracing::info_span!("act", operation = "modified_output").entered();
        bundle::record(&application, &app, target).expect("rebuilt inventory");
        std::fs::write(&content, b"user-modified resource").expect("modify output");
        let error = bundle::create(
            &application,
            &application_root(),
            &served,
            &executable,
            &output,
            target,
        )
        .expect_err("modified output retained");
        drop(act_modified);
        let _assert = tracing::info_span!("assert", operation = "modified_output").entered();
        assert_eq!(error.to_string(), "Apple output contains a modified file");
        assert_eq!(
            std::fs::read(&content).expect("protected modification"),
            b"user-modified resource"
        );
    }
    std::fs::remove_dir_all(root).expect("owned test cleanup");
}

#[cfg(unix)]
#[test]
fn apple_bundle_refuses_added_missing_and_linked_files_without_deletion() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_bundle_output_ownership").entered();
    let arrange = tracing::info_span!("arrange").entered();
    let parent = test_directory("apple-ownership");
    let served = parent.join("served");
    std::fs::create_dir_all(&served).expect("owned test assets");
    let asset = b"<!doctype html><title>ownership oracle</title>";
    std::fs::write(served.join("index.html"), asset).expect("source HTML");
    let application: WebApplication =
        serde_json::from_str(include_str!("../../../metis-starter/metis.json"))
            .expect("starter manifest");
    let executable = std::env::current_exe().expect("real executable");
    drop(arrange);
    for change in ["added", "missing", "linked"] {
        let _case = tracing::info_span!("mutation", change).entered();
        let act = tracing::info_span!("act").entered();
        let output = parent.join(change);
        let app = bundle::create(
            &application,
            &application_root(),
            &served,
            &executable,
            &output,
            Target::Simulator,
        )
        .expect("owned bundle");
        bundle::record(&application, &app, Target::Simulator).expect("owned inventory");
        let entry = app.join("app/index.html");
        match change {
            "added" => {
                std::fs::write(app.join("owner-note.txt"), b"owner edit")
                    .expect("added owner file");
            }
            "missing" => std::fs::remove_file(&entry).expect("remove owned fixture file"),
            "linked" => {
                std::fs::remove_file(&entry).expect("replace owned fixture file");
                std::os::unix::fs::symlink(served.join("index.html"), &entry)
                    .expect("linked fixture file");
            }
            _ => unreachable!("exhaustive test input"),
        }
        let before = std::fs::read(output.join("inventory.json")).expect("original inventory");
        let error = bundle::create(
            &application,
            &application_root(),
            &served,
            &executable,
            &output,
            Target::Simulator,
        )
        .expect_err("unowned output preserved");
        drop(act);
        let _assert = tracing::info_span!("assert").entered();
        assert_eq!(
            std::fs::read(output.join("inventory.json")).expect("preserved inventory"),
            before
        );
        match change {
            "added" | "missing" => assert_eq!(
                error.to_string(),
                "Apple output contains missing or unrecorded files"
            ),
            "linked" => assert_eq!(error.to_string(), format!("{} is a link", entry.display())),
            _ => unreachable!("exhaustive test input"),
        }
        if change == "added" {
            assert_eq!(
                std::fs::read(app.join("owner-note.txt")).expect("preserved owner file"),
                b"owner edit"
            );
        } else if change == "linked" {
            assert_eq!(
                std::fs::read_link(&entry).expect("preserved link"),
                served.join("index.html")
            );
        }
    }
    std::fs::remove_dir_all(parent).expect("owned test cleanup");
}

#[test]
fn apple_bundle_rejects_over_budget_resources_before_replacing_output() {
    use std::io::{Seek, SeekFrom, Write};
    let _trace = capture();
    let _test = tracing::info_span!("apple_resource_budget").entered();
    let arrange = tracing::info_span!("arrange").entered();
    let root = test_directory("apple-budget");
    let served = root.join("served");
    std::fs::create_dir(&served).expect("resource directory");
    let entry = b"<!doctype html><title>budget oracle</title>";
    std::fs::write(served.join("index.html"), entry).expect("entry resource");
    let application: WebApplication =
        serde_json::from_str(include_str!("../../../metis-starter/metis.json"))
            .expect("real starter manifest");
    let output = root.join("output");
    let executable = std::env::current_exe().expect("real executable");
    let app = bundle::create(
        &application,
        &application_root(),
        &served,
        &executable,
        &output,
        Target::Simulator,
    )
    .expect("initial owned bundle");
    bundle::record(&application, &app, Target::Simulator).expect("initial inventory");
    let inventory = std::fs::read(output.join("inventory.json")).expect("owned inventory");
    let oversized = served.join("oversized.bin");
    let limit = moirai_webview::MAX_RESOURCE_BYTES;
    let mut file = std::fs::File::create(&oversized).expect("real bounded-file fixture");
    file.write_all(b"resource budget boundary")
        .expect("input prefix");
    file.seek(SeekFrom::Start(limit))
        .expect("first rejected offset");
    file.write_all(b"!").expect("over-budget byte");
    drop(file);
    assert_eq!(
        std::fs::metadata(&oversized).expect("input metadata").len(),
        limit + 1
    );
    drop(arrange);
    let error = tracing::info_span!("act").in_scope(|| {
        bundle::create(
            &application,
            &application_root(),
            &served,
            &executable,
            &output,
            Target::Simulator,
        )
        .expect_err("native resource budget")
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(
            error.to_string(),
            format!(
                "{} exceeds the {}-byte native resource budget",
                oversized.display(),
                limit
            )
        );
        assert_eq!(
            std::fs::read(output.join("inventory.json")).expect("preserved inventory"),
            inventory
        );
        assert_eq!(
            std::fs::read(app.join("app/index.html")).expect("preserved entry"),
            entry
        );
    });
    std::fs::remove_dir_all(root).expect("owned test cleanup");
}
