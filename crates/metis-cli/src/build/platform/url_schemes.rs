//! URL-scheme registration in the Linux desktop entry and the macOS bundle.

use crate::manifest::Application;
use std::fmt::Write;

/// The desktop-entry lines that register `application`'s schemes: the
/// `%u` field code the launcher replaces with the clicked link, and the
/// `x-scheme-handler` MIME types. Empty when no scheme is declared.
pub(super) fn desktop_registration(application: &Application) -> (&'static str, String) {
    if application.url_schemes.is_empty() {
        return ("", String::new());
    }
    let mut mime = String::from("MimeType=");
    for scheme in &application.url_schemes {
        // Schemes are validated to `[a-z][a-z0-9+.-]*`, which needs no
        // desktop-entry escaping.
        let _ = write!(mime, "x-scheme-handler/{scheme};");
    }
    mime.push('\n');
    (" %u", mime)
}

/// The `CFBundleURLTypes` entry that registers `application`'s schemes with
/// Launch Services. Empty when no scheme is declared.
pub(super) fn plist_registration(application: &Application) -> String {
    if application.url_schemes.is_empty() {
        return String::new();
    }
    let mut entry = format!(
        "<key>CFBundleURLTypes</key><array><dict><key>CFBundleURLName</key><string>{}</string><key>CFBundleURLSchemes</key><array>",
        super::xml_escape(&application.id)
    );
    for scheme in &application.url_schemes {
        let _ = write!(entry, "<string>{scheme}</string>");
    }
    entry.push_str("</array></dict></array>");
    entry
}
