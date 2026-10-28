//! Iris backend adapter for a display list.

use super::list::DisplayList;
use metis_platform::framebuffer::Framebuffer;

impl iris::render::RenderBackend<DisplayList> for Framebuffer {
    type Error = std::convert::Infallible;
    type Frame<'frame> = &'frame [u32];

    fn render<'frame>(
        &'frame mut self,
        view: &DisplayList,
    ) -> std::result::Result<Self::Frame<'frame>, Self::Error> {
        view.render_to(self);
        Ok(self.pixels())
    }
}
