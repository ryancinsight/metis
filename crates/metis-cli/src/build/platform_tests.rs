use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_ID: AtomicUsize = AtomicUsize::new(0);

fn fixture() -> (Application, PathBuf, Vec<(PathBuf, String)>) {
    let mut application = crate::manifest::tests::fixture();
    application.resources.clear();
    let root = std::env::temp_dir().join(format!(
        "metis-platform-package-{}-{}",
        std::process::id(),
        TEST_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).expect("package fixture directory");
    let executable_name = format!("metis-app{}", std::env::consts::EXE_SUFFIX);
    let executable = root.join(&executable_name);
    let resource = root.join("manual.txt");
    fs::write(&executable, b"executable").expect("executable fixture");
    fs::write(&resource, b"resource").expect("resource fixture");
    (
        application,
        root,
        vec![
            (executable, executable_name),
            (resource, "manual.txt".to_owned()),
        ],
    )
}

#[test]
fn macos_bundle_writes_info_plist_and_payload() {
    let (application, root, staged) = fixture();
    let record = build_for(Platform::MacOs, &application, &root, "metis-app", &staged)
        .expect("macOS package");
    assert_eq!(record.format, "macos-app-bundle");
    let executable = format!("metis-app{}", std::env::consts::EXE_SUFFIX);
    assert!(
        root.join("org.atlas.metis.demo.app/Contents/MacOS")
            .join(&executable)
            .is_file()
    );
    let plist = fs::read_to_string(root.join("org.atlas.metis.demo.app/Contents/Info.plist"))
        .expect("Info.plist");
    assert!(plist.contains("CFBundleIdentifier"));
    assert!(
        fs::read(
            root.join("org.atlas.metis.demo.app/Contents/MacOS")
                .join(&executable)
        )
        .expect("bundle executable")
        .starts_with(b"executable")
    );
    assert_eq!(
        fs::read_to_string(root.join("org.atlas.metis.demo.app/Contents/Resources/manual.txt"))
            .expect("bundle resource"),
        "resource"
    );
    assert_eq!(record.files.len(), 3);
    fs::remove_dir_all(root).expect("remove package fixture");
}

#[test]
fn linux_archive_contains_fhs_payload_and_desktop_entry() {
    let (application, root, staged) = fixture();
    let record = build_for(Platform::Linux, &application, &root, "metis-app", &staged)
        .expect("Linux package");
    assert_eq!(record.format, "linux-tar");
    let archive = fs::read(root.join("org.atlas.metis.demo.tar")).expect("tar archive");
    let text = String::from_utf8_lossy(&archive);
    assert!(text.contains("usr/bin/metis-app"));
    assert!(text.contains("usr/share/applications/org.atlas.metis.demo.desktop"));
    assert!(text.contains("Exec=/usr/bin/metis-app"));
    assert!(
        archive
            .windows(b"executable".len())
            .any(|bytes| bytes == b"executable")
    );
    assert!(
        archive
            .windows(b"resource".len())
            .any(|bytes| bytes == b"resource")
    );
    assert_eq!(record.sha256, digest(&archive));
    assert_eq!(record.bytes, archive.len() as u64);
    assert_eq!(record.files.len(), 3);
    fs::remove_dir_all(root).expect("remove package fixture");
}

#[test]
fn tar_path_split_preserves_nested_destination() {
    let path = format!("usr/share/{}/{}", "x".repeat(100), "y".repeat(100));
    let (name, prefix) = split_tar_path(&path).expect("split path");
    assert_eq!(name.len(), 100);
    assert_eq!(prefix.len(), 110);
}

#[test]
fn desktop_words_preserve_empty_and_reserved_arguments() {
    assert_eq!(desktop_word(""), "\"\"");
    assert_eq!(desktop_word("a b"), "\"a b\"");
    assert_eq!(desktop_word("$HOME"), "\"\\$HOME\"");
    assert_eq!(desktop_word("a%b"), "\"a%%b\"");
    assert_eq!(desktop_word("a\u{60}b"), "\"a\\\u{60}b\"");
}

#[test]
fn desktop_icon_escapes_string_value_characters() {
    assert_eq!(desktop_icon("/tmp/a b;icon"), "/tmp/a\\sb\\;icon");
}

#[test]
fn declared_url_schemes_are_registered_in_desktop_entry_and_bundle() {
    let (mut application, root, _) = fixture();
    let unregistered = desktop_entry(&application, "metis-app");
    assert!(!unregistered.contains("MimeType") && !unregistered.contains("%u"));
    assert!(url_schemes::plist_registration(&application).is_empty());

    application.url_schemes = vec!["org.atlas.viewer".to_owned(), "atlas-viewer".to_owned()];
    let entry = desktop_entry(&application, "metis-app");
    let exec = entry
        .lines()
        .find(|line| line.starts_with("Exec="))
        .expect("Exec line");
    assert!(exec.ends_with(" %u"), "{exec}");
    assert!(
        entry.contains(
            "MimeType=x-scheme-handler/org.atlas.viewer;x-scheme-handler/atlas-viewer;\n"
        )
    );
    let plist = url_schemes::plist_registration(&application);
    assert!(plist.contains("<key>CFBundleURLName</key><string>org.atlas.metis.demo</string>"));
    assert!(
        plist.contains(
            "<array><string>org.atlas.viewer</string><string>atlas-viewer</string></array>"
        )
    );
    fs::remove_dir_all(root).expect("remove package fixture");
}
