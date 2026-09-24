use super::{MAX_OPEN_URL_BYTES, OpenError, OpenTarget};

#[test]
fn targets_admit_only_plain_http_urls() {
    let target = OpenTarget::parse("https://Docs.Example.org/guide?q=a%20b#part").expect("url");
    assert_eq!(target.origin().as_str(), "https://docs.example.org");
    for rejected in [
        "",
        "docs.example.org/guide",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "metis://app/page",
        "https://user:secret@example.org/",
        "https://example.org/a b",
        "https://example.org/\"quoted\"",
        "https://example.org/<tag>",
        "https://example.org/back\\slash",
        "https://example.org/%zz",
        "https://example.org/%4",
        "https://exämple.org/",
        "-https://example.org/",
    ] {
        assert!(
            matches!(OpenTarget::parse(rejected), Err(OpenError::InvalidUrl)),
            "{rejected:?}"
        );
    }
    let long = format!("https://example.org/{}", "a".repeat(MAX_OPEN_URL_BYTES));
    assert!(matches!(
        OpenTarget::parse(&long),
        Err(OpenError::InvalidUrl)
    ));
}

#[cfg(unix)]
mod launch {
    use super::super::{MAX_OPEN_DEADLINE, OpenLauncher, ScopedOpener};
    use super::*;
    use metis_core::capability::{CapabilityGrantSpec, CapabilityScope};
    use metis_core::host::{
        HostOrigin, HostPolicy, HostSessionId, VerifiedHostCapability, WindowId,
    };
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("metis-opener-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch directory");
        root
    }

    /// A launcher that records its arguments and exits with `status`.
    fn recording_launcher(root: &std::path::Path, status: u8) -> (OpenLauncher, PathBuf) {
        let record = root.join("arguments.txt");
        let script = root.join("launcher.sh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit {status}\n",
                record.display()
            ),
        )
        .expect("launcher script");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).expect("executable");
        let launcher = OpenLauncher::new(&script, ["--new-window"]).expect("launcher");
        (launcher, record)
    }

    const KEY: &[u8] = b"metis-scoped-opener-test-key";

    fn witness() -> VerifiedHostCapability<{ CapabilityScope::OPEN_EXTERNAL.0 }> {
        let policy = HostPolicy::new(
            HostOrigin::parse("http://127.0.0.1:8080").expect("test origin"),
            WindowId::new(1).expect("test window"),
        );
        let session = HostSessionId::new([3; 16]).expect("test session");
        let context = policy.context_for(session);
        let token = context
            .issue_capability(
                CapabilityGrantSpec {
                    token_id: 1,
                    principal_id: session.as_bytes(),
                    scope: CapabilityScope::OPEN_EXTERNAL,
                    issued_at_secs: 10,
                    duration_secs: 100,
                    nonce: 1,
                },
                KEY,
            )
            .expect("test token");
        policy
            .authorize::<{ CapabilityScope::OPEN_EXTERNAL.0 }>(&token, &context, 11, KEY)
            .expect("open capability")
    }

    #[test]
    fn allowlisted_urls_reach_the_launcher_as_one_argument() {
        let root = scratch("open");
        let (launcher, record) = recording_launcher(&root, 0);
        let opener = ScopedOpener::new(launcher, ["https://docs.example.org"]).expect("opener");
        opener
            .open(
                &witness(),
                "https://docs.example.org/guide?x=1&y=2;rm",
                Duration::from_secs(5),
            )
            .expect("opened");
        assert_eq!(
            fs::read_to_string(&record).expect("record"),
            "--new-window\nhttps://docs.example.org/guide?x=1&y=2;rm\n"
        );
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn denied_urls_and_failures_never_open_silently() {
        let root = scratch("deny");
        let (launcher, record) = recording_launcher(&root, 3);
        let opener = ScopedOpener::new(launcher, ["https://docs.example.org"]).expect("opener");
        let deadline = Duration::from_secs(5);
        assert!(matches!(
            opener.open(&witness(), "https://evil.example.org/", deadline),
            Err(OpenError::OriginDenied)
        ));
        assert!(!record.exists(), "a denied URL never reaches the launcher");
        assert!(matches!(
            opener.open(&witness(), "https://docs.example.org/", Duration::ZERO),
            Err(OpenError::InvalidDeadline)
        ));
        assert!(matches!(
            opener.open(
                &witness(),
                "https://docs.example.org/",
                MAX_OPEN_DEADLINE * 2
            ),
            Err(OpenError::InvalidDeadline)
        ));
        assert!(matches!(
            opener.open(&witness(), "https://docs.example.org/", deadline),
            Err(OpenError::LauncherFailed)
        ));
        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn allowlists_admit_only_web_origins() {
        let root = scratch("origins");
        let (launcher, _) = recording_launcher(&root, 0);
        for origins in [&[][..], &["metis://app"][..], &["https://a.org/path"][..]] {
            assert!(matches!(
                ScopedOpener::new(launcher.clone(), origins.iter().copied()),
                Err(OpenError::InvalidOrigins)
            ));
        }
        let opener =
            ScopedOpener::new(launcher, ["https://A.org", "https://a.org:443"]).expect("opener");
        assert_eq!(opener.allowed_origins().len(), 1);
        assert!(OpenLauncher::new("relative/launcher", Vec::<String>::new()).is_err());
        fs::remove_dir_all(&root).expect("cleanup");
    }
}
