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
struct PinchBaseline {
    center_x: f64,
    center_y: f64,
    distance: f64,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GestureViewport {
    active: [Option<ActivePointer>; MAX_ACTIVE_POINTERS],
    pinch: Option<PinchBaseline>,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
}

const MAX_ACTIVE_POINTERS: usize = 2;
const MIN_ZOOM: f64 = 0.5;
const MAX_ZOOM: f64 = 3.0;
const PAN_LIMIT: f64 = 1024.0;
const LINE_HEIGHT_PX: f64 = 16.0;
const PAGE_HEIGHT_PX: f64 = 640.0;
const ZOOM_PER_PIXEL: f64 = 1.0 / 600.0;

impl Default for GestureViewport {
    fn default() -> Self {
        Self {
            active: [None; MAX_ACTIVE_POINTERS],
            pinch: None,
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl GestureViewport {
    pub(crate) fn press(&mut self, id: i32, x: i32, y: i32) -> bool {
        if self.active.iter().flatten().any(|pointer| pointer.id == id) {
            return false;
        }
        let Some(slot) = self.active.iter_mut().find(|pointer| pointer.is_none()) else {
            return false;
        };
        *slot = Some(ActivePointer { id, x, y });
        if self.active_pointer_count() == MAX_ACTIVE_POINTERS {
            self.begin_pinch();
        }
        true
    }

    pub(crate) fn move_pointer(&mut self, id: i32, x: i32, y: i32) -> bool {
        let Some(active) = self
            .active
            .iter_mut()
            .find_map(|pointer| pointer.as_mut().filter(|active| active.id == id))
        else {
            return false;
        };
        let previous_x = active.x;
        let previous_y = active.y;
        active.x = x;
        active.y = y;
        if self.active_pointer_count() == MAX_ACTIVE_POINTERS {
            self.update_pinch();
        } else {
            self.pan_x = clamp_pan(self.pan_x + f64::from(x) - f64::from(previous_x));
            self.pan_y = clamp_pan(self.pan_y + f64::from(y) - f64::from(previous_y));
        }
        true
    }

    pub(crate) fn release(&mut self, id: i32) -> bool {
        let Some(slot) = self
            .active
            .iter_mut()
            .find(|pointer| pointer.is_some_and(|active| active.id == id))
        else {
            return false;
        };
        *slot = None;
        if self.active_pointer_count() < MAX_ACTIVE_POINTERS {
            self.pinch = None;
        }
        true
    }

    pub(crate) const fn active_pointer_count(self) -> usize {
        let mut count = 0;
        let mut index = 0;
        while index < MAX_ACTIVE_POINTERS {
            if self.active[index].is_some() {
                count += 1;
            }
            index += 1;
        }
        count
    }

    pub(crate) const fn is_pinching(self) -> bool {
        self.pinch.is_some()
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

    fn begin_pinch(&mut self) {
        let Some((first, second)) = self.pointer_pair() else {
            self.pinch = None;
            return;
        };
        let (center_x, center_y) = center(first, second);
        let distance = distance(first, second);
        if distance.is_finite() && distance > 0.0 {
            self.pinch = Some(PinchBaseline {
                center_x,
                center_y,
                distance,
                pan_x: self.pan_x,
                pan_y: self.pan_y,
                zoom: self.zoom,
            });
        } else {
            self.pinch = None;
        }
    }

    fn update_pinch(&mut self) {
        let Some((first, second)) = self.pointer_pair() else {
            self.pinch = None;
            return;
        };
        let (center_x, center_y) = center(first, second);
        let distance = distance(first, second);
        if !distance.is_finite() || distance <= 0.0 {
            return;
        }
        let Some(baseline) = self.pinch else {
            self.begin_pinch();
            return;
        };
        self.pan_x = clamp_pan(baseline.pan_x + center_x - baseline.center_x);
        self.pan_y = clamp_pan(baseline.pan_y + center_y - baseline.center_y);
        self.zoom = (baseline.zoom * distance / baseline.distance).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn pointer_pair(&self) -> Option<(ActivePointer, ActivePointer)> {
        let mut pointers = self.active.iter().flatten().copied();
        let first = pointers.next()?;
        let second = pointers.next()?;
        Some((first, second))
    }
}

fn center(first: ActivePointer, second: ActivePointer) -> (f64, f64) {
    (
        f64::midpoint(f64::from(first.x), f64::from(second.x)),
        f64::midpoint(f64::from(first.y), f64::from(second.y)),
    )
}

fn distance(first: ActivePointer, second: ActivePointer) -> f64 {
    let delta_x = f64::from(first.x) - f64::from(second.x);
    let delta_y = f64::from(first.y) - f64::from(second.y);
    delta_x.hypot(delta_y)
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
    fn pointer_drag_updates_pan_and_rejects_a_third_pointer() {
        let mut viewport = GestureViewport::default();
        assert!(viewport.press(1, 10, 20));
        assert!(viewport.move_pointer(1, 30, 5));
        assert_eq!(viewport.pan_x.to_bits(), 20.0f64.to_bits());
        assert_eq!(viewport.pan_y.to_bits(), (-15.0f64).to_bits());
        assert!(viewport.press(2, 10, 20));
        assert_eq!(viewport.active_pointer_count(), 2);
        assert!(!viewport.press(3, 40, 40));
        assert!(!viewport.move_pointer(3, 40, 40));
        assert!(viewport.release(2));
        assert!(viewport.release(1));
        assert!(!viewport.release(1));
    }

    #[test]
    fn pinch_scales_from_its_baseline_and_pans_by_its_centroid() {
        let mut viewport = GestureViewport::default();
        assert!(viewport.press(1, 0, 0));
        assert!(viewport.press(2, 10, 0));
        assert!(viewport.is_pinching());
        assert!(viewport.move_pointer(2, 20, 0));
        assert_eq!(viewport.pan_x.to_bits(), 5.0f64.to_bits());
        assert_eq!(viewport.pan_y.to_bits(), 0.0f64.to_bits());
        assert_eq!(viewport.zoom.to_bits(), 2.0f64.to_bits());
        assert!(viewport.move_pointer(1, -10, 0));
        assert_eq!(viewport.pan_x.to_bits(), 0.0f64.to_bits());
        assert_eq!(viewport.zoom.to_bits(), 3.0f64.to_bits());
        assert!(viewport.release(2));
        assert!(!viewport.is_pinching());
        assert_eq!(viewport.active_pointer_count(), 1);
    }

    #[test]
    fn zero_distance_pinch_waits_for_a_valid_baseline() {
        let mut viewport = GestureViewport::default();
        assert!(viewport.press(1, 0, 0));
        assert!(viewport.press(2, 0, 0));
        assert!(!viewport.is_pinching());
        assert!(viewport.move_pointer(2, 5, 0));
        assert!(viewport.is_pinching());
        assert!(viewport.move_pointer(1, -5, 0));
        assert_eq!(viewport.zoom.to_bits(), 2.0f64.to_bits());
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
