use super::*;
use crate::test_support::traced;
use serde_json::json;
use std::{
    io,
    time::{SystemTime, UNIX_EPOCH},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("actual epoch")
            .as_nanos();
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("native temporary owner")
            .join(format!(
                "metis-dependency-records-{}-{epoch}",
                std::process::id()
            ));
        fs::create_dir(&root).expect("exclusive native directory");
        Self(root)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).expect("real fixture bytes");
        path
    }
    fn unit(&self, name: &str, types: &[&str], outputs: &[&str]) -> Unit {
        Unit::parse(&json!({
            "target":{"name":name,"kind":types,"crate_types":types},
            "filenames":outputs.iter().map(|name|self.0.join(name)).collect::<Vec<_>>()
        }))
        .expect("analytically owned output roles")
    }
    fn custom(&self) -> Unit {
        self.write("build-script-build", b"native primary ownership");
        Unit::parse(&json!({
            "target":{"name":"build-script-build","kind":["custom-build"],"crate_types":["bin"]},
            "filenames":[self.0.join("build-script-build")]
        }))
        .expect("dedicated compiler unit")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove exclusively owned native records");
    }
}

#[test]
fn primary_records_preserve_logical_prefix_and_every_distinct_dependency_set() {
    traced(|| {
        let phase = tracing::info_span!("arrange_records").entered();
        let directory = Directory::new();
        for name in ["liblib_owner-ab12.rmeta", "liblib_owner.rlib"] {
            directory.write(name, b"actual primary ownership boundary");
        }
        let raw = directory.write("lib_owner-ab12.d", b"raw: raw.rs\n# env-dep:RAW_ONLY=one\n");
        let exported = directory.write("lib_owner.d", b"exported: exported.rs\n");
        let unit = directory.unit(
            "lib_owner",
            &["rlib"],
            &["liblib_owner-ab12.rmeta", "liblib_owner.rlib"],
        );
        drop(phase);
        let phase = tracing::info_span!("act_collect_distinct_records").entered();
        let records = collect(&unit, "actual-package", None).expect("distinct primary records");
        drop(phase);
        let phase = tracing::info_span!("assert_distinct_records").entered();
        assert_eq!(
            records
                .iter()
                .map(|record| record.path.clone())
                .collect::<std::collections::BTreeSet<_>>(),
            [raw, exported].into_iter().collect()
        );
        let bytes = records
            .iter()
            .map(|record| read(&unit, "actual-package", record).expect("actual bounded record"))
            .collect::<Vec<_>>();
        assert!(
            bytes
                .iter()
                .any(|bytes| bytes == b"raw: raw.rs\n# env-dep:RAW_ONLY=one\n")
        );
        assert!(
            bytes
                .iter()
                .any(|bytes| bytes == b"exported: exported.rs\n")
        );
        drop(phase);
        let phase = tracing::info_span!("arrange_logical_prefix_case").entered();
        directory.write("lib_owner.dll", b"actual representation naming boundary");
        let unit = directory.unit("lib_owner", &["cdylib"], &["lib_owner.dll"]);
        drop(phase);
        let phase = tracing::info_span!("act_collect_logical_prefix").entered();
        let records = collect(&unit, "actual-package", None).expect("logical lib prefix retained");
        drop(phase);
        let phase = tracing::info_span!("assert_logical_prefix").entered();
        assert_eq!(records[0].path, directory.0.join("lib_owner.d"));
        assert_eq!(records.len(), 1);
        drop(phase);
    });
}

#[test]
fn explicit_root_and_reported_metadata_own_their_complete_compiler_unit() {
    traced(|| {
        let phase = tracing::info_span!("arrange_compiler_unit").entered();
        let directory = Directory::new();
        for name in ["liblib_owner-ab12.rmeta", "liblib_owner.dylib"] {
            directory.write(name, b"actual primary ownership boundary");
        }
        let raw = directory.write(
            "lib_owner-ab12.d",
            b"root: root.rs\n# env-dep:BUILD_ONLY=actual\n",
        );
        let unit = directory.unit(
            "lib_owner",
            &["dylib"],
            &["liblib_owner.dylib", "liblib_owner-ab12.rmeta"],
        );
        drop(phase);
        let phase = tracing::info_span!("act_collect_metadata_owner").entered();
        let records = collect(&unit, "actual-package", None)
            .expect("raw metadata owns uplifted companion primary");
        drop(phase);
        let phase = tracing::info_span!("assert_metadata_owner").entered();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, raw);
        drop(phase);
        let phase = tracing::info_span!("arrange_explicit_raw_output").entered();
        fs::rename(&raw, directory.0.join("owned-root.d"))
            .expect("actual explicit raw output location");
        let owned = directory.0.join("owned-root.d");
        drop(phase);
        let phase = tracing::info_span!("act_collect_explicit_output").entered();
        let records =
            collect(&unit, "actual-package", Some(&owned)).expect("explicit actual raw output");
        drop(phase);
        let phase = tracing::info_span!("assert_explicit_output").entered();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, owned);
        assert_eq!(
            read(&unit, "actual-package", &records[0]).expect("raw declaration"),
            b"root: root.rs\n# env-dep:BUILD_ONLY=actual\n"
        );
        drop(phase);
        let phase = tracing::info_span!("act_collect_suppressed_default").entered();
        let error = collect(&unit, "actual-package", None)
            .err()
            .expect("no guessed missing default");
        drop(phase);
        let phase = tracing::info_span!("assert_suppressed_default").entered();
        assert!(error.to_string().contains("admit raw dep-info"));
        assert!(error.to_string().contains("lib_owner-ab12.d"));
        drop(phase);
    });
}

#[test]
fn absent_raw_record_retains_package_artifact_operation_and_native_error_chain() {
    traced(|| {
        let phase = tracing::info_span!("arrange_missing_native_record").entered();
        let directory = Directory::new();
        let artifact = directory.write("libowner-ab12.rmeta", b"actual primary ownership");
        let unit = directory.unit("owner", &["rlib"], &["libowner-ab12.rmeta"]);
        let missing = directory.0.join("owner-ab12.d");
        let expected = fs::symlink_metadata(&missing).expect_err("actual absent native path");
        drop(phase);
        let phase = tracing::info_span!("act_admit_missing_record").entered();
        let error = collect(&unit, "actual-package", None)
            .err()
            .expect("missing raw record rejects");
        drop(phase);
        let phase = tracing::info_span!("assert_native_error_chain").entered();
        let owner = error
            .downcast_ref::<DependencyError>()
            .expect("typed ownership context");
        assert_eq!(owner.package, "actual-package");
        assert_eq!(owner.target, "owner");
        assert_eq!(owner.artifact, artifact);
        assert_eq!(owner.path, missing);
        assert_eq!(owner.operation, "admit raw dep-info");
        let mut source = error.source();
        let mut native = None;
        while let Some(error) = source {
            if let Some(error) = error.downcast_ref::<io::Error>()
                && error.raw_os_error().is_some()
            {
                native = Some(error);
            }
            source = error.source();
        }
        let native = native.expect("original native IO source retained");
        assert_eq!(native.kind(), expected.kind());
        assert_eq!(native.raw_os_error(), expected.raw_os_error());
        assert_eq!(native.to_string(), expected.to_string());
        assert!(error.to_string().contains("symlink_metadata"));
        drop(phase);
    });
}

#[test]
fn custom_instance_requires_one_unlinked_logically_owned_raw_record() {
    traced(|| {
        let phase = tracing::info_span!("arrange_missing_custom_record").entered();
        let directory = Directory::new();
        let unit = directory.custom();
        drop(phase);
        let phase = tracing::info_span!("act_locate_missing_custom").entered();
        let error = collect(&unit, "custom-package", None)
            .err()
            .expect("missing custom record");
        drop(phase);
        let phase = tracing::info_span!("assert_missing_custom").entered();
        assert!(
            error
                .to_string()
                .contains("custom-build instance contains no raw dep-info")
        );
        drop(phase);
        let phase = tracing::info_span!("arrange_unique_custom_record").entered();
        let raw = directory.write("build_script_build-ab12.d", b"custom: build.rs\n");
        drop(phase);
        let phase = tracing::info_span!("act_locate_unique_custom").entered();
        let records = collect(&unit, "custom-package", None).expect("single actual custom record");
        drop(phase);
        let phase = tracing::info_span!("assert_unique_custom").entered();
        assert_eq!(records[0].path, raw);
        assert_eq!(records.len(), 1);
        drop(phase);
        let phase = tracing::info_span!("arrange_ambiguous_custom_records").entered();
        directory.write("build_script_build-cd34.d", b"other: build.rs\n");
        drop(phase);
        let phase = tracing::info_span!("act_locate_ambiguous_custom").entered();
        let error = collect(&unit, "custom-package", None)
            .err()
            .expect("ambiguous custom record");
        drop(phase);
        let phase = tracing::info_span!("assert_ambiguous_custom").entered();
        assert!(error.to_string().contains("ambiguous raw dep-info"));
        drop(phase);
        let phase = tracing::info_span!("arrange_linked_custom_record").entered();
        fs::remove_file(directory.0.join("build_script_build-cd34.d"))
            .expect("remove owned second record");
        fs::remove_file(&raw).expect("replace owned regular record with real link");
        let actual = directory.write("actual-record", b"custom: build.rs\n");
        link(&actual, &raw);
        drop(phase);
        let phase = tracing::info_span!("act_locate_linked_custom").entered();
        let error = collect(&unit, "custom-package", None)
            .err()
            .expect("linked record rejects");
        drop(phase);
        let phase = tracing::info_span!("assert_linked_custom").entered();
        assert!(error.to_string().contains("linked"));
        fs::remove_file(&raw).expect("remove exclusively owned link");
        drop(phase);
    });
}

#[test]
fn unsupported_uplifted_dylib_and_overbudget_custom_instances_reject() {
    traced(|| {
        let phase = tracing::info_span!("arrange_uplifted_dylib").entered();
        let directory = Directory::new();
        directory.write("libowner.dylib", b"actual primary ownership");
        directory.write("owner.d", b"exported: source.rs\n");
        let unit = directory.unit("owner", &["dylib"], &["libowner.dylib"]);
        drop(phase);
        let phase = tracing::info_span!("act_admit_uplifted_dylib").entered();
        let error = collect(&unit, "actual-package", None)
            .err()
            .expect("exported env absence is not raw evidence");
        drop(phase);
        let phase = tracing::info_span!("assert_uplifted_rejection").entered();
        assert_eq!(
            error.to_string(),
            "compiler package actual-package target owner has an uplifted-only dylib without a reported raw metadata record"
        );
        drop(phase);
        let phase = tracing::info_span!("arrange_overbudget_custom_instance").entered();
        fs::remove_file(directory.0.join("owner.d")).expect("remove owned library dependency");
        let unit = directory.custom();
        directory.write("build_script_build-ab12.d", b"custom: build.rs\n");
        for index in 0..FILE_LIMIT {
            directory.write(&format!("entry-{index}"), b"entry");
        }
        drop(phase);
        let phase = tracing::info_span!("act_walk_custom_instance").entered();
        let error = collect(&unit, "custom-package", None)
            .err()
            .expect("bounded directory walk");
        drop(phase);
        let phase = tracing::info_span!("assert_entry_budget").entered();
        assert!(error.to_string().contains("canonical input-entry budget"));
        drop(phase);
    });
}

#[cfg(unix)]
fn link(actual: &Path, link: &Path) {
    std::os::unix::fs::symlink(actual, link).expect("actual native file link");
}
#[cfg(windows)]
fn link(actual: &Path, link: &Path) {
    std::os::windows::fs::symlink_file(actual, link).expect("actual native file link");
}
