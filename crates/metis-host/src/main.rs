//! Installed application entry; compilation and packaging stay in metis-cli.

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn main() -> std::io::Result<()> {
    use std::{io, path::PathBuf};
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init()
        .map_err(io::Error::other)?;
    let mut arguments = std::env::args_os().skip(1);
    let mut resources = None;
    let mut title = None;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--resources") => {
                resources = Some(PathBuf::from(arguments.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--resources requires a directory",
                    )
                })?));
            }
            Some("--title") => {
                title = Some(
                    arguments
                        .next()
                        .and_then(|value| value.into_string().ok())
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "--title requires Unicode text",
                            )
                        })?,
                );
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unknown application argument",
                ));
            }
        }
    }
    let resources = if let Some(root) = resources {
        root
    } else {
        let executable = std::env::current_exe()?;
        let directory = executable
            .parent()
            .ok_or_else(|| io::Error::other("application executable has no directory"))?;
        #[cfg(target_os = "macos")]
        let directory = directory
            .parent()
            .ok_or_else(|| io::Error::other("application has no Contents directory"))?
            .join("Resources");
        #[cfg(target_os = "ios")]
        let directory = directory.to_path_buf();
        directory.join("app")
    };
    moirai_webview::run(
        moirai_webview::ResourceRoot::new(resources)?,
        title.as_deref(),
    )
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn main() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the installed Apple host requires macOS or iOS",
    ))
}
