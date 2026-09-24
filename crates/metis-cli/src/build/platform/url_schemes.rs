//! URL-scheme registration in the Linux desktop entry and the macOS bundle.

use crate::manifest::Application;
use std::fmt::Write;

/// The `x-scheme-handler` MIME types the desktop entry declares.
pub(super) fn desktop_mime_types(application: &Application) -> impl Iterator<Item = String> {
    // Schemes are validated to `[a-z][a-z0-9+.-]*`, which needs no
    // desktop-entry escaping.
    application
        .url_schemes
        .iter()
        .map(|scheme| format!("x-scheme-handler/{scheme}"))
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
