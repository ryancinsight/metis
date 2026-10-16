//! A native development session owns one canonical source/watch domain.
//!
//! The watcher and snapshot baseline are established for the initial owner.
//! Every reloaded manifest must resolve to that same owner before Cargo source
//! selection, compilation or process launch. A changed declaration may spell
//! the same owner; selecting another owner requires a new development session.
use crate::{Result, manifest::Application};
use std::path::Path;

pub(super) fn read(manifest: &Path, session_root: &Path) -> Result<Application> {
    let (application, current_root) = Application::read(manifest)?;
    if current_root != session_root {
        return Err(format!(
            "dev application_root changed from {} to {}; restart dev to select another root",
            session_root.display(),
            current_root.display()
        )
        .into());
    }
    Ok(application)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    struct Tree(PathBuf);
    impl Drop for Tree {
        fn drop(&mut self) {
            if let Err(error) = fs::remove_dir_all(&self.0) {
                tracing::warn!(%error, path = %self.0.display(), "owned development tree cleanup failed");
            }
        }
    }
    #[test]
    fn reloaded_values_keep_the_session_owner_and_changed_owner_rejects_before_launch() {
        let subscriber = tracing_subscriber::fmt()
            .with_test_writer()
            .without_time()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish();
        let _trace = tracing::subscriber::set_default(subscriber);
        let _test = tracing::info_span!("dev_session_ownership").entered();
        let (tree, path, mut application, session_root) =
            tracing::info_span!("arrange").in_scope(|| {
                let root = std::env::temp_dir()
                    .canonicalize()
                    .expect("canonical temporary directory")
                    .join(format!(
                        "metis-dev-owner-{}-{}",
                        std::process::id(),
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .expect("test clock")
                            .as_nanos()
                    ));
                fs::create_dir_all(root.join(".config/app")).expect("manifest directory");
                fs::create_dir(root.join("owner")).expect("same-owner locator component");
                let tree = Tree(root);
                let path = tree.0.join(".config/app/metis.json");
                let mut application = crate::manifest::tests::fixture();
                application.root = Some("../..".into());
                fs::write(
                    &path,
                    serde_json::to_vec(&application).expect("initial bytes"),
                )
                .expect("initial manifest");
                let (_, session_root) = Application::read(&path).expect("initial owner");
                (tree, path, application, session_root)
            });
        tracing::info_span!("assert", phase = "initial_owner")
            .in_scope(|| assert_eq!(session_root, tree.0));
        tracing::info_span!("arrange", phase = "same_owner_update").in_scope(|| {
            application.root = Some("../../owner/..".into());
            application.name = "Updated Owner Application".into();
            application.arguments = vec!["--stage=verified".into()];
            fs::write(
                &path,
                serde_json::to_vec(&application).expect("updated bytes"),
            )
            .expect("same-owner update");
        });
        let updated = tracing::info_span!("act", phase = "same_owner")
            .in_scope(|| read(&path, &session_root).expect("same canonical owner"));
        tracing::info_span!("assert", phase = "same_owner").in_scope(|| {
            assert_eq!(updated.name, "Updated Owner Application");
            assert_eq!(updated.arguments, ["--stage=verified"]);
            assert_eq!(updated.cargo_manifest, "Cargo.toml");
            assert_eq!(updated.root.as_deref(), Some("../../owner/.."));
        });
        tracing::info_span!("arrange", phase = "changed_owner").in_scope(|| {
            application.root = Some("..".into());
            fs::write(
                &path,
                serde_json::to_vec(&application).expect("changed bytes"),
            )
            .expect("different-owner update");
        });
        let error = tracing::info_span!("act", phase = "changed_owner")
            .in_scope(|| read(&path, &session_root).expect_err("different owner"));
        tracing::info_span!("assert", phase = "changed_owner").in_scope(|| {
            assert_eq!(
                error.to_string(),
                format!(
                    "dev application_root changed from {} to {}; restart dev to select another root",
                    tree.0.display(),
                    tree.0.join(".config").display()
                )
            );
            let (_, current_root) = Application::read(&path).expect("valid independent new owner");
            assert_eq!(current_root, tree.0.join(".config"));
            assert_eq!(session_root, tree.0);
        });
    }
}
