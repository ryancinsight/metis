//! Direct workspace/repository inputs retain the canonical portable domain.
use super::*;
use crate::test_support::traced;

#[test]
fn direct_sources_admit_complete_portable_bytes_and_reject_unportable_components() {
    traced(|| {
        let root = {
            let _phase = tracing::info_span!("arrange").entered();
            let root = std::env::temp_dir()
                .canonicalize()
                .expect("real temp root")
                .join(format!("metis-direct-source-{}", std::process::id()));
            fs::create_dir(&root).expect("owned source domain");
            root
        };
        let (path, content) = {
            let _phase = tracing::info_span!("arrange_portable").entered();
            let path = root.join("source $$ \u{00a0} 日本.rs");
            let content = b"pub const SOURCE: &str = \"portable compiler input\";\n";
            fs::write(&path, content).expect("actual source bytes");
            (path, content)
        };
        let identity = {
            let _phase = tracing::info_span!("act_portable").entered();
            direct_source(&root, &path).expect("real admitted source")
        };
        {
            let _phase = tracing::info_span!("assert_portable").entered();
            assert_eq!(
                identity,
                FileIdentity::from_bytes(content).expect("complete-byte oracle")
            );
        }
        for name in ["trailing ", "trailing."] {
            let path = {
                let _phase = tracing::info_span!("arrange_rejection", name).entered();
                let path = root.join(name);
                fs::write(&path, content).expect("actual unportable source");
                path
            };
            let error = {
                let _phase = tracing::info_span!("act_rejection").entered();
                direct_source(&root, &path)
                    .expect_err("ambiguous source must reject before hashing")
            };
            {
                let _phase = tracing::info_span!("assert_rejection").entered();
                let error = error
                    .downcast_ref::<std::io::Error>()
                    .expect("preserved path cause");
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
                assert_eq!(
                    error.to_string(),
                    "payload path contains a reserved or ambiguous component"
                );
            }
        }
        #[cfg(unix)]
        {
            let path = {
                let _phase = tracing::info_span!("arrange_unix_backslash").entered();
                let path = root.join("a_trailing\\");
                fs::write(&path, content).expect("actual Unix literal backslash source");
                path
            };
            let error = {
                let _phase = tracing::info_span!("act_unix_backslash").entered();
                direct_source(&root, &path).expect_err("unrepresentable source name must reject")
            };
            {
                let _phase = tracing::info_span!("assert_unix_backslash").entered();
                let error = error
                    .downcast_ref::<std::io::Error>()
                    .expect("preserved path cause");
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
                assert_eq!(
                    error.to_string(),
                    "payload path must be a bounded relative path without platform syntax"
                );
            }
        }
        fs::remove_dir_all(root).expect("owned source cleanup");
    });
}
