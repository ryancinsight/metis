#[cfg(not(windows))]
use super::MAX_AUTOSTART_ARGUMENTS;
use super::{Autostart, entry};

#[cfg(not(windows))]
fn at(root: &std::path::Path, name: &str, arguments: &[&str]) -> std::io::Result<Autostart> {
    Autostart::build(
        name,
        std::path::Path::new("/opt/Metis Viewer/viewer"),
        arguments.iter().copied(),
        root,
    )
}

#[test]
fn entries_quote_the_command_for_their_parser() {
    let arguments = ["--minimized".to_owned(), "100%".to_owned()];
    let desktop = entry::desktop_entry("org.metis.viewer", "/opt/Metis Viewer/viewer", &arguments);
    assert!(desktop.contains("Exec=\"/opt/Metis Viewer/viewer\" --minimized \"100%%\"\n"));
    assert!(desktop.starts_with("[Desktop Entry]\nType=Application\nName=org.metis.viewer\n"));
    let agent = entry::launch_agent("org.metis.viewer", "/Apps/A&B.app/a", &arguments);
    assert!(agent.contains("<key>Label</key><string>org.metis.viewer</string>"));
    assert!(agent.contains("<string>/Apps/A&amp;B.app/a</string><string>--minimized</string>"));
    assert!(agent.contains("<key>RunAtLoad</key><true/>"));
}

#[cfg(not(windows))]
#[test]
fn login_items_are_validated() {
    let root = std::env::temp_dir();
    assert!(at(&root, "org.metis.viewer", &[]).is_ok());
    for name in ["", "Upper", ".hidden", "a/b", &"a".repeat(65)] {
        assert!(at(&root, name, &[]).is_err(), "{name:?}");
    }
    assert!(at(&root, "ok", &["line\nbreak"]).is_err());
    assert!(at(&root, "ok", &["x"; MAX_AUTOSTART_ARGUMENTS + 1]).is_err());
    assert!(Autostart::build("ok", std::path::Path::new("relative"), [""; 0], &root).is_err());
}

#[cfg(not(windows))]
#[test]
fn login_items_are_enabled_checked_and_disabled() {
    let root = std::env::temp_dir().join(format!("metis-autostart-{}", std::process::id()));
    let item = at(&root, "org.metis.viewer", &["--minimized"]).expect("item");
    assert!(!item.is_enabled().expect("absent"));
    assert!(!item.disable().expect("nothing to disable"));
    item.enable().expect("enable");
    assert!(item.is_enabled().expect("enabled"));
    let changed = at(&root, "org.metis.viewer", &["--other"]).expect("item");
    assert!(!changed.is_enabled().expect("different command"));
    assert!(item.disable().expect("disable"));
    assert!(!item.is_enabled().expect("disabled"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(windows)]
#[test]
fn login_items_round_trip_through_the_run_key() {
    let name = format!("org.metis.autostart-test-{}", std::process::id());
    let item =
        Autostart::new(&name, "C:\\Program Files\\Metis\\app.exe", ["--minimized"]).expect("item");
    assert!(!item.is_enabled().expect("absent"));
    item.enable().expect("enable");
    assert!(item.is_enabled().expect("enabled"));
    assert!(item.disable().expect("disable"));
    assert!(!item.is_enabled().expect("disabled"));
    assert!(Autostart::new("Bad Name", "C:\\app.exe", [""; 0]).is_err());
}
