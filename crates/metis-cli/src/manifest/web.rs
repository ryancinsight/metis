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
        relative(&self.frontend.directory)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Manifest;
    use std::fs;

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
