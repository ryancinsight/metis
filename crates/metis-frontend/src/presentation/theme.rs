//! Theme palette projection for the authored software-rendered surface.

use crate::app::FrontendApp;
use crate::commands::ApplicationTheme;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::IpcTransport;
use metis_ui_lang::Color;

impl<T: IpcTransport> FrontendApp<T> {
    pub(super) fn apply_theme(&mut self) -> Result<()> {
        let palette = match self.theme {
            ApplicationTheme::System => ThemePalette {
                page: Color::rgb(240, 244, 248),
                surface: Color::WHITE,
                header: Color::DARK_BLUE,
                text: Color::rgb(45, 55, 72),
                muted: Color::rgb(74, 85, 104),
                border: Color::rgb(226, 232, 240),
                accent: Color::BLUE,
                status: Color::GREEN,
            },
            ApplicationTheme::Dark => ThemePalette {
                page: Color::rgb(15, 23, 42),
                surface: Color::rgb(30, 41, 59),
                header: Color::rgb(51, 65, 85),
                text: Color::rgb(226, 232, 240),
                muted: Color::rgb(186, 230, 253),
                border: Color::rgb(100, 116, 139),
                accent: Color::rgb(8, 145, 178),
                status: Color::rgb(103, 232, 249),
            },
        };
        self.set_background("main-screen", palette.page)?;
        self.set_background("header", palette.header)?;
        self.set_background("application-navigation", palette.border)?;
        self.set_background("command-menu", palette.surface)?;
        self.set_background("patient-card", palette.surface)?;
        self.set_background("results-card", palette.surface)?;
        for id in ["patient-card", "results-card"] {
            self.set_border(id, palette.border)?;
        }
        self.set_border("command-menu", palette.muted)?;
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
        self.set_text_color("status-badge", palette.status)?;
        self.set_text_color("output-rate", palette.accent)?;
        self.set_text_color("header", Color::WHITE)?;
        self.set_text_color("patient-card", palette.text)?;
        self.set_text_color("results-card", palette.text)?;
        for id in ["command-menu-toggle", "command-focus-patient"] {
            self.set_background(id, palette.accent)?;
            self.set_text_color(id, Color::WHITE)?;
        }
        for id in ["command-theme-dark", "command-theme-system"] {
            self.set_background(id, palette.surface)?;
        }
        for id in ["command-theme-dark-label", "command-theme-system-label"] {
            self.set_text_color(id, palette.text)?;
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

#[derive(Clone, Copy)]
struct ThemePalette {
    page: Color,
    surface: Color,
    header: Color,
    text: Color,
    muted: Color,
    border: Color,
    accent: Color,
    status: Color,
}
