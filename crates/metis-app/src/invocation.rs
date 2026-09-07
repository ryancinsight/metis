//! Closed process roles; argument selection never grants session authority.

pub(crate) const FRONTEND_ROLE: &str = "--metis-frontend";
pub(crate) const BROWSER_SERVICE_ROLE: &str = "--metis-browser-service";
pub(crate) const USAGE: &str = "usage: metis-app WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --metis-browser-service ORIGIN PORT PRINCIPAL_HEX\n       metis-app --help";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Invocation {
    Backend([String; 3]),
    Frontend([String; 3]),
    BrowserService {
        origin: String,
        port: u16,
        principal: [u8; 16],
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
            if arguments.next().is_some() {
                return Err(InvocationError);
            }
            return Ok(Self::BrowserService {
                origin,
                port,
                principal,
            });
        }
        let child = first == FRONTEND_ROLE;
        let weight = if child {
            arguments.next().ok_or(InvocationError)??
        } else {
            first
        };
        let concentration = arguments.next().ok_or(InvocationError)??;
        let dose = arguments.next().ok_or(InvocationError)??;
        let inputs = [weight, concentration, dose];
        if arguments.next().is_some() || inputs.iter().any(|input| input.starts_with("--")) {
            return Err(InvocationError);
        }
        Ok(if child {
            Self::Frontend(inputs)
        } else {
            Self::Backend(inputs)
        })
    }
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
    use super::{BROWSER_SERVICE_ROLE, FRONTEND_ROLE, Invocation, InvocationError};

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
            })
        );
    }

    #[test]
    fn malformed_dispatch_never_defaults_to_parent() {
        for arguments in [
            vec![],
            vec![FRONTEND_ROLE],
            vec!["--unknown", "2", "0.2"],
            vec![FRONTEND_ROLE, FRONTEND_ROLE, "2", "0.2"],
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
