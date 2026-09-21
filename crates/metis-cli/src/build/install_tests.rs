use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

const BLOCK_BYTES: usize = 512;
static TEST_ID: AtomicUsize = AtomicUsize::new(0);

#[test]
fn install_and_remove_preserve_user_files() {
    let root = temp_root();
    let prefix = root.join("user prefix");
    fs::create_dir(&prefix).expect("prefix");
    fs::write(prefix.join("sentinel.txt"), b"keep").expect("sentinel");
    let archive = root.join("package.tar");
    fs::write(&archive, fixture_archive()).expect("archive");

    if cfg!(target_os = "linux") {
        run(&archive, &prefix).expect("install");
    } else {
        install_archive(&archive, &prefix).expect("install");
    }
    let executable = prefix.join("bin/metis-app");
    assert_eq!(fs::read(&executable).expect("executable"), b"executable");
    let installed_prefix = prefix.canonicalize().expect("canonical prefix");
    let installed_executable = installed_prefix.join("bin/metis-app");
    let desktop = fs::read_to_string(prefix.join("share/applications/org.metis.demo.desktop"))
        .expect("desktop");
    assert!(desktop.contains(&format!(
        "Exec={}",
        desktop_word(&installed_executable.to_string_lossy())
    )));
    let installed_icon = installed_prefix
        .join("share")
        .join("org.metis.demo")
        .join("icon.svg");
    assert!(desktop.contains(&format!(
        "Icon={}",
        desktop_icon(&installed_icon.to_string_lossy())
    )));
    fs::write(prefix.join("user-note.txt"), b"user").expect("user note");
    if cfg!(target_os = "linux") {
        uninstall("org.metis.demo", &prefix).expect("remove");
    } else {
        remove_installation("org.metis.demo", &installed_prefix).expect("remove");
    }
    assert!(prefix.join("sentinel.txt").is_file());
    assert!(prefix.join("user-note.txt").is_file());
    assert!(!executable.exists());
    assert!(
        !prefix
            .join("share/metis/org.metis.demo/installation.json")
            .exists()
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn install_rejects_existing_payload_without_partial_files() {
    let root = temp_root();
    let prefix = root.join("prefix");
    fs::create_dir(&prefix).expect("prefix");
    fs::create_dir_all(prefix.join("bin")).expect("bin");
    fs::write(prefix.join("bin/metis-app"), b"existing").expect("existing");
    let archive = root.join("package.tar");
    fs::write(&archive, fixture_archive()).expect("archive");
    let result = if cfg!(target_os = "linux") {
        run(&archive, &prefix)
    } else {
        install_archive(&archive, &prefix)
    };
    assert!(result.is_err());
    assert!(
        !prefix
            .join("share/applications/org.metis.demo.desktop")
            .exists()
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn uninstall_refuses_modified_package_files() {
    let root = temp_root();
    let prefix = root.join("prefix");
    fs::create_dir(&prefix).expect("prefix");
    let archive = root.join("package.tar");
    fs::write(&archive, fixture_archive()).expect("archive");
    if cfg!(target_os = "linux") {
        run(&archive, &prefix).expect("install");
    } else {
        install_archive(&archive, &prefix).expect("install");
    }
    fs::write(prefix.join("bin/metis-app"), b"changed").expect("modify");
    let installed_prefix = prefix.canonicalize().expect("canonical prefix");
    let result = if cfg!(target_os = "linux") {
        uninstall("org.metis.demo", &prefix)
    } else {
        remove_installation("org.metis.demo", &installed_prefix)
    };
    let error = result.expect_err("modified package must refuse removal");
    assert!(error.to_string().contains("user-modified"));
    assert!(
        installed_prefix
            .join("share/metis/org.metis.demo/installation.json")
            .is_file()
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn install_rejects_traversal_before_writing() {
    let root = temp_root();
    let prefix = root.join("prefix");
    fs::create_dir(&prefix).expect("prefix");
    let archive = root.join("package.tar");
    let mut bytes = Vec::new();
    tar_entry(&mut bytes, "usr/bin/../escape", 0o755, b"escape");
    bytes.extend([0_u8; BLOCK_BYTES * 2]);
    fs::write(&archive, bytes).expect("archive");
    let result = if cfg!(target_os = "linux") {
        run(&archive, &prefix)
    } else {
        install_archive(&archive, &prefix)
    };
    let error = result.expect_err("traversal entry must be rejected");
    assert!(
        error
            .to_string()
            .contains("payload path contains a reserved or ambiguous component")
    );
    assert!(!root.join("escape").exists());
    assert!(
        fs::read_dir(&prefix)
            .expect("prefix entries")
            .next()
            .is_none()
    );
    fs::remove_dir_all(root).expect("cleanup");
}

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "metis-install-{}-{}",
        std::process::id(),
        TEST_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).expect("root");
    root
}

fn fixture_archive() -> Vec<u8> {
    let mut bytes = Vec::new();
    tar_entry(&mut bytes, "usr/bin/metis-app", 0o755, b"executable");
    tar_entry(
        &mut bytes,
        "usr/share/org.metis.demo/manual.txt",
        0o644,
        b"resource",
    );
    tar_entry(
        &mut bytes,
        "usr/share/org.metis.demo/icon.svg",
        0o644,
        b"<svg></svg>",
    );
    tar_entry(
        &mut bytes,
        "usr/share/applications/org.metis.demo.desktop",
        0o644,
        b"[Desktop Entry]\nType=Application\nName=Demo\nExec=/usr/bin/metis-app \"%f\"\nIcon=/usr/share/org.metis.demo/icon.svg\nTerminal=false\nCategories=Utility;\n",
    );
    bytes.extend([0_u8; BLOCK_BYTES * 2]);
    bytes
}

fn tar_entry(output: &mut Vec<u8>, path: &str, mode: u32, payload: &[u8]) {
    let mut header = [0_u8; BLOCK_BYTES];
    header[..path.len()].copy_from_slice(path.as_bytes());
    let mode = format!("{mode:07o}");
    header[100..107].copy_from_slice(mode.as_bytes());
    let size = format!("{:011o}", payload.len());
    header[124..135].copy_from_slice(size.as_bytes());
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    let checksum = header.iter().map(|byte| u32::from(*byte)).sum::<u32>();
    let checksum = format!("{checksum:06o}");
    header[148..154].copy_from_slice(checksum.as_bytes());
    header[154] = 0;
    header[155] = b' ';
    output.extend(header);
    output.extend(payload);
    let padding = (BLOCK_BYTES - (payload.len() % BLOCK_BYTES)) % BLOCK_BYTES;
    output.extend(std::iter::repeat_n(0_u8, padding));
}
