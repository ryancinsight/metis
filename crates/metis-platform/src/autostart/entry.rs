//! The per-user login-item files on macOS and freedesktop systems.

#[cfg(not(windows))]
use std::io;
#[cfg(not(windows))]
use std::path::{Path, PathBuf};

/// The directory login items live in: `~/Library/LaunchAgents` on macOS,
/// otherwise `$XDG_CONFIG_HOME/autostart` or `~/.config/autostart`.
#[cfg(not(windows))]
pub(super) fn user_directory() -> io::Result<PathBuf> {
    let absolute = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let missing = || io::Error::new(io::ErrorKind::NotFound, "no absolute home directory");
    if cfg!(target_os = "macos") {
        return Ok(absolute("HOME")
            .ok_or_else(missing)?
            .join("Library/LaunchAgents"));
    }
    let config = absolute("XDG_CONFIG_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".config")))
        .ok_or_else(missing)?;
    Ok(config.join("autostart"))
}

/// The login-item file for `name` below `directory`.
#[cfg(not(windows))]
pub(super) fn path(directory: &Path, name: &str) -> PathBuf {
    let extension = if cfg!(target_os = "macos") {
        "plist"
    } else {
        "desktop"
    };
    directory.join(format!("{name}.{extension}"))
}

/// The login-item document for this platform.
#[cfg(not(windows))]
pub(super) fn document(name: &str, program: &str, arguments: &[String]) -> String {
    if cfg!(target_os = "macos") {
        launch_agent(name, program, arguments)
    } else {
        desktop_entry(name, program, arguments)
    }
}

/// An XDG autostart desktop entry.
pub(super) fn desktop_entry(name: &str, program: &str, arguments: &[String]) -> String {
    use metis_core::command_line::desktop_exec_word;
    let mut exec = desktop_exec_word(program);
    for argument in arguments {
        exec.push(' ');
        exec.push_str(&desktop_exec_word(argument));
    }
    // The name is validated to letters, digits, `.` and `-`.
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

/// A launch-agent property list that runs the program at login.
pub(super) fn launch_agent(name: &str, program: &str, arguments: &[String]) -> String {
    let mut plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>{name}</string><key>ProgramArguments</key><array><string>{}</string>",
        xml_escape(program)
    );
    for argument in arguments {
        plist.push_str("<string>");
        plist.push_str(&xml_escape(argument));
        plist.push_str("</string>");
    }
    plist.push_str("</array><key>RunAtLoad</key><true/></dict></plist>\n");
    plist
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
