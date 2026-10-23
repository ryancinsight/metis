//! Damage reporting tests for host.

use crate::{Damage, Framebuffer};
use crate::native::application::{NativeApplication, NativeFlow};
use crate::native::application::tests::probe::ProbeError;
use crate::native::WindowEvent;
use std::collections::VecDeque;

/// Reports a scripted damage for each repaint it requests.
pub struct DamageApplication {
    pub framebuffer: Framebuffer,
    pub damages: VecDeque<Damage>,
    pub pending: Damage,
}

impl NativeApplication for DamageApplication {
    type Error = ProbeError;

    fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    fn take_damage(&mut self) -> Damage {
        std::mem::replace(&mut self.pending, Damage::Unchanged)
    }

    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
        if events
            .iter()
            .any(|event| matches!(event, WindowEvent::CloseRequested))
        {
            return Ok(NativeFlow::Exit);
        }
        self.pending = self
            .pending
            .merge(self.damages.pop_front().ok_or(ProbeError)?);
        Ok(NativeFlow::Continue { repaint: true })
    }
}