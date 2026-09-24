//! Document-type registration in the Linux package and the macOS bundle.

use crate::manifest::Application;
use std::fmt::Write;

/// The MIME types the desktop entry declares it opens.
pub(super) fn desktop_mime_types(application: &Application) -> impl Iterator<Item = &str> {
    application
        .file_associations
        .iter()
        .map(|association| association.mime_type.as_str())
}

/// The shared-mime-info package that maps each extension to its MIME type,
/// installed under `usr/share/mime/packages`. `None` without associations.
pub(super) fn mime_package(application: &Application) -> Option<String> {
    if application.file_associations.is_empty() {
        return None;
    }
    let mut package = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n",
    );
    for association in &application.file_associations {
        // MIME types and extensions are validated to token characters.
        let _ = write!(
            package,
            "  <mime-type type=\"{}\">\n    <comment>{}</comment>\n",
            association.mime_type,
            super::xml_escape(&association.description),
        );
        for extension in &association.extensions {
            let _ = writeln!(package, "    <glob pattern=\"*.{extension}\"/>");
        }
        package.push_str("  </mime-type>\n");
    }
    package.push_str("</mime-info>\n");
    Some(package)
}

/// The `CFBundleDocumentTypes` entry that lets Launch Services open the
/// declared documents with the bundle. Empty without associations.
pub(super) fn plist_registration(application: &Application) -> String {
    if application.file_associations.is_empty() {
        return String::new();
    }
    let mut entry = String::from("<key>CFBundleDocumentTypes</key><array>");
    for association in &application.file_associations {
        let _ = write!(
            entry,
            "<dict><key>CFBundleTypeName</key><string>{}</string><key>CFBundleTypeRole</key><string>Viewer</string><key>CFBundleTypeExtensions</key><array>",
            super::xml_escape(&association.description),
        );
        for extension in &association.extensions {
            let _ = write!(entry, "<string>{extension}</string>");
        }
        let _ = write!(
            entry,
            "</array><key>CFBundleTypeMIMETypes</key><array><string>{}</string></array></dict>",
            association.mime_type,
        );
    }
    entry.push_str("</array>");
    entry
}
