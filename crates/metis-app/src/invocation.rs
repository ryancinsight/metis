//! Closed process roles; argument selection never grants session authority.

pub(crate) const FRONTEND_ROLE: &str = "--metis-frontend";
pub(crate) const USAGE: &str =
    "usage: metis-app WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN\n       metis-app --help";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Invocation {
    Backend([String; 3]),
    Frontend([String; 3]),
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

#[cfg(test)]
mod tests {
    use super::{FRONTEND_ROLE, Invocation, InvocationError};

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
