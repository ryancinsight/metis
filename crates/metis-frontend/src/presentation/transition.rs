//! Bounded transitions sampled where a frame is painted.
//!
//! A declared `transition` is honoured without touching the document: layout
//! emits the target colors once, and this module writes the interpolated
//! color into the commands the animated element owns. The semantic projection,
//! the accessibility tree and every layout box stay exactly as they were for
//! the transition's whole duration, and a sampled frame damages the animated
//! element rather than the surface (ADR 0060).

use metis_platform::MotionPreference;
use metis_platform::framebuffer::Rect;
use metis_ui_lang::{Color, DisplayList, ElementIndex, Transition, TransitionProperty};
use std::sync::Arc;
use std::time::Duration;

/// How long a host waits between samples while a transition moves.
///
/// Sixteen milliseconds is sixty frames a second, the rate the software host
/// presents at; a slower host still reaches the exact target, because the
/// sample is a function of elapsed time and not of the tick count.
pub(crate) const TICK_INTERVAL: Duration = Duration::from_millis(16);

/// One element property moving from the value it painted to the value it
/// paints now.
struct Run {
    /// The element's authored identifier, which survives a relayout that
    /// renumbers elements.
    id: Arc<str>,
    /// The element's position in the list this run last sampled.
    element: ElementIndex,
    /// The interpolated property.
    property: TransitionProperty,
    /// The declared duration and curve.
    transition: Transition,
    /// The value the run started from.
    from: Color,
    /// The value the element's style now paints.
    to: Color,
    /// When the run started, on the host clock.
    started: Duration,
}

/// The transitions a frame is sampling.
///
/// A run exists only while a declared property is moving, so a document that
/// declares no transition keeps an empty vector and requests no memory.
#[derive(Default)]
pub(crate) struct Transitions {
    runs: Vec<Run>,
}

impl Transitions {
    /// Starts, retargets and retires runs against a freshly laid-out list.
    ///
    /// `painted` is the frame the surface shows, whose colors are what the
    /// user last saw, so a run starts from them and an interruption continues
    /// from the sampled value instead of jumping back. `now` is the host
    /// clock; a host that supplies none gets the target in one frame, which
    /// is the reduced-motion outcome and never leaves a value stuck partway.
    pub(super) fn observe(
        &mut self,
        next: &DisplayList,
        painted: Option<&DisplayList>,
        preference: MotionPreference,
        now: Option<Duration>,
    ) {
        let Some(now) = now.filter(|_| preference == MotionPreference::Full) else {
            // No clock or reduced motion: nothing samples, so the list keeps
            // the colors its elements declare.
            self.runs.clear();
            return;
        };
        // A run whose element or declaration is gone stops, and the element
        // each surviving run addresses is resolved again from its identifier.
        self.runs.retain_mut(|run| {
            let Some(element) = next.element_index(&run.id) else {
                return false;
            };
            if next.element_transition(element) != Some(run.transition) {
                return false;
            }
            run.element = element;
            true
        });
        // A run whose element now paints another value starts again from the
        // value it has reached, so an interruption does not jump.
        for run in &mut self.runs {
            let Some(target) = next.element_color(run.element, run.property) else {
                continue;
            };
            if target == run.to {
                continue;
            }
            let elapsed = now.saturating_sub(run.started);
            run.from = run.transition.sample(run.from, run.to, elapsed);
            run.to = target;
            run.started = now;
        }
        let Some(painted) = painted else {
            return;
        };
        // An element that declares a transition and paints a different value
        // than the shown frame moves from what the user saw.
        for element in next.elements() {
            let Some(transition) = next.element_transition(element) else {
                continue;
            };
            let Some(id) = next.element_id(element) else {
                continue;
            };
            let property = transition.property();
            if self
                .runs
                .iter()
                .any(|run| &*run.id == id && run.property == property)
            {
                continue;
            }
            let Some(target) = next.element_color(element, property) else {
                continue;
            };
            let Some(from) = painted
                .element_index(id)
                .and_then(|shown| painted.element_color(shown, property))
            else {
                continue;
            };
            if from == target {
                continue;
            }
            self.runs.push(Run {
                id: Arc::from(id),
                element,
                property,
                transition,
                from,
                to: target,
                started: now,
            });
        }
    }

    /// Writes each run's sample into `display` and reports the area the runs
    /// repaint.
    ///
    /// A run that has reached its target is retired after the frame that
    /// paints it, so a finished transition costs nothing afterwards.
    pub(super) fn apply(
        &mut self,
        display: &mut DisplayList,
        now: Option<Duration>,
    ) -> Option<Rect> {
        let now = now?;
        let mut covered: Option<Rect> = None;
        for run in &self.runs {
            let elapsed = now.saturating_sub(run.started);
            let color = run.transition.sample(run.from, run.to, elapsed);
            if let Some(area) = display.recolor(run.element, run.property, color) {
                covered = Some(covered.map_or(area, |painted| painted.union(area)));
            }
        }
        self.runs
            .retain(|run| now.saturating_sub(run.started) < run.transition.duration());
        covered
    }

    /// How long until the host must sample again, or `None` when nothing
    /// moves.
    pub(super) fn next_tick(&self) -> Option<Duration> {
        (!self.runs.is_empty()).then_some(TICK_INTERVAL)
    }
}
