//! Rust-owned bounded pan and zoom state for browser gestures.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WheelUnit {
    Pixel,
    Line,
    Page,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActivePointer {
    id: i32,
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GestureViewport {
    active: Option<ActivePointer>,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
}

const MIN_ZOOM: f64 = 0.5;
const MAX_ZOOM: f64 = 3.0;
const PAN_LIMIT: f64 = 1024.0;
const LINE_HEIGHT_PX: f64 = 16.0;
const PAGE_HEIGHT_PX: f64 = 640.0;
const ZOOM_PER_PIXEL: f64 = 1.0 / 600.0;

impl Default for GestureViewport {
    fn default() -> Self {
        Self {
            active: None,
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl GestureViewport {
    pub(crate) fn press(&mut self, id: i32, x: i32, y: i32) -> bool {
        if self.active.is_some() {
            return false;
        }
        self.active = Some(ActivePointer { id, x, y });
        true
    }

    pub(crate) fn move_pointer(&mut self, id: i32, x: i32, y: i32) -> bool {
        let Some(active) = self.active else {
            return false;
        };
        if active.id != id {
            return false;
        }
        self.pan_x = clamp_pan(self.pan_x + f64::from(x - active.x));
        self.pan_y = clamp_pan(self.pan_y + f64::from(y - active.y));
        self.active = Some(ActivePointer { id, x, y });
        true
    }

    pub(crate) fn release(&mut self, id: i32) -> bool {
        if self.active.is_some_and(|active| active.id == id) {
            self.active = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn wheel(
        &mut self,
        delta_x: f64,
        delta_y: f64,
        unit: WheelUnit,
        ctrl: bool,
    ) -> bool {
        if !delta_x.is_finite() || !delta_y.is_finite() {
            return false;
        }
        let (delta_x, delta_y) = normalize_wheel(delta_x, delta_y, unit);
        if ctrl {
            self.zoom = (self.zoom - delta_y * ZOOM_PER_PIXEL).clamp(MIN_ZOOM, MAX_ZOOM);
        } else {
            self.pan_x = clamp_pan(self.pan_x + delta_x);
            self.pan_y = clamp_pan(self.pan_y + delta_y);
        }
        true
    }

    pub(crate) fn summary(self, action: &str) -> String {
        format!(
            "Gesture: {action}; pan ({:.1}, {:.1}) CSS px; zoom {:.0}%",
            self.pan_x,
            self.pan_y,
            self.zoom * 100.0,
        )
    }

    pub(crate) fn transform(self) -> String {
        format!(
            "transform: translate({:.1}px, {:.1}px) scale({:.3});",
            self.pan_x, self.pan_y, self.zoom
        )
    }

    pub(crate) const fn zoom(self) -> f64 {
        self.zoom
    }
}

fn clamp_pan(value: f64) -> f64 {
    value.clamp(-PAN_LIMIT, PAN_LIMIT)
}

fn normalize_wheel(delta_x: f64, delta_y: f64, unit: WheelUnit) -> (f64, f64) {
    let scale = match unit {
        WheelUnit::Line => LINE_HEIGHT_PX,
        WheelUnit::Page => PAGE_HEIGHT_PX,
        WheelUnit::Pixel | WheelUnit::Other => 1.0,
    };
    (delta_x * scale, delta_y * scale)
}

#[cfg(test)]
mod tests {
    use super::{GestureViewport, WheelUnit, normalize_wheel};

    #[test]
    fn pointer_drag_updates_pan_and_rejects_a_second_pointer() {
        let mut viewport = GestureViewport::default();
        assert!(viewport.press(1, 10, 20));
        assert!(!viewport.press(2, 10, 20));
        assert!(viewport.move_pointer(1, 30, 5));
        assert_eq!(viewport.pan_x.to_bits(), 20.0f64.to_bits());
        assert_eq!(viewport.pan_y.to_bits(), (-15.0f64).to_bits());
        assert!(!viewport.move_pointer(2, 40, 40));
        assert!(viewport.release(1));
        assert!(!viewport.release(1));
    }

    #[test]
    fn wheel_units_are_normalized_and_ctrl_zoom_is_bounded() {
        assert_eq!(normalize_wheel(2.0, -3.0, WheelUnit::Line), (32.0, -48.0));
        assert_eq!(
            normalize_wheel(2.0, -3.0, WheelUnit::Page),
            (1280.0, -1920.0)
        );
        assert_eq!(normalize_wheel(2.0, -3.0, WheelUnit::Other), (2.0, -3.0));
        let mut viewport = GestureViewport::default();
        assert!(viewport.wheel(4.0, -8.0, WheelUnit::Pixel, false));
        assert_eq!(viewport.pan_x.to_bits(), 4.0f64.to_bits());
        assert_eq!(viewport.pan_y.to_bits(), (-8.0f64).to_bits());
        assert!(viewport.wheel(0.0, -10_000.0, WheelUnit::Pixel, true));
        assert_eq!(viewport.zoom.to_bits(), 3.0f64.to_bits());
        assert!(viewport.wheel(0.0, 10_000.0, WheelUnit::Pixel, true));
        assert_eq!(viewport.zoom.to_bits(), 0.5f64.to_bits());
        assert_eq!(viewport.zoom().to_bits(), 0.5f64.to_bits());
        assert!(viewport.summary("zoom").contains("zoom 50%"));
        assert!(viewport.transform().contains("scale(0.500)"));
    }

    #[test]
    fn non_finite_wheel_input_is_rejected_without_mutating_state() {
        let mut viewport = GestureViewport::default();
        assert!(!viewport.wheel(f64::NAN, 1.0, WheelUnit::Pixel, false));
        assert_eq!(viewport, GestureViewport::default());
    }
}
