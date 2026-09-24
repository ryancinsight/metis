use super::WindowStateFile;
use metis_core::window_state::{MAX_WINDOW_STATE_BYTES, WindowState};
use std::{fs, io, path::PathBuf};

fn scratch(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("metis-window-state-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    root
}

#[test]
fn paths_must_be_absolute_files() {
    assert!(WindowStateFile::new("relative/window.state").is_err());
    assert!(WindowStateFile::new(std::env::temp_dir().join("..")).is_err());
}

#[test]
fn a_missing_file_is_no_saved_state() {
    let root = scratch("missing");
    let file = WindowStateFile::new(root.join("window.state")).expect("store");
    assert_eq!(file.load().expect("load"), None);
}

#[test]
fn saves_replace_the_previous_state() {
    let root = scratch("replace");
    let file = WindowStateFile::new(root.join("nested").join("window.state")).expect("store");
    let first = WindowState::new(10, 20, 800, 600, false).expect("state");
    let second = WindowState::new(-1_280, 0, 1_024, 768, true).expect("state");
    file.save(&first).expect("first save");
    assert_eq!(file.load().expect("load"), Some(first));
    file.save(&second).expect("second save");
    assert_eq!(file.load().expect("load"), Some(second));
    let entries = fs::read_dir(root.join("nested"))
        .expect("directory")
        .count();
    assert_eq!(entries, 1, "the staging file must not remain");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn damaged_or_oversized_files_are_invalid_data() {
    let root = scratch("damaged");
    fs::create_dir_all(&root).expect("directory");
    let file = WindowStateFile::new(root.join("window.state")).expect("store");
    for contents in [
        b"metis-window-state 1\nleft=1\n".to_vec(),
        vec![b' '; MAX_WINDOW_STATE_BYTES + 1],
        vec![0xff, 0xfe],
    ] {
        fs::write(file.path(), contents).expect("write");
        let error = file.load().expect_err("damaged state");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
    fs::remove_dir_all(root).expect("cleanup");
}
