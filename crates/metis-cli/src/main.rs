//! Build and distribute applications from a single validated manifest.
mod apple;
mod bounded_read;
mod build;
mod commands;
mod dev;
mod init;
mod manifest;
mod process;
mod serve;
#[cfg(test)]
mod test_support;
mod tree;
#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "Native Windows Installer ABI is isolated behind checked RAII handles"
)]
mod windows;

use manifest::{Application, DEFAULT_MANIFEST, Manifest};
use std::{ffi::OsString, path::Path};

/// The CLI's cold error-reporting boundary preserves concrete source errors.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    // This process owns its subscriber; command data stays on stdout.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .without_time()
        .try_init()
        // Condition 1: the provider returns an erased error whose concrete
        // type is unknown at this cold process reporting boundary.
        .map_err(|error| -> Box<dyn std::error::Error> { error })?;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || args.as_slice() == ["--help"] || args.as_slice() == ["-h"] {
        print!("{}", commands::help());
        return Ok(());
    }
    match args.first().and_then(|value| value.to_str()) {
        Some("asset-budgets") => {
            if args.len() != 1 {
                return Err(usage_error("asset-budgets takes no arguments"));
            }
            commands::asset_budgets::write(std::io::stdout().lock())
        }
        Some("init") => {
            if args.len() != 2 {
                return Err(usage_error("init requires exactly one DIRECTORY"));
            }
            init::create(Path::new(&args[1]))
        }
        Some("dev") => dev::run(&args),
        Some("apple") => apple::run(&args[1..]),
        Some("completions") => {
            if args.len() != 2 {
                return Err(usage_error("completions requires exactly one SHELL"));
            }
            let shell = args[1]
                .to_str()
                .ok_or("completion shell must be valid Unicode")?;
            print!("{}", commands::completions(shell)?);
            Ok(())
        }
        Some("install") => {
            let [_, archive, prefix] = args.as_slice() else {
                return Err(usage_error("install requires ARCHIVE and PREFIX"));
            };
            build::install(Path::new(archive), Path::new(prefix))
        }
        Some("uninstall") => {
            let [_, application_id, prefix] = args.as_slice() else {
                return Err(usage_error("uninstall requires APPLICATION_ID and PREFIX"));
            };
            let application_id = application_id
                .to_str()
                .ok_or("application identity must be valid Unicode")?;
            build::uninstall(application_id, Path::new(prefix))
        }
        Some("build") => {
            let (manifest, output) = commands::arguments::manifest_output(&args[1..], "build")?;
            match Manifest::read(manifest)? {
                (Manifest::Native(application), root) => {
                    let output = output.ok_or_else(|| {
                        usage_error("building a native application requires OUTPUT")
                    })?;
                    build::application(*application, &root, output, build::OutputKind::Portable)
                }
                (Manifest::Web(application), root) => {
                    let served = build::web::application(&application, &root, output)?;
                    println!("{}", served.display());
                    Ok(())
                }
            }
        }
        Some("prepare") => {
            let (manifest, output) = commands::arguments::manifest_output(&args[1..], "prepare")?;
            let (application, root) = Application::read(manifest)?;
            let preparation = build::prepare::application(&application, &root, output)?
                .ok_or("native manifest omits embedded_module")?;
            println!("{}", preparation.inventory_path.display());
            Ok(())
        }
        Some("package") => {
            let [_, input, output] = args.as_slice() else {
                return Err(usage_error("package requires MANIFEST and OUTPUT"));
            };
            let (application, root) = Application::read(Path::new(input))?;
            build::application(
                application,
                &root,
                Path::new(output),
                build::OutputKind::Installer,
            )
        }
        Some("serve") => {
            let (manifest, port) = serve_arguments(&args[1..])?;
            let (Manifest::Web(application), root) = Manifest::read(manifest)? else {
                return Err(
                    "serve requires a browser application manifest; run a native one with metis dev"
                        .into(),
                );
            };
            let served = build::web::application(&application, &root, None)?;
            serve::serve(serve::Site::load(&served)?, &application.name, port)
        }
        _ => Err(usage_error("unknown command")),
    }
}

/// `serve [MANIFEST] [--port PORT]`, defaulting to `./metis.json` on port 1420.
fn serve_arguments(args: &[OsString]) -> Result<(&Path, u16)> {
    let mut manifest = None;
    let mut port = None;
    let mut rest = args.iter();
    while let Some(argument) = rest.next() {
        if argument.to_str() == Some("--port") {
            let value = rest
                .next()
                .and_then(|value| value.to_str())
                .and_then(|value| value.parse::<u16>().ok())
                .ok_or_else(|| usage_error("--port requires a number from 0 to 65535"))?;
            if port.replace(value).is_some() {
                return Err(usage_error("serve accepts one --port"));
            }
        } else if manifest.replace(Path::new(argument)).is_some() {
            return Err(usage_error("serve accepts one MANIFEST"));
        }
    }
    Ok((
        manifest.unwrap_or(Path::new(DEFAULT_MANIFEST)),
        port.unwrap_or(serve::DEFAULT_PORT),
    ))
}

fn usage_error(message: &str) -> Box<dyn std::error::Error> {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{message}\n\n{}", commands::help()),
    )
    .into()
}
