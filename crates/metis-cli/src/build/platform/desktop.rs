//! The freedesktop.org desktop entry that launches a Linux package.

use crate::manifest::Application;
use std::path::Path;

pub(super) fn desktop_entry(application: &Application, entry: &str) -> String {
    let icon = application.resources.iter().find_map(|resource| {
        let extension = Path::new(&resource.destination).extension()?;
        extension
            .eq_ignore_ascii_case("svg")
            .then(|| {
                desktop_icon(&format!(
                    "/usr/share/{}/{}",
                    application.id, resource.destination
                ))
            })
            .or_else(|| {
                extension.eq_ignore_ascii_case("png").then(|| {
                    desktop_icon(&format!(
                        "/usr/share/{}/{}",
                        application.id, resource.destination
                    ))
                })
            })
    });
    let mut output = format!(
        "[Desktop Entry]\nVersion=1.0\nType=Application\nName={}\nComment={} application\nExec=/usr/bin/{}",
        desktop_escape(&application.name),
        desktop_escape(&application.name),
        desktop_word(entry),
    );
    for argument in &application.arguments {
        output.push(' ');
        output.push_str(&desktop_word(argument));
    }
    let mime_types: Vec<String> = super::url_schemes::desktop_mime_types(application)
        .chain(super::file_associations::desktop_mime_types(application).map(str::to_owned))
        .collect();
    if !mime_types.is_empty() {
        // `%u` receives the clicked link or the opened document.
        output.push_str(" %u");
    }
    output.push_str("\nTerminal=false\n");
    if !mime_types.is_empty() {
        output.push_str("MimeType=");
        for mime_type in &mime_types {
            output.push_str(mime_type);
            output.push(';');
        }
        output.push('\n');
    }
    if let Some(icon) = icon {
        output.push_str("Icon=");
        output.push_str(&icon);
        output.push('\n');
    }
    output.push_str("Categories=Utility;\n");
    output
}

pub(crate) fn desktop_word(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.bytes().any(|byte| {
            byte.is_ascii_whitespace()
                || byte.is_ascii_control()
                || matches!(byte, b'"' | 96 | b'$' | b'\\' | b'%')
        });
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' | '"' | '\u{60}' | '$' => {
                escaped.push('\\');
                escaped.push(character);
            }
            '%' => escaped.push_str("%%"),
            _ => escaped.push(character),
        }
    }
    if needs_quotes {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

pub(crate) fn desktop_icon(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            ' ' => escaped.push_str("\\s"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            ';' => escaped.push_str("\\;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn desktop_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
