//! Layout tests for `flex-grow`.
//!
//! Growth is planned from intrinsic measures before children paint, so these
//! tests pin two things: where grown children land, and that the height
//! measure the column plan relies on agrees with the height layout produces.

use super::grow::shares;
use super::intrinsic::max_content_height;
use super::{LayoutViewport, compute_layout};
use crate::dom::{DomElement, DomNode};
use crate::parse_markup;
use crate::style::FlexGrow;
use metis_platform::DisplayScale;
use metis_platform::framebuffer::Rect;

/// The laid-out rectangle of every id in `ids`, in order.
fn rects(markup: &str, viewport: LayoutViewport, ids: &[&str]) -> Vec<Rect> {
    let document = parse_markup(markup).expect("authored markup parses");
    let display = compute_layout(&document, viewport).expect("lays out");
    ids.iter()
        .map(|id| {
            display
                .element_rect(id)
                .unwrap_or_else(|| panic!("#{id} laid out"))
        })
        .collect()
}

fn weights(factors: &[u32]) -> Vec<FlexGrow> {
    factors
        .iter()
        .map(|&thousandths| FlexGrow::from_thousandths(thousandths).expect("admitted factor"))
        .collect()
}

#[test]
fn shares_sum_to_exactly_the_space_they_distribute() {
    // Floored running totals: 100/3 rounds down twice and the last share
    // takes the remainder, so no pixel is lost or invented.
    assert_eq!(
        shares(100, &weights(&[1000, 1000, 1000])).expect("shares"),
        vec![33, 33, 34]
    );
    assert_eq!(
        shares(300, &weights(&[1000, 2000])).expect("shares"),
        vec![100, 200]
    );
    // A non-growing child in the list takes nothing and shifts nothing.
    assert_eq!(
        shares(90, &weights(&[0, 1000, 0, 2000])).expect("shares"),
        vec![0, 30, 0, 60]
    );
    for free in [0, 1, 7, 99, 1_000_003] {
        for factors in [
            &[1000_u32][..],
            &[1, 2, 3],
            &[250, 250, 500],
            &[999, 1, 1000, 7],
        ] {
            let total: i32 = shares(free, &weights(factors))
                .expect("shares")
                .iter()
                .sum();
            let summed: u32 = factors.iter().sum();
            let expected = if summed >= FlexGrow::ONE {
                free
            } else {
                i32::try_from(i64::from(free) * i64::from(summed) / 1000).expect("fits")
            };
            assert_eq!(total, expected, "free {free}, weights {factors:?}");
        }
    }
}

#[test]
fn weights_below_one_distribute_only_their_fraction() {
    // CSS Flexbox 1 §9.7: factors summing below one take that fraction of the
    // free space rather than all of it.
    assert_eq!(shares(200, &weights(&[500])).expect("shares"), vec![100]);
    assert_eq!(
        shares(200, &weights(&[250, 250])).expect("shares"),
        vec![50, 50]
    );
    let markup = "<card style=\"flex-direction: row; width: 200px;\">         <card id=\"half\" style=\"flex-grow: 0.5; height: 10px;\"></card>         </card>";
    assert_eq!(
        rects(markup, LayoutViewport::new(200, 50), &["half"])[0].width,
        100
    );
}

#[test]
fn a_growing_field_takes_the_row_its_label_leaves() {
    // The label keeps its declared width; the field grows into the rest.
    let markup = "<card style=\"flex-direction: row; width: 300px;\">         <card id=\"label\" style=\"width: 80px; height: 20px;\"></card>         <card id=\"field\" style=\"flex-grow: 1; height: 20px;\"></card>         </card>";
    let [label, field] = rects(markup, LayoutViewport::new(300, 50), &["label", "field"])[..]
    else {
        unreachable!("two ids")
    };
    assert_eq!((label.x, label.width), (0, 80));
    assert_eq!((field.x, field.width), (80, 220));
}

#[test]
fn growth_follows_the_weights_after_gaps_are_reserved() {
    // Three hundred pixels less a thirty-pixel gap leaves 270, split 1:2.
    let markup = "<card style=\"flex-direction: row; width: 300px; gap: 30px;\">         <card id=\"a\" style=\"flex-grow: 1;\"></card>         <card id=\"b\" style=\"flex-grow: 2;\"></card>         </card>";
    let [a, b] = rects(markup, LayoutViewport::new(300, 50), &["a", "b"])[..] else {
        unreachable!("two ids")
    };
    assert_eq!((a.x, a.width), (0, 90));
    assert_eq!((b.x, b.width), (120, 180));
}

#[test]
fn a_declared_width_is_the_base_growth_adds_to() {
    let markup = "<card style=\"flex-direction: row; width: 300px;\">         <card id=\"fixed\" style=\"width: 50px;\"></card>         <card id=\"grows\" style=\"width: 50px; flex-grow: 1;\"></card>         </card>";
    let [fixed, grows] = rects(markup, LayoutViewport::new(300, 50), &["fixed", "grows"])[..]
    else {
        unreachable!("two ids")
    };
    assert_eq!(fixed.width, 50);
    assert_eq!((grows.x, grows.width), (50, 250));
}

#[test]
fn an_overfull_row_grows_nothing() {
    // The bases already exceed the row, so there is no free space to share;
    // nothing shrinks either, since shrinking is outside the subset.
    let markup = "<card style=\"flex-direction: row; width: 100px;\">         <card id=\"a\" style=\"width: 80px;\"></card>         <card id=\"b\" style=\"width: 60px; flex-grow: 1;\"></card>         </card>";
    let [a, b] = rects(markup, LayoutViewport::new(100, 50), &["a", "b"])[..] else {
        unreachable!("two ids")
    };
    assert_eq!((a.width, b.x, b.width), (80, 80, 60));
}

#[test]
fn a_column_body_grows_between_its_header_and_footer() {
    for container in ["height: 200px;", "min-height: 200px;"] {
        let markup = format!(
            "<card style=\"{container}\">             <card id=\"header\" style=\"height: 30px;\"></card>             <card id=\"body\" style=\"flex-grow: 1;\"></card>             <card id=\"footer\" style=\"height: 30px;\"></card>             </card>"
        );
        let [header, body, footer] = rects(
            &markup,
            LayoutViewport::new(100, 300),
            &["header", "body", "footer"],
        )[..] else {
            unreachable!("three ids")
        };
        assert_eq!((header.y, header.height), (0, 30), "{container}");
        assert_eq!((body.y, body.height), (30, 140), "{container}");
        assert_eq!((footer.y, footer.height), (170, 30), "{container}");
    }
}

#[test]
fn an_automatic_column_has_no_free_space_to_grow_into() {
    let markup = "<card>         <card id=\"header\" style=\"height: 30px;\"></card>         <card id=\"body\" style=\"flex-grow: 1;\"></card>         <card id=\"footer\" style=\"height: 30px;\"></card>         </card>";
    let [body, footer] = rects(markup, LayoutViewport::new(100, 300), &["body", "footer"])[..]
    else {
        unreachable!("two ids")
    };
    assert_eq!((body.y, body.height, footer.y), (30, 0, 30));
}

#[test]
fn growth_scales_with_the_display() {
    // At 200 percent every authored length doubles, so the shares do too.
    let scale = DisplayScale::from_milli(2_000).expect("200 percent");
    let markup = "<card style=\"flex-direction: row; width: 150px;\">         <card id=\"label\" style=\"width: 50px;\"></card>         <card id=\"field\" style=\"flex-grow: 1;\"></card>         </card>";
    let [label, field] = rects(
        markup,
        LayoutViewport::with_scale(300, 100, scale),
        &["label", "field"],
    )[..] else {
        unreachable!("two ids")
    };
    assert_eq!((label.width, field.x, field.width), (100, 100, 200));
}

/// Documents covering every rule the height measure mirrors.
const HEIGHT_CORPUS: &[&str] = &[
    "<card id=\"t\">One line</card>",
    "<card id=\"t\" style=\"font-size: 22px; padding: 3px 4px; border-width: 2px;\">Big</card>",
    "<card id=\"c\" style=\"gap: 7px;\"><card id=\"a\" style=\"height: 11px;\"></card>Text<card id=\"b\" style=\"height: 13px; margin: 5px;\"></card></card>",
    "<card id=\"r\" style=\"flex-direction: row; gap: 9px;\"><card id=\"a\" style=\"height: 11px;\"></card><card id=\"b\" style=\"height: 40px;\"></card>Label</card>",
    "<card id=\"m\" style=\"min-height: 90px;\"><card id=\"a\" style=\"height: 11px;\"></card></card>",
    "<card id=\"p\" style=\"height: 50%;\"><card id=\"a\" style=\"height: 25%;\"></card><card id=\"b\" style=\"min-height: 10%;\"></card></card>",
    "<card id=\"h\"><card id=\"gone\" style=\"display: none; height: 99px;\"></card><card id=\"a\" style=\"height: 12px;\"></card></card>",
    "<card id=\"n\" style=\"padding: 4px;\"><card id=\"r\" style=\"flex-direction: row;\"><card id=\"c\" style=\"gap: 3px;\">A<card id=\"x\" style=\"height: 5px;\"></card>B</card><card id=\"y\" style=\"height: 9px;\"></card></card></card>",
    "<card id=\"g\" style=\"height: 200px;\"><card id=\"a\" style=\"height: 30px;\"></card><card id=\"b\" style=\"flex-grow: 1;\"><card id=\"i\" style=\"height: 8px;\"></card></card></card>",
    "<card id=\"s\" style=\"min-height: 120px; gap: 4px;\"><card id=\"a\" style=\"flex-grow: 2;\">Top</card><card id=\"b\" style=\"flex-grow: 1; flex-direction: row;\"><card id=\"c\" style=\"flex-grow: 1;\">Left</card>Right</card></card>",
];

fn visit<'element>(element: &'element DomElement, found: &mut Vec<&'element DomElement>) {
    if element.id().is_some() {
        found.push(element);
    }
    for child in &element.children {
        if let DomNode::Element(child) = child {
            visit(child, found);
        }
    }
}

#[test]
fn measured_heights_equal_laid_out_heights() {
    // The column plan sizes children from this measure before they paint, so
    // a disagreement would grow a child into the wrong space. Every element
    // in the corpus, at two display scales, must measure what it lays out as.
    for scale_milli in [1_000, 1_500] {
        let scale = DisplayScale::from_milli(scale_milli).expect("admitted scale");
        let viewport = LayoutViewport::with_scale(240, 300, scale);
        for markup in HEIGHT_CORPUS {
            let document = parse_markup(markup).expect("corpus parses");
            let display = compute_layout(&document, viewport).expect("corpus lays out");
            let mut elements = Vec::new();
            visit(&document.root, &mut elements);
            for element in elements {
                let id = element.id().expect("visited ids");
                if element.computed_style.display == crate::style::Display::None {
                    continue;
                }
                let laid_out = display.element_rect(id).expect("laid out").height;
                let measured =
                    max_content_height(element, viewport.height(), scale).expect("measures");
                // A grown child is taller than its natural height by exactly
                // its share; everything else must match to the pixel.
                if element.computed_style.flex_grow.grows() {
                    assert!(laid_out >= measured, "#{id} in {markup} at {scale_milli}");
                } else {
                    assert_eq!(laid_out, measured, "#{id} in {markup} at {scale_milli}");
                }
            }
        }
    }
}
