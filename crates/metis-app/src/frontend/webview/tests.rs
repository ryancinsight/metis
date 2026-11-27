use super::{
    MAX_PATIENT_ID_BYTES, WebViewAction, WebViewRequest,
    assets::{INDEX_HTML, PAGE_JS, PAGE_WASM, PERMISSION_PROBE_INDEX_HTML, STYLES_CSS},
    is_permission_probe_complete,
    package::{Package, Page},
    permission_denied_message,
};
use metis_platform::native::WebViewPermission;

#[test]
fn package_assets_are_script_scoped_and_wasm_capable() {
    assert!(INDEX_HTML.contains("script-src 'self' 'wasm-unsafe-eval'"));
    assert!(INDEX_HTML.contains("default-src 'none'"));
    for directive in [
        "img-src 'none'",
        "font-src 'none'",
        "media-src 'none'",
        "connect-src https://metis.invalid/page_bg.wasm;",
        "object-src 'none'",
        "frame-src 'none'",
        "child-src 'none'",
        "worker-src 'none'",
        "manifest-src 'none'",
        "form-action 'none'",
        "base-uri 'none'",
    ] {
        assert!(
            INDEX_HTML.contains(directive),
            "missing CSP directive: {directive}"
        );
    }
    assert!(STYLES_CSS.contains("#0f172a"));
    assert!(INDEX_HTML.contains("data-metis-theme=\"system\""));
    assert!(INDEX_HTML.contains("id=\"theme-mode\" name=\"theme-mode\""));
    for theme in ["system", "light", "dark", "high-contrast"] {
        assert!(
            INDEX_HTML.contains(&format!("<option value=\"{theme}\"")),
            "missing WebView2 theme option: {theme}"
        );
        assert!(
            STYLES_CSS.contains(&format!("body[data-metis-theme=\"{theme}\"]")),
            "missing WebView2 theme selector: {theme}"
        );
    }
    for page in [INDEX_HTML, PERMISSION_PROBE_INDEX_HTML] {
        assert!(
            page.contains("script type=\"module\" src=\"./page.js\""),
            "the packaged page must load only the generated page loader"
        );
        assert!(!page.contains("<script src=\"./app.js\""));
        // No inline script bodies: the CSP admits same-origin modules only.
        for tag in page.split("<script").skip(1) {
            let body = tag.split("</script>").next().unwrap_or_default();
            assert!(
                !body.contains('>') || !body.contains('{'),
                "the packaged page must not embed an inline script body"
            );
        }
    }
}

#[test]
fn page_loader_is_generated_and_mechanically_started() {
    assert!(PAGE_JS.starts_with("/* @ts-self-types=\"./page.d.ts\" */"));
    assert!(PAGE_JS.contains("export { initSync, __wbg_init as default };"));
    assert!(PAGE_JS.ends_with("\nawait __wbg_init();\n"));
    assert!(PAGE_JS.contains("new URL('page_bg.wasm', import.meta.url)"));
    assert_eq!(PAGE_WASM.get(..8), Some(b"\0asm\x01\0\0\0".as_slice()));
}

#[test]
fn packaged_command_surface_is_local_and_keyboard_closable() {
    for fragment in [
        "id=\"application-navigation\"",
        "role=\"toolbar\"",
        "id=\"command-menu-toggle\"",
        "aria-haspopup=\"menu\"",
        "id=\"command-menu\" role=\"menu\"",
        "id=\"command-theme-dark\" role=\"menuitem\"",
        "id=\"command-theme-system\" role=\"menuitem\"",
        "id=\"command-status\" role=\"status\"",
    ] {
        assert!(
            INDEX_HTML.contains(fragment),
            "missing WebView2 command-surface fragment: {fragment}"
        );
    }
    for command in metis_frontend::ApplicationCommand::ALL {
        let start = INDEX_HTML
            .find(&format!("id=\"{}\"", command.id()))
            .expect("packaged command button");
        let tag = INDEX_HTML[start..].split('>').next().unwrap_or_default();
        let announced = format!(
            "aria-keyshortcuts=\"{}\"",
            command.accelerator().aria_keyshortcuts()
        );
        assert!(
            tag.contains(&announced),
            "{} must announce its declared accelerator",
            command.id()
        );
    }
    assert!(STYLES_CSS.contains("#command-menu[data-command-menu-open=\"false\"]"));
    assert!(STYLES_CSS.contains("@media (forced-colors: active)"));
}

#[test]
fn page_loader_has_no_unscoped_authority_bridge() {
    for forbidden in [
        "hostObjects",
        "XMLHttpRequest",
        "WebSocket",
        "window.open",
        "navigator.geolocation",
        "coreWebView2",
    ] {
        assert!(
            !PAGE_JS.contains(forbidden),
            "page loader must not acquire unscoped authority: {forbidden}"
        );
    }
    assert_eq!(
        PAGE_JS.matches("fetch(").count(),
        1,
        "WASM initialization fetch"
    );
}

#[test]
fn permission_probe_is_separate_from_the_calculation_page() {
    assert!(!INDEX_HTML.contains("Metis permission probe"));
    assert!(PERMISSION_PROBE_INDEX_HTML.contains("Metis permission probe"));
    for permission in ["geolocation", "camera", "microphone", "notifications"] {
        assert!(
            PERMISSION_PROBE_INDEX_HTML.contains(&format!("permission-{permission}")),
            "missing permission result row: {permission}"
        );
    }
    // Both packaged pages share one generated loader; the probe behavior is
    // dispatched inside the Rust module (see the owning crate's tests).
    assert!(PAGE_JS.contains("initSync"));
}

#[test]
fn packaged_module_uses_the_reserved_secure_origin() {
    for page in [Page::Form, Page::PermissionProbe] {
        let package = Package::create(page, None).expect("packaged assets");
        let config = package.config().expect("mapped WebView2 origin");
        assert_eq!(config.start_uri(), "https://metis.invalid/index.html");
        assert_eq!(
            std::fs::read_to_string(package.root.join("assets/index.html"))
                .expect("packaged entry"),
            page.assets()
        );
        assert_eq!(
            std::fs::read(package.root.join("assets/page_bg.wasm")).expect("packaged WASM"),
            PAGE_WASM
        );
        package.cleanup().expect("release package");
    }
}

#[test]
fn patient_limit_matches_the_page_contract() {
    assert_eq!(MAX_PATIENT_ID_BYTES, 128);
}

#[test]
fn permission_denial_message_is_typed_for_the_page() {
    let message = permission_denied_message(WebViewPermission::Geolocation, false);
    let json = serde_json::to_string(&message).expect("permission error payload");
    assert_eq!(
        json,
        r#"{"type":"permission_denied","status":"permission_denied","error_code":8207,"permission":"geolocation","message":"WebView2 denied geolocation access request","user_initiated":false}"#
    );
}

#[test]
fn permission_denial_message_preserves_user_initiation() {
    let message = permission_denied_message(WebViewPermission::Camera, true);
    let json = serde_json::to_string(&message).expect("permission error payload");
    assert_eq!(
        json,
        r#"{"type":"permission_denied","status":"permission_denied","error_code":8207,"permission":"camera","message":"WebView2 denied camera access request","user_initiated":true}"#
    );
}

#[test]
fn request_parser_rejects_unknown_fields_and_preserves_inputs() {
    let request: WebViewRequest = serde_json::from_str(
        r#"{"action":"submit","patient_id":"PT-1","weight_kg":72.5,"concentration_mg_ml":4.0,"target_dose_mcg_kg_min":0.5}"#,
    )
    .expect("bounded request");
    assert_eq!(request.action, WebViewAction::Submit);
    assert_eq!(request.patient_id, "PT-1");
    assert!((request.weight_kg - 72.5).abs() <= f64::EPSILON);
    assert!(serde_json::from_str::<WebViewRequest>(
        r#"{"action":"submit","patient_id":"PT-1","weight_kg":72.5,"concentration_mg_ml":4.0,"target_dose_mcg_kg_min":0.5,"extra":true}"#,
    )
    .is_err());
}

#[test]
fn permission_capture_waits_for_typed_completion_message() {
    assert!(is_permission_probe_complete(
        r#"{"action":"permission_probe_complete"}"#
    ));
    assert!(!is_permission_probe_complete(
        r#"{"action":"permission_probe_complete","extra":true}"#
    ));
    assert!(!is_permission_probe_complete(r#"{"action":"submit"}"#));
}
