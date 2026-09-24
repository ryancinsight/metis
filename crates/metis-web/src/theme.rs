//! Theme modes shared by the browser host and application stylesheets.

/// A bounded presentation mode applied to the browser document.
///
/// The mode selects the semantic CSS variable set supplied by the host. An
/// application can retain the mode contract and replace those variables in its
/// own stylesheet without changing Rust state or event handling.
///
/// # Examples
///
/// ```
/// use metis_web::Theme;
///
/// let mode = Theme::parse("dark").expect("dark is a declared theme mode");
/// assert_eq!(mode.css_value(), "dark");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    /// Follow the browser's `prefers-color-scheme` preference.
    System,
    /// Use the light semantic palette.
    Light,
    /// Use the dark semantic palette.
    Dark,
    /// Use the high-contrast semantic palette.
    HighContrast,
}

impl Theme {
    /// The default mode used by a newly mounted browser application.
    pub const DEFAULT: Self = Self::System;

    /// Parses the stable HTML option value for a theme mode.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "high-contrast" => Some(Self::HighContrast),
            _ => None,
        }
    }

    /// Returns the value used by the document's `data-metis-theme` attribute.
    #[must_use]
    pub const fn css_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high-contrast",
        }
    }

    /// Every mode, in the order the theme control lists them.
    pub const ALL: [Self; 4] = [Self::System, Self::Light, Self::Dark, Self::HighContrast];

    /// Returns the label the theme control shows for this mode.
    #[must_use]
    pub const fn option_label(self) -> &'static str {
        match self {
            Self::System => "System preference",
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::HighContrast => "High contrast",
        }
    }

    /// Returns the `<option>` list for a theme `<select>` with `selected`
    /// marked, so a host can reflect a mode chosen outside the control.
    #[must_use]
    pub fn options_markup(selected: Self) -> String {
        let mut markup = String::with_capacity(192);
        for mode in Self::ALL {
            markup.push_str("<option value=\"");
            markup.push_str(mode.css_value());
            markup.push('"');
            if mode == selected {
                markup.push_str(" selected");
            }
            markup.push('>');
            markup.push_str(mode.option_label());
            markup.push_str("</option>");
        }
        markup
    }

    /// Returns the reader-facing label for this mode.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "system preference",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high contrast",
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::Theme;

    #[test]
    fn parses_each_bounded_mode_and_preserves_css_value() {
        for (value, expected) in [
            ("system", Theme::System),
            ("light", Theme::Light),
            ("dark", Theme::Dark),
            ("high-contrast", Theme::HighContrast),
        ] {
            assert_eq!(Theme::parse(value), Some(expected));
            assert_eq!(expected.css_value(), value);
        }
        assert_eq!(Theme::default(), Theme::System);
    }

    #[test]
    fn options_markup_lists_every_mode_and_marks_one() {
        let markup = Theme::options_markup(Theme::Dark);
        assert_eq!(markup.matches("<option ").count(), Theme::ALL.len());
        assert_eq!(markup.matches(" selected").count(), 1);
        assert!(markup.contains(r#"<option value="dark" selected>Dark</option>"#));
        assert!(markup.contains(r#"<option value="system">System preference</option>"#));
        // The mounted markup starts from the same options with the default
        // selected, so the control and the declaration cannot drift.
        let initial = Theme::options_markup(Theme::DEFAULT);
        let authored = include_str!("controls.rs");
        for option in initial.split_inclusive("</option>") {
            assert!(authored.contains(option), "{option}");
        }
    }

    #[test]
    fn rejects_unknown_theme_values() {
        for value in ["", "sepia", "Dark", "high_contrast"] {
            assert_eq!(Theme::parse(value), None, "accepted {value:?}");
        }
    }
}
