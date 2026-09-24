//! Validation of the custom URL schemes an application registers.

use crate::Result;
use metis_core::deep_link::DeepLinkScheme;

/// Most schemes one application may register.
pub(crate) const URL_SCHEME_LIMIT: usize = 8;

/// Admits each scheme under the rule the application's link parser applies,
/// so the installers never register a scheme the application would refuse.
pub(super) fn validate(schemes: &[String]) -> Result<()> {
    if schemes.len() > URL_SCHEME_LIMIT {
        return Err("an application registers at most 8 URL schemes".into());
    }
    for (index, scheme) in schemes.iter().enumerate() {
        DeepLinkScheme::new(scheme).map_err(|error| format!("url_schemes[{index}]: {error}"))?;
        if schemes[..index].contains(scheme) {
            return Err(format!("url_schemes[{index}] repeats {scheme}").into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;

    #[test]
    fn schemes_follow_the_link_parser_rule() {
        let valid =
            |values: &[&str]| validate(&values.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>());
        assert!(valid(&[]).is_ok());
        assert!(valid(&["org.atlas.viewer", "atlas-viewer"]).is_ok());
        assert!(valid(&["https"]).is_err());
        assert!(valid(&["Viewer"]).is_err());
        assert!(valid(&["viewer", "viewer"]).is_err());
        assert!(valid(&["a", "b", "c", "d", "e", "f", "g", "h", "i"]).is_err());
    }
}
