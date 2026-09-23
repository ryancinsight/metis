//! Layout tests for sizing and alignment.
//!
//! Flow, painter order and text geometry stay in `layout_tests.rs`; this file
//! covers the extents an element resolves and where its children land inside
//! the free space their container leaves.

use super::{DisplayCommand, LayoutViewport, compute_layout};
use crate::parse_markup;
use crate::style::Color;
use metis_platform::DisplayScale;
use metis_platform::framebuffer::Rect;

/// Every child rectangle a laid-out document emits, in painter order.
fn child_rects(markup: &str, viewport: LayoutViewport) -> Vec<Rect> {
    let document = parse_markup(markup).expect("authored markup parses");
    let display = compute_layout(&document, viewport).expect("lays out");
    display
        .commands
        .iter()
        .filter_map(|command| match command {
            DisplayCommand::FillRect { rect, color, .. } if *color == Color::rgb(0, 0, 255) => {
                Some(*rect)
            }
            _ => None,
        })
        .collect()
}

/// Three fixed-size children in a taller column, under one alignment pair.
fn column_children(justify: &str, align: &str) -> Vec<Rect> {
    let markup = format!(
        "<card id=\"root\" style=\"height: 200px; width: 100px;          justify-content: {justify}; align-items: {align};\">         <card id=\"a\" style=\"background-color: #0000ff; width: 20px; height: 30px;\"></card>         <card id=\"b\" style=\"background-color: #0000ff; width: 20px; height: 30px;\"></card>         <card id=\"c\" style=\"background-color: #0000ff; width: 20px; height: 30px;\"></card>         </card>"
    );
    child_rects(&markup, LayoutViewport::new(100, 200))
}

#[test]
fn start_alignment_leaves_every_child_where_it_was_laid_out() {
    // The contract that keeps every existing document and capture unchanged:
    // the default pair introduces no offset at all.
    let defaults = column_children("flex-start", "stretch");
    assert_eq!(
        defaults.iter().map(|r| (r.x, r.y)).collect::<Vec<_>>(),
        vec![(0, 0), (0, 30), (0, 60)]
    );
}

#[test]
fn main_axis_distribution_places_children_in_the_free_space() {
    // Ninety of the two hundred pixels are occupied, leaving a hundred and ten.
    for (keyword, expected) in [
        ("flex-start", vec![0, 30, 60]),
        ("center", vec![55, 85, 115]),
        ("flex-end", vec![110, 140, 170]),
        // The first child holds the start edge and the last reaches the end.
        ("space-between", vec![0, 85, 170]),
    ] {
        let tops: Vec<_> = column_children(keyword, "stretch")
            .iter()
            .map(|rect| rect.y)
            .collect();
        assert_eq!(tops, expected, "justify-content: {keyword}");
    }
}

#[test]
fn cross_axis_alignment_places_children_across_the_container() {
    // Each child is twenty wide in a hundred-wide container.
    for (keyword, expected) in [
        ("flex-start", 0),
        ("center", 40),
        ("flex-end", 80),
        ("stretch", 0),
    ] {
        let lefts: Vec<_> = column_children("flex-start", keyword)
            .iter()
            .map(|rect| rect.x)
            .collect();
        assert_eq!(lefts, vec![expected; 3], "align-items: {keyword}");
    }
}

#[test]
fn a_full_container_distributes_nothing() {
    // Three thirty-pixel children exactly fill ninety pixels, so every
    // keyword must agree with start alignment.
    let markup = |justify: &str| {
        format!(
            "<card id=\"root\" style=\"height: 90px; width: 100px; justify-content: {justify};\">             <card id=\"a\" style=\"background-color: #0000ff; height: 30px;\"></card>             <card id=\"b\" style=\"background-color: #0000ff; height: 30px;\"></card>             <card id=\"c\" style=\"background-color: #0000ff; height: 30px;\"></card>             </card>"
        )
    };
    let start = child_rects(&markup("flex-start"), LayoutViewport::new(100, 90));
    for keyword in ["center", "flex-end", "space-between"] {
        assert_eq!(
            child_rects(&markup(keyword), LayoutViewport::new(100, 90)),
            start,
            "justify-content: {keyword} moved a child with no free space"
        );
    }
}

#[test]
fn alignment_moves_every_command_a_child_emits() {
    // A child carrying text must move with its box, or the run tears away
    // from the surface it labels.
    let markup = "<card id=\"root\" style=\"height: 200px; width: 200px;          justify-content: flex-end;\">         <card id=\"a\" style=\"background-color: #0000ff; height: 30px;\">Rate</card>         </card>";
    let document = parse_markup(markup).expect("parses");
    let display = compute_layout(&document, LayoutViewport::new(200, 200)).expect("lays out");
    let box_top = display
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::FillRect { rect, color, .. } if *color == Color::rgb(0, 0, 255) => {
                Some(rect.y)
            }
            _ => None,
        })
        .expect("child fill");
    let text_top = display
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::DrawText { y, .. } => Some(*y),
            _ => None,
        })
        .expect("child text");
    assert_eq!(box_top, 170, "the child did not reach the end edge");
    assert_eq!(text_top, box_top, "the text run did not move with its box");
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

/// The blue child of a 200-pixel column under `align`, with `child` styles.
fn sized_child(align: &str, child: &str, content: &str) -> Rect {
    let markup = format!(
        "<card style=\"width: 200px; align-items: {align};\"><card style=\"background-color: #0000ff; {child}\">{content}</card></card>"
    );
    let rects = child_rects(&markup, LayoutViewport::new(200, 100));
    assert_eq!(rects.len(), 1, "{markup}");
    rects[0]
}

/// Whole-pixel advance of `text` at the default 14-pixel regular size.
fn advance(text: &str) -> i32 {
    let style = metis_platform::typeface::TextStyle::new(
        Color::BLACK,
        metis_platform::typeface::TextSize::new(14.0).expect("valid size"),
    );
    let width = style.advance(text).ceil();
    assert!((0.0..1000.0).contains(&width));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole advance checked to lie in 0..1000"
    )]
    let width = width as i32;
    width
}

#[test]
fn a_column_that_does_not_stretch_sizes_children_to_their_content() {
    // Content width: the label's advance plus four pixels of padding each
    // side. Height: the 14-pixel line box, 1240 units at 14/1000 px rounded
    // up to 18, plus the same padding.
    let width = advance("Go") + 8;
    assert_eq!(
        sized_child("center", "padding: 4px;", "Go"),
        Rect::new((200 - width) / 2, 0, width, 26)
    );
    assert_eq!(
        sized_child("flex-start", "padding: 4px;", "Go"),
        Rect::new(0, 0, width, 26)
    );
    assert_eq!(
        sized_child("flex-end", "padding: 4px;", "Go"),
        Rect::new(200 - width, 0, width, 26)
    );
    // Stretch, the default, keeps filling the column.
    assert_eq!(
        sized_child("stretch", "padding: 4px;", "Go"),
        Rect::new(0, 0, 200, 26)
    );
}

#[test]
fn content_sizing_respects_declared_widths_rows_and_the_available_width() {
    // A declared width is taken as written.
    assert_eq!(
        sized_child("center", "width: 50px; height: 10px;", ""),
        Rect::new(75, 0, 50, 10)
    );
    // A row sums its children and gaps: 30 + 6 + 40.
    let row = "<card style=\"width: 30px; height: 5px;\"></card><card style=\"width: 40px; height: 5px;\"></card>";
    assert_eq!(
        sized_child("center", "flex-direction: row; gap: 6px;", row),
        Rect::new(62, 0, 76, 5)
    );
    // Content wider than the column is capped at the column's width.
    let long = "A label far longer than the two hundred pixel column it sits in";
    assert!(advance(long) > 200);
    assert_eq!(sized_child("center", "", long).width, 200);
}
