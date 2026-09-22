use super::{DisplayCommand, DisplayList, LayoutViewport, compute_layout};
use crate::dom::{DomDocument, DomElement, DomNode};
use crate::parse_markup;
use crate::parser::{MAX_DEPTH, MAX_NODES};
use crate::style::{Color, Display, EdgeValues, JustifyContent, Size};
use metis_core::error::ErrorCode;
use metis_platform::DisplayScale;
use metis_platform::GlyphWeight;
use metis_platform::framebuffer::{Framebuffer, Rect};
use metis_platform::rasterizer::CornerRadius;
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
            radius: CornerRadius::SQUARE,
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
    root.computed_style.justify_content = JustifyContent::Center;
    root.computed_style.background_color = Some(Color::RED);
    let error = compute_layout(&DomDocument::new(root), LayoutViewport::new(4, 4))
        .expect_err("unsupported style must not be silently ignored");
    assert_eq!(error.code, ErrorCode::InvalidCssStyle);
    assert!(error.message.contains("justify-content"));
}

#[test]
fn programmatic_border_radius_reaches_the_fill_and_border_commands() {
    let mut root = DomElement::new("root");
    root.computed_style.border_radius = 6;
    root.computed_style.background_color = Some(Color::RED);
    root.computed_style.border_width = EdgeValues::all(2);
    root.computed_style.width = Size::Px(40);
    root.computed_style.height = Size::Px(30);
    let display = compute_layout(&DomDocument::new(root), LayoutViewport::new(60, 60))
        .expect("an admitted radius lays out");
    let radii: Vec<_> = display
        .commands
        .iter()
        .filter_map(|command| match command {
            DisplayCommand::FillRect { radius, .. } | DisplayCommand::DrawBorder { radius, .. } => {
                Some(radius.pixels())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        radii,
        vec![6, 6],
        "fill and border carry the authored radius"
    );
}

#[test]
fn an_authored_border_radius_clears_the_painted_corner() {
    let markup = concat!(
        "<card id=\"root\" style=\"background-color: #3182ce; border-radius: 10px; ",
        "width: 40px; height: 40px;\"></card>"
    );
    let document = parse_markup(markup).expect("authored radius parses");
    let display = compute_layout(&document, LayoutViewport::new(40, 40)).expect("lays out");
    let mut rounded = Framebuffer::new(40, 40).expect("surface");
    rounded.clear(Color::WHITE);
    display.render_to(&mut rounded);

    let square_markup = concat!(
        "<card id=\"root\" style=\"background-color: #3182ce; ",
        "width: 40px; height: 40px;\"></card>"
    );
    let square_document = parse_markup(square_markup).expect("square parses");
    let square_display =
        compute_layout(&square_document, LayoutViewport::new(40, 40)).expect("lays out");
    let mut square = Framebuffer::new(40, 40).expect("surface");
    square.clear(Color::WHITE);
    square_display.render_to(&mut square);

    // The square fill paints its extreme corner; the rounded one leaves it.
    assert_eq!(square.get_pixel(0, 0), Color::rgb(49, 130, 206));
    assert_eq!(rounded.get_pixel(0, 0), Color::WHITE);
    // Both keep the centre and the straight edge midpoints.
    for surface in [&rounded, &square] {
        assert_eq!(surface.get_pixel(20, 20), Color::rgb(49, 130, 206));
        assert_eq!(surface.get_pixel(20, 0), Color::rgb(49, 130, 206));
        assert_eq!(surface.get_pixel(0, 20), Color::rgb(49, 130, 206));
    }
    // The authored radius antialiases, so the arc carries partial coverage.
    let partial = (0..12)
        .flat_map(|x| (0..12).map(move |y| (x, y)))
        .filter(|(x, y)| {
            let pixel = rounded.get_pixel(*x, *y);
            pixel != Color::WHITE && pixel != Color::rgb(49, 130, 206)
        })
        .count();
    assert!(
        partial >= 8,
        "authored radius is not antialiased: {partial}"
    );
}

#[test]
fn an_authored_bold_weight_paints_heavier_strokes() {
    let ink = |weight: &str| {
        let markup = format!(
            "<card id=\"root\" style=\"font-weight: {weight}; width: 120px; height: 20px;\">Backend</card>"
        );
        let document = parse_markup(&markup).expect("authored weight parses");
        let display = compute_layout(&document, LayoutViewport::new(120, 20)).expect("lays out");
        let mut fb = Framebuffer::new(120, 20).expect("surface");
        display.render_to(&mut fb);
        (
            display
                .commands
                .iter()
                .filter_map(|command| match command {
                    DisplayCommand::DrawText { weight, .. } => Some(*weight),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            fb.pixels().iter().filter(|pixel| **pixel != 0).count(),
        )
    };
    let (regular_weights, regular_ink) = ink("normal");
    let (bold_weights, bold_ink) = ink("bold");
    assert_eq!(regular_weights, vec![GlyphWeight::Regular]);
    assert_eq!(bold_weights, vec![GlyphWeight::Bold]);
    assert!(
        bold_ink > regular_ink,
        "bold painted {bold_ink}, regular {regular_ink}"
    );
}

/// The laid-out rectangle of the only element in a document.
fn only_rect(markup: &str, viewport: LayoutViewport) -> Rect {
    let document = parse_markup(markup).expect("authored markup parses");
    let display = compute_layout(&document, viewport).expect("lays out");
    display
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::FillRect { rect, .. } => Some(*rect),
            _ => None,
        })
        .expect("the background fill carries the laid-out rectangle")
}

#[test]
fn a_minimum_raises_a_smaller_extent_and_leaves_a_larger_one() {
    let viewport = LayoutViewport::new(200, 200);
    // Raised: the declared extent is below the minimum on both axes.
    let raised = only_rect(
        "<card id=\"root\" style=\"background-color: #ff0000; width: 10px; height: 12px;          min-width: 44px; min-height: 40px;\"></card>",
        viewport,
    );
    assert_eq!((raised.width, raised.height), (44, 40));
    // Unchanged: the declared extent already clears the minimum.
    let clear = only_rect(
        "<card id=\"root\" style=\"background-color: #ff0000; width: 80px; height: 60px;          min-width: 44px; min-height: 40px;\"></card>",
        viewport,
    );
    assert_eq!((clear.width, clear.height), (80, 60));
}

#[test]
fn a_minimum_raises_an_automatic_extent() {
    // An empty element has no content, so its automatic height is its edges;
    // the minimum is what keeps it visible.
    let rect = only_rect(
        "<card id=\"root\" style=\"background-color: #ff0000; min-height: 36px;\"></card>",
        LayoutViewport::new(120, 120),
    );
    assert_eq!(rect.height, 36);
}

#[test]
fn a_percentage_minimum_resolves_against_the_available_extent() {
    let rect = only_rect(
        "<card id=\"root\" style=\"background-color: #ff0000; width: 10px; min-width: 50%;\"></card>",
        LayoutViewport::new(120, 80),
    );
    assert_eq!(rect.width, 60);
}

#[test]
fn a_minimum_scales_with_the_host_display_scale() {
    let markup = "<card id=\"root\" style=\"background-color: #ff0000; width: 10px;          min-width: 40px; min-height: 20px;\"></card>";
    let document = parse_markup(markup).expect("parses");
    let scaled = compute_layout(
        &document,
        LayoutViewport::with_scale(
            200,
            200,
            DisplayScale::from_milli(1_500).expect("150 percent"),
        ),
    )
    .expect("lays out");
    let rect = scaled
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::FillRect { rect, .. } => Some(*rect),
            _ => None,
        })
        .expect("background fill");
    // A minimum is a length, so it scales exactly as width and height do.
    assert_eq!((rect.width, rect.height), (60, 30));
}

#[test]
fn border_radius_is_clamped_to_the_laid_out_rectangle() {
    let mut root = DomElement::new("root");
    // Half of the shorter side is ten, so a larger request cannot round past it.
    root.computed_style.border_radius = 400;
    root.computed_style.background_color = Some(Color::RED);
    root.computed_style.width = Size::Px(40);
    root.computed_style.height = Size::Px(20);
    let display = compute_layout(&DomDocument::new(root), LayoutViewport::new(60, 60))
        .expect("an oversized radius clamps rather than failing");
    let radius = display
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::FillRect { radius, .. } => Some(radius.pixels()),
            _ => None,
        })
        .expect("the background fill is emitted");
    assert_eq!(radius, 10);
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
            DisplayCommand::FillRect { rect, color, .. } if *color == Color::rgb(255, 0, 0) => {
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
            DisplayCommand::FillRect { rect, color, .. } if *color == Color::rgb(255, 0, 0) => {
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
