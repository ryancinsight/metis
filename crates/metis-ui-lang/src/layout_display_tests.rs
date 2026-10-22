//! Command ownership: an animated element names the commands it paints.
//!
//! The attribution is what lets a host change one element's paint without
//! laying the document out again, so these tests pin the mapping in both
//! directions: a command of an animated element resolves to it, and an element
//! that declares no transition owns nothing at all.

use super::{DisplayCommand, DisplayList, LayoutViewport, compute_layout};
use crate::style::Color;
use crate::transition::{Easing, Transition, TransitionProperty};
use crate::{DomDocument, ElementIndex, parse_markup};

const BADGE: &str = "badge";
const PANEL: &str = "panel";

/// The authored form, with `transition` declarations only when `animated`.
fn document(animated: bool) -> DomDocument {
    let (badge, panel) = if animated {
        (
            "transition: color 150ms linear;",
            "transition: border-color 150ms ease-in-out;",
        )
    } else {
        ("", "")
    };
    let markup = format!(
        r#"<screen id="root" style="display: flex; flex-direction: column; width: 100%; height: 100%; background-color: #ffffff;">
  <text id="badge" style="color: #9ae6b4; font-size: 13px; {badge}">SYSTEM READY</text>
  <div id="panel" style="padding: 4px; border-width: 2px; border-color: #e2e8f0; {panel}">
    <text id="label" style="color: #4a5568; font-size: 13px;">Label</text>
  </div>
</screen>"#
    );
    parse_markup(&markup).expect("authored markup")
}

fn layout(animated: bool) -> DisplayList {
    compute_layout(&document(animated), LayoutViewport::new(400, 200)).expect("bounded layout")
}

/// The commands an element paints, in painter order.
fn owned(display: &DisplayList, element: ElementIndex) -> Vec<&DisplayCommand> {
    display
        .commands()
        .iter()
        .zip(0..display.commands().len())
        .filter_map(|(command, index)| {
            (display.command_owner(index) == Some(element)).then_some(command)
        })
        .collect()
}

#[test]
fn a_document_without_a_transition_carries_no_attribution() {
    let display = layout(false);
    assert!(!display.commands().is_empty());
    assert_eq!(display.elements().count(), 0);
    for index in 0..display.commands().len() {
        assert_eq!(
            display.command_owner(index),
            None,
            "command {index} of a static form was attributed"
        );
    }
}

#[test]
fn every_command_of_an_animated_element_is_attributed() {
    let display = layout(true);
    let attributed = (0..display.commands().len())
        .filter(|index| display.command_owner(*index).is_some())
        .count();
    // The badge paints its box and one text run; the panel paints its box and
    // its trailing border. Everything else in the form is static.
    assert_eq!(
        attributed, 4,
        "an animated element must own every command it paints"
    );

    // The label declares no transition, so it is not addressable even though
    // it paints: only an animated element can be recolored in place.
    assert!(display.element_rect("label").is_some());
    assert_eq!(display.element_index("label"), None);
}

#[test]
fn an_element_owns_the_commands_it_paints_and_no_others() {
    let display = layout(true);
    let badge = display.element_index(BADGE).expect("authored badge");
    assert_eq!(display.element_id(badge), Some(BADGE));
    assert_eq!(
        display.element_transition(badge),
        Transition::new(TransitionProperty::Color, 150, Easing::Linear)
    );
    let painted = owned(&display, badge);
    assert_eq!(painted.len(), 2, "{painted:?}");
    assert!(matches!(
        painted[0],
        DisplayCommand::ElementRect { id, .. } if &**id == BADGE
    ));
    assert!(matches!(painted[1], DisplayCommand::DrawText { .. }));

    // The panel owns its box and its trailing border, not the label it holds.
    let panel = display.element_index(PANEL).expect("authored panel");
    let painted = owned(&display, panel);
    assert_eq!(painted.len(), 2, "{painted:?}");
    assert!(matches!(painted[1], DisplayCommand::DrawBorder { .. }));
}

#[test]
fn a_border_color_transition_addresses_the_border_command() {
    let display = layout(true);
    let panel = display.element_index(PANEL).expect("authored panel");
    assert_eq!(
        display.element_color(panel, TransitionProperty::BorderColor),
        Some(Color::rgb(226, 232, 240))
    );
    // The panel paints no text of its own, so its text color is absent rather
    // than inherited from the label it holds.
    assert_eq!(
        display.element_color(panel, TransitionProperty::Color),
        None
    );
}

#[test]
fn recolor_changes_only_the_element_it_names() {
    let mut display = layout(true);
    let badge = display.element_index(BADGE).expect("authored badge");
    let panel = display.element_index(PANEL).expect("authored panel");
    let untouched = display.element_color(panel, TransitionProperty::BorderColor);
    let area = display
        .recolor(badge, TransitionProperty::Color, Color::rgb(255, 0, 0))
        .expect("a recolored run");
    assert_eq!(
        display.element_color(badge, TransitionProperty::Color),
        Some(Color::rgb(255, 0, 0))
    );
    assert_eq!(
        display.element_color(panel, TransitionProperty::BorderColor),
        untouched
    );
    // The report is the glyph paint extent, which a line box may carry past
    // its border box, but it reaches nothing the panel paints below it.
    let panel_rect = display.element_rect(PANEL).expect("panel box");
    assert!(area.width > 0 && area.height > 0);
    assert!(
        area.y + area.height <= panel_rect.y,
        "{area:?} reached into the panel at {panel_rect:?}"
    );
}

#[test]
fn a_recolor_that_changes_no_pixel_reports_nothing() {
    let mut display = layout(true);
    let badge = display.element_index(BADGE).expect("authored badge");
    let red = Color::rgb(255, 0, 0);
    assert!(
        display
            .recolor(badge, TransitionProperty::Color, red)
            .is_some()
    );
    assert_eq!(
        display.recolor(badge, TransitionProperty::Color, red),
        None,
        "a sample that changes no pixel must not claim a repaint"
    );
}

#[test]
fn a_host_overlay_belongs_to_no_element() {
    let mut display = layout(true);
    display
        .append_line((0, 0), (10, 0), Color::rgb(255, 0, 0))
        .expect("overlay line");
    let last = display.commands().len() - 1;
    assert_eq!(display.command_owner(last), None);
}

#[test]
fn a_copy_shares_its_payloads_and_keeps_its_owners() {
    let display = layout(true);
    let copy = display.copy_with_room(1).expect("bounded copy");
    assert_eq!(copy.commands().len(), display.commands().len());
    for index in 0..display.commands().len() {
        assert_eq!(copy.command_owner(index), display.command_owner(index));
    }
    let badge = copy.element_index(BADGE).expect("copied badge");
    assert_eq!(
        copy.element_transition(badge),
        display.element_transition(badge)
    );
}
