use super::{DisplayCommand, DisplayList, LayoutViewport, compute_layout};
use crate::dom::{DomDocument, DomElement, DomNode};
use crate::parse_markup;
use crate::parser::{MAX_DEPTH, MAX_NODES};
use crate::style::{Color, Display, Size};
use metis_core::error::ErrorCode;
use metis_platform::framebuffer::{Framebuffer, Rect};
use metis_platform::rasterizer::{LineCap, LineJoin, MAX_STROKE_POINTS, StrokeWidth};

#[test]
fn parent_background_precedes_child_and_gap_is_between_children() {
    let doc = parse_markup("<a style='background:#f00;gap:3px'><b style='height:2px;background:#00f'/><c style='height:2px;background:#0f0'/></a>").expect("markup");
    let list = compute_layout(&doc, LayoutViewport::new(4, 7)).expect("layout");
    let mut fb = Framebuffer::new(4, 7).expect("surface");
    list.render_to(&mut fb);
    assert_eq!(fb.get_pixel(0, 0), Color::rgb(0, 0, 255));
    assert_eq!(fb.get_pixel(0, 3), Color::rgb(255, 0, 0));
    assert_eq!(fb.get_pixel(0, 6), Color::rgb(0, 255, 0));
    assert_eq!(
        list.commands[0],
        DisplayCommand::FillRect {
            rect: Rect::new(0, 0, 4, 7),
            color: Color::rgb(255, 0, 0)
        }
    );
}

#[test]
fn extreme_styles_return_errors_without_wrapping() {
    let doc = parse_markup("<a style='padding:2147483647px'>x</a>").expect("markup");
    assert_eq!(
        compute_layout(&doc, LayoutViewport::new(8, 16))
            .expect_err("coordinate overflow")
            .code,
        ErrorCode::LayoutOverflow
    );
    let mut element = DomElement::new("a");
    element.computed_style.width = Size::Percent(f32::NAN);
    let doc = DomDocument::new(element);
    assert_eq!(
        compute_layout(&doc, LayoutViewport::new(8, 16))
            .expect_err("invalid size")
            .code,
        ErrorCode::LayoutOverflow
    );
}

#[test]
fn programmatic_unsupported_style_is_rejected_before_painting() {
    let mut root = DomElement::new("root");
    root.computed_style.border_radius = 2;
    root.computed_style.background_color = Some(Color::RED);
    let error = compute_layout(&DomDocument::new(root), LayoutViewport::new(4, 4))
        .expect_err("unsupported style must not be silently ignored");
    assert_eq!(error.code, ErrorCode::InvalidCssStyle);
    assert!(error.message.contains("border-radius"));
}

#[test]
fn hidden_programmatic_trees_still_obey_resource_limits() {
    let mut root = DomElement::new("root");
    root.computed_style.display = Display::None;
    root.children = vec![DomNode::Text(String::new()); MAX_NODES];
    assert_eq!(
        compute_layout(&DomDocument::new(root), LayoutViewport::new(1, 1))
            .expect_err("node bound")
            .code,
        ErrorCode::LayoutOverflow
    );
    let mut root = DomElement::new("leaf");
    for _ in 0..MAX_DEPTH {
        let mut parent = DomElement::new("parent");
        parent.children.push(DomNode::Element(root));
        root = parent;
    }
    root.computed_style.display = Display::None;
    assert_eq!(
        compute_layout(&DomDocument::new(root), LayoutViewport::new(1, 1))
            .expect_err("depth bound")
            .code,
        ErrorCode::LayoutOverflow
    );
}

#[test]
fn iris_backend_borrows_the_rendered_frame() {
    use iris::render::RenderBackend;
    let document = parse_markup("<root style='background:#102030;height:2px'/>").expect("markup");
    let display = compute_layout(&document, LayoutViewport::new(2, 2)).expect("layout");
    let mut framebuffer = Framebuffer::new(2, 2).expect("surface");
    let storage = framebuffer.pixels().as_ptr();
    let frame = framebuffer
        .render(&display)
        .expect("infallible clipped drawing");
    assert_eq!(frame, &[0xff10_2030; 4]);
    assert_eq!(frame.as_ptr(), storage);
}

#[test]
fn fractional_display_scale_maps_geometry_and_text_to_device_pixels() {
    let document = parse_markup(
        "<root style='background:#102030;padding:20px'><box style='width:10px;height:10px;background:#ff0000'><text style='color:#ffffff'>A</text></box></root>",
    )
    .expect("markup");
    let scale = metis_platform::DisplayScale::from_milli(1_250).expect("125 percent");
    let list = compute_layout(&document, LayoutViewport::with_scale(100, 100, scale))
        .expect("scaled layout");
    let child = list
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::FillRect { rect, color } if *color == Color::rgb(255, 0, 0) => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("scaled child fill");
    assert_eq!(child, Rect::new(25, 25, 13, 13));
    let text_scale = list
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::DrawText {
                display_scale,
                scale,
                ..
            } => Some((*display_scale, *scale)),
            _ => None,
        })
        .expect("scaled text");
    assert_eq!(text_scale, (scale, 1));

    let mut framebuffer = Framebuffer::new(100, 100).expect("surface");
    list.render_to(&mut framebuffer);
    assert_eq!(framebuffer.get_pixel(25, 25), Color::rgb(255, 0, 0));
    assert!((25..38).any(|x| { (25..38).any(|y| framebuffer.get_pixel(x, y) == Color::WHITE) }));
    assert_eq!(framebuffer.get_pixel(24, 24), Color::rgb(16, 32, 48));
}

#[test]
fn percentage_dimensions_use_the_physical_viewport_once() {
    let document = parse_markup(
        "<root style='background:#102030'><box style='width:50%;height:10px;background:#ff0000'/></root>",
    )
    .expect("markup");
    let scale = metis_platform::DisplayScale::from_milli(1_500).expect("150 percent");
    let list = compute_layout(&document, LayoutViewport::with_scale(100, 80, scale))
        .expect("scaled percentage layout");
    let child = list
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::FillRect { rect, color } if *color == Color::rgb(255, 0, 0) => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("percentage child fill");
    assert_eq!(child.width, 50);
    assert_eq!(child.height, 15);
}

#[test]
fn display_line_command_renders_through_the_same_framebuffer() {
    let mut display = DisplayList::default();
    display
        .append_line((-8, -8), (8, 8), Color::RED)
        .expect("line command");
    let mut framebuffer = Framebuffer::new(3, 3).expect("surface");
    display.render_to(&mut framebuffer);
    assert_eq!(display.commands.len(), 1);
    assert_eq!(framebuffer.get_pixel(0, 0), Color::RED);
    assert_eq!(framebuffer.get_pixel(1, 1), Color::RED);
    assert_eq!(framebuffer.get_pixel(2, 2), Color::RED);
    assert_eq!(framebuffer.get_pixel(2, 0), Color::TRANSPARENT);
}

#[test]
fn display_polyline_command_preserves_style_and_bounds_points() {
    let width = StrokeWidth::new(2).expect("positive stroke width");
    let points = [(1, 1), (3, 1), (3, 3)];
    let mut display = DisplayList::default();
    display
        .append_polyline(&points, width, LineCap::Round, LineJoin::Miter, Color::BLUE)
        .expect("polyline command");
    let mut framebuffer = Framebuffer::new(5, 5).expect("surface");
    display.render_to(&mut framebuffer);
    assert_eq!(display.commands.len(), 1);
    assert_eq!(framebuffer.get_pixel(1, 1), Color::BLUE);
    assert!(matches!(
        &display.commands[0],
        DisplayCommand::DrawPolyline {
            points: stored,
            width: stored_width,
            cap: LineCap::Round,
            join: LineJoin::Miter,
            color: Color::BLUE,
        } if stored == &points && *stored_width == width
    ));
}

#[test]
fn empty_or_oversized_polylines_are_rejected_before_storage() {
    let width = StrokeWidth::new(1).expect("positive stroke width");
    let mut display = DisplayList::default();
    assert_eq!(
        display
            .append_polyline(&[], width, LineCap::Butt, LineJoin::Bevel, Color::RED)
            .expect_err("empty path")
            .code,
        ErrorCode::LayoutOverflow
    );
    let points = vec![(0, 0); MAX_STROKE_POINTS + 1];
    assert_eq!(
        display
            .append_polyline(&points, width, LineCap::Butt, LineJoin::Bevel, Color::RED)
            .expect_err("point limit")
            .code,
        ErrorCode::LayoutOverflow
    );
    assert!(display.commands.is_empty());
}

#[test]
fn anchored_popup_is_out_of_flow_and_paints_after_document() {
    let document = parse_markup(
        "<root style='width:20px;background:#111111'><anchor id='trigger' style='width:6px;height:4px;background:#ff0000'/><popup id='menu' popover-anchor='trigger' style='width:8px;height:5px;background:#0000ff'/><sibling id='content' style='width:10px;height:3px;background:#00ff00'/></root>",
    )
    .expect("popup markup");
    let display = compute_layout(&document, LayoutViewport::new(20, 20)).expect("popup layout");

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
    let popup_paint = display
        .commands
        .iter()
        .position(|command| matches!(command, DisplayCommand::FillRect { color, .. } if *color == Color::rgb(0, 0, 255)))
        .expect("popup paint");
    assert!(popup_paint > content_paint);

    let mut framebuffer = Framebuffer::new(20, 20).expect("surface");
    display.render_to(&mut framebuffer);
    assert_eq!(framebuffer.get_pixel(0, 4), Color::rgb(0, 0, 255));
    assert_eq!(framebuffer.get_pixel(9, 4), Color::rgb(0, 255, 0));
}

#[test]
fn anchored_popup_fits_horizontally_and_flips_above() {
    let document = parse_markup(
        "<root style='padding:8px 0 0 12px'><anchor id='trigger' style='width:4px;height:4px'/><popup id='menu' popover-anchor='trigger' style='width:9px;height:7px;background:#0000ff'/></root>",
    )
    .expect("popup markup");
    let display = compute_layout(&document, LayoutViewport::new(16, 15)).expect("popup layout");

    assert_eq!(
        display.element_rect("trigger"),
        Some(Rect::new(12, 8, 4, 4))
    );
    assert_eq!(display.element_rect("menu"), Some(Rect::new(7, 1, 9, 7)));
}

#[test]
fn visible_popup_requires_a_laid_out_anchor() {
    for markup in [
        "<root><popup popover-anchor='missing' style='width:4px;height:4px'/></root>",
        "<root><anchor id='hidden' style='display:none'/><popup popover-anchor='hidden' style='width:4px;height:4px'/></root>",
    ] {
        let document = parse_markup(markup).expect("popup markup");
        let error = compute_layout(&document, LayoutViewport::new(20, 20))
            .expect_err("missing popup anchor");
        assert_eq!(error.code, ErrorCode::MalformedMarkup);
    }
}

#[test]
fn anchored_popup_uses_scaled_physical_coordinates() {
    let document = parse_markup(
        "<root style='padding:2px'><anchor id='trigger' style='width:4px;height:4px'/><popup id='menu' popover-anchor='trigger' style='width:6px;height:4px'/></root>",
    )
    .expect("popup markup");
    let scale = metis_platform::DisplayScale::from_milli(1_500).expect("150 percent");
    let display = compute_layout(&document, LayoutViewport::with_scale(30, 30, scale))
        .expect("scaled popup layout");

    assert_eq!(display.element_rect("trigger"), Some(Rect::new(3, 3, 6, 6)));
    assert_eq!(display.element_rect("menu"), Some(Rect::new(3, 9, 9, 6)));
}

#[test]
fn oversized_popup_keeps_anchor_edge_and_relies_on_framebuffer_clipping() {
    let document = parse_markup(
        "<root style='padding:2px 0 0 0;background:#00ff00'><anchor id='trigger' style='width:2px;height:2px'/><popup id='menu' popover-anchor='trigger' style='width:14px;height:12px;background:#0000ff'/></root>",
    )
    .expect("popup markup");
    let display = compute_layout(&document, LayoutViewport::new(10, 10)).expect("popup layout");

    assert_eq!(display.element_rect("menu"), Some(Rect::new(0, 4, 14, 12)));
    let mut framebuffer = Framebuffer::new(10, 10).expect("surface");
    display.render_to(&mut framebuffer);
    assert_eq!(framebuffer.get_pixel(9, 9), Color::rgb(0, 0, 255));
}

#[test]
fn hidden_popup_is_absent_without_resolving_its_anchor() {
    let document = parse_markup(
        "<root><popup id='menu' popover-anchor='missing' style='display:none;width:4px;height:4px;background:#0000ff'/><content id='content' style='width:3px;height:2px;background:#00ff00'/></root>",
    )
    .expect("popup markup");
    let display = compute_layout(&document, LayoutViewport::new(10, 10)).expect("hidden popup");

    assert_eq!(display.element_rect("menu"), None);
    assert_eq!(display.element_rect("content"), Some(Rect::new(0, 0, 3, 2)));
    assert!(!display.commands.iter().any(
        |command| matches!(command, DisplayCommand::FillRect { color, .. } if *color == Color::rgb(0, 0, 255))
    ));
}

#[test]
fn nested_popup_can_anchor_to_content_laid_out_in_its_parent_popup() {
    let document = parse_markup(
        "<root><anchor id='trigger' style='width:4px;height:2px'/><popup id='menu' popover-anchor='trigger' style='width:10px;height:6px'><anchor id='submenu-trigger' style='width:3px;height:2px'/><popup id='submenu' popover-anchor='submenu-trigger' style='width:4px;height:3px'/></popup></root>",
    )
    .expect("nested popup markup");
    let display = compute_layout(&document, LayoutViewport::new(20, 20)).expect("nested popup");

    assert_eq!(display.element_rect("menu"), Some(Rect::new(0, 2, 10, 6)));
    assert_eq!(
        display.element_rect("submenu-trigger"),
        Some(Rect::new(0, 2, 3, 2))
    );
    assert_eq!(display.element_rect("submenu"), Some(Rect::new(0, 4, 4, 3)));
}
