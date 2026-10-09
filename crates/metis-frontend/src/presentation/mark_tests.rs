//! Tests for the application mark painted into its authored anchor box.

use super::mark::{APP_MARK_ID, APP_MARK_PNG};
use crate::app::FrontendApp;
use metis_ipc::MemoryTransport;
use metis_ui_lang::{DisplayCommand, RasterImage, Rect};

/// The initial rendered form and its mark anchor box.
fn painted() -> (FrontendApp<MemoryTransport>, Rect) {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("initial form");
    let display = app.painted.as_ref().expect("painted frame");
    let anchor = display.element_rect(APP_MARK_ID).expect("anchor box");
    (app, anchor)
}

#[test]
fn app_mark_fills_the_authored_anchor_box() {
    let (app, anchor) = painted();
    let display = app.painted.as_ref().expect("painted frame");
    let placement = display
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::DrawImage { placement } => Some(placement.as_ref()),
            _ => None,
        })
        .expect("mark command");
    assert_eq!(placement.destination(), anchor);
    assert_eq!(anchor.width, anchor.height, "the mark box is square");
    assert!(anchor.width > 0, "the mark box is visible");
    let header = display.element_rect("header").expect("header box");
    assert!(
        header.contains(anchor.x, anchor.y)
            && header.contains(anchor.x + anchor.width - 1, anchor.y + anchor.height - 1),
        "the mark box lies inside the header"
    );
    let image = RasterImage::decode(APP_MARK_PNG).expect("committed mark decodes");
    assert_eq!(
        placement.source(),
        Rect::new(
            0,
            0,
            i32::try_from(image.width()).expect("mark width"),
            i32::try_from(image.height()).expect("mark height"),
        ),
        "the whole committed mark is placed"
    );
}

#[test]
fn app_mark_pixels_are_painted_from_the_committed_mark() {
    let (app, anchor) = painted();
    let image = RasterImage::decode(APP_MARK_PNG).expect("committed mark decodes");
    let opaque: std::collections::HashSet<[u8; 4]> = image
        .pixels()
        .iter()
        .filter(|pixel| pixel.a == 255)
        .map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
        .collect();
    assert!(!opaque.is_empty(), "the mark has opaque pixels");
    let mut marked = 0;
    for y in anchor.y..anchor.y + anchor.height {
        for x in anchor.x..anchor.x + anchor.width {
            let pixel = app.framebuffer.get_pixel(x, y);
            if opaque.contains(&[pixel.r, pixel.g, pixel.b, pixel.a]) {
                marked += 1;
            }
        }
    }
    assert!(marked > 0, "committed mark pixels reach the anchor box");
}

/// The node with `id` anywhere in the tree, at any depth.
fn find<'a>(
    node: &'a metis_ui_lang::SemanticNode,
    id: &str,
) -> Option<&'a metis_ui_lang::SemanticNode> {
    if node.id.as_deref() == Some(id) {
        return Some(node);
    }
    node.children.iter().find_map(|child| find(child, id))
}

#[test]
fn the_mark_box_is_hidden_and_inert_in_the_semantics() {
    let (transport, _peer) = MemoryTransport::pair();
    let app = FrontendApp::new(transport, 800, 600).expect("initial form");
    let tree = app.semantic_tree().expect("semantic tree");
    let mark = find(&tree.root, APP_MARK_ID).expect("mark node");
    assert!(mark.hidden, "the decorative mark stays out of the tree");
    assert!(!mark.focusable);
    assert!(mark.actions.is_empty());
    assert!(find(&tree.root, "app-title").is_some(), "the title stays");
}
