use super::*;
use std::fs;

#[test]
fn declared_scopes_include_all_members_and_package_policy_excludes_only_root_infrastructure() {
    traced(|| {
        let root = {
            let _phase = tracing::info_span!("arrange").entered();
            let root = std::env::temp_dir()
                .canonicalize()
                .expect("real temp root")
                .join(format!("metis-directory-policy-{}", std::process::id()));
            for path in ["target", ".git", "fixtures/target", "fixtures/.git"] {
                fs::create_dir_all(root.join(path)).expect("real tree");
                fs::write(root.join(path).join("data"), path.as_bytes())
                    .expect("distinct source value");
            }
            root
        };
        let (declared, package, scoped) = {
            let _phase = tracing::info_span!("act").entered();
            (
                DirectorySnapshot::read(&root, None, DirectoryPolicy::Declared)
                    .expect("declared tree"),
                DirectorySnapshot::read(&root, None, DirectoryPolicy::CargoPackage)
                    .expect("package tree"),
                DirectorySnapshot::read(&root, Some("fixtures"), DirectoryPolicy::Declared)
                    .expect("explicit scope"),
            )
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            let expected = [
                ".git/data",
                "fixtures/.git/data",
                "fixtures/target/data",
                "target/data",
            ]
            .map(|name| {
                (
                    name.to_owned(),
                    identity(name.strip_suffix("/data").expect("member scope").as_bytes()),
                )
            });
            assert_eq!(declared.files(), expected);
            assert_eq!(package.files(), &expected[1..3]);
            assert_eq!(scoped.files(), &expected[1..3]);
            assert_ne!(declared.digest(), package.digest());
        }
        let before = scoped.digest();
        let changed = {
            let _phase = tracing::info_span!("act_nested_target_change").entered();
            fs::write(root.join("fixtures/target/data"), b"new actual content")
                .expect("real declared change");
            DirectorySnapshot::read(&root, Some("fixtures"), DirectoryPolicy::Declared)
                .expect("updated scope")
        };
        {
            let _phase = tracing::info_span!("assert_nested_target_change").entered();
            assert_ne!(changed.digest(), before);
            assert_eq!(
                changed.files()[1],
                (
                    "fixtures/target/data".into(),
                    identity(b"new actual content")
                )
            );
        }
        fs::remove_dir_all(root).expect("owned cleanup");
    });
}
