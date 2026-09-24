use super::{
    BROWSER_SERVICE_ROLE, BrowserResponseDelay, FRONTEND_ROLE, HTTP_SERVICE_ROLE, Invocation,
    InvocationError, NATIVE_FRONTEND_ROLE, NATIVE_WINDOW_ROLE, RESPONSE_DELAY_FLAG,
    SEMANTIC_CAPTURE_ROLE, WEBVIEW_FRONTEND_ROLE, WEBVIEW_PERMISSION_PROBE_CAPTURE_FRONTEND_ROLE,
    WEBVIEW_PERMISSION_PROBE_CAPTURE_ROLE, WEBVIEW_PERMISSION_PROBE_FRONTEND_ROLE,
    WEBVIEW_PERMISSION_PROBE_ROLE, WEBVIEW_ROLE, WEBVIEW_THEME_CAPTURE_FRONTEND_ROLE,
    WEBVIEW_THEME_CAPTURE_ROLE, WebViewTheme,
};
use std::time::Duration;

/// An absolute capture path on the host running the test; the roles reject
/// relative outputs, and absoluteness is platform-defined.
fn capture_path(name: &str) -> String {
    if cfg!(windows) {
        format!(r"C:\captures\{name}")
    } else {
        format!("/captures/{name}")
    }
}

#[test]
fn dispatch_preserves_values_and_selects_one_role() {
    let inputs = ["60".to_owned(), "2".to_owned(), "0.2".to_owned()];
    assert_eq!(
        Invocation::parse(inputs.clone()),
        Ok(Invocation::Backend(inputs.clone()))
    );
    assert_eq!(
        Invocation::parse([FRONTEND_ROLE.to_owned()].into_iter().chain(inputs.clone())),
        Ok(Invocation::Frontend(inputs))
    );
    let native_inputs = ["60".to_owned(), "2".to_owned(), "0.2".to_owned()];
    assert_eq!(
        Invocation::parse(
            [NATIVE_WINDOW_ROLE.to_owned()]
                .into_iter()
                .chain(native_inputs.clone())
        ),
        Ok(Invocation::NativeWindow(native_inputs.clone()))
    );
    assert_eq!(
        Invocation::parse(
            [NATIVE_FRONTEND_ROLE.to_owned()]
                .into_iter()
                .chain(native_inputs.clone())
        ),
        Ok(Invocation::NativeFrontend(native_inputs))
    );
    let webview_inputs = ["60".to_owned(), "2".to_owned(), "0.2".to_owned()];
    assert_eq!(
        Invocation::parse(
            [WEBVIEW_ROLE.to_owned()]
                .into_iter()
                .chain(webview_inputs.clone())
        ),
        Ok(Invocation::WebView(webview_inputs.clone()))
    );
    assert_eq!(
        Invocation::parse(
            [WEBVIEW_FRONTEND_ROLE.to_owned()]
                .into_iter()
                .chain(webview_inputs.clone())
        ),
        Ok(Invocation::WebViewFrontend(webview_inputs))
    );
    assert_eq!(
        Invocation::parse(["--help".to_owned()]),
        Ok(Invocation::Help)
    );
    assert_eq!(
        Invocation::parse([
            BROWSER_SERVICE_ROLE.to_owned(),
            "http://127.0.0.1:8080".to_owned(),
            "8765".to_owned(),
            "66".repeat(16),
        ]),
        Ok(Invocation::BrowserService {
            origin: "http://127.0.0.1:8080".to_owned(),
            port: 8765,
            principal: [0x66; 16],
            response_delay: None,
        })
    );
    assert_eq!(
        Invocation::parse([
            HTTP_SERVICE_ROLE.to_owned(),
            "http://127.0.0.1:8080".to_owned(),
            "8765".to_owned(),
            "66".repeat(16),
        ]),
        Ok(Invocation::HttpService {
            origin: "http://127.0.0.1:8080".to_owned(),
            port: 8765,
            principal: [0x66; 16],
            response_delay: None,
        })
    );
    assert_eq!(
        Invocation::parse([
            BROWSER_SERVICE_ROLE.to_owned(),
            "http://127.0.0.1:8080".to_owned(),
            "8765".to_owned(),
            "66".repeat(16),
            RESPONSE_DELAY_FLAG.to_owned(),
            "4000".to_owned(),
        ]),
        Ok(Invocation::BrowserService {
            origin: "http://127.0.0.1:8080".to_owned(),
            port: 8765,
            principal: [0x66; 16],
            response_delay: Some(BrowserResponseDelay(Duration::from_secs(4))),
        })
    );
}

#[test]
fn permission_probe_roles_preserve_values() {
    let inputs = ["60".to_owned(), "2".to_owned(), "0.2".to_owned()];
    assert_eq!(
        Invocation::parse(
            [WEBVIEW_PERMISSION_PROBE_ROLE.to_owned()]
                .into_iter()
                .chain(inputs.clone())
        ),
        Ok(Invocation::WebViewPermissionProbe(inputs.clone()))
    );
    assert_eq!(
        Invocation::parse(
            [WEBVIEW_PERMISSION_PROBE_FRONTEND_ROLE.to_owned()]
                .into_iter()
                .chain(inputs.clone())
        ),
        Ok(Invocation::WebViewPermissionProbeFrontend(inputs))
    );
}

#[test]
fn permission_probe_capture_roles_preserve_output_and_values() {
    let inputs = ["60".to_owned(), "2".to_owned(), "0.2".to_owned()];
    let output = &capture_path("permission-probe.png");
    assert_eq!(
        Invocation::parse(
            [
                WEBVIEW_PERMISSION_PROBE_CAPTURE_ROLE.to_owned(),
                output.to_owned()
            ]
            .into_iter()
            .chain(inputs.clone())
        ),
        Ok(Invocation::WebViewPermissionProbeCapture {
            output: output.into(),
            inputs: inputs.clone(),
        })
    );
    assert_eq!(
        Invocation::parse(
            [
                WEBVIEW_PERMISSION_PROBE_CAPTURE_FRONTEND_ROLE.to_owned(),
                output.to_owned(),
            ]
            .into_iter()
            .chain(inputs.clone())
        ),
        Ok(Invocation::WebViewPermissionProbeCaptureFrontend {
            output: output.into(),
            inputs,
        })
    );
}

#[test]
fn theme_capture_roles_preserve_mode_output_and_values() {
    let inputs = ["60".to_owned(), "2".to_owned(), "0.2".to_owned()];
    let output = &capture_path("theme-light.png");
    assert_eq!(
        Invocation::parse(
            [
                WEBVIEW_THEME_CAPTURE_ROLE.to_owned(),
                output.to_owned(),
                "light".to_owned(),
            ]
            .into_iter()
            .chain(inputs.clone())
        ),
        Ok(Invocation::WebViewThemeCapture {
            output: output.into(),
            theme: WebViewTheme::Light,
            inputs: inputs.clone(),
        })
    );
    assert_eq!(
        Invocation::parse(
            [
                WEBVIEW_THEME_CAPTURE_FRONTEND_ROLE.to_owned(),
                output.to_owned(),
                "high-contrast".to_owned(),
            ]
            .into_iter()
            .chain(inputs.clone())
        ),
        Ok(Invocation::WebViewThemeCaptureFrontend {
            output: output.into(),
            theme: WebViewTheme::HighContrast,
            inputs,
        })
    );
}

#[test]
fn semantic_capture_role_requires_absolute_json_output() {
    let inputs = ["60".to_owned(), "2".to_owned(), "0.2".to_owned()];
    assert_eq!(
        Invocation::parse(
            [
                SEMANTIC_CAPTURE_ROLE.to_owned(),
                capture_path("semantic.json")
            ]
            .into_iter()
            .chain(inputs.clone())
        ),
        Ok(Invocation::SemanticCapture {
            output: capture_path("semantic.json").into(),
            inputs,
        })
    );
    assert!(
        Invocation::parse(
            [SEMANTIC_CAPTURE_ROLE.to_owned(), "semantic.png".to_owned()]
                .into_iter()
                .chain(["60", "2", "0.2"].map(str::to_owned))
        )
        .is_err()
    );
}

#[test]
fn http_service_accepts_bounded_response_delay() {
    assert_eq!(
        Invocation::parse([
            HTTP_SERVICE_ROLE.to_owned(),
            "http://127.0.0.1:8080".to_owned(),
            "8765".to_owned(),
            "66".repeat(16),
            RESPONSE_DELAY_FLAG.to_owned(),
            "4000".to_owned(),
        ]),
        Ok(Invocation::HttpService {
            origin: "http://127.0.0.1:8080".to_owned(),
            port: 8765,
            principal: [0x66; 16],
            response_delay: Some(BrowserResponseDelay(Duration::from_secs(4))),
        })
    );
}

#[test]
fn malformed_dispatch_never_defaults_to_parent() {
    for arguments in [
        vec![],
        vec![FRONTEND_ROLE],
        vec![NATIVE_WINDOW_ROLE],
        vec![NATIVE_FRONTEND_ROLE],
        vec![WEBVIEW_ROLE],
        vec![WEBVIEW_FRONTEND_ROLE],
        vec![WEBVIEW_PERMISSION_PROBE_ROLE],
        vec![WEBVIEW_PERMISSION_PROBE_FRONTEND_ROLE],
        vec![WEBVIEW_PERMISSION_PROBE_CAPTURE_ROLE],
        vec![WEBVIEW_PERMISSION_PROBE_CAPTURE_FRONTEND_ROLE],
        vec!["--unknown", "2", "0.2"],
        vec![FRONTEND_ROLE, FRONTEND_ROLE, "2", "0.2"],
        vec![NATIVE_WINDOW_ROLE, NATIVE_FRONTEND_ROLE, "2", "0.2"],
        vec![WEBVIEW_ROLE, WEBVIEW_FRONTEND_ROLE, "2", "0.2"],
        vec![
            WEBVIEW_PERMISSION_PROBE_ROLE,
            WEBVIEW_PERMISSION_PROBE_FRONTEND_ROLE,
            "2",
            "0.2",
        ],
        vec![
            WEBVIEW_PERMISSION_PROBE_CAPTURE_ROLE,
            r"C:\captures\permission-probe.png",
            "2",
            "0.2",
        ],
        vec![
            WEBVIEW_PERMISSION_PROBE_CAPTURE_FRONTEND_ROLE,
            r"C:\captures\permission-probe.png",
            "2",
            "0.2",
        ],
        vec![
            WEBVIEW_THEME_CAPTURE_ROLE,
            r"C:\captures\theme.png",
            "invalid",
            "60",
            "2",
            "0.2",
        ],
        vec!["60", "2"],
        vec!["60", "2", "0.2", "extra"],
        vec!["60", "--metis-frontend", "0.2"],
        vec!["--help", "60"],
        vec![
            BROWSER_SERVICE_ROLE,
            "http://127.0.0.1:8080",
            "0",
            &"66".repeat(16),
        ],
        vec![BROWSER_SERVICE_ROLE, "http://127.0.0.1:8080", "8765", "00"],
        vec![
            BROWSER_SERVICE_ROLE,
            "http://127.0.0.1:8080",
            "8765",
            &"66".repeat(16),
            RESPONSE_DELAY_FLAG,
        ],
        vec![
            BROWSER_SERVICE_ROLE,
            "http://127.0.0.1:8080",
            "8765",
            &"66".repeat(16),
            RESPONSE_DELAY_FLAG,
            "0",
        ],
        vec![
            BROWSER_SERVICE_ROLE,
            "http://127.0.0.1:8080",
            "8765",
            &"66".repeat(16),
            RESPONSE_DELAY_FLAG,
            "30001",
        ],
        vec![
            BROWSER_SERVICE_ROLE,
            "http://127.0.0.1:8080",
            "8765",
            &"66".repeat(16),
            "--unexpected",
            "4000",
        ],
        vec![
            HTTP_SERVICE_ROLE,
            "http://127.0.0.1:8080",
            "8765",
            &"66".repeat(16),
            RESPONSE_DELAY_FLAG,
            "0",
        ],
    ] {
        assert_eq!(
            Invocation::parse(arguments.into_iter().map(str::to_owned)),
            Err(InvocationError)
        );
    }
}

#[test]
#[cfg(windows)]
fn malformed_unicode_argument_returns_an_error() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    let malformed = OsString::from_wide(&[0xd800]);
    assert_eq!(
        Invocation::parse([malformed, "2".into(), "0.2".into()]),
        Err(InvocationError)
    );
}
