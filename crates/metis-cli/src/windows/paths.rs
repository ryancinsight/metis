//! Legacy installer tools accept DOS/UNC paths rather than Rust verbatim paths.
//!
//! [Windows namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)
//! differ in normalization. Only known drive/UNC prefixes and ordinary components
//! are converted; opaque device paths and normalization-sensitive names reject.
use std::{
    error::Error,
    ffi::OsString,
    os::windows::ffi::OsStrExt,
    path::{Component, Path, PathBuf, Prefix},
};

const MAX_PATH_UNITS: usize = 260;

pub(super) fn legacy(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err("Installer tool path must be a fully qualified DOS or UNC path".into());
    };
    let mut converted = match prefix.kind() {
        Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => {
            PathBuf::from(format!("{}:\\", char::from(drive)))
        }
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
            let mut root = OsString::from(r"\\");
            root.push(server);
            root.push(r"\");
            root.push(share);
            root.push(r"\");
            PathBuf::from(root)
        }
        _ => return Err("Installer tools do not support device or opaque verbatim paths".into()),
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err("Installer tool path must have an absolute root".into());
    }
    for component in components {
        let Component::Normal(name) = component else {
            return Err("Installer tool paths cannot contain relative components".into());
        };
        let text = name
            .to_str()
            .ok_or("Installer tool paths require Unicode components")?;
        if text.ends_with(['.', ' ']) || text.contains('/') || text.chars().any(char::is_control) {
            return Err(
                "Installer tool path changes meaning outside the verbatim namespace".into(),
            );
        }
        converted.push(name);
    }
    // MAX_PATH includes the terminating NUL. No long-path policy is changed and
    // no 8.3 alias is assumed to exist on the source volume.
    if converted.as_os_str().encode_wide().count() >= MAX_PATH_UNITS {
        return Err("Installer tool path exceeds the 259 UTF-16 character legacy limit".into());
    }
    Ok(converted)
}

#[cfg(test)]
mod tests {
    use super::legacy;
    use std::path::{Path, PathBuf};

    #[test]
    fn converts_only_known_equivalent_namespaces() {
        for (source, expected) in [
            (r"\\?\D:\build\app.exe", r"D:\build\app.exe"),
            (r"\\?\UNC\server\share\app.exe", r"\\server\share\app.exe"),
            (r"D:\build\app.exe", r"D:\build\app.exe"),
        ] {
            assert_eq!(
                legacy(Path::new(source)).expect("equivalent DOS/UNC path"),
                PathBuf::from(expected)
            );
        }
        assert_eq!(
            legacy(Path::new(r"\\?\GLOBALROOT\Device\HarddiskVolume1\app.exe"))
                .expect_err("opaque namespace")
                .to_string(),
            "Installer tools do not support device or opaque verbatim paths"
        );
        assert_eq!(
            legacy(Path::new(r"\\?\D:\build\..\app.exe"))
                .expect_err("relative semantics")
                .to_string(),
            "Installer tool paths cannot contain relative components"
        );
        assert_eq!(
            legacy(Path::new(r"\\?\D:\build\name.\app.exe"))
                .expect_err("trailing dot semantics")
                .to_string(),
            "Installer tool path changes meaning outside the verbatim namespace"
        );
    }

    #[test]
    fn rejects_long_paths_instead_of_truncating() {
        assert_eq!(
            legacy(Path::new(&format!("D:\\{}", "x".repeat(256))))
                .expect("259 units")
                .as_os_str()
                .len(),
            259
        );
        assert_eq!(
            legacy(Path::new(&format!("D:\\{}", "x".repeat(257))))
                .expect_err("260 units")
                .to_string(),
            "Installer tool path exceeds the 259 UTF-16 character legacy limit"
        );
    }
}
