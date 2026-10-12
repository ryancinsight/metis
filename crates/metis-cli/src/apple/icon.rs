//! Snapshot and decode declared macOS shell artwork before replacing output.

use super::Target;
use crate::{
    Result, bounded_read,
    manifest::{self, WebApplication},
    process,
};
use std::collections::BTreeSet;
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) const FILE_NAME: &str = "application.icns";

/// An absent icon retains the OS default. A declared icon must decode; it never
/// falls back to a framework mark or a generic image on validation failure.
pub(super) fn read(
    application: &WebApplication,
    root: &Path,
    target: Target,
) -> Result<Option<Vec<u8>>> {
    if target != Target::Desktop {
        return Ok(None);
    }
    let Some(relative) = &application.icon else {
        return Ok(None);
    };
    let source = manifest::source(root, relative)?;
    let bytes = bounded_read::read_file(
        &source,
        manifest::ICON_LIMIT,
        "application icon exceeds the 1 MiB budget",
    )?;
    let expected = representations(&bytes)?;
    // Decode the immutable snapshot with Apple's real codec. The directory is
    // exclusively created and removed on both successful and failed decoding.
    let directory = std::env::temp_dir().canonicalize()?.join(format!(
        "metis-icon-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&directory)?;
    let decoded: Result<()> = (|| -> Result<()> {
        let snapshot = directory.join(FILE_NAME);
        let decoded = directory.join("Decoded.iconset");
        fs::write(&snapshot, &bytes)?;
        process::run(
            &process::tool("iconutil")?,
            &["-c".into(), "iconset".into(), "-o".into(), decoded.as_os_str().to_owned(), snapshot.into_os_string()],
            Duration::from_secs(30),
        )?;
        // iconutil can exit successfully after discarding malformed images.
        // Every supplied representation must survive the actual decode.
        let actual = crate::tree::regular_files(&decoded, |_| false)?.into_iter()
            .map(|path| crate::tree::relative_name(&decoded, &path))
            .collect::<Result<BTreeSet<_>>>()?;
        if actual != expected {
            return Err(format!("decoded representations differ from ICNS entries: expected {expected:?}, received {actual:?}").into());
        }
        Ok(())
    })().map_err(|error| { format!("cannot decode application icon {}: {error}", source.display()).into() });
    let cleanup = fs::remove_dir_all(&directory);
    match (decoded, cleanup) {
        (Ok(()), Ok(())) => Ok(Some(bytes)),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!("icon validation cleanup failed: {error}").into()),
        (Err(error), Err(cleanup)) => {
            Err(format!("{error}; icon validation cleanup failed: {cleanup}").into())
        }
    }
}

/// Apple point/scale entries must each have a distinct decoded filename.
fn representations(bytes: &[u8]) -> Result<BTreeSet<String>> {
    if bytes.get(..4) != Some(b"icns")
        || bytes
            .get(4..8)
            .and_then(|length| length.try_into().ok())
            .map(u32::from_be_bytes)
            != Some(u32::try_from(bytes.len())?)
        || bytes.len() <= 8
    {
        return Err("application icon has an invalid ICNS container".into());
    }
    let mut images = BTreeSet::new();
    let mut offset = 8;
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset + 8)
            .ok_or("ICNS entry header is truncated")?;
        let size = usize::try_from(u32::from_be_bytes(header[4..8].try_into()?))?;
        if size < 8 || size > bytes.len() - offset {
            return Err("ICNS entry lies outside the container".into());
        }
        let name = match &header[..4] {
            b"ic04" | b"icp4" => Some("icon_16x16.png"),
            b"ic11" => Some("icon_16x16@2x.png"),
            b"ic05" | b"icp5" => Some("icon_32x32.png"),
            b"ic12" => Some("icon_32x32@2x.png"),
            b"ic07" => Some("icon_128x128.png"),
            b"ic13" => Some("icon_128x128@2x.png"),
            b"ic08" => Some("icon_256x256.png"),
            b"ic14" => Some("icon_256x256@2x.png"),
            b"ic09" => Some("icon_512x512.png"),
            b"ic10" => Some("icon_512x512@2x.png"),
            b"info" | b"TOC " => None,
            _ => return Err("ICNS entry is outside the supported Apple point/scale set".into()),
        };
        if let Some(name) = name
            && !images.insert(name.to_owned())
        {
            return Err("ICNS contains duplicate point/scale representations".into());
        }
        offset += size;
    }
    if images.is_empty() {
        return Err("ICNS contains no image representations".into());
    }
    Ok(images)
}
