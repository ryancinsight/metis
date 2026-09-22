use super::{
    MAX_PATIENT_ID_BYTES, WebViewAction, WebViewRequest,
    assets::{
        APP_JS, INDEX_HTML, PERMISSION_PROBE_APP_JS, PERMISSION_PROBE_INDEX_HTML, STYLES_CSS,
    },
    package::file_uri,
    permission_denied_message,
};
use metis_platform::native::WebViewPermission;

#[test]
fn package_assets_are_script_scoped_and_bridge_bound() {
    assert!(INDEX_HTML.contains("script-src 'self'"));
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
        assert!(
            APP_JS.contains(&format!("['{theme}'")),
            "missing theme value: {theme}"
        );
    }
    assert!(APP_JS.contains("applyTheme"));
    assert!(APP_JS.contains("URLSearchParams"));
    assert!(APP_JS.contains("themes.has(requestedTheme)"));
    assert!(APP_JS.contains("if (!label) return;"));
    assert!(APP_JS.contains("chrome.webview"));
}

#[test]
fn page_script_has_no_unscoped_authority_bridge() {
    for forbidden in [
        "hostObjects",
        "fetch(",
        "XMLHttpRequest",
        "WebSocket",
        "window.open",
        "navigator.geolocation",
    ] {
        assert!(
            !APP_JS.contains(forbidden),
            "page script must not acquire unscoped authority: {forbidden}"
        );
    }
}

#[test]
fn permission_probe_is_separate_from_the_calculation_page() {
    assert!(!APP_JS.contains("navigator.geolocation"));
    assert!(PERMISSION_PROBE_INDEX_HTML.contains("Metis permission probe"));
    for permission in ["geolocation", "camera", "microphone", "notifications"] {
        assert!(
            PERMISSION_PROBE_INDEX_HTML.contains(&format!("permission-{permission}")),
            "missing permission result row: {permission}"
        );
        assert!(
            PERMISSION_PROBE_APP_JS.contains(&format!("'{permission}'")),
            "missing permission probe: {permission}"
        );
    }
    assert!(PERMISSION_PROBE_APP_JS.contains("navigator.geolocation"));
    assert!(PERMISSION_PROBE_APP_JS.contains("navigator.mediaDevices"));
    assert!(PERMISSION_PROBE_APP_JS.contains("Notification.requestPermission"));
    assert!(PERMISSION_PROBE_APP_JS.contains("message.status"));
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
