use super::{
    APP_JS, INDEX_HTML, MAX_PATIENT_ID_BYTES, STYLES_CSS, WebViewAction, WebViewRequest, file_uri,
};

#[test]
fn package_assets_are_script_scoped_and_bridge_bound() {
    assert!(INDEX_HTML.contains("script-src 'self'"));
    assert!(INDEX_HTML.contains("connect-src 'none'"));
    assert!(STYLES_CSS.contains("#0f172a"));
    assert!(APP_JS.contains("chrome.webview"));
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
