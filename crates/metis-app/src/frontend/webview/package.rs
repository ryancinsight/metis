//! Caller-owned content and fresh profile use separate sibling directories.

use super::assets;
use crate::invocation::WebViewTheme;
use metis_core::embedded_assets::{
    AssetKind, Digest, EmbeddedModuleInventory, FileIdentity, MODULE_LIMIT,
};
use metis_platform::native::{
    MAX_WEBVIEW_MESSAGE_BYTES, MAX_WEBVIEW_WAIT_MILLISECONDS, WebViewConfig,
};
use metis_web::embedded::{PageCommand, PageIdentity, PageRole, SequenceId, encode_page_command};
use std::{fs, io, path::PathBuf, time::Duration};

const MAX_PACKAGE_ATTEMPTS: u32 = 8;
const HOST: &str = "metis.invalid";

pub(super) struct Package {
    pub(super) root: PathBuf,
    pub(super) content: PathBuf,
    pub(super) profile: PathBuf,
    identity: PageIdentity,
}

impl Package {
    pub(super) fn create(page: PageRole, initial_theme: Option<WebViewTheme>) -> io::Result<Self> {
        let digest = Digest::hash(assets::EMBEDDED_INVENTORY);
        if digest.as_bytes() != &assets::EMBEDDED_INVENTORY_DIGEST {
            return Err(invalid(
                "Compiled embedded inventory digest differs from admitted bytes",
            ));
        }
        let inventory = EmbeddedModuleInventory::decode(assets::EMBEDDED_INVENTORY)?;
        let identity = PageIdentity::new(page, SequenceId::new(1)?, digest);
        let index = match page {
            PageRole::Form => assets::INDEX_HTML,
            PageRole::PermissionProbe => assets::PERMISSION_PROBE_INDEX_HTML,
        };
        let command =
            encode_page_command(&PageCommand::Mount { identity }, MAX_WEBVIEW_MESSAGE_BYTES)?;
        if index.matches("{{mount-command}}").count() != 1 {
            return Err(invalid(
                "Packaged markup must carry exactly one mount command",
            ));
        }
        let index = index.replace("{{mount-command}}", &attribute(&command));
        let index = crate::frontend::webview_command_observation::mark_document(
            index,
            crate::frontend::webview_command_observation::selected(page),
        )?;
        let index = match initial_theme {
            Some(theme) => index.replace(
                "data-metis-theme=\"system\"",
                &format!("data-metis-theme=\"{}\"", theme.query_value()),
            ),
            None => index,
        };
        let process = std::process::id();
        for attempt in 0..MAX_PACKAGE_ATTEMPTS {
            let root = std::env::temp_dir().join(format!("metis-webview-{process}-{attempt}"));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let content = root.join("content");
                    let profile = root.join("profile");
                    let result = (|| {
                        fs::create_dir(&content)?;
                        fs::create_dir(&profile)?;
                        fs::write(content.join("index.html"), index.as_bytes())?;
                        fs::write(content.join("styles.css"), assets::STYLES_CSS)?;
                        fs::write(content.join("bootstrap.js"), assets::BOOTSTRAP_JS)?;
                        for record in &inventory.spec().assets {
                            let (name, bytes) = match record.kind {
                                AssetKind::JavaScript => {
                                    ("metis_web.js", assets::EMBEDDED_MODULE_JS)
                                }
                                AssetKind::WebAssembly => {
                                    ("metis_web_bg.wasm", assets::EMBEDDED_MODULE_WASM)
                                }
                            };
                            if record.destination != name
                                || record.identity.digest != Digest::hash(bytes)
                                || record.identity.bytes
                                    != u64::try_from(bytes.len())
                                        .map_err(|_| invalid("Embedded byte length exceeds u64"))?
                            {
                                return Err(invalid(
                                    "Compiled module does not match its admitted asset record",
                                ));
                            }
                            let path = content.join(name);
                            fs::write(&path, bytes)?;
                            if FileIdentity::read(&path, MODULE_LIMIT)? != record.identity {
                                return Err(invalid(
                                    "Extracted module bytes differ from their verified identity",
                                ));
                            }
                        }
                        fs::write(content.join("inventory.bin"), assets::EMBEDDED_INVENTORY)?;
                        Ok((content.canonicalize()?, profile.canonicalize()?))
                    })();
                    return match result {
                        Ok((content, profile)) => Ok(Self {
                            root,
                            content,
                            profile,
                            identity,
                        }),
                        Err(error) => Err(cleanup_error(root, error)),
                    };
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Bounded WebView2 package names are exhausted",
        ))
    }

    pub(super) const fn identity(&self) -> PageIdentity {
        self.identity
    }

    pub(super) fn config(&self) -> io::Result<WebViewConfig> {
        WebViewConfig::folder(
            HOST,
            &self.content,
            "index.html",
            Duration::from_millis(u64::from(MAX_WEBVIEW_WAIT_MILLISECONDS)),
        )?
        .with_user_data_folder(&self.profile)
    }

    pub(super) fn cleanup(self) -> io::Result<()> {
        fs::remove_dir_all(self.root)
    }
}

pub(super) fn package_failure(package: Package, error: io::Error) -> io::Error {
    cleanup_error(package.root, error)
}

fn cleanup_error(path: PathBuf, error: io::Error) -> io::Error {
    match fs::remove_dir_all(path) {
        Ok(()) => error,
        Err(cleanup) => io::Error::new(
            error.kind(),
            format!("{error}; WebView2 package cleanup failed: {cleanup}"),
        ),
    }
}

fn attribute(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::attribute;
    #[test]
    fn mount_attribute_escapes_html_without_changing_unicode() {
        assert_eq!(
            attribute("{\"patient\":\"<&>患者\"}"),
            "{&quot;patient&quot;:&quot;&lt;&amp;&gt;患者&quot;}"
        );
    }
}
