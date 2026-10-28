//! A declared transition sampled at the paint boundary.
//!
//! These tests drive the sampler against real layouts: the document declares
//! the transition and the colors, and the overlay writes the interpolated
//! value into the commands the animated element owns.

use super::transition::{TICK_INTERVAL, Transitions};
use metis_platform::MotionPreference;
use metis_platform::framebuffer::Damage;
use metis_ui_lang::{
    Color, DisplayList, LayoutViewport, Rect, TransitionProperty, compute_layout, parse_markup,
};
use std::time::Duration;

const SOURCE: Color = Color::rgb(255, 255, 255);
const TARGET: Color = Color::rgb(0, 0, 0);

/// A form whose badge paints `color` and declares a 150 ms color transition.
fn layout(color: &str) -> DisplayList {
    render("", color)
}

/// The same form with one element ahead of the badge, so every later element
/// index shifts.
fn shifted(color: &str) -> DisplayList {
    render(
        r#"  <text id="lead" style="color: #4a5568; font-size: 13px;">Lead</text>
"#,
        color,
    )
}

fn render(leading: &str, color: &str) -> DisplayList {
    let markup = format!(
        r#"<screen id="root" style="display: flex; flex-direction: column; width: 100%; height: 100%; background-color: #ffffff;">
{leading}  <text id="badge" style="color: {color}; font-size: 13px; transition: color 150ms linear;">SYSTEM READY</text>
</screen>"#
    );
    let document = parse_markup(&markup).expect("authored markup");
    compute_layout(&document, LayoutViewport::new(400, 200)).expect("bounded layout")
}

fn badge_color(display: &DisplayList) -> Option<Color> {
    let badge = display.element_index("badge")?;
    display.element_color(badge, TransitionProperty::Color)
}

#[test]
fn a_transition_samples_from_the_shown_color_to_the_painted_one() {
    let painted = layout("#ffffff");
    let mut next = layout("#000000");
    let mut transitions = Transitions::default();
    transitions.observe(
        &next,
        Some(&painted),
        MotionPreference::Full,
        Some(Duration::ZERO),
    );
    assert_eq!(transitions.next_tick(), Some(TICK_INTERVAL));

    // The frame that starts the run paints the color the surface already
    // showed, so the change is never a jump.
    transitions.apply(&mut next, Some(Duration::ZERO));
    assert_eq!(badge_color(&next), Some(SOURCE));

    transitions.apply(&mut next, Some(Duration::from_millis(150)));
    assert_eq!(badge_color(&next), Some(TARGET));
    assert_eq!(
        transitions.next_tick(),
        None,
        "a settled run schedules no tick"
    );
}

#[test]
fn a_sample_between_the_endpoints_moves_without_reaching_either() {
    let painted = layout("#ffffff");
    let mut next = layout("#000000");
    let mut transitions = Transitions::default();
    transitions.observe(
        &next,
        Some(&painted),
        MotionPreference::Full,
        Some(Duration::ZERO),
    );
    transitions.apply(&mut next, Some(Duration::from_millis(75)));
    let middle = badge_color(&next).expect("sampled badge");
    assert_ne!(middle, SOURCE);
    assert_ne!(middle, TARGET);
    assert!(middle.r > 0 && middle.r < 255, "{middle:?}");

    // Sampling is a function of elapsed time, so the same instant repeats.
    let mut repeated = layout("#000000");
    transitions.observe(
        &repeated,
        Some(&painted),
        MotionPreference::Full,
        Some(Duration::ZERO),
    );
    transitions.apply(&mut repeated, Some(Duration::from_millis(75)));
    assert_eq!(badge_color(&repeated), Some(middle));
}

#[test]
fn an_interruption_starts_from_the_sampled_value() {
    let painted = layout("#ffffff");
    let mut mid = layout("#000000");
    let mut transitions = Transitions::default();
    transitions.observe(
        &mid,
        Some(&painted),
        MotionPreference::Full,
        Some(Duration::ZERO),
    );
    transitions.apply(&mut mid, Some(Duration::from_millis(75)));
    let sampled = badge_color(&mid).expect("sampled badge");

    // The style paints another target while the run is still moving.
    let mut retargeted = layout("#ff0000");
    transitions.observe(
        &retargeted,
        Some(&mid),
        MotionPreference::Full,
        Some(Duration::from_millis(75)),
    );
    transitions.apply(&mut retargeted, Some(Duration::from_millis(75)));
    assert_eq!(
        badge_color(&retargeted),
        Some(sampled),
        "an interruption jumped instead of continuing from the sampled value"
    );
}

#[test]
fn a_run_follows_its_element_when_a_relayout_renumbers_it() {
    let painted = layout("#ffffff");
    let next = layout("#000000");
    let mut transitions = Transitions::default();
    transitions.observe(
        &next,
        Some(&painted),
        MotionPreference::Full,
        Some(Duration::ZERO),
    );

    // A leading element shifts every later index; the run is keyed on the
    // authored identifier, so it still addresses the badge.
    let mut moved = shifted("#000000");
    transitions.observe(
        &moved,
        Some(&next),
        MotionPreference::Full,
        Some(Duration::from_millis(75)),
    );
    transitions.apply(&mut moved, Some(Duration::from_millis(75)));
    let sampled = badge_color(&moved).expect("sampled badge");
    assert_ne!(sampled, TARGET, "the run lost its element on relayout");
    assert_ne!(sampled, SOURCE);
}

#[test]
fn reduced_motion_paints_the_target_in_one_frame() {
    let painted = layout("#ffffff");
    let mut next = layout("#000000");
    let mut transitions = Transitions::default();
    transitions.observe(
        &next,
        Some(&painted),
        MotionPreference::Reduced,
        Some(Duration::ZERO),
    );
    assert_eq!(transitions.next_tick(), None);
    assert_eq!(transitions.apply(&mut next, Some(Duration::ZERO)), None);
    assert_eq!(badge_color(&next), Some(TARGET));
}

#[test]
fn a_host_without_a_clock_paints_the_target() {
    let painted = layout("#ffffff");
    let mut next = layout("#000000");
    let mut transitions = Transitions::default();
    transitions.observe(&next, Some(&painted), MotionPreference::Full, None);
    assert_eq!(transitions.next_tick(), None);
    assert_eq!(transitions.apply(&mut next, None), None);
    assert_eq!(
        badge_color(&next),
        Some(TARGET),
        "an untimed host must not leave a value part-way"
    );
}

#[test]
fn a_transition_damages_only_its_element() {
    let painted = layout("#ffffff");
    let mut next = layout("#000000");
    let mut transitions = Transitions::default();
    transitions.observe(
        &next,
        Some(&painted),
        MotionPreference::Full,
        Some(Duration::ZERO),
    );
    transitions.apply(&mut next, Some(Duration::from_millis(75)));

    let surface = Rect::new(0, 0, 400, 200);
    let Damage::Region(region) = next.damage_since(&painted, surface) else {
        panic!("a recolored run must damage a region, not the surface");
    };

    // The damage is exactly the extent the sampler recolored: no other command
    // moved, and the frame claims no pixel the transition did not touch.
    let badge = next.element_index("badge").expect("authored badge");
    let mut probe = next.copy_with_room(0).expect("bounded copy");
    let repainted = probe
        .recolor(badge, TransitionProperty::Color, Color::rgb(1, 2, 3))
        .expect("a recolored run");
    assert_eq!(region, repainted);
    assert!(region.width < surface.width && region.height < surface.height);
}

#[test]
fn a_tick_reuses_the_frame_buffer_it_samples() {
    let painted = layout("#ffffff");
    let mut next = layout("#000000");
    let mut transitions = Transitions::default();
    transitions.observe(
        &next,
        Some(&painted),
        MotionPreference::Full,
        Some(Duration::ZERO),
    );
    transitions.apply(&mut next, Some(Duration::ZERO));

    // A tick rewrites colors in the frame the surface shows; it never rebuilds
    // the command storage, which is what keeps it allocation-free.
    let buffer = next.commands().as_ptr();
    let count = next.commands().len();
    transitions
        .apply(&mut next, Some(Duration::from_millis(16)))
        .expect("bounded sample");
    assert_eq!(next.commands().as_ptr(), buffer, "a tick moved the frame");
    assert_eq!(next.commands().len(), count);
}
