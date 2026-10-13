//! Static admission and extraction use the actual compiler-produced module.

use super::{
    MAX_PATIENT_ID_BYTES,
    assets::{BOOTSTRAP_JS, INDEX_HTML, PERMISSION_PROBE_INDEX_HTML, STYLES_CSS},
    package::Package,
};
use metis_core::embedded_assets::{Digest, EmbeddedModuleInventory};
use metis_web::embedded::PageRole;
use tracing_subscriber::fmt::format::FmtSpan;

fn trace() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_span_events(FmtSpan::FULL)
            .finish(),
    )
}

#[test]
fn package_assets_admit_only_local_module_execution() {
    let _trace = trace();
    let _span = tracing::info_span!("assert", contract = "local module CSP").entered();
    for page in [INDEX_HTML, PERMISSION_PROBE_INDEX_HTML] {
        for directive in [
            "default-src 'none'",
            "script-src 'self' 'wasm-unsafe-eval'",
            "style-src 'self'",
            "connect-src 'self'",
            "img-src 'none'",
            "font-src 'none'",
            "media-src 'none'",
            "object-src 'none'",
            "frame-src 'none'",
            "child-src 'none'",
            "worker-src 'none'",
            "manifest-src 'none'",
            "form-action 'none'",
            "base-uri 'none'",
        ] {
            assert!(
                page.contains(directive),
                "missing CSP directive: {directive}"
            );
        }
        assert_eq!(page.matches("{{mount-command}}").count(), 1);
        assert_eq!(page.matches("<script ").count(), 1);
        assert!(page.contains("<script src=\"./bootstrap.js\" type=\"module\"></script>"));
        assert!(!page.contains("unsafe-inline"));
    }
    // The authored JavaScript is only the standard module initialization ABI.
    assert_eq!(BOOTSTRAP_JS.lines().count(), 3);
    assert_eq!(
        BOOTSTRAP_JS,
        "import init from './metis_web.js';\nconst module = await init({ module_or_path: './metis_web_bg.wasm' });\nmodule.metis_embedded_start();\n"
    );
}

#[test]
fn packaged_command_surface_announces_the_canonical_shortcuts() {
    let _trace = trace();
    let _span = tracing::info_span!("assert", contract = "command metadata").entered();
    for fragment in [
        "id=\"application-navigation\"",
        "role=\"toolbar\"",
        "id=\"command-menu-toggle\"",
        "aria-haspopup=\"menu\"",
        "id=\"command-menu\" role=\"menu\"",
        "aria-expanded=\"false\"",
        "aria-hidden=\"true\"",
        "id=\"command-status\" role=\"status\"",
        "id=\"calculation-submit\" type=\"submit\"",
    ] {
        assert!(
            INDEX_HTML.contains(fragment),
            "missing command metadata: {fragment}"
        );
    }
    for command in metis_frontend::ApplicationCommand::ALL {
        let start = INDEX_HTML
            .find(&format!("id=\"{}\"", command.id()))
            .expect("command button");
        let tag = INDEX_HTML[start..].split('>').next().expect("button tag");
        let shortcut = format!(
            "aria-keyshortcuts=\"{}\"",
            command.accelerator().aria_keyshortcuts()
        );
        assert!(tag.contains(&shortcut), "{} shortcut", command.id());
    }
    assert!(STYLES_CSS.contains("#command-menu[data-command-menu-open=\"false\"]"));
    assert!(STYLES_CSS.contains("@media (forced-colors: active)"));
    assert!(!STYLES_CSS.contains("{{mount-command}}"));
    for theme in ["system", "light", "dark", "high-contrast"] {
        assert!(INDEX_HTML.contains(&format!("<option value=\"{theme}\"")));
        assert!(STYLES_CSS.contains(&format!("body[data-metis-theme=\"{theme}\"]")));
    }
}

#[test]
fn permission_markup_is_separate_and_uses_the_same_rust_entry_point() {
    let _trace = trace();
    let _span = tracing::info_span!("assert", contract = "capability role").entered();
    for permission in ["geolocation", "camera", "microphone", "notifications"] {
        let row = format!("id=\"permission-{permission}\"");
        assert!(PERMISSION_PROBE_INDEX_HTML.contains(&row));
        assert!(!INDEX_HTML.contains(&row));
    }
    assert!(!PERMISSION_PROBE_INDEX_HTML.contains("id=\"calculation\""));
    assert_eq!(MAX_PATIENT_ID_BYTES, 128);
}

#[test]
fn package_extraction_matches_the_compiled_inventory_and_cleans_owned_directories() {
    let _trace = trace();
    for role in [PageRole::Form, PageRole::PermissionProbe] {
        let package = {
            let _span = tracing::info_span!("arrange", ?role).entered();
            Package::create(role, None).expect("actual prepared module package")
        };
        let _span = tracing::info_span!("act", ?role).entered();
        let root = package.root.clone();
        assert_eq!(package.profile.parent(), package.content.parent());
        assert_ne!(package.profile, package.content);
        let inventory_bytes =
            std::fs::read(package.content.join("inventory.bin")).expect("inventory");
        let inventory = EmbeddedModuleInventory::decode(&inventory_bytes).expect("typed inventory");
        assert_eq!(
            Digest::hash(&inventory_bytes),
            package.identity().module_digest()
        );
        for asset in &inventory.spec().assets {
            let bytes =
                std::fs::read(package.content.join(&asset.destination)).expect("extracted asset");
            assert_eq!(Digest::hash(&bytes), asset.identity.digest);
            assert_eq!(
                u64::try_from(bytes.len()).expect("asset byte length"),
                asset.identity.bytes
            );
        }
        let index = std::fs::read_to_string(package.content.join("index.html")).expect("index");
        assert!(!index.contains("{{mount-command}}"));
        assert!(index.contains("&quot;schema&quot;:1"));
        assert_eq!(
            std::fs::read_to_string(package.content.join("bootstrap.js")).expect("bootstrap"),
            BOOTSTRAP_JS
        );
        package.cleanup().expect("owned package cleanup");
        let _span = tracing::info_span!("assert", contract = "closed package absence").entered();
        assert!(!root.exists());
    }
}
