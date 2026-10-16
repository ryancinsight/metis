//! Native filesystem witnesses retain canonical paths, links and OS failures.
use super::*;
use crate::embedded_assets::tests::traced;

struct Tree(PathBuf);

impl Tree {
    fn new(role: &str) -> Self {
        let parent = std::env::temp_dir()
            .canonicalize()
            .expect("actual native temporary directory");
        let root = parent.join(format!("metis-identity-{role}-{}", std::process::id()));
        fs::create_dir(&root).expect("exclusively owned filesystem witness");
        Self(root)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            tracing::warn!(path = ?self.0, %error, "owned filesystem witness cleanup failed");
        }
    }
}

#[test]
fn actual_created_and_canonical_paths_preserve_identity_and_exact_bound() {
    traced(|| {
        let (tree, created_path, canonical_path) = {
            let _phase = tracing::info_span!("arrange").entered();
            let tree = Tree::new("canonical");
            fs::create_dir(tree.0.join("nested Δ 日本語")).expect("actual Unicode directory");
            let created_path = tree.0.join("nested Δ 日本語/payload.txt");
            fs::write(&created_path, b"abc").expect("published SHA-256 input vector");
            let canonical_path = created_path.canonicalize().expect("native canonical path");
            (tree, created_path, canonical_path)
        };
        let (directory, files, identities, bound_error) = {
            let _phase = tracing::info_span!("act").entered();
            #[cfg(windows)]
            {
                use std::path::Component;
                let mut components = canonical_path.components();
                let Component::Prefix(prefix) = components.next().expect("actual Windows prefix")
                else {
                    panic!("native Windows canonical path must carry a prefix");
                };
                let bare = Path::new(prefix.as_os_str());
                match fs::symlink_metadata(bare) {
                    Ok(metadata) => tracing::info!(path = ?bare, directory = metadata.is_dir(),
                        "actual prefix-only metadata observation"),
                    Err(error) => tracing::info!(path = ?bare, kind = ?error.kind(),
                        raw_os_error = ?error.raw_os_error(), %error,
                        "actual prefix-only metadata observation"),
                }
                let root_component = components.next().expect("actual native root separator");
                assert_eq!(root_component, Component::RootDir);
                let mut root = PathBuf::from(prefix.as_os_str());
                root.push(root_component);
                let metadata = fs::symlink_metadata(&root).expect("assembled native root exists");
                assert!(
                    metadata.is_dir(),
                    "actual canonical root must be a directory"
                );
                tracing::info!(path = ?root, directory = metadata.is_dir(),
                    "actual assembled-root metadata observation");
            }
            let directory = canonical_directory(&tree.0).expect("actual canonical owner");
            let paths = [&created_path, &canonical_path];
            let files = paths.map(|path| canonical_file(path).expect("actual canonical input"));
            let identities =
                paths.map(|path| FileIdentity::read(path, 3).expect("bounded identity"));
            let bound_error =
                FileIdentity::read(&canonical_path, 2).expect_err("exact bound rejects");
            (directory, files, identities, bound_error)
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(
            directory,
            tree.0.canonicalize().expect("native owner oracle")
        );
        assert_eq!(files, [canonical_path.clone(), canonical_path]);
        // FIPS 180-4 SHA-256("abc") is the independent digest oracle.
        let expected = FileIdentity {
            bytes: 3,
            digest: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                .parse()
                .expect("published SHA-256 digest"),
        };
        assert_eq!(identities, [expected; 2]);
        assert_eq!(bound_error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            bound_error.to_string(),
            format!(
                "input is linked, nonregular or oversized: {}",
                files[0].display()
            )
        );
    });
}

#[test]
fn actual_missing_ancestor_retains_operation_native_path_and_os_cause() {
    traced(|| {
        let (tree, missing, expected) = {
            let _phase = tracing::info_span!("arrange").entered();
            let tree = Tree::new("missing");
            let missing = tree.0.join("missing Δ 日本語");
            let expected = fs::symlink_metadata(&missing).expect_err("actual missing ancestor");
            (tree, missing, expected)
        };
        let errors = {
            let _phase = tracing::info_span!("act").entered();
            let path = missing.join("payload.txt");
            [
                canonical_file(&path).expect_err("missing canonical file"),
                canonical_directory(&path).expect_err("missing canonical directory"),
                FileIdentity::read(&path, 3).expect_err("missing identity input"),
                source(&tree.0, "missing Δ 日本語/payload.txt").expect_err("missing owned source"),
            ]
        };
        let _phase = tracing::info_span!("assert").entered();
        for error in errors {
            assert_metadata_cause(&error, &missing, &expected);
        }
    });
}

#[cfg(any(unix, windows))]
#[test]
fn actual_non_unicode_missing_path_retains_native_units_and_os_cause() {
    traced(|| {
        let (_tree, missing, expected, name) = {
            let _phase = tracing::info_span!("arrange").entered();
            let tree = Tree::new("native-units");
            #[cfg(unix)]
            let name = {
                use std::os::unix::ffi::OsStringExt;
                std::ffi::OsString::from_vec(b"missing-\xff".to_vec())
            };
            #[cfg(windows)]
            let name = {
                use std::os::windows::ffi::OsStringExt;
                // Native Windows paths contain UTF-16 units, including an
                // unpaired surrogate that has no Unicode scalar spelling.
                std::ffi::OsString::from_wide(&[0x006d, 0x002d, 0xd800])
            };
            let missing = tree.0.join(&name);
            let expected = fs::symlink_metadata(&missing).expect_err("actual absent native name");
            (tree, missing, expected, name)
        };
        let error = {
            let _phase = tracing::info_span!("act").entered();
            canonical_file(&missing).expect_err("native absent input")
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(
            missing
                .file_name()
                .expect("native final name")
                .as_encoded_bytes(),
            name.as_encoded_bytes()
        );
        assert_eq!(name.to_str(), None);
        assert_metadata_cause(&error, &missing, &expected);
        assert!(!error.to_string().contains('�'));
    });
}

fn assert_metadata_cause(error: &io::Error, path: &Path, expected: &io::Error) {
    assert_eq!(error.kind(), expected.kind());
    assert_eq!(
        error.to_string(),
        format!("symlink_metadata({:?}): {expected}", path.as_os_str())
    );
    let original = error
        .get_ref()
        .expect("owned filesystem operation context")
        .source()
        .expect("original native IO cause")
        .downcast_ref::<io::Error>()
        .expect("retained IO error type");
    assert_eq!(original.kind(), expected.kind());
    assert_eq!(original.raw_os_error(), expected.raw_os_error());
    assert_eq!(original.to_string(), expected.to_string());
    assert!(format!("{error:?}").contains("symlink_metadata"));
}

#[cfg(windows)]
#[test]
fn actual_drive_prefix_without_root_retains_native_directory_semantics() {
    traced(|| {
        let (drive, expected) = {
            let _phase = tracing::info_span!("arrange").entered();
            let current = std::env::current_dir().expect("actual native current directory");
            let std::path::Component::Prefix(prefix) = current
                .components()
                .next()
                .expect("actual current-directory prefix")
            else {
                panic!("actual Windows current directory has a prefix");
            };
            let drive = PathBuf::from(prefix.as_os_str());
            assert_eq!(
                drive.components().count(),
                1,
                "actual prefix has no root component"
            );
            let expected = fs::symlink_metadata(&drive);
            (drive, expected)
        };
        let observed = {
            let _phase = tracing::info_span!("act").entered();
            canonical_directory(&drive)
        };
        let _phase = tracing::info_span!("assert").entered();
        match expected {
            Ok(metadata) => {
                assert!(
                    metadata.is_dir(),
                    "the actual drive-relative object is a directory"
                );
                assert!(
                    !linked(&metadata),
                    "actual drive-current-directory is unlinked"
                );
                let expected = drive
                    .canonicalize()
                    .expect("native drive-relative resolution");
                assert_eq!(observed.expect("same native directory admission"), expected);
            }
            Err(expected) => {
                let error = observed.expect_err("same native prefix-only failure");
                assert_metadata_cause(&error, &drive, &expected);
            }
        }
    });
}

#[test]
fn actual_linked_directory_rejects_before_parent_component_resolution() {
    traced(|| {
        let (tree, link) = {
            let _phase = tracing::info_span!("arrange").entered();
            let tree = Tree::new("ancestor-link");
            let actual = tree.0.join("actual");
            fs::create_dir(&actual).expect("actual directory link target");
            fs::write(actual.join("payload.txt"), b"abc").expect("actual target file");
            let link = tree.0.join("linked");
            directory_link(&actual, &link);
            assert!(linked(
                &fs::symlink_metadata(&link).expect("actual link metadata")
            ));
            (tree, link)
        };
        let errors = {
            let _phase = tracing::info_span!("act").entered();
            let direct = link.join("payload.txt");
            let with_parent = lexical_append(&link, ["..", "actual", "payload.txt"]);
            let directory_with_parent = lexical_append(&link, ["..", "actual"]);
            assert_eq!(
                with_parent
                    .strip_prefix(&tree.0)
                    .expect("actual owner-relative lexical input")
                    .components()
                    .map(Component::as_os_str)
                    .collect::<Vec<_>>(),
                ["linked", "..", "actual", "payload.txt"].map(std::ffi::OsStr::new)
            );
            [
                canonical_file(&direct).expect_err("linked file ancestor"),
                canonical_file(&with_parent).expect_err("link cannot be erased by parent syntax"),
                canonical_directory(&directory_with_parent).expect_err("linked directory ancestor"),
                FileIdentity::read(&with_parent, 3).expect_err("hash rejects linked ancestor"),
            ]
        };
        let _phase = tracing::info_span!("assert").entered();
        for error in errors {
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(
                error.to_string(),
                format!("linked input component: {}", link.display())
            );
        }
        let source_error = source(&tree.0, "linked/payload.txt").expect_err("linked owned source");
        assert_eq!(source_error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            source_error.to_string(),
            "linked application input is not admitted"
        );
        #[cfg(windows)]
        fs::remove_dir(&link).expect("remove exclusively owned junction");
    });
}

fn lexical_append<const N: usize>(base: &Path, parts: [&str; N]) -> PathBuf {
    // Rust 1.97 PathBuf::push normalizes '..' under verbatim Windows prefixes.
    // Build the adversarial native input without applying that transformation;
    // the admission owner must observe and reject the actual preceding link.
    let mut native = base.as_os_str().to_os_string();
    for part in parts {
        native.push(std::path::MAIN_SEPARATOR_STR);
        native.push(part);
    }
    PathBuf::from(native)
}

#[cfg(unix)]
fn directory_link(actual: &Path, link: &Path) {
    std::os::unix::fs::symlink(actual, link).expect("actual native directory symlink");
}

#[cfg(windows)]
fn directory_link(actual: &Path, link: &Path) {
    use crate::embedded_assets::{TOOL_IDENTITY_DEADLINE, capture_command};
    let command =
        PathBuf::from(std::env::var_os("COMSPEC").expect("actual Windows command processor"));
    let bytes = capture_command(
        &command,
        &[
            "/d".into(),
            "/c".into(),
            "mklink".into(),
            "/J".into(),
            link.as_os_str().into(),
            actual.as_os_str().into(),
        ],
        TOOL_IDENTITY_DEADLINE,
        &[],
        None,
    )
    .expect("actual bounded junction creation");
    tracing::info!(stdout_bytes = bytes.len(), path = ?link, target = ?actual,
        "actual junction command completes");
    let metadata = fs::symlink_metadata(link).expect("actual junction filesystem oracle");
    assert!(metadata.is_dir());
    assert!(linked(&metadata));
}
