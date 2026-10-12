//! Apple application compilation, inventory and simulator launch.

mod bundle;
mod icon;
mod target;

use crate::{
    Result, build,
    manifest::{DEFAULT_MANIFEST, Manifest, WebApplication},
    process,
};
#[cfg(target_os = "macos")]
pub(crate) use bundle::program;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};
pub(crate) use target::Target;

struct Options {
    run: bool,
    manifest: PathBuf,
    output: Option<PathBuf>,
    target: Target,
    device: Option<OsString>,
}

pub(crate) fn run(args: &[OsString]) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err("Apple builds require macOS and the matching Xcode SDK".into());
    }
    let options = arguments(args)?;
    let (Manifest::Web(application), root) = Manifest::read(&options.manifest)? else {
        return Err("Apple WebView packaging requires a frontend manifest".into());
    };
    let served = build::web::application(&application, &root, None)?;
    let executable = executable(options.target)?;
    let output = options
        .output
        .unwrap_or_else(|| root.join("output/apple").join(options.target.label()));
    let bundle = package(
        &application,
        &root,
        &served,
        &executable,
        &output,
        options.target,
    )?;
    println!("{}", bundle.display());
    if options.run {
        launch(
            &bundle,
            &application,
            options.target,
            options.device.as_deref(),
        )?;
    }
    Ok(())
}

/// One signed bundle path for development and installed application launches.
pub(crate) fn package(
    application: &WebApplication,
    root: &Path,
    served: &Path,
    executable: &Path,
    output: &Path,
    target: Target,
) -> Result<PathBuf> {
    let bundle = bundle::create(application, root, served, executable, output, target)?;
    if target != Target::Device {
        process::run(
            &process::tool("codesign")?,
            &[
                "--force".into(),
                "--sign".into(),
                "-".into(),
                bundle.as_os_str().to_owned(),
            ],
            Duration::from_secs(30),
        )?;
    }
    bundle::record(application, &bundle, target)?;
    Ok(bundle)
}

/// The installed CLI and its application entry belong to one Cargo workspace.
/// Select the executable from compiler messages instead of guessing a cache path.
pub(crate) fn executable(target: Target) -> Result<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    if !manifest.is_file() {
        return Err("Apple host compilation requires the Metis source workspace".into());
    }
    let sdk = process::capture(
        &process::tool("xcrun")?,
        &[
            "--sdk".into(),
            target.sdk().into(),
            "--show-sdk-path".into(),
        ],
        Duration::from_secs(30),
        &[],
    )?;
    let sdk = String::from_utf8(sdk)?;
    let sdk = Path::new(sdk.trim());
    if !sdk.is_dir() {
        return Err("Xcode SDK is not a directory".into());
    }
    let (deployment, version) = target.deployment();
    let environment = [
        ("SDKROOT", sdk.as_os_str().to_owned()),
        (deployment, version.into()),
    ];
    let messages = process::capture(
        &process::tool("cargo")?,
        &[
            "build".into(),
            "--locked".into(),
            "--release".into(),
            "--manifest-path".into(),
            manifest.into_os_string(),
            "--target".into(),
            target.triple()?.into(),
            "--package".into(),
            "metis-host".into(),
            "--bin".into(),
            "metis-host".into(),
            "--message-format=json-render-diagnostics".into(),
        ],
        Duration::from_mins(15),
        &environment,
    )?;
    let mut found = None;
    for line in messages
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let message: serde_json::Value = serde_json::from_slice(line)?;
        if message["reason"] == "compiler-artifact"
            && message["target"]["name"] == "metis-host"
            && message["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
        {
            let path = message["executable"]
                .as_str()
                .ok_or("Cargo omitted the application executable")?;
            if !Path::new(path).is_absolute() || found.replace(PathBuf::from(path)).is_some() {
                return Err(
                    "Cargo reported an ambiguous or relative application executable".into(),
                );
            }
        }
    }
    found.ok_or_else(|| "Cargo did not produce the Apple application executable".into())
}

fn launch(
    bundle: &Path,
    application: &WebApplication,
    target: Target,
    device: Option<&std::ffi::OsStr>,
) -> Result<()> {
    match target {
        Target::Desktop => process::run(&process::tool("open")?, &[bundle.as_os_str().to_owned()], Duration::from_secs(30)),
        Target::Device => Err("device installation requires the owner's Apple signing and provisioning; use apple build --target ios".into()),
        Target::Simulator => {
            let device = device.unwrap_or_else(|| std::ffi::OsStr::new("booted"));
            let xcrun = process::tool("xcrun")?;
            let install = [OsString::from("simctl"), "install".into(), device.to_owned(), bundle.as_os_str().to_owned()];
            process::run(&xcrun, &install, Duration::from_mins(1))?;
            process::run(&xcrun, &["simctl".into(), "launch".into(), device.to_owned(), bundle::identifier(application).into()], Duration::from_mins(1))
        }
    }
}

fn arguments(args: &[OsString]) -> Result<Options> {
    let mut rest = args.iter();
    let run = match rest.next().and_then(|value| value.to_str()) {
        Some("build") => false,
        Some("run") => true,
        _ => return Err("apple requires build or run".into()),
    };
    let mut manifest = None;
    let mut target = None;
    let mut output = None;
    let mut device = None;
    while let Some(argument) = rest.next() {
        match argument.to_str() {
            Some("--target") => {
                let value = rest
                    .next()
                    .and_then(|value| value.to_str())
                    .ok_or("--target requires macos, ios or ios-simulator")?;
                if target.replace(Target::parse(value)?).is_some() {
                    return Err("apple accepts one --target".into());
                }
            }
            Some("--output") => {
                let value = rest.next().ok_or("--output requires a directory")?;
                if output.replace(PathBuf::from(value)).is_some() {
                    return Err("apple accepts one --output".into());
                }
            }
            Some("--device") => {
                let value = rest
                    .next()
                    .ok_or("--device requires a simulator identity")?;
                if device.replace(value.clone()).is_some() {
                    return Err("apple accepts one --device".into());
                }
            }
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unknown apple option {flag}").into());
            }
            _ => {
                if manifest.replace(PathBuf::from(argument)).is_some() {
                    return Err("apple accepts one MANIFEST".into());
                }
            }
        }
    }
    let target = target.unwrap_or(Target::Desktop);
    if device.is_some() && target != Target::Simulator {
        return Err("--device requires --target ios-simulator".into());
    }
    if run && target == Target::Device {
        return Err(
            "device installation requires Apple provisioning; build the ios bundle first".into(),
        );
    }
    Ok(Options {
        run,
        manifest: manifest.unwrap_or_else(|| DEFAULT_MANIFEST.into()),
        output,
        target,
        device,
    })
}

#[cfg(test)]
mod tests;
