//! Theme palette projection for the authored software-rendered surface.

use crate::app::FrontendApp;
use crate::commands::ApplicationTheme;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::IpcTransport;
use metis_platform::rasterizer::GradientStop;
use metis_ui_lang::{Color, LinearGradient};

impl<T: IpcTransport> FrontendApp<T> {
    pub(super) fn apply_theme(&mut self) -> Result<()> {
        let palette = ThemePalette::for_theme(self.theme);
        self.set_background("main-screen", palette.page)?;
        self.set_gradient("header", &palette.header)?;
        self.set_background("application-navigation", palette.panel)?;
        self.set_background("command-menu", palette.surface)?;
        self.set_background("patient-card", palette.surface)?;
        self.set_background("results-card", palette.surface)?;
        for id in ["patient-card", "results-card", "command-menu"] {
            self.set_border(id, palette.border)?;
        }
        for id in [
            "label-patient",
            "label-weight",
            "label-conc",
            "label-dose",
            "command-status",
        ] {
            self.set_text_color(id, palette.muted)?;
        }
        for id in ["patient-heading", "results-heading"] {
            self.set_text_color(id, palette.text)?;
        }
        for id in ["output-status", "output-signature"] {
            self.set_text_color(id, palette.muted)?;
        }
        self.set_text_color("output-rate", palette.accent)?;
        self.set_text_color("header", Color::WHITE)?;
        self.set_text_color("patient-card", palette.text)?;
        self.set_text_color("results-card", palette.text)?;
        for id in [
            "command-menu-toggle",
            "command-focus-patient",
            "command-theme-dark",
            "command-theme-system",
            "btn-calc",
        ] {
            self.set_gradient(id, &palette.control)?;
            self.set_text_color(id, Color::WHITE)?;
        }
        Ok(())
    }

    fn set_background(&mut self, id: &str, color: Color) -> Result<()> {
        self.doc
            .find_element_by_id_mut(id)
            .ok_or_else(|| {
                MetisError::ui(
                    ErrorCode::MalformedMarkup,
                    format!("Authored form is missing styled element {id}"),
                )
            })?
            .computed_style
            .background_color = Some(color);
        Ok(())
    }

    /// Replaces the element's background with `gradient`, as the
    /// `background` shorthand does.
    fn set_gradient(&mut self, id: &str, gradient: &LinearGradient) -> Result<()> {
        let style = &mut self
            .doc
            .find_element_by_id_mut(id)
            .ok_or_else(|| {
                MetisError::ui(
                    ErrorCode::MalformedMarkup,
                    format!("Authored form is missing styled element {id}"),
                )
            })?
            .computed_style;
        style.background_color = None;
        style.background_gradient = Some(gradient.clone());
        Ok(())
    }

    fn set_border(&mut self, id: &str, color: Color) -> Result<()> {
        self.doc
            .find_element_by_id_mut(id)
            .ok_or_else(|| {
                MetisError::ui(
                    ErrorCode::MalformedMarkup,
                    format!("Authored form is missing bordered element {id}"),
                )
            })?
            .computed_style
            .border_color = color;
        Ok(())
    }

    fn set_text_color(&mut self, id: &str, color: Color) -> Result<()> {
        self.doc
            .find_element_by_id_mut(id)
            .ok_or_else(|| {
                MetisError::ui(
                    ErrorCode::MalformedMarkup,
                    format!("Authored form is missing colored element {id}"),
                )
            })?
            .computed_style
            .text_color = color;
        Ok(())
    }
}

/// A two-stop gradient between `from` and `to` at `degrees`.
fn ramp(degrees: f64, from: Color, to: Color) -> LinearGradient {
    let stops = [from, to].map(|color| GradientStop {
        color,
        position: None,
    });
    LinearGradient::new(degrees, &stops)
        .expect("invariant: two stops at a finite angle form a gradient")
}

/// Theme colors. Every text color meets WCAG 2.2 criterion 1.4.3 on the
/// fill it is painted over, as the theme contrast tests assert.
pub(super) struct ThemePalette {
    page: Color,
    surface: Color,
    header: LinearGradient,
    text: Color,
    muted: Color,
    border: Color,
    /// Fill of the command bar, which carries muted status text.
    panel: Color,
    /// Rate readout color.
    accent: Color,
    /// Keyboard focus ring; at least 3:1 against every fill it can border
    /// (WCAG 2.2 criterion 1.4.11).
    pub(super) focus: Color,
    /// Command control fill; every stop keeps white labels at a 4.5:1
    /// contrast or better (WCAG 2.2 criterion 1.4.3).
    control: LinearGradient,
}

impl ThemePalette {
    pub(super) fn for_theme(theme: ApplicationTheme) -> Self {
        match theme {
            ApplicationTheme::System => ThemePalette {
                page: Color::rgb(240, 244, 248),
                surface: Color::WHITE,
                header: ramp(135.0, Color::DARK_BLUE, Color::rgb(44, 82, 130)),
                text: Color::rgb(45, 55, 72),
                muted: Color::rgb(74, 85, 104),
                border: Color::rgb(226, 232, 240),
                panel: Color::rgb(226, 232, 240),
                accent: Color::rgb(43, 108, 176),
                focus: Color::rgb(43, 108, 176),
                control: ramp(180.0, Color::rgb(44, 120, 196), Color::rgb(38, 98, 168)),
            },
            ApplicationTheme::Dark => ThemePalette {
                page: Color::rgb(15, 23, 42),
                surface: Color::rgb(30, 41, 59),
                header: ramp(135.0, Color::rgb(51, 65, 85), Color::rgb(38, 50, 70)),
                text: Color::rgb(226, 232, 240),
                muted: Color::rgb(186, 230, 253),
                border: Color::rgb(100, 116, 139),
                panel: Color::rgb(39, 52, 72),
                accent: Color::rgb(56, 189, 248),
                focus: Color::rgb(56, 189, 248),
                control: ramp(180.0, Color::rgb(14, 116, 144), Color::rgb(21, 94, 117)),
            },
        }
    }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
