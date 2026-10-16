use super::{
    MAX_PATIENT_ID_BYTES, WebViewAction, WebViewRequest,
    assets::{INDEX_HTML, PAGE_JS, PERMISSION_PROBE_INDEX_HTML, STYLES_CSS},
    is_permission_probe_complete,
    package::file_uri,
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
        "connect-src 'none'",
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
    // The loader is wasm-bindgen output plus one mechanical auto-init suffix;
    // authored page behavior lives in the metis-webview-page Rust module.
    assert!(PAGE_JS.starts_with("/* @ts-self-types=\"./page.d.ts\" */"));
    assert!(PAGE_JS.contains("export { initSync, __wbg_init as default };"));
    let suffix = "Mechanical auto-init appended by scripts/webview_page.py";
    let suffix_index = PAGE_JS.find(suffix).expect("mechanical auto-init suffix");
    assert!(PAGE_JS[suffix_index..].contains("initSync({ module: new WebAssembly.Module("));
    assert!(PAGE_JS[suffix_index..].contains("atob("));
    // The suffix is the only place after the generated exports.
    assert!(
        PAGE_JS[suffix_index..].matches("append").count() == 1,
        "the generator suffix must appear exactly once"
    );
    // The generated module is embedded once as one base64 payload.
    assert_eq!(PAGE_JS.matches("atob(").count(), 1);
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
    // The generated glue retains an async fetch default path that the page
    // never runs; the mechanical suffix embeds the module instead. Its one
    // occurrence must be the unused `__wbg_init` default.
    let fetch_count = PAGE_JS.matches("fetch(").count();
    assert_eq!(fetch_count, 1, "only the unused generated fetch default");
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
fn file_uri_percent_encodes_package_path_without_separators() {
    let uri = file_uri(std::path::Path::new(r"C:\Metis App\index.html")).expect("file URI");
    assert_eq!(uri, "file:///C:/Metis%20App/index.html");
    assert!(!uri.contains("%2F"));
}

#[test]
fn file_uri_removes_windows_extended_drive_prefix() {
    let uri =
        file_uri(std::path::Path::new(r"\\?\C:\Metis App\index.html")).expect("extended file URI");
    assert_eq!(uri, "file:///C:/Metis%20App/index.html");
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
