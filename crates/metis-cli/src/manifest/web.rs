//! A browser application: a static page and the package compiled for it.
use super::{identifier, label, relative};
use crate::Result;
use serde::Deserialize;

/// A `metis.json` for a page served to a browser.
///
/// `metis build` compiles `frontend.package` to WebAssembly, generates its
/// loader, and stages it beside a copy of `frontend.directory`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebApplication {
    pub(crate) schema: u32,
    pub(crate) name: String,
    pub(crate) cargo_manifest: String,
    /// Optional macOS shell icon, relative to the application manifest.
    #[serde(default)]
    pub(crate) icon: Option<String>,
    /// Exact HTTPS popup destinations granted by the signed Apple bundle.
    ///
    /// Omission grants no links. Moirai owns validation and native admission;
    /// this field cannot grant broker capabilities or existing-frame navigation.
    #[serde(default)]
    pub(crate) external_links: Vec<String>,
    pub(crate) frontend: Frontend,
}

/// The page and the Cargo package that runs in it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Frontend {
    /// Workspace package whose `cdylib` target is the page's module.
    pub(crate) package: String,
    /// Directory of static files served beside the generated module.
    pub(crate) directory: String,
}

impl WebApplication {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.schema != 1 {
            return Err("unsupported application manifest schema".into());
        }
        label(&self.name)?;
        relative(&self.cargo_manifest)?;
        identifier(&self.frontend.package)?;
        moirai_webview::ExternalLinks::new(&self.external_links)?;
        if let Some(icon) = &self.icon {
            relative(icon)?;
            if !std::path::Path::new(icon)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("icns"))
            {
                return Err("frontend application icon must be an ICNS file".into());
            }
        }
        relative(&self.frontend.directory)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Manifest;
    use std::fs;

    fn capture() -> tracing::subscriber::DefaultGuard {
        tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .with_test_writer()
                .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
                .finish(),
        )
    }

    fn read(document: &str) -> crate::Result<Manifest> {
        let path = std::env::temp_dir().join(format!(
            "metis-web-manifest-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock")
                .as_nanos()
        ));
        fs::write(&path, document).expect("manifest");
        let result = Manifest::read(&path).map(|(manifest, _)| manifest);
        fs::remove_file(path).expect("test cleanup");
        result
    }

    const STARTER: &str = include_str!("../../../metis-starter/metis.json");

    fn with_links(links: serde_json::Value) -> String {
        let mut document: serde_json::Value = serde_json::from_str(STARTER).expect("starter JSON");
        document["external_links"] = links;
        serde_json::to_string(&document).expect("manifest JSON")
    }

    #[test]
    fn omitted_links_grant_no_destinations_and_starter_names_exact_links() {
        let _trace = capture();
        let _test = tracing::info_span!("external_link_manifest_values").entered();
        let document = tracing::info_span!("arrange").in_scope(|| {
            let mut document: serde_json::Value =
                serde_json::from_str(STARTER).expect("starter JSON");
            document
                .as_object_mut()
                .expect("manifest object")
                .remove("external_links");
            serde_json::to_string(&document).expect("manifest JSON")
        });
        let (omitted, declared) = tracing::info_span!("act").in_scope(|| {
            (
                read(&document).expect("omitted policy"),
                read(STARTER).expect("declared policy"),
            )
        });
        let _assert = tracing::info_span!("assert").entered();
        let Manifest::Web(omitted) = omitted else {
            panic!("browser manifest")
        };
        let Manifest::Web(declared) = declared else {
            panic!("browser manifest")
        };
        assert_eq!(omitted.external_links, Vec::<String>::new());
        assert_eq!(
            declared.external_links,
            [
                "https://github.com/ryancinsight/metis",
                "https://webassembly.org/",
            ]
        );
    }

    #[test]
    fn external_link_validation_preserves_provider_rejections() {
        let _trace = capture();
        let _test = tracing::info_span!("external_link_manifest_rejection").entered();
        for destination in [
            "http://example.test/",
            "file:///tmp/index.html",
            "javascript:alert(1)",
            "https://owner:secret@example.test/",
            "https://example.test/white space",
            "https://example.test/%GG",
            "https://example.test:65536/",
        ] {
            let document = tracing::info_span!("arrange", destination)
                .in_scope(|| with_links(serde_json::json!([destination])));
            let error = tracing::info_span!("act")
                .in_scope(|| read(&document).err().expect("invalid destination"));
            let expected = moirai_webview::ExternalLinks::new([destination])
                .expect_err("provider must reject destination");
            let _assert = tracing::info_span!("assert").entered();
            let actual = error
                .downcast_ref::<std::io::Error>()
                .expect("provider error retained");
            assert_eq!(actual.kind(), std::io::ErrorKind::InvalidInput);
            assert_eq!(actual.to_string(), expected.to_string());
        }
    }

    #[test]
    fn external_link_manifest_obeys_inclusive_provider_bounds() {
        let _trace = capture();
        let _test = tracing::info_span!("external_link_manifest_bounds").entered();
        let (valid, excessive_count, excessive_bytes) =
            tracing::info_span!("arrange").in_scope(|| {
                let prefix = "https://example.test/";
                let exact = format!(
                    "{prefix}{}",
                    "a".repeat(moirai_webview::MAX_EXTERNAL_LINK_BYTES - prefix.len())
                );
                (
                    with_links(serde_json::json!(vec![
                        exact.clone();
                        moirai_webview::MAX_EXTERNAL_LINKS
                    ])),
                    with_links(serde_json::json!(vec![
                        exact.clone();
                        moirai_webview::MAX_EXTERNAL_LINKS
                            + 1
                    ])),
                    with_links(serde_json::json!([format!("{exact}a")])),
                )
            });
        let (valid, count, bytes) = tracing::info_span!("act").in_scope(|| {
            (
                read(&valid).expect("inclusive bounds"),
                read(&excessive_count).err().expect("excessive input count"),
                read(&excessive_bytes)
                    .err()
                    .expect("excessive destination bytes"),
            )
        });
        let _assert = tracing::info_span!("assert").entered();
        let Manifest::Web(valid) = valid else {
            panic!("browser manifest")
        };
        assert_eq!(
            valid.external_links.len(),
            moirai_webview::MAX_EXTERNAL_LINKS
        );
        let prefix = "https://example.test/";
        let expected = format!(
            "{prefix}{}",
            "a".repeat(moirai_webview::MAX_EXTERNAL_LINK_BYTES - prefix.len())
        );
        for link in valid.external_links {
            assert_eq!(link, expected);
        }
        assert_eq!(
            count.to_string(),
            "external destination count exceeds the host bound"
        );
        assert_eq!(
            bytes.to_string(),
            "external destination violates the URI byte contract"
        );
    }

    #[test]
    fn frontend_selects_the_browser_shape() {
        let Manifest::Web(application) = read(STARTER).expect("starter manifest") else {
            panic!("the starter manifest must read as a browser application");
        };
        assert_eq!(application.frontend.package, "metis-starter");
        assert_eq!(application.frontend.directory, "frontend");
        assert_eq!(application.cargo_manifest, "Cargo.toml");
    }

    #[test]
    fn each_shape_rejects_the_other_fields() {
        let mixed = STARTER.replacen('{', "{\n  \"entry\": \"metis-app\",", 1);
        let error = read(&mixed)
            .err()
            .expect("native field in a browser manifest");
        assert!(
            error.to_string().contains("unknown field `entry`"),
            "{error}"
        );
        let native = include_str!("../../../../metis.json");
        assert!(matches!(read(native), Ok(Manifest::Native(_))));
    }

    #[test]
    fn frontend_paths_and_package_are_validated() {
        for (field, value) in [
            (
                "\"directory\": \"frontend\"",
                "\"directory\": \"../outside\"",
            ),
            (
                "\"cargo_manifest\": \"Cargo.toml\"",
                "\"cargo_manifest\": \"C:/Cargo.toml\"",
            ),
            (
                "\"package\": \"metis-starter\"",
                "\"package\": \"-leading-dash\"",
            ),
        ] {
            let document = STARTER.replacen(field, value, 1);
            assert_ne!(document, STARTER, "fixture lacks {field}");
            assert!(read(&document).is_err(), "accepted {value}");
        }
        let document = STARTER.replacen("\"schema\": 1", "\"schema\": 2", 1);
        assert_ne!(document, STARTER);
        assert!(read(&document).is_err(), "accepted schema 2");
    }
}
