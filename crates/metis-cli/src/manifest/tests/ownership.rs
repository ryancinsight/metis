//! Real manifest ownership is independent of invocation and payload location.
use super::super::Manifest;
use std::{fs, path::PathBuf};

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary directory")
            .join(format!(
                "metis-manifest-ownership-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("test clock")
                    .as_nanos()
            ));
        fs::create_dir(&root).expect("owned tree");
        Self(root)
    }
    fn manifest(&self, native: bool, locator: Option<&str>, nested: bool) -> PathBuf {
        let directory = if nested {
            self.0.join(".config/browser")
        } else {
            self.0.clone()
        };
        fs::create_dir_all(&directory).expect("manifest directory");
        let original = if native {
            include_str!("../../../../../metis.json")
        } else {
            include_str!("../../../../metis-starter/metis.json")
        };
        let mut value: serde_json::Value = serde_json::from_str(original).expect("real manifest");
        if let Some(locator) = locator {
            value["application_root"] = locator.into();
        }
        let path = directory.join("metis.json");
        fs::write(&path, serde_json::to_vec(&value).expect("manifest bytes"))
            .expect("physical manifest");
        path
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            tracing::warn!(%error, path = %self.0.display(), "owned test tree cleanup failed");
        }
    }
}
fn capture() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .without_time()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    )
}

#[test]
fn both_shapes_resolve_explicit_owner_and_preserve_omitted_manifest_parent() {
    let _trace = capture();
    let _test = tracing::info_span!("manifest_application_owner").entered();
    for native in [false, true] {
        let tree = tracing::info_span!("arrange", native).in_scope(Tree::new);
        for (locator, nested) in [
            (None, false),
            (None, true),
            (Some("."), false),
            (Some("../.."), true),
        ] {
            let path = tracing::info_span!("arrange", ?locator, nested)
                .in_scope(|| tree.manifest(native, locator, nested));
            let (manifest, root) = tracing::info_span!("act")
                .in_scope(|| Manifest::read(&path).expect("declared owner"));
            tracing::info_span!("assert").in_scope(|| {
                let expected = if locator.is_none() && nested {
                    tree.0.join(".config/browser")
                } else {
                    tree.0.clone()
                };
                assert_eq!(root, expected);
                let (actual_native, cargo_manifest) = match manifest {
                    Manifest::Native(application) => (true, application.cargo_manifest),
                    Manifest::Web(application) => (false, application.cargo_manifest),
                };
                assert_eq!(actual_native, native);
                assert_eq!(cargo_manifest, "Cargo.toml");
            });
        }
    }
}

#[test]
fn locator_admits_exact_budget_and_rejects_platform_and_ambiguous_syntax() {
    let _trace = capture();
    let _test = tracing::info_span!("manifest_locator_domain").entered();
    let tree = tracing::info_span!("arrange").in_scope(Tree::new);
    let exact = format!(
        "{}/..",
        "a".repeat(metis_core::embedded_assets::PORTABLE_PATH_LIMIT - 3)
    );
    fs::create_dir(tree.0.join(exact.split('/').next().expect("component")))
        .expect("exact-budget directory");
    let path = tree.manifest(false, Some(&exact), false);
    let (_, root) = tracing::info_span!("act", boundary = "inclusive")
        .in_scope(|| Manifest::read(&path).expect("inclusive locator budget"));
    tracing::info_span!("assert").in_scope(|| assert_eq!(root, tree.0));
    for locator in [
        "",
        "/absolute",
        "C:/owner",
        "dir\\owner",
        "./owner",
        "owner//..",
        "owner/",
        "CON/..",
        "owner./..",
        "owner\n/..",
        "owner/./..",
        "../../../",
    ] {
        let path = tracing::info_span!("arrange", locator)
            .in_scope(|| tree.manifest(false, Some(locator), false));
        let error = tracing::info_span!("act")
            .in_scope(|| Manifest::read(&path).err().expect("unadmitted locator"));
        tracing::info_span!("assert").in_scope(|| {
            let message = error.to_string();
            assert!(
                message.contains("bounded directory locator")
                    || message.starts_with("payload path"),
                "{message}"
            );
        });
    }
    let excessive = format!("{exact}a");
    let path = tree.manifest(false, Some(&excessive), false);
    let error = tracing::info_span!("act", boundary = "exclusive").in_scope(|| {
        Manifest::read(&path)
            .err()
            .expect("excessive locator bytes")
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(
            error.to_string(),
            "application_root must be a bounded directory locator"
        );
    });
}

#[test]
fn explicit_owner_rejects_missing_files_and_manifest_outside_root() {
    let _trace = capture();
    let _test = tracing::info_span!("manifest_owner_admission").entered();
    let tree = tracing::info_span!("arrange").in_scope(Tree::new);
    fs::create_dir(tree.0.join("other")).expect("other root");
    fs::write(tree.0.join("file"), b"regular file").expect("non-directory root");
    for (locator, cause) in [
        (
            "other",
            "manifest must remain beneath declared application_root",
        ),
        ("file", "canonical input is not a directory"),
    ] {
        let path = tracing::info_span!("arrange", locator)
            .in_scope(|| tree.manifest(false, Some(locator), false));
        let error = tracing::info_span!("act")
            .in_scope(|| Manifest::read(&path).err().expect("invalid owner"));
        tracing::info_span!("assert").in_scope(|| assert_eq!(error.to_string(), cause));
    }
    let path = tree.manifest(false, Some("missing"), false);
    let error =
        tracing::info_span!("act").in_scope(|| Manifest::read(&path).err().expect("missing owner"));
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .expect("IO cause")
                .kind(),
            std::io::ErrorKind::NotFound
        );
    });
}

#[test]
fn explicit_owner_rejects_links_and_omission_retains_canonical_parent() {
    let _trace = capture();
    let _test = tracing::info_span!("manifest_owner_links").entered();
    let tree = tracing::info_span!("arrange").in_scope(Tree::new);
    let root_link = tree.0.join("root-link");
    directory_link(&tree.0, &root_link);
    for locator in [None, Some("../..")] {
        let path = tracing::info_span!("arrange", ?locator)
            .in_scope(|| tree.manifest(false, locator, true));
        let linked_manifest = root_link.join(".config/browser/metis.json");
        let manifest_link = tree.0.join("linked.json");
        file_link(&path, &manifest_link);
        for (candidate, linked_component) in [
            (&linked_manifest, &root_link),
            (&manifest_link, &manifest_link),
        ] {
            if locator.is_some() {
                let error =
                    tracing::info_span!("act", role = "explicit_link_rejection").in_scope(|| {
                        Manifest::read(candidate)
                            .err()
                            .expect("explicit linked manifest")
                    });
                tracing::info_span!("assert").in_scope(|| {
                    assert_eq!(
                        error.to_string(),
                        format!("linked input component: {}", linked_component.display())
                    );
                });
            } else {
                let (_, owner) = tracing::info_span!("act", role = "omitted_canonical_parent")
                    .in_scope(|| Manifest::read(candidate).expect("existing alias admission"));
                tracing::info_span!("assert")
                    .in_scope(|| assert_eq!(owner, tree.0.join(".config/browser")));
            }
        }
        fs::remove_file(manifest_link).expect("owned file link cleanup");
    }
    let path = tree.manifest(false, Some("../../root-link"), true);
    let error = tracing::info_span!("act", role = "declared_root")
        .in_scope(|| Manifest::read(&path).err().expect("linked owner"));
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(
            error.to_string(),
            format!(
                "linked input component: {}",
                tree.0.join(".config/browser/../../root-link").display()
            )
        );
    });
    #[cfg(windows)]
    fs::remove_dir(root_link).expect("owned directory link cleanup");
    #[cfg(unix)]
    fs::remove_file(root_link).expect("owned directory link cleanup");
}

#[test]
fn explicit_root_does_not_grant_parent_or_platform_syntax_to_payload_paths() {
    let _trace = capture();
    let _test = tracing::info_span!("manifest_payload_owner_domain").entered();
    let tree = tracing::info_span!("arrange").in_scope(Tree::new);
    for native in [false, true] {
        for invalid in ["../Cargo.toml", "C:/Cargo.toml", "dir\\Cargo.toml"] {
            let path = tracing::info_span!("arrange", native, invalid).in_scope(|| {
                let path = tree.manifest(native, Some("../.."), true);
                let mut document: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).expect("manifest bytes"))
                        .expect("manifest JSON");
                document["cargo_manifest"] = invalid.into();
                fs::write(&path, serde_json::to_vec(&document).expect("updated bytes"))
                    .expect("updated manifest");
                path
            });
            let error = tracing::info_span!("act").in_scope(|| {
                Manifest::read(&path)
                    .err()
                    .expect("unadmitted payload path")
            });
            let expected = metis_core::embedded_assets::relative(invalid)
                .expect_err("canonical payload rejection");
            tracing::info_span!("assert")
                .in_scope(|| assert_eq!(error.to_string(), expected.to_string()));
        }
    }
}
#[cfg(unix)]
fn directory_link(source: &std::path::Path, destination: &std::path::Path) {
    std::os::unix::fs::symlink(source, destination).expect("actual directory link");
}
#[cfg(windows)]
fn directory_link(source: &std::path::Path, destination: &std::path::Path) {
    std::os::windows::fs::symlink_dir(source, destination).expect("actual directory link");
}
#[cfg(unix)]
fn file_link(source: &std::path::Path, destination: &std::path::Path) {
    std::os::unix::fs::symlink(source, destination).expect("actual file link");
}
#[cfg(windows)]
fn file_link(source: &std::path::Path, destination: &std::path::Path) {
    std::os::windows::fs::symlink_file(source, destination).expect("actual file link");
}
