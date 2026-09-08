//! Closed process roles; argument selection never grants session authority.

use std::time::Duration;

pub(crate) const FRONTEND_ROLE: &str = "--metis-frontend";
pub(crate) const NATIVE_WINDOW_ROLE: &str = "--metis-native-window";
pub(crate) const NATIVE_FRONTEND_ROLE: &str = "--metis-native-frontend";
pub(crate) const BROWSER_SERVICE_ROLE: &str = "--metis-browser-service";
pub(crate) const RESPONSE_DELAY_FLAG: &str = "--response-delay-ms";
const MAX_RESPONSE_DELAY_MILLISECONDS: u64 = 30_000;
pub(crate) const USAGE: &str = "usage: metis-app WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-native-window WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-browser-service ORIGIN PORT PRINCIPAL_HEX [--response-delay-ms MILLISECONDS]\n       metis-app --help";

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
    BrowserService {
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
        if first == BROWSER_SERVICE_ROLE {
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
            return Ok(Self::BrowserService {
                origin,
                port,
                principal,
                response_delay,
            });
        }
        let role = match first.as_str() {
            FRONTEND_ROLE => InputRole::Frontend,
            NATIVE_WINDOW_ROLE => InputRole::NativeWindow,
            NATIVE_FRONTEND_ROLE => InputRole::NativeFrontend,
            _ => InputRole::Backend,
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
            InputRole::Backend => Self::Backend(inputs),
            InputRole::Frontend => Self::Frontend(inputs),
            InputRole::NativeWindow => Self::NativeWindow(inputs),
            InputRole::NativeFrontend => Self::NativeFrontend(inputs),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputRole {
    Backend,
    Frontend,
    NativeWindow,
    NativeFrontend,
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
mod tests {
    use super::{
        BROWSER_SERVICE_ROLE, BrowserResponseDelay, FRONTEND_ROLE, Invocation, InvocationError,
        NATIVE_FRONTEND_ROLE, NATIVE_WINDOW_ROLE, RESPONSE_DELAY_FLAG,
    };
    use std::time::Duration;

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
    fn malformed_dispatch_never_defaults_to_parent() {
        for arguments in [
            vec![],
            vec![FRONTEND_ROLE],
            vec![NATIVE_WINDOW_ROLE],
            vec![NATIVE_FRONTEND_ROLE],
            vec!["--unknown", "2", "0.2"],
            vec![FRONTEND_ROLE, FRONTEND_ROLE, "2", "0.2"],
            vec![NATIVE_WINDOW_ROLE, NATIVE_FRONTEND_ROLE, "2", "0.2"],
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
}
