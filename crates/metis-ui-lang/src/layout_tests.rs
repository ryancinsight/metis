use super::{DisplayCommand, DisplayList, LayoutViewport, compute_layout};
use crate::dom::{DomDocument, DomElement, DomNode};
use crate::parse_markup;
use crate::parser::{MAX_DEPTH, MAX_NODES};
use crate::style::{Color, Display, EdgeValues, Size};
use metis_core::error::ErrorCode;
use metis_platform::DisplayScale;
use metis_platform::framebuffer::{Framebuffer, Rect};
use metis_platform::rasterizer::{BoxShadow, CornerRadius};
use metis_platform::rasterizer::{LineCap, LineJoin, MAX_STROKE_POINTS, StrokeWidth};
use metis_platform::typeface::GlyphWeight;

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
                    DisplayCommand::DrawText { style, .. } => Some(style.weight),
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
        parent.children.push(DomNode::Element(Box::new(root)));
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
    let text_size = list
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::DrawText { style, .. } => Some(style.size.pixels()),
            _ => None,
        })
        .expect("scaled text");
    // The default 14-pixel size at 125 percent: 17.5 device pixels per em,
    // exact in binary.
    assert!((text_size - 17.5).abs() < f64::EPSILON, "{text_size}");

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

#[test]
fn box_shadow_is_painted_beneath_the_background_it_belongs_to() {
    let doc = parse_markup(
        "<a style='width:40px;height:20px;margin:10px;background:#fff;         border-radius:6px;box-shadow:0 4px 8px #00000080'/>",
    )
    .expect("markup");
    let list = compute_layout(&doc, LayoutViewport::new(80, 60)).expect("layout");
    let border_box = Rect::new(10, 10, 40, 20);
    let radius = CornerRadius::clamped(6, border_box);
    assert_eq!(
        list.commands[..2],
        [
            DisplayCommand::DrawShadow {
                rect: border_box,
                radius,
                shadow: BoxShadow::new(0, 4, 8, Color::rgba(0, 0, 0, 0x80)).expect("blur"),
            },
            DisplayCommand::FillRect {
                rect: border_box,
                radius,
                color: Color::WHITE,
            },
        ]
    );
    let background = Color::rgb(200, 210, 220);
    let mut fb = Framebuffer::new(80, 60).expect("surface");
    fb.clear(background);
    list.render_to(&mut fb);
    // The box paints over its own shadow; the offset shadow shows below it
    // and fades with distance.
    assert_eq!(fb.get_pixel(30, 20), Color::WHITE);
    let near = fb.get_pixel(30, 31);
    let far = fb.get_pixel(30, 38);
    assert!(
        near.r < far.r && far.r < background.r,
        "{near:?} then {far:?}"
    );
    // Five pixels outside either horizontal edge, the downward offset leaves
    // the pixel above the box lighter than the one below it.
    let above = fb.get_pixel(30, 4);
    let below = fb.get_pixel(30, 35);
    assert!(above.r > below.r, "{above:?} above, {below:?} below");
}

#[test]
fn box_shadow_follows_the_display_scale_and_its_blur_bound() {
    let scaled = LayoutViewport::with_scale(
        200,
        200,
        DisplayScale::from_milli(2_000).expect("200 percent"),
    );
    let doc = parse_markup("<a style='width:20px;height:10px;box-shadow:-1px 3px 5px #000'/>")
        .expect("markup");
    let list = compute_layout(&doc, scaled).expect("layout");
    assert_eq!(
        list.commands[0],
        DisplayCommand::DrawShadow {
            rect: Rect::new(0, 0, 40, 20),
            radius: CornerRadius::SQUARE,
            shadow: BoxShadow::new(-2, 6, 10, Color::BLACK).expect("blur"),
        }
    );
    // 200 authored pixels at twice the scale exceed the renderer's bound.
    let doc = parse_markup("<a style='width:20px;height:10px;box-shadow:0 0 200px #000'/>")
        .expect("markup");
    assert_eq!(
        compute_layout(&doc, scaled).expect_err("blur bound").code,
        ErrorCode::LayoutOverflow
    );
    const { assert!(200 * 2 > BoxShadow::MAX_BLUR && 200 <= BoxShadow::MAX_BLUR) };
}

#[test]
fn aligned_children_carry_their_shadow_with_them() {
    let doc = parse_markup(
        "<a style='width:100px;height:40px;justify-content:center;align-items:center'>         <b style='width:30px;height:10px;background:#fff;box-shadow:0 2px 4px #0003'/></a>",
    )
    .expect("markup");
    let list = compute_layout(&doc, LayoutViewport::new(100, 40)).expect("layout");
    let rects: Vec<Rect> = list
        .commands
        .iter()
        .filter_map(|command| match command {
            DisplayCommand::DrawShadow { rect, .. } | DisplayCommand::FillRect { rect, .. } => {
                Some(*rect)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        rects,
        [Rect::new(35, 15, 30, 10), Rect::new(35, 15, 30, 10)]
    );
}

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
