//! Cabinet input validation and bounded, shell-free invocation of the OS tool.
use super::InstallerSpec;
use crate::manifest::{self, FILE_LIMIT, ICON_LIMIT, PAYLOAD_LIMIT};
use std::{
    collections::BTreeSet, error::Error, ffi::OsString, fmt::Write as _, path::Path, time::Duration,
};

pub(super) fn validate(spec: &InstallerSpec<'_>, output: &Path) -> Result<(), Box<dyn Error>> {
    if !cfg!(target_arch = "x86_64") {
        return Err("MSI packaging currently requires an x86-64 Windows build host".into());
    }
    if !output.is_absolute() || output.try_exists()? {
        return Err("MSI output must be an absent absolute file path".into());
    }
    if spec.files.is_empty() || spec.files.len() > FILE_LIMIT {
        return Err("MSI requires 1..=4096 payload files".into());
    }
    for (label, text, limit) in [
        ("identity", spec.id, 100),
        ("display name", spec.name, 100),
        ("manufacturer", spec.manufacturer, 100),
    ] {
        if text.is_empty() || text.len() > limit || text.chars().any(char::is_control) {
            return Err(
                format!("MSI {label} must contain 1..={limit} bytes without controls").into(),
            );
        }
    }
    manifest::relative(spec.id)?;
    manifest::relative(spec.name)?;
    if spec.name.contains('/') {
        return Err("MSI display name must be one filename component".into());
    }
    if !spec
        .id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b".-_".contains(&byte))
    {
        return Err(
            "MSI identity must use ASCII letters, digits, dot, underscore or hyphen".into(),
        );
    }
    let version = spec
        .version
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;
    if !matches!(version.as_slice(), [major, minor, build] if *major <= 255 && *minor <= 255 && *build <= 65535)
    {
        return Err("MSI version must have three fields bounded by 255.255.65535".into());
    }
    if !valid_guid(spec.upgrade_code) {
        return Err("MSI upgrade code must be an uppercase braced GUID".into());
    }
    if let Some(icon) = spec.icon {
        if !icon.is_absolute() {
            return Err("MSI icon source must be an absolute path".into());
        }
        let metadata = icon.symlink_metadata()?;
        if !metadata.is_file() || manifest::linked(&metadata) {
            return Err("MSI icon source must be a regular file, not a reparse point".into());
        }
        if metadata.len() > ICON_LIMIT {
            return Err("MSI icon exceeds the 1 MiB budget".into());
        }
        manifest::validate_icon_file(icon)?;
    }
    let mut destinations = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut bytes = 0_u64;
    for (source, destination) in spec.files {
        if !source.is_absolute() {
            return Err("MSI source must be an absolute path".into());
        }
        let metadata = source.symlink_metadata()?;
        if !metadata.is_file() || manifest::linked(&metadata) {
            return Err("MSI source must be a regular file, not a reparse point".into());
        }
        bytes = bytes
            .checked_add(metadata.len())
            .ok_or("MSI payload size overflow")?;
        if bytes > PAYLOAD_LIMIT {
            return Err("MSI payload exceeds the 1 GiB cabinet budget".into());
        }
        manifest::relative(destination)?;
        let mut partial = String::new();
        for component in destination.split('/') {
            if !partial.is_empty() {
                directories.insert(partial.clone());
                partial.push('/');
            }
            partial.push_str(&component.to_lowercase());
        }
        if !destinations.insert(partial) {
            return Err("MSI destination is duplicated".into());
        }
    }
    if destinations.iter().any(|name| directories.contains(name)) {
        return Err("MSI destination is both a file and directory".into());
    }
    if !spec
        .files
        .iter()
        .any(|(_, destination)| destination == spec.entry)
    {
        return Err("MSI entry executable is absent from its payload".into());
    }
    Ok(())
}

fn valid_guid(text: &str) -> bool {
    text.len() == 38
        && text.bytes().enumerate().all(|(index, byte)| match index {
            0 => byte == b'{',
            37 => byte == b'}',
            9 | 14 | 19 | 24 => byte == b'-',
            _ => byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte),
        })
}

fn directive_path(path: &Path) -> Result<String, Box<dyn Error>> {
    let converted = super::paths::legacy(path)?;
    let value = converted
        .to_str()
        .ok_or("Cabinet source path must be Unicode")?;
    // makecab parses legacy directive files; until a verified encoding contract
    // exists, reject paths that cannot be represented without interpretation.
    if !value.is_ascii()
        || value
            .chars()
            .any(|character| character.is_control() || "\"%".contains(character))
    {
        return Err(
            "Cabinet source paths must be ASCII without quote, percent or control characters"
                .into(),
        );
    }
    Ok(value.to_owned())
}

pub(super) fn cabinet(
    spec: &InstallerSpec<'_>,
    staging: &Path,
) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let directory = directive_path(staging)?;
    let mut directives = format!(
        ".OPTION EXPLICIT\r\n.Set Cabinet=on\r\n.Set Compress=on\r\n.Set CompressionType=MSZIP\r\n.Set CabinetNameTemplate=payload.cab\r\n.Set DiskDirectoryTemplate=\"{directory}\"\r\n.Set MaxDiskSize=0\r\n.Set InfFileName=\"{directory}\\payload.inf\"\r\n.Set RptFileName=\"{directory}\\payload.rpt\"\r\n"
    );
    for (index, (source, _)) in spec.files.iter().enumerate() {
        writeln!(directives, "\"{}\" F{index}\r", directive_path(source)?)?;
    }
    let directive_file = staging.join("payload.ddf");
    std::fs::write(&directive_file, directives)?;
    let mut system_directory = vec![0_u16; 32768];
    // SAFETY: Buffer is writable for the supplied UTF-16 capacity; the OS fills
    // its own system directory, independent of PATH/SystemRoot environment input.
    let length = unsafe {
        super::ffi::GetSystemDirectoryW(
            system_directory.as_mut_ptr(),
            u32::try_from(system_directory.len())?,
        )
    };
    if length == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let directory = system_directory
        .get(..usize::try_from(length)?)
        .ok_or("Windows system directory exceeds buffer")?;
    let program = std::path::PathBuf::from(String::from_utf16(directory)?).join("makecab.exe");
    crate::process::run(
        &program,
        &[
            OsString::from("/F"),
            super::paths::legacy(&directive_file)?.into_os_string(),
        ],
        Duration::from_mins(1),
    )?;
    let cabinet = staging.join("payload.cab");
    let metadata = cabinet.symlink_metadata()?;
    if !metadata.is_file()
        || manifest::linked(&metadata)
        || metadata.len() > PAYLOAD_LIMIT + (1 << 20)
    {
        return Err("Cabinet output violates the regular-file or size budget".into());
    }
    Ok(cabinet)
}

pub(super) fn cleanup(staging: &Path) -> Result<(), Box<dyn Error>> {
    for name in [
        "payload.ddf",
        "payload.cab",
        "payload.inf",
        "payload.rpt",
        "package.msi",
        "codepage.idt",
    ] {
        match std::fs::remove_file(staging.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    std::fs::remove_dir(staging)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::valid_guid;

    #[test]
    fn pins_upgrade_identity_grammar() {
        assert!(valid_guid("{01234567-89AB-CDEF-0123-456789ABCDEF}"));
        for guid in [
            "01234567-89AB-CDEF-0123-456789ABCDEF",
            "{01234567-89ab-CDEF-0123-456789ABCDEF}",
            "{01234567-89AB-CDEF-0123-456789ABCDEG}",
        ] {
            assert!(!valid_guid(guid));
        }
    }
}
