//! Build and distribute applications from a single validated manifest.
mod build;
mod commands;
mod dev;
mod init;
mod manifest;
mod process;
#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "Native Windows Installer ABI is isolated behind checked RAII handles"
)]
mod windows;

use std::path::Path;

/// The CLI's cold error-reporting boundary preserves concrete source errors.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || args.as_slice() == ["--help"] || args.as_slice() == ["-h"] {
        print!("{}", commands::help());
        return Ok(());
    }
    match args.first().and_then(|value| value.to_str()) {
        Some("init") => {
            if args.len() != 2 {
                return Err(usage_error("init requires exactly one DIRECTORY"));
            }
            init::create(Path::new(&args[1]))
        }
        Some("dev") => dev::run(&args),
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
        Some("build" | "package") => {
            let [action, input, output] = args.as_slice() else {
                return Err(usage_error("build and package require MANIFEST and OUTPUT"));
            };
            let kind = match action.to_str() {
                Some("build") => build::OutputKind::Portable,
                Some("package") => build::OutputKind::Installer,
                _ => return Err(usage_error("unknown build action")),
            };
            build::application(Path::new(input), Path::new(output), kind)
        }
        _ => Err(usage_error("unknown command")),
    }
}

fn usage_error(message: &str) -> Box<dyn std::error::Error> {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{message}\n\n{}", commands::help()),
    )
    .into()
}
