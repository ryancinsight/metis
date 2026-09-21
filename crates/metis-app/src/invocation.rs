//! Closed process roles; argument selection never grants session authority.

use std::{path::PathBuf, time::Duration};

pub(crate) const FRONTEND_ROLE: &str = "--metis-frontend";
pub(crate) const NATIVE_WINDOW_ROLE: &str = "--metis-native-window";
pub(crate) const NATIVE_FRONTEND_ROLE: &str = "--metis-native-frontend";
pub(crate) const WEBVIEW_ROLE: &str = "--metis-webview";
pub(crate) const WEBVIEW_FRONTEND_ROLE: &str = "--metis-webview-frontend";
pub(crate) const WEBVIEW_PERMISSION_PROBE_ROLE: &str = "--metis-webview-permission-probe";
pub(crate) const WEBVIEW_PERMISSION_PROBE_FRONTEND_ROLE: &str =
    "--metis-webview-permission-probe-frontend";
pub(crate) const WEBVIEW_PERMISSION_PROBE_CAPTURE_ROLE: &str =
    "--metis-webview-permission-probe-capture";
pub(crate) const WEBVIEW_PERMISSION_PROBE_CAPTURE_FRONTEND_ROLE: &str =
    "--metis-webview-permission-probe-capture-frontend";
pub(crate) const WEBVIEW_THEME_CAPTURE_ROLE: &str = "--metis-webview-theme-capture";
pub(crate) const WEBVIEW_THEME_CAPTURE_FRONTEND_ROLE: &str =
    "--metis-webview-theme-capture-frontend";
pub(crate) const SEMANTIC_CAPTURE_ROLE: &str = "--metis-semantic-capture";
pub(crate) const BROWSER_SERVICE_ROLE: &str = "--metis-browser-service";
pub(crate) const HTTP_SERVICE_ROLE: &str = "--metis-http-service";
pub(crate) const RESPONSE_DELAY_FLAG: &str = "--response-delay-ms";
const MAX_RESPONSE_DELAY_MILLISECONDS: u64 = 30_000;
pub(crate) const USAGE: &str = "usage: metis-app WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-native-window WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-webview WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-webview-permission-probe WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-webview-permission-probe-capture OUTPUT_PNG WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-webview-theme-capture OUTPUT_PNG THEME WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-semantic-capture OUTPUT_JSON WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-browser-service ORIGIN PORT PRINCIPAL_HEX [--response-delay-ms MILLISECONDS]\n       metis-app --metis-http-service ORIGIN PORT PRINCIPAL_HEX [--response-delay-ms MILLISECONDS]\n       metis-app --help";

/// Presentation mode requested by the packaged `WebView2` capture command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WebViewTheme {
    System,
    Light,
    Dark,
    HighContrast,
}

impl WebViewTheme {
    fn parse(value: &str) -> Result<Self, InvocationError> {
        match value {
            "system" => Ok(Self::System),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            "high-contrast" => Ok(Self::HighContrast),
            _ => Err(InvocationError),
        }
    }

    pub(crate) const fn query_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high-contrast",
        }
    }
}

/// Bounded delay used by the browser stale-response conformance probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BrowserResponseDelay(Duration);

impl BrowserResponseDelay {
    fn parse(value: &str) -> Result<Self, InvocationError> {
        let milliseconds = value
            .parse::<u64>()
            .ok()
            .filter(|milliseconds| (1..=MAX_RESPONSE_DELAY_MILLISECONDS).contains(milliseconds))
            .ok_or(InvocationError)?;
        Ok(Self(Duration::from_millis(milliseconds)))
    }

    pub(crate) const fn duration(self) -> Duration {
        self.0
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Invocation {
    Backend([String; 3]),
    Frontend([String; 3]),
    NativeWindow([String; 3]),
    NativeFrontend([String; 3]),
    WebView([String; 3]),
    WebViewFrontend([String; 3]),
    WebViewPermissionProbe([String; 3]),
    WebViewPermissionProbeFrontend([String; 3]),
    WebViewPermissionProbeCapture {
        output: PathBuf,
        inputs: [String; 3],
    },
    WebViewPermissionProbeCaptureFrontend {
        output: PathBuf,
        inputs: [String; 3],
    },
    WebViewThemeCapture {
        output: PathBuf,
        theme: WebViewTheme,
        inputs: [String; 3],
    },
    WebViewThemeCaptureFrontend {
        output: PathBuf,
        theme: WebViewTheme,
        inputs: [String; 3],
    },
    SemanticCapture {
        output: PathBuf,
        inputs: [String; 3],
    },
    BrowserService {
        origin: String,
        port: u16,
        principal: [u8; 16],
        response_delay: Option<BrowserResponseDelay>,
    },
    HttpService {
        origin: String,
        port: u16,
        principal: [u8; 16],
        response_delay: Option<BrowserResponseDelay>,
    },
    Help,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InvocationError;

impl std::fmt::Display for InvocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(USAGE)
    }
}

impl std::error::Error for InvocationError {}

impl Invocation {
    pub(crate) fn parse(
        arguments: impl IntoIterator<Item = impl Into<std::ffi::OsString>>,
    ) -> Result<Self, InvocationError> {
        // At most one role selector and three values are admitted. Stop reading
        // after the fifth argument so an arbitrary iterator cannot grow storage.
        let mut arguments = arguments
            .into_iter()
            .map(|value| value.into().into_string().map_err(|_| InvocationError));
        let first = arguments.next().ok_or(InvocationError)??;
        if first == "--help" {
            return if arguments.next().is_none() {
                Ok(Self::Help)
            } else {
                Err(InvocationError)
            };
        }
        if first == BROWSER_SERVICE_ROLE || first == HTTP_SERVICE_ROLE {
            return parse_service(&first, &mut arguments);
        }
        parse_role(first, &mut arguments)
    }
}

fn parse_service(
    role: &str,
    arguments: &mut impl Iterator<Item = Result<String, InvocationError>>,
) -> Result<Invocation, InvocationError> {
    let origin = arguments.next().ok_or(InvocationError)??;
    let port = arguments
        .next()
        .ok_or(InvocationError)??
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or(InvocationError)?;
    let principal = parse_principal(&arguments.next().ok_or(InvocationError)??)?;
    let response_delay = match arguments.next() {
        None => None,
        Some(flag) => {
            if flag? != RESPONSE_DELAY_FLAG {
                return Err(InvocationError);
            }
            Some(BrowserResponseDelay::parse(
                &arguments.next().ok_or(InvocationError)??,
            )?)
        }
    };
    if arguments.next().is_some() {
        return Err(InvocationError);
    }
    if role == BROWSER_SERVICE_ROLE {
        Ok(Invocation::BrowserService {
            origin,
            port,
            principal,
            response_delay,
        })
    } else {
        Ok(Invocation::HttpService {
            origin,
            port,
            principal,
            response_delay,
        })
    }
}

fn parse_role(
    first: String,
    arguments: &mut impl Iterator<Item = Result<String, InvocationError>>,
) -> Result<Invocation, InvocationError> {
    let role = match first.as_str() {
        FRONTEND_ROLE => InputRole::Frontend,
        NATIVE_WINDOW_ROLE => InputRole::NativeWindow,
        NATIVE_FRONTEND_ROLE => InputRole::NativeFrontend,
        WEBVIEW_ROLE => InputRole::WebView,
        WEBVIEW_FRONTEND_ROLE => InputRole::WebViewFrontend,
        WEBVIEW_PERMISSION_PROBE_ROLE => InputRole::WebViewPermissionProbe,
        WEBVIEW_PERMISSION_PROBE_FRONTEND_ROLE => InputRole::WebViewPermissionProbeFrontend,
        WEBVIEW_PERMISSION_PROBE_CAPTURE_ROLE => InputRole::WebViewPermissionProbeCapture,
        WEBVIEW_PERMISSION_PROBE_CAPTURE_FRONTEND_ROLE => {
            InputRole::WebViewPermissionProbeCaptureFrontend
        }
        WEBVIEW_THEME_CAPTURE_ROLE => InputRole::WebViewThemeCapture,
        WEBVIEW_THEME_CAPTURE_FRONTEND_ROLE => InputRole::WebViewThemeCaptureFrontend,
        SEMANTIC_CAPTURE_ROLE => InputRole::SemanticCapture,
        _ => InputRole::Backend,
    };
    let capture_output = if matches!(
        role,
        InputRole::WebViewPermissionProbeCapture
            | InputRole::WebViewPermissionProbeCaptureFrontend
            | InputRole::WebViewThemeCapture
            | InputRole::WebViewThemeCaptureFrontend
            | InputRole::SemanticCapture
    ) {
        let value = arguments.next().ok_or(InvocationError)??;
        Some(if role == InputRole::SemanticCapture {
            parse_json_capture_output(&value)?
        } else {
            parse_capture_output(&value)?
        })
    } else {
        None
    };
    let theme = if matches!(
        role,
        InputRole::WebViewThemeCapture | InputRole::WebViewThemeCaptureFrontend
    ) {
        Some(WebViewTheme::parse(
            &arguments.next().ok_or(InvocationError)??,
        )?)
    } else {
        None
    };
    let weight = if role == InputRole::Backend {
        first
    } else {
        arguments.next().ok_or(InvocationError)??
    };
    let concentration = arguments.next().ok_or(InvocationError)??;
    let dose = arguments.next().ok_or(InvocationError)??;
    let inputs = [weight, concentration, dose];
    if arguments.next().is_some() || inputs.iter().any(|input| input.starts_with("--")) {
        return Err(InvocationError);
    }
    Ok(match role {
        InputRole::Backend => Invocation::Backend(inputs),
        InputRole::Frontend => Invocation::Frontend(inputs),
        InputRole::NativeWindow => Invocation::NativeWindow(inputs),
        InputRole::NativeFrontend => Invocation::NativeFrontend(inputs),
        InputRole::WebView => Invocation::WebView(inputs),
        InputRole::WebViewFrontend => Invocation::WebViewFrontend(inputs),
        InputRole::WebViewPermissionProbe => Invocation::WebViewPermissionProbe(inputs),
        InputRole::WebViewPermissionProbeFrontend => {
            Invocation::WebViewPermissionProbeFrontend(inputs)
        }
        InputRole::WebViewPermissionProbeCapture => Invocation::WebViewPermissionProbeCapture {
            output: capture_output.expect("invariant: capture role has an output path"),
            inputs,
        },
        InputRole::WebViewPermissionProbeCaptureFrontend => {
            Invocation::WebViewPermissionProbeCaptureFrontend {
                output: capture_output.expect("invariant: capture role has an output path"),
                inputs,
            }
        }
        InputRole::WebViewThemeCapture => Invocation::WebViewThemeCapture {
            output: capture_output.expect("invariant: capture role has an output path"),
            theme: theme.expect("invariant: theme capture role has a theme"),
            inputs,
        },
        InputRole::WebViewThemeCaptureFrontend => Invocation::WebViewThemeCaptureFrontend {
            output: capture_output.expect("invariant: capture role has an output path"),
            theme: theme.expect("invariant: theme capture role has a theme"),
            inputs,
        },
        InputRole::SemanticCapture => Invocation::SemanticCapture {
            output: capture_output.expect("invariant: semantic capture role has an output path"),
            inputs,
        },
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputRole {
    Backend,
    Frontend,
    NativeWindow,
    NativeFrontend,
    WebView,
    WebViewFrontend,
    WebViewPermissionProbe,
    WebViewPermissionProbeFrontend,
    WebViewPermissionProbeCapture,
    WebViewPermissionProbeCaptureFrontend,
    WebViewThemeCapture,
    WebViewThemeCaptureFrontend,
    SemanticCapture,
}

fn parse_capture_output(value: &str) -> Result<PathBuf, InvocationError> {
    let path = PathBuf::from(value);
    let units = value.encode_utf16().count();
    if value.is_empty()
        || units > 2 * 1024
        || !path.is_absolute()
        || path.extension().and_then(|extension| extension.to_str()) != Some("png")
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(InvocationError);
    }
    Ok(path)
}

fn parse_json_capture_output(value: &str) -> Result<PathBuf, InvocationError> {
    let path = PathBuf::from(value);
    let units = value.encode_utf16().count();
    if value.is_empty()
        || units > 2 * 1024
        || !path.is_absolute()
        || path.extension().and_then(|extension| extension.to_str()) != Some("json")
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(InvocationError);
    }
    Ok(path)
}

fn parse_principal(value: &str) -> Result<[u8; 16], InvocationError> {
    let bytes = value.as_bytes();
    if bytes.len() != 32 || !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err(InvocationError);
    }
    let mut principal = [0; 16];
    for (index, slot) in principal.iter_mut().enumerate() {
        let high = hex_digit(bytes[index * 2])?;
        let low = hex_digit(bytes[index * 2 + 1])?;
        *slot = (high << 4) | low;
    }
    if principal == [0; 16] {
        return Err(InvocationError);
    }
    Ok(principal)
}

fn hex_digit(value: u8) -> Result<u8, InvocationError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(InvocationError),
    }
}

#[cfg(test)]
mod tests;
