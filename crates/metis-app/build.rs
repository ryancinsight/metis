//! The native build embeds independently verified prepared bytes without a
//! recursive Cargo compilation. Admission runs bounded tool identity/version
//! queries, including `cargo --version`, but never Cargo build or metadata.
use metis_core::embedded_assets::{
    AssetKind, CargoLocations, FileIdentity, NativeContext, PAYLOAD_LIMIT, PreparedModule,
    canonical_directory, linked,
};
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-env-changed=METIS_WEBVIEW_ASSET_INVENTORY");
    println!("cargo:rustc-check-cfg=cfg(metis_embedded_webview)");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        // Other native targets have their existing shell and own no WebView2
        // page module. No empty generated assets or synthetic include is emitted.
        return Ok(());
    }
    let inventory = env::var_os("METIS_WEBVIEW_ASSET_INVENTORY").map(PathBuf::from).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Windows WebView2 assets require `metis prepare metis.json` and METIS_WEBVIEW_ASSET_INVENTORY, or `metis build/package/dev`"))?;
    if !inventory.is_absolute() {
        return Err(invalid(
            "prepared inventory environment path must be absolute",
        ));
    }
    let manifest = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| invalid("Cargo omitted the application manifest directory"))?,
    );
    let workspace = manifest
        .ancestors()
        .find(|directory| directory.join("Cargo.lock").is_file())
        .ok_or_else(|| invalid("application source has no owning workspace Cargo.lock"))?;
    let locations = CargoLocations::current(workspace)?;
    let prepared =
        PreparedModule::open(&inventory, workspace, locations.cargo_home(), NativeContext)?;
    if prepared.inventory().spec().module.package != "metis-web"
        || prepared.inventory().spec().module.target != "metis_web"
    {
        return Err(invalid("prepared module is not the Metis page owner"));
    }
    for path in prepared.rerun_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    for (key, _) in &prepared.inventory().spec().configuration.environment {
        println!("cargo:rerun-if-env-changed={key}");
    }
    for key in [
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET_DIR",
        "RUSTC",
        "CARGO",
        "WASM_BINDGEN",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let output = canonical_directory(&PathBuf::from(
        env::var_os("OUT_DIR").ok_or_else(|| invalid("Cargo omitted application OUT_DIR"))?,
    ))?;
    let mut js = None;
    let mut wasm = None;
    for asset in prepared.assets() {
        let name = match asset.record().kind {
            AssetKind::JavaScript => "metis_web.js",
            AssetKind::WebAssembly => "metis_web_bg.wasm",
        };
        if asset.record().destination != name {
            return Err(invalid(
                "prepared generated asset filename differs from the native package contract",
            ));
        }
        let path = output.join(name);
        publish_owned(&path, asset.bytes())?;
        match asset.record().kind {
            AssetKind::JavaScript => js = Some(path),
            AssetKind::WebAssembly => wasm = Some(path),
        }
    }
    let inventory_path = output.join("metis_embedded_inventory.bin");
    publish_owned(&inventory_path, prepared.inventory_bytes())?;
    let source = format!(
        "pub(crate) const EMBEDDED_MODULE_JS: &[u8] = include_bytes!({:?});\npub(crate) const EMBEDDED_MODULE_WASM: &[u8] = include_bytes!({:?});\npub(crate) const EMBEDDED_INVENTORY: &[u8] = include_bytes!({:?});\npub(crate) const EMBEDDED_INVENTORY_DIGEST: [u8; 32] = {:?};\n",
        js.ok_or_else(|| invalid("verified JavaScript asset is missing"))?
            .to_str()
            .ok_or_else(|| invalid("owned JavaScript path is not UTF-8"))?,
        wasm.ok_or_else(|| invalid("verified WASM asset is missing"))?
            .to_str()
            .ok_or_else(|| invalid("owned WASM path is not UTF-8"))?,
        inventory_path
            .to_str()
            .ok_or_else(|| invalid("owned inventory path is not UTF-8"))?,
        prepared.inventory_digest().into_bytes()
    );
    publish_owned(&output.join("metis_embedded_assets.rs"), source.as_bytes())?;
    println!("cargo:rustc-cfg=metis_embedded_webview");
    Ok(())
}
fn publish_owned(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || linked(&metadata) => {
            return Err(invalid("owned embedded output is linked or nonregular"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let directory = path
        .parent()
        .ok_or_else(|| invalid("embedded output has no parent"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("embedded output name is not UTF-8"))?;
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    for attempt in 0..8 {
        let temporary = directory.join(format!(".{name}-{}-{epoch}-{attempt}", std::process::id()));
        let mut file = match fs::File::options()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)?;
            let expected = FileIdentity::from_bytes(bytes)?;
            if FileIdentity::read(path, PAYLOAD_LIMIT)? != expected {
                return Err(invalid("owned embedded output differs from verified bytes"));
            }
            Ok(())
        })();
        if let Err(primary) = &result {
            match temporary.try_exists().and_then(|exists| {
                if exists {
                    fs::remove_file(&temporary)
                } else {
                    Ok(())
                }
            }) {
                Ok(()) => {}
                Err(cleanup) => {
                    return Err(io::Error::other(format!(
                        "{primary}; owned output cleanup failed: {cleanup}"
                    )));
                }
            }
        }
        return result;
    }
    Err(invalid(
        "cannot acquire owned embedded output after eight attempts",
    ))
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
