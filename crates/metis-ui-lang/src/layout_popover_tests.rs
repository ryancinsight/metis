//! Layout tests for anchored popovers.

use super::{DisplayCommand, LayoutViewport, compute_layout};
use crate::parse_markup;
use crate::style::Color;
use metis_core::error::ErrorCode;
use metis_platform::DisplayScale;
use metis_platform::framebuffer::{Framebuffer, Rect};

#[test]
fn anchored_popover_is_out_of_flow_and_paints_after_document() {
    let document = parse_markup(
        "<root style='width:20px;background:#111111'><anchor id='trigger' style='width:6px;height:4px;background:#ff0000'/><popover id='menu' popover-anchor='trigger' style='width:8px;height:5px;background:#0000ff'/><sibling id='content' style='width:10px;height:3px;background:#00ff00'/></root>",
    )
    .expect("popover markup");
    let display = compute_layout(&document, LayoutViewport::new(20, 20)).expect("popover layout");

    assert_eq!(display.element_rect("trigger"), Some(Rect::new(0, 0, 6, 4)));
    assert_eq!(
        display.element_rect("content"),
        Some(Rect::new(0, 4, 10, 3))
    );
    assert_eq!(display.element_rect("menu"), Some(Rect::new(0, 4, 8, 5)));
    let content_paint = display
        .commands
        .iter()
        .position(|command| matches!(command, DisplayCommand::FillRect { color, .. } if *color == Color::rgb(0, 255, 0)))
        .expect("content paint");
    let popover_paint = display
        .commands
        .iter()
        .position(|command| matches!(command, DisplayCommand::FillRect { color, .. } if *color == Color::rgb(0, 0, 255)))
        .expect("popover paint");
    assert!(popover_paint > content_paint);

    let mut framebuffer = Framebuffer::new(20, 20).expect("surface");
    display.render_to(&mut framebuffer);
    assert_eq!(framebuffer.get_pixel(0, 4), Color::rgb(0, 0, 255));
    assert_eq!(framebuffer.get_pixel(9, 4), Color::rgb(0, 255, 0));
}

#[test]
fn anchored_popover_measures_proportional_text() {
    let width = |label: &str| {
        let document = parse_markup(&format!(
            "<root><anchor id='trigger' style='width:4px;height:2px'/><popover id='menu' popover-anchor='trigger' style='padding:3px 4px'><text>{label}</text></popover></root>"
        ))
        .expect("popover markup");
        compute_layout(&document, LayoutViewport::new(100, 40))
            .expect("popover layout")
            .element_rect("menu")
            .expect("menu rectangle")
            .width
    };

    assert!(width("WWW") > width("iii"));
}

#[test]
fn anchored_popover_fits_horizontally_and_flips_above() {
    let document = parse_markup(
        "<root style='padding:8px 0 0 12px'><anchor id='trigger' style='width:4px;height:4px'/><popover id='menu' popover-anchor='trigger' style='width:9px;height:7px;background:#0000ff'/></root>",
    )
    .expect("popover markup");
    let display = compute_layout(&document, LayoutViewport::new(16, 15)).expect("popover layout");

    assert_eq!(
        display.element_rect("trigger"),
        Some(Rect::new(12, 8, 4, 4))
    );
    assert_eq!(display.element_rect("menu"), Some(Rect::new(7, 1, 9, 7)));
}

#[test]
fn visible_popover_requires_a_laid_out_anchor() {
    for markup in [
        "<root><popover popover-anchor='missing' style='width:4px;height:4px'/></root>",
        "<root><anchor id='hidden' style='display:none'/><popover popover-anchor='hidden' style='width:4px;height:4px'/></root>",
    ] {
        let document = parse_markup(markup).expect("popover markup");
        let error = compute_layout(&document, LayoutViewport::new(20, 20))
            .expect_err("missing popover anchor");
        assert_eq!(error.code, ErrorCode::MalformedMarkup);
        assert!(error.message.contains("anchor absent"));
    }
}

#[test]
fn anchored_popover_uses_scaled_physical_coordinates() {
    let document = parse_markup(
        "<root style='padding:2px'><anchor id='trigger' style='width:4px;height:4px'/><popover id='menu' popover-anchor='trigger' style='width:6px;height:4px'/></root>",
    )
    .expect("popover markup");
    let scale = DisplayScale::from_milli(1_500).expect("150 percent");
    let display = compute_layout(&document, LayoutViewport::with_scale(30, 30, scale))
        .expect("scaled popover layout");

    assert_eq!(display.element_rect("trigger"), Some(Rect::new(3, 3, 6, 6)));
    assert_eq!(display.element_rect("menu"), Some(Rect::new(3, 9, 9, 6)));
}

#[test]
fn oversized_popover_keeps_anchor_edge_and_relies_on_framebuffer_clipping() {
    let document = parse_markup(
        "<root style='padding:2px 0 0 0;background:#00ff00'><anchor id='trigger' style='width:2px;height:2px'/><popover id='menu' popover-anchor='trigger' style='width:14px;height:12px;background:#0000ff'/></root>",
    )
    .expect("popover markup");
    let display = compute_layout(&document, LayoutViewport::new(10, 10)).expect("popover layout");

    assert_eq!(display.element_rect("menu"), Some(Rect::new(0, 4, 14, 12)));
    let mut framebuffer = Framebuffer::new(10, 10).expect("surface");
    display.render_to(&mut framebuffer);
    assert_eq!(framebuffer.get_pixel(9, 9), Color::rgb(0, 0, 255));
}

#[test]
fn hidden_popover_is_absent_without_resolving_its_anchor() {
    let document = parse_markup(
        "<root><popover id='menu' popover-anchor='missing' style='display:none;width:4px;height:4px;background:#0000ff'/><content id='content' style='width:3px;height:2px;background:#00ff00'/></root>",
    )
    .expect("popover markup");
    let display = compute_layout(&document, LayoutViewport::new(10, 10)).expect("hidden popover");

    assert_eq!(display.element_rect("menu"), None);
    assert_eq!(display.element_rect("content"), Some(Rect::new(0, 0, 3, 2)));
    assert!(!display.commands.iter().any(
        |command| matches!(command, DisplayCommand::FillRect { color, .. } if *color == Color::rgb(0, 0, 255))
    ));
}

#[test]
fn nested_popover_can_anchor_to_content_laid_out_in_its_parent_popover() {
    let document = parse_markup(
        "<root><anchor id='trigger' style='width:4px;height:2px'/><popover id='menu' popover-anchor='trigger' style='width:10px;height:6px'><anchor id='submenu-trigger' style='width:3px;height:2px'/><popover id='submenu' popover-anchor='submenu-trigger' style='width:4px;height:3px'/></popover></root>",
    )
    .expect("nested popover markup");
    let display = compute_layout(&document, LayoutViewport::new(20, 20)).expect("nested popover");

    assert_eq!(display.element_rect("menu"), Some(Rect::new(0, 2, 10, 6)));
    assert_eq!(
        display.element_rect("submenu-trigger"),
        Some(Rect::new(0, 2, 3, 2))
    );
    assert_eq!(display.element_rect("submenu"), Some(Rect::new(0, 4, 4, 3)));
}
