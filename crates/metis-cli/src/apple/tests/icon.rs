//! Real icon packaging and rejection preserve application output ownership.

use super::*;
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
};

fn application() -> WebApplication {
    serde_json::from_str(include_str!("../../../../metis-starter/metis.json"))
        .expect("starter manifest")
}

const MARK: &[u8] = include_bytes!("../../../../metis-starter/icons/application.icns");

#[test]
fn bundle_records_exact_declared_icon_and_removes_it_when_omitted() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_shell_icon").entered();
    let arrange = tracing::info_span!("arrange").entered();
    let root = test_directory("shell-icon");
    let served = root.join("served");
    fs::create_dir(&served).expect("assets");
    fs::write(
        served.join("index.html"),
        b"<!doctype html><title>icon oracle</title>",
    )
    .expect("real entry resource");
    let mut application = application();
    let executable = std::env::current_exe().expect("real executable");
    let output = root.join("output");
    drop(arrange);
    let app = tracing::info_span!("act").in_scope(|| {
        bundle::create(
            &application,
            &application_root(),
            &served,
            &executable,
            &output,
            Target::Desktop,
        )
        .expect("icon bundle")
    });
    tracing::info_span!("assert").in_scope(|| {
        let icon = app.join("Contents/Resources/application.icns");
        assert_eq!(fs::read(&icon).expect("packaged icon"), MARK);
        bundle::record(&application, &app, Target::Desktop).expect("inventory");
        let inventory: serde_json::Value = serde_json::from_slice(
            &fs::read(output.join("inventory.json")).expect("inventory bytes"),
        )
        .expect("inventory document");
        let files = inventory["files"].as_array().expect("inventory files");
        assert_eq!(files.len(), 4);
        let record = files
            .iter()
            .find(|file| {
                file["destination"] == "metis-starter.app/Contents/Resources/application.icns"
            })
            .expect("icon record");
        assert_eq!(record["bytes"], MARK.len());
        assert_eq!(
            record["sha256"],
            crate::build::digest_file(&icon).expect("icon digest")
        );
    });
    application.icon = None;
    let app = tracing::info_span!("act", variant = "no_declared_icon").in_scope(|| {
        bundle::create(
            &application,
            &application_root(),
            &served,
            &executable,
            &output,
            Target::Desktop,
        )
        .expect("bundle without custom branding")
    });
    tracing::info_span!("assert", variant = "no_declared_icon").in_scope(|| {
        assert!(!app.join("Contents/Resources/application.icns").exists());
        assert!(
            !fs::read_to_string(app.join("Contents/Info.plist"))
                .expect("plist")
                .contains("CFBundleIconFile")
        );
    });
    fs::remove_dir_all(root).expect("owned fixture cleanup");
}

#[test]
fn invalid_declared_icons_preserve_every_owned_output_byte() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_shell_icon_rejection").entered();
    let arrange = tracing::info_span!("arrange").entered();
    let root = test_directory("icon-rejection");
    fs::create_dir(root.join("icons")).expect("icon directory");
    let source = root.join("icons/application.icns");
    fs::write(&source, MARK).expect("canonical icon");
    let served = root.join("served");
    fs::create_dir(&served).expect("asset directory");
    fs::write(
        served.join("index.html"),
        b"<!doctype html><title>preservation oracle</title>",
    )
    .expect("entry");
    let application = application();
    let executable = std::env::current_exe().expect("executable");
    let output = root.join("output");
    let app = bundle::create(
        &application,
        &root,
        &served,
        &executable,
        &output,
        Target::Desktop,
    )
    .expect("initial bundle");
    bundle::record(&application, &app, Target::Desktop).expect("initial inventory");
    let snapshot = || {
        crate::tree::regular_files(&output, |_| false)
            .expect("owned files")
            .into_iter()
            .map(|path| (path.clone(), fs::read(path).expect("owned bytes")))
            .collect::<Vec<_>>()
    };
    let before = snapshot();
    drop(arrange);
    for failure in ["container", "decode", "oversized", "linked", "missing"] {
        let _case = tracing::info_span!("case", failure).entered();
        fs::write(&source, MARK).expect("restore icon source");
        corrupt_icon(&source, failure);
        let error = tracing::info_span!("act").in_scope(|| {
            bundle::create(
                &application,
                &root,
                &served,
                &executable,
                &output,
                Target::Desktop,
            )
            .expect_err("invalid declared icon")
        });
        tracing::info_span!("assert").in_scope(|| {
            match failure {
                "container" => assert_eq!(
                    error.to_string(),
                    "application icon has an invalid ICNS container"
                ),
                "decode" => assert!(error.to_string().starts_with(&format!(
                    "cannot decode application icon {}:",
                    source.display()
                ))),
                "oversized" => assert_eq!(
                    error.to_string(),
                    "application icon exceeds the 1 MiB budget"
                ),
                "linked" => assert_eq!(
                    error.to_string(),
                    "linked application input is not admitted"
                ),
                "missing" => assert_eq!(
                    error
                        .downcast_ref::<std::io::Error>()
                        .expect("missing source error")
                        .kind(),
                    std::io::ErrorKind::NotFound
                ),
                _ => unreachable!("exhaustive test inputs"),
            }
            assert_eq!(snapshot(), before);
        });
        if failure == "linked" {
            fs::remove_file(&source).expect("remove owned test link");
        }
    }
    fs::remove_dir_all(root).expect("owned fixture cleanup");
}

fn corrupt_icon(source: &Path, failure: &str) {
    match failure {
        "container" => {
            let mut corrupted = MARK.to_vec();
            corrupted[0] ^= 1;
            fs::write(source, corrupted).expect("corrupt container magic");
        }
        "decode" => {
            let mut corrupted = MARK.to_vec();
            let mut offset = 8;
            while offset < corrupted.len() {
                let size = usize::try_from(u32::from_be_bytes(
                    corrupted[offset + 4..offset + 8]
                        .try_into()
                        .expect("ICNS chunk length"),
                ))
                .expect("addressable chunk");
                if &corrupted[offset..offset + 4] != b"info" {
                    corrupted[offset + 8..offset + size].fill(0);
                }
                offset += size;
            }
            fs::write(source, corrupted).expect("corrupt all real image encodings");
        }
        "oversized" => {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .open(source)
                .expect("icon file");
            file.seek(SeekFrom::Start(crate::manifest::ICON_LIMIT))
                .expect("first rejected offset");
            file.write_all(b"!").expect("over-budget byte");
        }
        "linked" => {
            fs::remove_file(source).expect("replace fixture source");
            std::os::unix::fs::symlink(application_root().join("icons/application.icns"), source)
                .expect("source link");
        }
        "missing" => fs::remove_file(source).expect("remove fixture source"),
        _ => unreachable!("exhaustive test inputs"),
    }
}
