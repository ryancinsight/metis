use super::VirtualListView;
use crate::reactive::{ListModel, MAX_SUBSCRIBERS};
use metis_core::error::ErrorCode;
use metis_ui_lang::layout::{MAX_ITEM_EXTENT, VirtualList};
use metis_ui_lang::{DomElement, DomNode, Size};
use std::cell::Cell;
use std::rc::Rc;

const OVERSCAN: usize = 1;
const VIEWPORT: u32 = 25;

/// The measure every test uses: a row is its own extent.
fn height_of() -> impl Fn(&u32) -> u32 {
    |row: &u32| *row
}

/// A fixed linear congruential sequence, so the oracle replays identically.
struct Sequence(u64);

impl Sequence {
    fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).expect("invariant: a 31-bit value fits usize") % bound.max(1)
    }

    fn height(&mut self) -> u32 {
        u32::try_from(self.below(40)).expect("invariant: a value below 40 fits u32")
    }
}

fn rows_of(model: &ListModel<u32>) -> Vec<u32> {
    (0..model.row_count())
        .filter_map(|row| model.row_data(row))
        .collect()
}

/// Every window and total must equal a list built from scratch over the
/// model's current rows: the incremental mirror is only a cache.
fn assert_matches_fresh<F: Fn(&u32) -> u32>(
    view: &mut VirtualListView<u32, F>,
    model: &ListModel<u32>,
    step: usize,
) {
    let fresh = VirtualList::variable(&rows_of(model))
        .expect("invariant: model rows are within the list bounds")
        .with_overscan(OVERSCAN);
    let total = fresh.total_extent();
    assert_eq!(
        view.total_extent().expect("total extent"),
        total,
        "step {step}: total extent"
    );
    for offset in [0, total / 3, total / 2, total] {
        assert_eq!(
            view.window(offset, VIEWPORT).expect("window"),
            fresh.window(offset, VIEWPORT),
            "step {step}: window at offset {offset}"
        );
    }
}

fn heights(element: &DomElement) -> Vec<i32> {
    element
        .children
        .iter()
        .map(|node| match node {
            DomNode::Element(child) => match child.computed_style.height {
                Size::Px(px) => px,
                _ => panic!("the projection pins every child to pixels"),
            },
            DomNode::Text(_) => panic!("the projection has no text children"),
        })
        .collect()
}

#[test]
fn an_empty_model_shows_no_rows() {
    let model = ListModel::<u32>::new();
    let mut view = VirtualListView::new(model, height_of()).expect("subscriber slot");
    assert_eq!(view.window(0, VIEWPORT).expect("window").items, 0..0);
    assert_eq!(view.total_extent().expect("total extent"), 0);
}

#[test]
fn window_matches_a_fresh_list_after_every_change() {
    let model = ListModel::from_rows(vec![3, 7, 1, 9]).expect("bounded rows");
    let mut view = VirtualListView::new(model.clone(), height_of())
        .expect("subscriber slot")
        .with_overscan(OVERSCAN);
    assert_matches_fresh(&mut view, &model, 0);
    let mut sequence = Sequence(0x5eed_1157);
    for step in 1..=400 {
        match sequence.below(5) {
            0 => model.push_row(sequence.height()).expect("push"),
            1 => {
                let index = sequence.below(model.row_count() + 1);
                model.insert_row(index, sequence.height()).expect("insert");
            }
            2 if !model.is_empty() => {
                let index = sequence.below(model.row_count());
                model.remove_row(index).expect("remove");
            }
            3 if !model.is_empty() => {
                let index = sequence.below(model.row_count());
                model.set_row_data(index, sequence.height()).expect("set");
            }
            _ if step % 50 == 0 => {
                let count = sequence.below(12);
                let rows = (0..count).map(|_| sequence.height()).collect();
                model.replace_rows(rows).expect("replace");
            }
            _ => {}
        }
        assert_matches_fresh(&mut view, &model, step);
    }
}

#[test]
fn a_change_measures_only_the_rows_it_touched() {
    let model = ListModel::from_rows(vec![4, 8, 2]).expect("bounded rows");
    let measured = Rc::new(Cell::new(0_usize));
    let counter = Rc::clone(&measured);
    let mut view = VirtualListView::new(model.clone(), move |row: &u32| {
        counter.set(counter.get() + 1);
        *row
    })
    .expect("subscriber slot");
    view.total_extent().expect("total extent");
    assert_eq!(measured.get(), 3, "the first query measures every row");
    view.total_extent().expect("total extent");
    assert_eq!(
        measured.get(),
        3,
        "an unchanged model is not measured again"
    );
    model.set_row_data(1, 5).expect("set");
    view.total_extent().expect("total extent");
    assert_eq!(measured.get(), 4, "a replaced row is measured once");
    model.insert_row(0, 6).expect("insert");
    assert_eq!(
        view.total_extent().expect("total extent"),
        6 + 4 + 5 + 2,
        "the inserted row joins the total"
    );
    assert_eq!(
        measured.get(),
        5,
        "shifted rows keep their measurements; only the inserted row is measured"
    );
}

#[test]
fn dom_spacers_and_rows_sum_to_the_total_extent() {
    let model = ListModel::from_rows(vec![10, 20, 30, 40, 50]).expect("bounded rows");
    let mut view = VirtualListView::new(model, height_of()).expect("subscriber slot");
    let dom = view
        .dom(25, 35, |index, _| {
            let mut row = DomElement::new("span");
            row.attributes
                .insert("data-row".to_owned(), index.to_string());
            row
        })
        .expect("dom");
    // Offset 25 with viewport 35 covers [25, 60): rows 1 and 2 span [10, 30)
    // and [30, 60). The spacers hold the 10 pixels above and the 90 below.
    assert_eq!(heights(&dom), [10, 20, 30, 90]);
    let built: Vec<String> = dom.children[1..dom.children.len() - 1]
        .iter()
        .filter_map(|node| match node {
            DomNode::Element(wrapper) => match wrapper.children.as_slice() {
                [DomNode::Element(row)] => row.attributes.get("data-row").cloned(),
                _ => None,
            },
            DomNode::Text(_) => None,
        })
        .collect();
    assert_eq!(built, ["1", "2"]);
    let sum: i32 = heights(&dom).iter().sum();
    assert_eq!(u64::try_from(sum).expect("non-negative"), 150);
}

#[test]
fn dom_refuses_spacers_beyond_the_layout_pixel_range() {
    let model = ListModel::from_rows(vec![MAX_ITEM_EXTENT; 40_000]).expect("bounded rows");
    let mut view = VirtualListView::new(model, height_of()).expect("subscriber slot");
    let error = view
        .dom(0, 100, |_, _| DomElement::new("span"))
        .expect_err("the bottom spacer exceeds i32::MAX pixels");
    assert_eq!(error.code, ErrorCode::LayoutOverflow);
}

#[test]
fn a_model_without_a_free_subscriber_slot_refuses_the_view() {
    let model = ListModel::from_rows(vec![1]).expect("bounded rows");
    let peers: Vec<_> = (0..MAX_SUBSCRIBERS)
        .map(|_| model.subscribe(|_| {}))
        .collect();
    let refused = VirtualListView::new(model.clone(), height_of());
    assert!(matches!(refused, Err(ref error) if error.code == ErrorCode::QueueFull));
    drop(peers);
}

#[test]
fn dropping_the_view_unsubscribes_from_the_model() {
    let model = ListModel::from_rows(vec![2]).expect("bounded rows");
    let view = VirtualListView::new(model.clone(), height_of()).expect("subscriber slot");
    assert_eq!(model.subscriber_count(), 1);
    drop(view);
    assert_eq!(model.subscriber_count(), 0);
}
