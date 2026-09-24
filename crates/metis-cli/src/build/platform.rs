//! Host-native application packages that do not require a registry tool.

use super::FileRecord;
#[cfg(any(not(windows), test))]
mod url_schemes;
#[cfg(any(not(windows), test))]
use crate::{Result, manifest::Application};
#[cfg(any(not(windows), test))]
use moirai_crypto::Sha256;
use serde::Serialize;
#[cfg(any(not(windows), test))]
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[cfg(any(not(windows), test))]
const COPY_BUFFER_BYTES: usize = 16 * 1024;
#[cfg(any(not(windows), test))]
const TAR_BLOCK_BYTES: usize = 512;

/// The package formats produced by the host-native package command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(not(windows), test))]
enum Platform {
    MacOs,
    Linux,
}

/// Inventory for a non-Windows platform package.
#[derive(Serialize)]
pub(crate) struct PlatformPackageRecord {
    format: &'static str,
    file: String,
    bytes: u64,
    sha256: String,
    files: Vec<FileRecord>,
}

/// Builds the package format selected by the current host.
#[cfg(not(windows))]
pub(crate) fn build(
    application: &Application,
    output: &Path,
    entry: &str,
    staged: &[(PathBuf, String)],
) -> Result<PlatformPackageRecord> {
    let platform = match std::env::consts::OS {
        "macos" => Platform::MacOs,
        "linux" => Platform::Linux,
        _ => return Err("this installer format requires macOS or Linux".into()),
    };
    build_for(platform, application, output, entry, staged)
}

#[cfg(any(not(windows), test))]
fn build_for(
    platform: Platform,
    application: &Application,
    output: &Path,
    entry: &str,
    staged: &[(PathBuf, String)],
) -> Result<PlatformPackageRecord> {
    let binaries = application
        .binaries
        .iter()
        .map(|binary| format!("{}{}", binary.bin, std::env::consts::EXE_SUFFIX))
        .collect::<BTreeSet<_>>();
    match platform {
        Platform::MacOs => macos_bundle(application, output, &binaries, staged),
        Platform::Linux => linux_archive(application, output, entry, &binaries, staged),
    }
}

#[cfg(any(not(windows), test))]
fn macos_bundle(
    application: &Application,
    output: &Path,
    binaries: &BTreeSet<String>,
    staged: &[(PathBuf, String)],
) -> Result<PlatformPackageRecord> {
    let bundle_name = format!("{}.app", application.id);
    let bundle = output.join(&bundle_name);
    fs::create_dir(&bundle)?;
    let contents = bundle.join("Contents");
    let executable_directory = contents.join("MacOS");
    let resource_directory = contents.join("Resources");
    fs::create_dir(&contents)?;
    fs::create_dir(&executable_directory)?;
    fs::create_dir(&resource_directory)?;

    let mut records = Vec::new();
    let mut bytes = 0_u64;
    for (source, destination) in staged {
        let (directory, relative) = if binaries.contains(destination) {
            (
                &executable_directory,
                format!("Contents/MacOS/{destination}"),
            )
        } else {
            (
                &resource_directory,
                format!("Contents/Resources/{destination}"),
            )
        };
        let target = directory.join(destination);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        records.push(super::copy(source, &target, relative, &mut bytes)?);
    }
    let plist = info_plist(application, binaries, staged)?;
    let plist_record = write_bytes(
        &contents.join("Info.plist"),
        "Contents/Info.plist".to_owned(),
        plist.as_bytes(),
    )?;
    bytes = bytes
        .checked_add(plist_record.bytes)
        .ok_or("macOS bundle size overflow")?;
    records.push(plist_record);

    Ok(PlatformPackageRecord {
        format: "macos-app-bundle",
        file: bundle_name,
        bytes,
        sha256: digest_records(&records),
        files: records,
    })
}

#[cfg(any(not(windows), test))]
fn info_plist(
    application: &Application,
    binaries: &BTreeSet<String>,
    staged: &[(PathBuf, String)],
) -> Result<String> {
    let entry = format!("{}{}", application.entry, std::env::consts::EXE_SUFFIX);
    if !binaries.contains(&entry) || !staged.iter().any(|(_, name)| name == &entry) {
        return Err("macOS bundle entry is absent from its staged binaries".into());
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleDisplayName</key><string>{}</string><key>CFBundleExecutable</key><string>{}</string><key>CFBundleIdentifier</key><string>{}</string><key>CFBundleName</key><string>{}</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>{}</string><key>CFBundleVersion</key><string>{}</string>{}</dict></plist>\n",
        xml_escape(&application.name),
        xml_escape(&entry),
        xml_escape(&application.id),
        xml_escape(&application.name),
        xml_escape(&application.version),
        xml_escape(&application.version),
        url_schemes::plist_registration(application),
    ))
}

#[cfg(any(not(windows), test))]
fn linux_archive(
    application: &Application,
    output: &Path,
    entry: &str,
    binaries: &BTreeSet<String>,
    staged: &[(PathBuf, String)],
) -> Result<PlatformPackageRecord> {
    let archive_name = format!("{}.tar", application.id);
    let archive_path = output.join(&archive_name);
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&archive_path)?;
    let mut archive = TarWriter::new(file);
    let mut records = Vec::new();
    for (source, destination) in staged {
        let package_path = if binaries.contains(destination) {
            format!("usr/bin/{destination}")
        } else {
            format!("usr/share/{}/{destination}", application.id)
        };
        records.push(archive.add_file(
            source,
            &package_path,
            if binaries.contains(destination) {
                0o755
            } else {
                0o644
            },
        )?);
    }
    let desktop_path = format!("usr/share/applications/{}.desktop", application.id);
    let desktop = desktop_entry(application, entry);
    records.push(archive.add_bytes(&desktop_path, desktop.as_bytes(), 0o644)?);
    let (bytes, sha256) = archive.finish()?;
    Ok(PlatformPackageRecord {
        format: "linux-tar",
        file: archive_name,
        bytes,
        sha256,
        files: records,
    })
}

#[cfg(any(not(windows), test))]
fn desktop_entry(application: &Application, entry: &str) -> String {
    let icon = application.resources.iter().find_map(|resource| {
        let extension = Path::new(&resource.destination).extension()?;
        extension
            .eq_ignore_ascii_case("svg")
            .then(|| {
                desktop_icon(&format!(
                    "/usr/share/{}/{}",
                    application.id, resource.destination
                ))
            })
            .or_else(|| {
                extension.eq_ignore_ascii_case("png").then(|| {
                    desktop_icon(&format!(
                        "/usr/share/{}/{}",
                        application.id, resource.destination
                    ))
                })
            })
    });
    let mut output = format!(
        "[Desktop Entry]\nVersion=1.0\nType=Application\nName={}\nComment={} application\nExec=/usr/bin/{}",
        desktop_escape(&application.name),
        desktop_escape(&application.name),
        desktop_word(entry),
    );
    for argument in &application.arguments {
        output.push(' ');
        output.push_str(&desktop_word(argument));
    }
    let (link_field, mime_types) = url_schemes::desktop_registration(application);
    output.push_str(link_field);
    output.push_str("\nTerminal=false\n");
    output.push_str(&mime_types);
    if let Some(icon) = icon {
        output.push_str("Icon=");
        output.push_str(&icon);
        output.push('\n');
    }
    output.push_str("Categories=Utility;\n");
    output
}

#[cfg(any(not(windows), test))]
pub(crate) fn desktop_word(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.bytes().any(|byte| {
            byte.is_ascii_whitespace()
                || byte.is_ascii_control()
                || matches!(byte, b'"' | 96 | b'$' | b'\\' | b'%')
        });
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' | '"' | '\u{60}' | '$' => {
                escaped.push('\\');
                escaped.push(character);
            }
            '%' => escaped.push_str("%%"),
            _ => escaped.push(character),
        }
    }
    if needs_quotes {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

#[cfg(any(not(windows), test))]
pub(crate) fn desktop_icon(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            ' ' => escaped.push_str("\\s"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            ';' => escaped.push_str("\\;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(any(not(windows), test))]
fn desktop_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(any(not(windows), test))]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(any(not(windows), test))]
fn write_bytes(path: &Path, destination: String, bytes: &[u8]) -> Result<FileRecord> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(FileRecord {
        destination,
        bytes: bytes.len() as u64,
        sha256: digest(bytes),
    })
}

#[cfg(any(not(windows), test))]
fn digest_records(records: &[FileRecord]) -> String {
    let mut hash = Sha256::new();
    for record in records {
        hash.update(record.destination.as_bytes());
        hash.update(&record.bytes.to_le_bytes());
        hash.update(record.sha256.as_bytes());
    }
    hex(&hash.finalize())
}

#[cfg(any(not(windows), test))]
fn digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(bytes);
    hex(&hash.finalize())
}

#[cfg(any(not(windows), test))]
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("invariant: String formatting cannot fail");
    }
    output
}

#[cfg(any(not(windows), test))]
struct TarWriter {
    file: File,
    hash: Sha256,
    bytes: u64,
}

#[cfg(any(not(windows), test))]
impl TarWriter {
    fn new(file: File) -> Self {
        Self {
            file,
            hash: Sha256::new(),
            bytes: 0,
        }
    }

    fn add_file(&mut self, source: &Path, path: &str, mode: u32) -> Result<FileRecord> {
        let mut input = File::open(source)?;
        let size = input.metadata()?.len();
        let mut source_hash = Sha256::new();
        self.header(path, mode, size)?;
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        let mut copied = 0_u64;
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            copied = copied
                .checked_add(count as u64)
                .ok_or("tar source size overflow")?;
            source_hash.update(&buffer[..count]);
            self.write(&buffer[..count])?;
        }
        if copied != size {
            return Err("tar source changed during packaging".into());
        }
        self.padding(size)?;
        Ok(FileRecord {
            destination: path.to_owned(),
            bytes: size,
            sha256: hex(&source_hash.finalize()),
        })
    }

    fn add_bytes(&mut self, path: &str, bytes: &[u8], mode: u32) -> Result<FileRecord> {
        self.header(path, mode, bytes.len() as u64)?;
        self.write(bytes)?;
        self.padding(bytes.len() as u64)?;
        Ok(FileRecord {
            destination: path.to_owned(),
            bytes: bytes.len() as u64,
            sha256: digest(bytes),
        })
    }

    fn header(&mut self, path: &str, mode: u32, size: u64) -> Result<()> {
        let (name, prefix) = split_tar_path(path)?;
        let mut header = [0_u8; TAR_BLOCK_BYTES];
        field(&mut header[0..100], name.as_bytes())?;
        octal(&mut header[100..108], u64::from(mode))?;
        octal(&mut header[108..116], 0)?;
        octal(&mut header[116..124], 0)?;
        octal(&mut header[124..136], size)?;
        octal(&mut header[136..148], 0)?;
        header[148..156].fill(b' ');
        header[156] = b'0';
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        field(&mut header[265..297], b"root")?;
        field(&mut header[297..329], b"root")?;
        field(&mut header[345..500], prefix.as_bytes())?;
        let checksum = header.iter().map(|byte| u32::from(*byte)).sum::<u32>();
        let text = format!("{checksum:06o}");
        header[148..154].copy_from_slice(text.as_bytes());
        header[154] = 0;
        header[155] = b' ';
        self.write(&header)
    }

    fn padding(&mut self, size: u64) -> Result<()> {
        let remainder =
            usize::try_from(size % 512_u64).expect("invariant: tar remainder fits usize");
        if remainder == 0 {
            return Ok(());
        }
        self.write(&[0_u8; TAR_BLOCK_BYTES][..TAR_BLOCK_BYTES - remainder])
    }

    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.file.write_all(bytes)?;
        self.hash.update(bytes);
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or("tar size overflow")?;
        Ok(())
    }

    fn finish(mut self) -> Result<(u64, String)> {
        self.write(&[0_u8; TAR_BLOCK_BYTES * 2])?;
        self.file.sync_all()?;
        Ok((self.bytes, hex(&self.hash.finalize())))
    }
}

#[cfg(any(not(windows), test))]
fn split_tar_path(path: &str) -> Result<(&str, &str)> {
    if path.len() <= 100 {
        return Ok((path, ""));
    }
    let Some(index) = path.rfind('/') else {
        return Err("tar path exceeds the USTAR name field".into());
    };
    let (prefix, name) = path.split_at(index);
    let name = &name[1..];
    if prefix.len() > 155 || name.len() > 100 {
        return Err("tar path exceeds the USTAR prefix and name fields".into());
    }
    Ok((name, prefix))
}

#[cfg(any(not(windows), test))]
fn field(destination: &mut [u8], value: &[u8]) -> Result<()> {
    if value.len() > destination.len() {
        return Err("tar header field exceeds its bound".into());
    }
    destination[..value.len()].copy_from_slice(value);
    Ok(())
}

#[cfg(any(not(windows), test))]
fn octal(destination: &mut [u8], value: u64) -> Result<()> {
    let width = destination.len().checked_sub(1).ok_or("empty tar field")?;
    let text = format!("{value:0width$o}");
    if text.len() > width {
        return Err("tar numeric field exceeds its bound".into());
    }
    destination[..text.len()].copy_from_slice(text.as_bytes());
    destination[width] = 0;
    Ok(())
}

#[cfg(test)]
#[path = "platform_tests.rs"]
mod tests;
