//! Recognized query values have a finite domain; unknown values preserve markup.

use crate::Theme;
use std::io;

/// The longest declared theme spelling bounds every recognizable query value.
pub(super) const QUERY_BYTES: usize = Theme::HighContrast.css_value().len();

pub(super) fn requested(value: io::Result<Option<String>>) -> io::Result<Option<Theme>> {
    match value {
        Ok(value) => Ok(value.as_deref().and_then(Theme::parse)),
        // PAL reports InvalidInput only when the decoded value exceeds the
        // supplied bound. Such a value cannot match any declared theme name.
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{QUERY_BYTES, requested};
    use crate::Theme;
    use std::io;

    fn trace() -> tracing::subscriber::DefaultGuard {
        tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
                .with_test_writer()
                .finish(),
        )
    }

    #[test]
    fn each_recognized_query_retains_its_theme_and_unknowns_have_no_override() {
        let _trace = trace();
        let _span = tracing::info_span!("assert", contract = "finite theme domain").entered();
        for theme in [
            Theme::System,
            Theme::Light,
            Theme::Dark,
            Theme::HighContrast,
        ] {
            assert!(theme.css_value().len() <= QUERY_BYTES);
            assert_eq!(
                requested(Ok(Some(theme.css_value().into()))).expect("recognized query"),
                Some(theme)
            );
        }
        for value in [
            None,
            Some(String::new()),
            Some("Dark".into()),
            Some("sepia".into()),
        ] {
            assert_eq!(requested(Ok(value)).expect("unknown query"), None);
        }
    }

    #[test]
    fn a_query_beyond_the_recognized_bound_is_unknown_and_other_errors_keep_context() {
        let _trace = trace();
        let _span = tracing::info_span!("act", contract = "bounded parameter outcome").entered();
        // WebLocation::parameter's sole InvalidInput is length > caller bound.
        // No string longer than max declared spelling can name a valid Theme.
        let too_long = io::Error::new(
            io::ErrorKind::InvalidInput,
            "Browser query value exceeds its byte bound",
        );
        assert_eq!(
            requested(Err(too_long)).expect("oversized unknown query"),
            None
        );
        for kind in [
            io::ErrorKind::Unsupported,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::Other,
        ] {
            let expected = format!("actual page observation failure: {kind:?}");
            let actual = requested(Err(io::Error::new(kind, expected.clone())))
                .expect_err("non-domain failure");
            assert_eq!(actual.kind(), kind);
            assert_eq!(actual.to_string(), expected);
        }
    }
}
