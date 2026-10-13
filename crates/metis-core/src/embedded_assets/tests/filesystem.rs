use super::*;
use std::fs;

#[test]
fn file_identity_streams_complete_bytes_and_enforces_exact_bound() {
    traced(|| {
        let (directory, path, bytes) = {
            let _phase = tracing::info_span!("arrange").entered();
            let directory = std::env::temp_dir()
                .canonicalize()
                .expect("analytically valid contract value")
                .join(format!("metis-stream-{}", std::process::id()));
            fs::create_dir(&directory).expect("analytically valid contract value");
            let path = directory.join("bytes.dat");
            let bytes = (0..65_537)
                .map(|index| u8::try_from(index % 251).expect("analytically valid contract value"))
                .collect::<Vec<_>>();
            fs::write(&path, &bytes).expect("analytically valid contract value");
            (directory, path, bytes)
        };
        let observed = {
            let _phase = tracing::info_span!("act").entered();
            FileIdentity::read(&path, 65_537).expect("analytically valid contract value")
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(observed, identity(&bytes));
            assert_eq!(observed.bytes, 65_537);
            assert_eq!(
                FileIdentity::read(&path, 65_536)
                    .expect_err("adversarial value must be rejected")
                    .kind(),
                std::io::ErrorKind::InvalidData
            );
        }
        fs::remove_dir_all(directory).expect("analytically valid contract value");
    });
}

#[test]
fn directory_digest_matches_canonical_model_and_detects_empty_directory_addition() {
    traced(|| {
        let directory = {
            let _phase = tracing::info_span!("arrange").entered();
            let directory = std::env::temp_dir()
                .canonicalize()
                .expect("canonical temporary root")
                .join(format!("metis-directory-{}", std::process::id()));
            fs::create_dir(&directory).expect("owned test directory");
            fs::write(directory.join("a.txt"), b"abc").expect("published SHA vector file");
            fs::create_dir(directory.join("empty")).expect("empty member directory");
            directory
        };
        let snapshot = {
            let _phase = tracing::info_span!("act").entered();
            DirectorySnapshot::read(&directory, None, DirectoryPolicy::Declared)
                .expect("real directory admission")
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            let mut model = vec![1, 0, 0, 0, 5];
            model.extend_from_slice(b"a.txt");
            model.extend_from_slice(&3_u64.to_be_bytes());
            model.extend_from_slice(Digest::hash(b"abc").as_bytes());
            model.extend_from_slice(&[0, 0, 0, 0, 5]);
            model.extend_from_slice(b"empty");
            assert_eq!(snapshot.digest(), Digest::hash(&model));
            assert_eq!(snapshot.files(), &[("a.txt".into(), identity(b"abc"))]);
        }
        let changed = {
            let _phase = tracing::info_span!("act_add_member").entered();
            fs::create_dir(directory.join("second")).expect("second empty directory");
            DirectorySnapshot::read(&directory, None, DirectoryPolicy::Declared)
                .expect("new actual membership")
        };
        {
            let _phase = tracing::info_span!("assert_add_member").entered();
            assert_ne!(changed.digest(), snapshot.digest());
            assert_eq!(changed.files(), snapshot.files());
        }
        fs::remove_dir_all(directory).expect("owned directory cleanup");
    });
}

#[cfg(unix)]
#[test]
fn real_links_are_rejected_before_source_resolution_and_streaming() {
    traced(|| {
        let directory = {
            let _phase = tracing::info_span!("arrange").entered();
            let directory = std::env::temp_dir()
                .canonicalize()
                .expect("canonical temporary root")
                .join(format!("metis-link-{}", std::process::id()));
            fs::create_dir(&directory).expect("owned test directory");
            fs::write(directory.join("actual.rs"), b"pub const VALUE: usize = 3;")
                .expect("real source");
            std::os::unix::fs::symlink(directory.join("actual.rs"), directory.join("linked.rs"))
                .expect("real file link");
            directory
        };
        let errors = {
            let _phase = tracing::info_span!("act").entered();
            [
                source(&directory, "linked.rs").expect_err("linked source rejected"),
                canonical_file(&directory.join("linked.rs"))
                    .expect_err("linked canonical source rejected"),
                FileIdentity::read(&directory.join("linked.rs"), PAYLOAD_LIMIT)
                    .expect_err("linked hash source rejected"),
            ]
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            for error in errors {
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                assert!(error.to_string().contains("linked"));
            }
            assert_eq!(
                DirectorySnapshot::read(&directory, None, DirectoryPolicy::Declared)
                    .expect_err("linked directory member rejected")
                    .to_string(),
                "declared directory contains a link"
            );
        }
        fs::remove_dir_all(directory).expect("owned link directory cleanup");
    });
}
