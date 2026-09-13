use super::*;
use crate::parse_markup;
use metis_core::error::ErrorCode;

#[test]
fn parent_background_precedes_child_and_gap_is_between_children() {
    let doc = parse_markup("<a style='background:#f00;gap:3px'><b style='height:2px;background:#00f'/><c style='height:2px;background:#0f0'/></a>").expect("markup");
    let list = compute_layout(&doc, 4, 7).expect("layout");
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
        compute_layout(&doc, 8, 16)
            .expect_err("coordinate overflow")
            .code,
        ErrorCode::LayoutOverflow
    );
    let mut element = DomElement::new("a");
    element.computed_style.width = Size::Percent(f32::NAN);
    let doc = DomDocument::new(element);
    assert_eq!(
        compute_layout(&doc, 8, 16).expect_err("invalid size").code,
        ErrorCode::LayoutOverflow
    );
}

#[test]
fn programmatic_unsupported_style_is_rejected_before_painting() {
    let mut root = DomElement::new("root");
    root.computed_style.border_radius = 2;
    root.computed_style.background_color = Some(Color::RED);
    let error = compute_layout(&DomDocument::new(root), 4, 4)
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
        compute_layout(&DomDocument::new(root), 1, 1)
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
        compute_layout(&DomDocument::new(root), 1, 1)
            .expect_err("depth bound")
            .code,
        ErrorCode::LayoutOverflow
    );
}

#[test]
fn iris_backend_borrows_the_rendered_frame() {
    use iris::render::RenderBackend;
    let document = parse_markup("<root style='background:#102030;height:2px'/>").expect("markup");
    let display = compute_layout(&document, 2, 2).expect("layout");
    let mut framebuffer = Framebuffer::new(2, 2).expect("surface");
    let storage = framebuffer.pixels().as_ptr();
    let frame = framebuffer
        .render(&display)
        .expect("infallible clipped drawing");
    assert_eq!(frame, &[0xff10_2030; 4]);
    assert_eq!(frame.as_ptr(), storage);
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
