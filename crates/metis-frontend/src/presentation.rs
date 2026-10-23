//! Form markup and projection of owned application state.

pub mod cache;
#[cfg(test)]
mod cache_tests;
pub mod command;
mod focus_ring;
pub mod form_text;
pub mod format;
pub mod outcome;
pub mod render;
#[cfg(test)]
mod repaint_tests;
pub mod template;
mod theme;
mod transition;
#[cfg(test)]
mod transition_tests;

use crate::FrontendApp;
use metis_core::{ErrorCode, MetisError, Result};
use metis_ipc::IpcTransport;
use metis_platform::{Damage, Framebuffer, MotionPreference};
use metis_ui_lang::{Color, DisplayList, LayoutViewport};
use std::time::Duration;

pub(crate) use cache::RenderCache;
pub(crate) use form_text::{found, written};
pub(crate) use format::write_result_number;
pub use template::CLINICAL_SCREEN_XML;
pub(crate) use transition::Transitions;

/// Status badge color while a backend session is open.
///
/// Legible at 4.5:1 or better over either theme's header gradient (WCAG 2.2
/// criterion 1.4.3), as the theme contrast tests assert.
pub const BADGE_READY: Color = Color::rgb(154, 230, 180);
/// Status badge color once the backend session has closed, legible over
/// either theme's header like [`BADGE_READY`].
pub const BADGE_CLOSED: Color = Color::rgb(254, 178, 178);

impl<T: IpcTransport> FrontendApp<T> {
    /// Projects the owned state and renders the complete form.
    ///
    /// A render of unchanged state keeps the document, its semantic
    /// projection and its layout; the only memory it requests is the new
    /// display list's command storage.
    /// # Errors
    /// Rejects invalid layout or a missing authored label before changing pixels.
    pub fn render(&mut self) -> Result<()> {
        let themed = self.apply_theme()?;
        let badge = if self.client.is_none() {
            "SESSION CLOSED"
        } else if self
            .client
            .as_ref()
            .and_then(metis_ipc::IpcClient::active_token)
            .is_some()
        {
            "SESSION ACTIVE"
        } else {
            "SYSTEM READY"
        };
        crate::presentation::form_text::set_text(&mut self.doc, "status-badge", badge)?;
        self.render_command_surface()?;
        let badge_color = if self.client.is_none() {
            crate::BADGE_CLOSED
        } else {
            crate::BADGE_READY
        };
        crate::presentation::form_text::found(
            self.doc
                .restyle("status-badge", |style| style.text_color = badge_color),
            || "Authored form is missing status badge".to_owned(),
        )?;
        self.render_form_text()?;
        // Validate the custom renderer's host-neutral semantics before
        // painting so a malformed identity or action cannot be presented as
        // an accessible control; the same projection decides focus.
        let order = self.cache.focusable(&self.doc)?;
        self.focus.reconcile(&self.doc, order)?;
        let width = i32::try_from(self.framebuffer.width()).map_err(|_| layout_error())?;
        let height = i32::try_from(self.framebuffer.height()).map_err(|_| layout_error())?;
        let mut display = self.cache.frame(
            &self.doc,
            LayoutViewport::with_scale(width, height, self.display_scale),
        )?;
        self.append_focus_ring(&mut display)?;
        // A declared transition is sampled here, at the paint boundary: the
        // document, its semantics and its layout are already final, so only
        // the colors of the animated element's commands move. A palette change
        // recolors every command, so it settles instead of interpolating.
        let motion = if themed {
            MotionPreference::Reduced
        } else {
            self.motion
        };
        self.transitions
            .observe(&display, self.painted.as_ref(), motion, self.now);
        self.transitions.apply(&mut display, self.now);
        // Repaint only what changed since the painted frame: a keystroke
        // changes one field, not the form.
        let surface = metis_ui_lang::Rect::new(0, 0, width, height);
        let damage = self.painted.as_ref().map_or(Damage::Full, |painted| {
            display.damage_since(painted, surface)
        });
        let repaint = |framebuffer: &mut Framebuffer| render::paint(framebuffer, &display);
        match damage {
            Damage::Unchanged => {}
            Damage::Region(region) => self.framebuffer.render_clipped(region, repaint),
            Damage::Full => repaint(&mut self.framebuffer),
        }
        self.unpresented = self.unpresented.merge(damage);
        self.painted = Some(display);
        Ok(())
    }

    /// The display list the surface shows.
    ///
    /// A host reads the frame it painted from here — the attribution and the
    /// sampled colors — instead of laying the document out again. `None`
    /// before the first frame.
    #[must_use]
    pub fn painted_display(&self) -> Option<&DisplayList> {
        self.painted.as_ref()
    }

    /// Supplies the host clock a declared transition samples against.
    ///
    /// The clock is monotonic and host-owned: the software host reads it from
    /// its platform timer before each batch. A host that never supplies one
    /// paints every declared target in the frame the change lands in, which is
    /// also the reduced-motion outcome, so a declaration is never left
    /// part-way through an animation the host cannot time.
    pub fn set_now(&mut self, now: Duration) {
        self.now = Some(now);
    }

    /// Sets how much motion the host asks the presentation to show.
    ///
    /// [`MotionPreference::Reduced`] applies a declared transition's target in
    /// the frame the change lands in and schedules no tick.
    pub fn set_motion_preference(&mut self, preference: MotionPreference) {
        self.motion = preference;
    }

    /// How long until a host must give this form another frame with no input.
    ///
    /// `None` when nothing is moving, so an idle surface schedules no frame.
    #[must_use]
    pub fn next_tick(&self) -> Option<Duration> {
        self.transitions.next_tick()
    }

    /// Samples the transitions the painted frame is running and repaints only
    /// the elements that moved.
    ///
    /// A host calls this when [`Self::next_tick`] elapses with no input. The
    /// tick requests no memory: the frame the surface shows is recolored in
    /// place, and a sample that changes no pixel repaints nothing.
    ///
    /// Returns whether any pixel changed.
    #[must_use]
    pub fn advance_transitions(&mut self) -> bool {
        let now = self.now;
        let Some(painted) = self.painted.as_mut() else {
            return false;
        };
        let Some(region) = self.transitions.apply(painted, now) else {
            return false;
        };
        self.framebuffer
            .render_clipped(region, |framebuffer| render::paint(framebuffer, painted));
        self.unpresented = self.unpresented.merge(Damage::Region(region));
        true
    }
}

fn layout_error() -> MetisError {
    MetisError::ui(
        ErrorCode::LayoutOverflow,
        "Surface exceeds layout coordinates",
    )
}

#[cfg(test)]
mod presentation_tests;
