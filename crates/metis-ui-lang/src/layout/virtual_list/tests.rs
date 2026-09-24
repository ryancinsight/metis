use super::{MAX_ITEM_EXTENT, MAX_VIRTUAL_ITEMS, ScrollAlign, VirtualList, VisibleWindow};

fn window(items: std::ops::Range<usize>, start: u64, end: u64, offset: u64) -> VisibleWindow {
    VisibleWindow {
        items,
        start,
        end,
        offset,
    }
}

#[test]
fn uniform_window_covers_exactly_the_intersecting_items() {
    let list = VirtualList::uniform(1_000, 20).expect("list");
    assert_eq!(list.total_extent(), 20_000);
    assert_eq!(list.window(0, 100), window(0..5, 0, 100, 0));
    // A partially visible first and last item are both built.
    assert_eq!(list.window(30, 100), window(1..7, 20, 140, 30));
    // Offsets past the end clamp to the last full viewport.
    assert_eq!(
        list.window(u64::MAX, 100),
        window(995..1_000, 19_900, 20_000, 19_900)
    );
}

#[test]
fn overscan_widens_the_window_within_bounds() {
    let list = VirtualList::uniform(10, 10).expect("list").with_overscan(2);
    assert_eq!(list.window(0, 30).items, 0..5);
    assert_eq!(list.window(40, 30).items, 2..9);
    assert_eq!(list.window(70, 30).items, 5..10);
}

#[test]
fn variable_extents_match_a_linear_reference_after_updates() {
    let mut extents: Vec<u32> = (0..257_u32).map(|index| index * 7 % 31).collect();
    let mut list = VirtualList::variable(&extents).expect("list");
    for (index, extent) in [(0, 40), (100, 0), (256, 5), (128, 64)] {
        list.set_extent(index, extent).expect("measured extent");
        extents[index] = extent;
    }
    let prefix = |count: usize| {
        extents[..count]
            .iter()
            .map(|value| u64::from(*value))
            .sum::<u64>()
    };
    assert_eq!(list.total_extent(), prefix(extents.len()));
    for index in 0..extents.len() {
        assert_eq!(
            list.item_span(index),
            Some(prefix(index)..prefix(index + 1))
        );
    }
    for offset in (0..list.total_extent()).step_by(13) {
        let viewport = 97;
        let got = list.window(offset, viewport);
        let clamped = offset.min(list.max_offset(viewport));
        let end = clamped + u64::from(viewport);
        let expected: Vec<usize> = (0..extents.len())
            .filter(|index| {
                let (start, stop) = (prefix(*index), prefix(index + 1));
                start < stop && stop > clamped && start < end
            })
            .collect();
        let first_visible = *expected.first().expect("a visible item");
        let final_visible = *expected.last().expect("a visible item");
        assert!(
            got.items.start <= first_visible && got.items.end > final_visible,
            "offset {offset}"
        );
        // Nothing wholly outside the viewport is built except zero-extent
        // items that share a boundary with a visible one.
        for index in got.items.clone() {
            let (start, stop) = (prefix(index), prefix(index + 1));
            assert!(
                stop >= clamped && start <= end,
                "offset {offset} item {index}"
            );
        }
        assert_eq!(got.start, prefix(got.items.start));
        assert_eq!(got.end, prefix(got.items.end));
    }
}

#[test]
fn scroll_to_aligns_and_clamps() {
    let list = VirtualList::uniform(100, 10).expect("list");
    assert_eq!(list.scroll_to(50, 40, 0, ScrollAlign::Start), Some(500));
    assert_eq!(list.scroll_to(50, 40, 0, ScrollAlign::End), Some(470));
    assert_eq!(list.scroll_to(50, 40, 0, ScrollAlign::Center), Some(485));
    assert_eq!(list.scroll_to(99, 40, 0, ScrollAlign::Start), Some(960));
    assert_eq!(list.scroll_to(100, 40, 0, ScrollAlign::Start), None);
    // Nearest leaves a fully visible item alone and otherwise moves least.
    assert_eq!(list.scroll_to(12, 40, 100, ScrollAlign::Nearest), Some(100));
    assert_eq!(list.scroll_to(5, 40, 100, ScrollAlign::Nearest), Some(50));
    assert_eq!(list.scroll_to(20, 40, 100, ScrollAlign::Nearest), Some(170));
    let tall = VirtualList::variable(&[10, 200, 10]).expect("list");
    assert_eq!(tall.scroll_to(1, 50, 0, ScrollAlign::Nearest), Some(10));
}

#[test]
fn empty_lists_and_zero_viewports_build_nothing() {
    let empty = VirtualList::variable(&[]).expect("empty list");
    assert!(empty.is_empty());
    assert_eq!(empty.window(10, 100), window(0..0, 0, 0, 0));
    let list = VirtualList::uniform(4, 10).expect("list");
    assert_eq!(list.window(15, 0).items, 1..1);
    let hollow = VirtualList::variable(&[0, 0, 0]).expect("zero-extent list");
    assert_eq!(hollow.window(0, 50), window(3..3, 0, 0, 0));
}

#[test]
fn bounds_are_rejected_before_allocation() {
    assert!(VirtualList::uniform(MAX_VIRTUAL_ITEMS + 1, 1).is_err());
    assert!(VirtualList::uniform(1, 0).is_err());
    assert!(VirtualList::uniform(1, MAX_ITEM_EXTENT + 1).is_err());
    assert!(VirtualList::variable(&[MAX_ITEM_EXTENT + 1]).is_err());
    let mut uniform = VirtualList::uniform(3, 10).expect("list");
    assert!(uniform.set_extent(1, 10).is_ok());
    assert!(uniform.set_extent(1, 12).is_err());
    let mut variable = VirtualList::variable(&[1, 2]).expect("list");
    assert!(variable.set_extent(2, 1).is_err());
}

#[test]
fn maximal_lists_stay_within_u64() {
    let list = VirtualList::uniform(MAX_VIRTUAL_ITEMS, MAX_ITEM_EXTENT).expect("list");
    let total = u64::try_from(MAX_VIRTUAL_ITEMS).expect("fits") * u64::from(MAX_ITEM_EXTENT);
    assert_eq!(list.total_extent(), total);
    let bottom = list.window(u64::MAX, 1_000);
    assert_eq!(bottom.items.end, MAX_VIRTUAL_ITEMS);
}
