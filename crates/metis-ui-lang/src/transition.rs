//! Bounded property transitions: the declared vocabulary and its sampler.
//!
//! [ADR 0060](../../docs/adr/0060-bounded-property-transitions.md) admits one
//! transition per element: one property, one bounded duration and one easing.
//! A transition is metadata on a [`ComputedStyle`](crate::ComputedStyle); it
//! never changes layout, and the sampler is a pure function of elapsed time, so
//! a value never depends on when a frame happened to arrive.

use metis_platform::framebuffer::Color;
use std::time::Duration;

/// Longest admitted transition, in milliseconds.
pub const MAX_TRANSITION_MILLIS: u32 = 2000;

/// Property a transition animates.
///
/// The set is the properties whose paint result is unambiguous: each is drawn
/// only by commands owned by the declaring element. `background-color` waits
/// for the gradient-under-fill decision, `box-shadow` for its memoized mask
/// decision (ADR 0052) and `opacity` for group compositing (ADR 0051).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionProperty {
    /// Text color, declared as `color`.
    Color,
    /// Border color, declared as `border-color`.
    BorderColor,
}

impl TransitionProperty {
    /// The declaration keyword.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::BorderColor => "border-color",
        }
    }

    /// Parses a declaration keyword; every other property is refused.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "color" => Some(Self::Color),
            "border-color" => Some(Self::BorderColor),
            _ => None,
        }
    }
}

/// Easing keyword, with the CSS cubic-bezier control points it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    /// Constant progress.
    Linear,
    /// CSS `ease`.
    Ease,
    /// CSS `ease-in`.
    EaseIn,
    /// CSS `ease-out`.
    EaseOut,
    /// CSS `ease-in-out`.
    EaseInOut,
}

impl Easing {
    /// The declaration keyword.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Ease => "ease",
            Self::EaseIn => "ease-in",
            Self::EaseOut => "ease-out",
            Self::EaseInOut => "ease-in-out",
        }
    }

    /// Parses a declaration keyword; every other easing is refused.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "linear" => Some(Self::Linear),
            "ease" => Some(Self::Ease),
            "ease-in" => Some(Self::EaseIn),
            "ease-out" => Some(Self::EaseOut),
            "ease-in-out" => Some(Self::EaseInOut),
            _ => None,
        }
    }

    /// The control points of the CSS timing function this keyword names.
    #[must_use]
    pub const fn control_points(self) -> (f64, f64, f64, f64) {
        match self {
            Self::Linear => (0.0, 0.0, 1.0, 1.0),
            Self::Ease => (0.25, 0.1, 0.25, 1.0),
            Self::EaseIn => (0.42, 0.0, 1.0, 1.0),
            Self::EaseOut => (0.0, 0.0, 0.58, 1.0),
            Self::EaseInOut => (0.42, 0.0, 0.58, 1.0),
        }
    }

    /// Eased progress for a linear fraction, both in `[0, 1]`.
    ///
    /// The curve is the CSS timing function of [`Easing::control_points`]:
    /// `x(u)` is solved for `u` by a fixed bisection count and `y(u)` is
    /// returned, so the same input yields the same output on every host and in
    /// every test run. The endpoints are exact rather than asymptotic.
    #[must_use]
    pub fn progress(self, fraction: f64) -> f64 {
        let fraction = fraction.clamp(0.0, 1.0);
        if fraction <= 0.0 {
            return 0.0;
        }
        if fraction >= 1.0 {
            return 1.0;
        }
        if matches!(self, Self::Linear) {
            return fraction;
        }
        let (x1, y1, x2, y2) = self.control_points();
        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..32 {
            let middle = 0.5 * (low + high);
            if bezier(middle, x1, x2) < fraction {
                low = middle;
            } else {
                high = middle;
            }
        }
        bezier(0.5 * (low + high), y1, y2).clamp(0.0, 1.0)
    }
}

/// One admitted transition: a property, a duration and an easing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    property: TransitionProperty,
    millis: u32,
    easing: Easing,
}

impl Transition {
    /// A transition bounded to [`MAX_TRANSITION_MILLIS`]; `None` when the
    /// duration exceeds it.
    #[must_use]
    pub fn new(property: TransitionProperty, millis: u32, easing: Easing) -> Option<Self> {
        if millis > MAX_TRANSITION_MILLIS {
            return None;
        }
        Some(Self {
            property,
            millis,
            easing,
        })
    }

    /// The animated property.
    #[must_use]
    pub const fn property(self) -> TransitionProperty {
        self.property
    }

    /// The declared duration in milliseconds.
    #[must_use]
    pub const fn duration_millis(self) -> u32 {
        self.millis
    }

    /// The declared easing.
    #[must_use]
    pub const fn easing(self) -> Easing {
        self.easing
    }

    /// Whether the transition reaches its target in the first frame.
    #[must_use]
    pub const fn is_immediate(self) -> bool {
        self.millis == 0
    }

    /// The color at `elapsed`, from `from` towards `to`.
    ///
    /// Channels are interpolated in the stored 8-bit sRGB encoding, rounded
    /// once, half away from zero. Elapsed time at or past the duration, and a
    /// zero duration, return `to` exactly; zero elapsed returns `from` exactly.
    #[must_use]
    pub fn sample(self, from: Color, to: Color, elapsed: Duration) -> Color {
        if self.millis == 0 || elapsed >= self.duration() {
            return to;
        }
        let fraction = elapsed.as_secs_f64() / (f64::from(self.millis) / 1000.0);
        let progress = self.easing.progress(fraction);
        Color {
            r: mix(from.r, to.r, progress),
            g: mix(from.g, to.g, progress),
            b: mix(from.b, to.b, progress),
            a: mix(from.a, to.a, progress),
        }
    }

    /// The declared duration.
    #[must_use]
    pub fn duration(self) -> Duration {
        Duration::from_millis(u64::from(self.millis))
    }
}

/// One channel moved `progress` of the way from `from` to `to`.
fn mix(from: u8, to: u8, progress: f64) -> u8 {
    let value = f64::from(from) + (f64::from(to) - f64::from(from)) * progress;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a progress in [0, 1] keeps the value in [0, 255]"
    )]
    let byte = value.round().clamp(0.0, 255.0) as u8;
    byte
}

/// One coordinate of a cubic bezier whose endpoints are `0` and `1`.
fn bezier(u: f64, first: f64, second: f64) -> f64 {
    let inverse = 1.0 - u;
    3.0 * inverse * inverse * u * first + 3.0 * inverse * u * u * second + u * u * u
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: Color = Color::rgba(0, 0, 0, 255);
    const TARGET: Color = Color::rgba(255, 255, 255, 255);
    const EASINGS: [Easing; 5] = [
        Easing::Linear,
        Easing::Ease,
        Easing::EaseIn,
        Easing::EaseOut,
        Easing::EaseInOut,
    ];

    #[test]
    fn endpoints_are_exact_for_every_easing() {
        for easing in EASINGS {
            let transition =
                Transition::new(TransitionProperty::Color, 150, easing).expect("bounded");
            assert_eq!(
                transition.sample(SOURCE, TARGET, Duration::ZERO),
                SOURCE,
                "{easing:?}"
            );
            let end = Duration::from_millis(150);
            assert_eq!(transition.sample(SOURCE, TARGET, end), TARGET, "{easing:?}");
            assert_eq!(
                transition.sample(SOURCE, TARGET, Duration::from_secs(1)),
                TARGET,
                "{easing:?}"
            );
        }
    }

    #[test]
    fn a_zero_duration_is_immediate() {
        let transition =
            Transition::new(TransitionProperty::BorderColor, 0, Easing::Ease).expect("bounded");
        assert!(transition.is_immediate());
        assert_eq!(transition.sample(SOURCE, TARGET, Duration::ZERO), TARGET);
    }

    #[test]
    fn progress_is_monotonic_and_bounded() {
        for easing in EASINGS {
            let mut previous = 0.0;
            for step in 0..=20 {
                let progress = easing.progress(f64::from(step) / 20.0);
                assert!((0.0..=1.0).contains(&progress), "{easing:?} at {step}");
                assert!(progress >= previous, "{easing:?} at {step}");
                previous = progress;
            }
        }
    }

    #[test]
    fn the_curve_keywords_keep_their_css_shape() {
        // A quarter of the way in, ease-out is ahead of linear and ease-in is
        // behind it; ease-in-out is symmetric about the midpoint.
        let quarter = Easing::EaseOut.progress(0.25);
        let linear = Easing::Linear.progress(0.25);
        let behind = Easing::EaseIn.progress(0.25);
        assert!(quarter > linear, "ease-out {quarter} vs linear {linear}");
        assert!(behind < linear, "ease-in {behind} vs linear {linear}");
        let middle = Easing::EaseInOut.progress(0.5);
        assert!((middle - 0.5).abs() < 1e-6, "ease-in-out midpoint {middle}");
    }

    #[test]
    fn alpha_interpolates_with_the_channels() {
        let from = Color::rgba(0, 0, 0, 0);
        let to = Color::rgba(0, 0, 0, 200);
        let transition =
            Transition::new(TransitionProperty::Color, 100, Easing::Linear).expect("bounded");
        let middle = transition.sample(from, to, Duration::from_millis(50));
        assert_eq!(middle.a, 100);
    }

    #[test]
    fn sampling_repeats_exactly() {
        let transition =
            Transition::new(TransitionProperty::Color, 250, Easing::Ease).expect("bounded");
        let elapsed = Duration::from_millis(90);
        assert_eq!(
            transition.sample(SOURCE, TARGET, elapsed),
            transition.sample(SOURCE, TARGET, elapsed)
        );
    }

    #[test]
    fn durations_above_the_bound_are_refused() {
        assert!(
            Transition::new(
                TransitionProperty::Color,
                MAX_TRANSITION_MILLIS,
                Easing::Linear
            )
            .is_some()
        );
        assert!(
            Transition::new(
                TransitionProperty::Color,
                MAX_TRANSITION_MILLIS + 1,
                Easing::Linear
            )
            .is_none()
        );
    }

    #[test]
    fn keywords_round_trip() {
        for property in [TransitionProperty::Color, TransitionProperty::BorderColor] {
            assert_eq!(TransitionProperty::parse(property.as_str()), Some(property));
        }
        for easing in EASINGS {
            assert_eq!(Easing::parse(easing.as_str()), Some(easing));
        }
        assert_eq!(TransitionProperty::parse("width"), None);
        assert_eq!(Easing::parse("cubic-bezier"), None);
    }
}
