use super::*;

enum Axis {
    Horizontal,
    Vertical,
}

fn region(extent: Rect, bounds: Rect, obstacles: &[PlacementObstacle]) -> PlacementRegion {
    PlacementRegion::new(extent, bounds, obstacles).expect("bounded placement region")
}

fn count(value: &PlacementRegion) -> u64 {
    value.count().expect("bounded count")
}

fn place(value: &PlacementRegion, preferred: (i32, i32)) -> Option<Rect> {
    value.place(preferred).expect("bounded placement")
}

fn brute_place(value: &PlacementRegion, preferred: (i32, i32)) -> Option<Rect> {
    let bounds = value.bounds?;
    let requested_x =
        (i64::from(preferred.0) + i64::from(value.extent.x)).clamp(bounds.x.0, bounds.x.1);
    let requested_y =
        (i64::from(preferred.1) + i64::from(value.extent.y)).clamp(bounds.y.0, bounds.y.1);
    let rows = std::iter::once(requested_y)
        .chain(requested_y + 1..=bounds.y.1)
        .chain((bounds.y.0..requested_y).rev());
    for y in rows {
        let columns = (requested_x..=bounds.x.1).chain((bounds.x.0..requested_x).rev());
        for x in columns {
            let anchor = (
                i32::try_from(x - i64::from(value.extent.x)).expect("representable x anchor"),
                i32::try_from(y - i64::from(value.extent.y)).expect("representable y anchor"),
            );
            if value.contains(anchor) {
                return Some(Rect::new(
                    i32::try_from(x).expect("representable x origin"),
                    i32::try_from(y).expect("representable y origin"),
                    value.extent.width,
                    value.extent.height,
                ));
            }
        }
    }
    None
}

fn assert_extreme_axis(axis: &Axis) {
    let rect = |x, y, width, height| match axis {
        Axis::Horizontal => Rect::new(x, y, width, height),
        Axis::Vertical => Rect::new(y, x, height, width),
    };
    let point = |x, y| match axis {
        Axis::Horizontal => (x, y),
        Axis::Vertical => (y, x),
    };
    let check = |extent, bounds, clear, outside| {
        let value = region(extent, bounds, &[]);
        assert!(value.contains(clear));
        assert!(!value.contains(outside));
    };
    check(
        rect(0, 0, 1, 1),
        rect(i32::MAX, i32::MIN, 1, 1),
        point(i32::MAX, i32::MIN),
        point(i32::MAX - 1, i32::MIN),
    );
    check(
        rect(1, 0, 1, 1),
        rect(i32::MIN, 0, 2, 1),
        point(i32::MIN, 0),
        point(i32::MIN + 1, 0),
    );
    check(
        rect(i32::MAX, 0, 1, 1),
        rect(0, 0, 1, 1),
        point(-i32::MAX, 0),
        point(-i32::MAX + 1, 0),
    );
    for (bearing, start) in [(-1, i32::MAX), (i32::MIN, 0)] {
        assert!(!region(rect(bearing, 0, 1, 1), rect(start, 0, 1, 1), &[]).contains(point(0, 0)));
    }
}

#[test]
fn rectangle_bearings_and_empty_sets_are_exact() {
    let obstacle = [PlacementObstacle::Rect(Rect::new(3, 2, 2, 2))];
    let value = region(Rect::new(-1, -2, 3, 2), Rect::new(0, 0, 8, 6), &obstacle);
    for (anchor, expected) in [
        ((1, 2), true),
        ((0, 2), false),
        ((3, 3), false),
        ((6, 4), true),
        ((6, 6), true),
        ((7, 6), false),
    ] {
        assert_eq!(value.contains(anchor), expected, "anchor={anchor:?}");
    }
    for (extent, bounds) in [
        (Rect::new(0, 0, 0, 1), Rect::new(0, 0, 1, 1)),
        (Rect::new(0, 0, 1, -1), Rect::new(0, 0, 1, 1)),
        (Rect::new(0, 0, 1, 1), Rect::new(0, 0, 0, 1)),
        (Rect::new(0, 0, 1, 1), Rect::new(0, 0, 1, -1)),
        (Rect::new(0, 0, 3, 1), Rect::new(0, 0, 2, 1)),
        (Rect::new(0, 0, 1, 3), Rect::new(0, 0, 1, 2)),
    ] {
        assert!(!region(extent, bounds, &[]).contains((0, 0)));
    }
    let empty = [PlacementObstacle::Rect(Rect::new(0, 0, 0, 1))];
    assert!(region(Rect::new(0, 0, 1, 1), Rect::new(0, 0, 1, 1), &empty).contains((0, 0)));
}

#[test]
fn line_clearance_matches_explicit_all_octant_pixels() {
    const FORWARD: [(i32, i32); 5] = [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2)];
    const REVERSE: [(i32, i32); 5] = [(4, 2), (3, 1), (2, 1), (1, 0), (0, 0)];
    for pixels in [FORWARD, REVERSE] {
        for swap in [false, true] {
            for sx in [-1, 1] {
                for sy in [-1, 1] {
                    let map = |(x, y)| {
                        let (x, y) = if swap { (y, x) } else { (x, y) };
                        (4 + sx * x, 4 + sy * y)
                    };
                    let expected = pixels.map(map);
                    for width in 1..=3 {
                        for height in 1..=3 {
                            let value = region(
                                Rect::new(0, 0, width, height),
                                Rect::new(0, 0, 9, 9),
                                &[PlacementObstacle::Line(RasterLine::new(
                                    map(pixels[0]),
                                    map(pixels[4]),
                                ))],
                            );
                            for x in -1..=9 {
                                for y in -1..=9 {
                                    let fits =
                                        x >= 0 && y >= 0 && x + width <= 9 && y + height <= 9;
                                    let overlaps = expected.iter().any(|&(px, py)| {
                                        px >= x && px < x + width && py >= y && py < y + height
                                    });
                                    assert_eq!(value.contains((x, y)), fits && !overlaps);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for (start, end, blocked, clear) in [
        ((0, 4), (8, 4), (4, 4), (4, 3)),
        ((4, 0), (4, 8), (4, 4), (3, 4)),
        ((0, 0), (8, 8), (4, 4), (4, 3)),
        ((8, 0), (0, 8), (4, 4), (4, 3)),
        ((4, 4), (4, 4), (4, 4), (4, 3)),
    ] {
        let value = region(
            Rect::new(0, 0, 1, 1),
            Rect::new(0, 0, 9, 9),
            &[PlacementObstacle::Line(RasterLine::new(start, end))],
        );
        assert!(!value.contains(blocked));
        assert!(value.contains(clear));
    }
}

#[test]
fn extreme_bearings_and_snapshot_ownership_are_explicit() {
    assert_extreme_axis(&Axis::Horizontal);
    assert_extreme_axis(&Axis::Vertical);

    let mut obstacle = [PlacementObstacle::Rect(Rect::new(0, 0, 1, 1))];
    let before = region(Rect::new(0, 0, 1, 1), Rect::new(0, 0, 2, 1), &obstacle);
    obstacle[0] = PlacementObstacle::Rect(Rect::new(1, 0, 1, 1));
    let after = region(Rect::new(0, 0, 1, 1), Rect::new(0, 0, 2, 1), &obstacle);
    assert_eq!(
        [before.contains((0, 0)), before.contains((1, 0))],
        [false, true]
    );
    assert_eq!(
        [after.contains((0, 0)), after.contains((1, 0))],
        [true, false]
    );
}

#[test]
fn count_matches_enumeration_and_coordinate_extremes() {
    let occupied = [
        PlacementObstacle::Line(RasterLine::new((2, 0), (2, 2))),
        PlacementObstacle::Rect(Rect::new(4, 0, 2, 1)),
    ];
    let value = region(Rect::new(0, 0, 2, 1), Rect::new(0, 0, 6, 3), &occupied);
    assert_eq!(count(&value), 7);
    let enumerated = (0..=4)
        .flat_map(|x| (0..=2).map(move |y| (x, y)))
        .filter(|&anchor| value.contains(anchor))
        .count();
    assert_eq!(
        usize::try_from(count(&value)).expect("small count"),
        enumerated
    );
    for (bearing, bounds, xs, ys) in [
        ((0, 0), Rect::new(0, 0, 5, 5), (0, 4), (0, 4)),
        ((-1, 2), Rect::new(-2, -2, 7, 7), (-4, 6), (-6, 4)),
    ] {
        for (start, end) in [
            ((0, 0), (4, 2)),
            ((4, 2), (0, 0)),
            ((2, 0), (2, 4)),
            ((0, 2), (4, 2)),
            ((2, 2), (2, 2)),
            ((-3, -2), (1, 2)),
            ((1, 2), (-3, -2)),
            ((-3, 2), (1, -2)),
            ((1, -2), (-3, 2)),
        ] {
            for width in 1..=2 {
                for height in 1..=2 {
                    let line = [PlacementObstacle::Line(RasterLine::new(start, end))];
                    let extent = Rect::new(bearing.0, bearing.1, width, height);
                    let value = region(extent, bounds, &line);
                    let expected = (xs.0..=xs.1)
                        .flat_map(|x| (ys.0..=ys.1).map(move |y| (x, y)))
                        .filter(|&anchor| value.contains(anchor))
                        .count();
                    assert_eq!(count(&value), u64::try_from(expected).expect("small count"));
                }
            }
        }
    }

    let full = [PlacementObstacle::Rect(Rect::new(0, 0, 3, 3))];
    assert_eq!(
        count(&region(Rect::new(0, 0, 1, 1), Rect::new(0, 0, 3, 3), &full)),
        0
    );
    assert_eq!(
        count(&region(Rect::new(0, 0, 2, 1), Rect::new(0, 0, 1, 1), &[])),
        0
    );

    for height in [8, i32::MAX] {
        let line = [PlacementObstacle::Line(RasterLine::new(
            (0, 0),
            (0, height - 1),
        ))];
        let value = region(Rect::new(0, 0, 1, 1), Rect::new(0, 0, 2, height), &line);
        assert_eq!(count(&value), u64::from(height.unsigned_abs()));
    }
    let side = u64::from(i32::MAX.unsigned_abs());
    let huge = region(
        Rect::new(0, 0, 1, 1),
        Rect::new(i32::MIN, i32::MIN, i32::MAX, i32::MAX),
        &[],
    );
    assert_eq!(count(&huge), side * side);
}

#[test]
fn placement_order_handles_rectangles_and_extreme_domains() {
    let clear = region(Rect::new(0, 0, 3, 4), Rect::new(0, 0, 20, 20), &[]);
    assert_eq!(place(&clear, (7, 6)), Some(Rect::new(7, 6, 3, 4)));
    let extreme = region(Rect::new(0, 0, 1, 1), Rect::new(i32::MAX, 0, 1, 1), &[]);
    assert_eq!(
        place(&extreme, (i32::MAX, 0)),
        Some(Rect::new(i32::MAX, 0, 1, 1))
    );

    let extent = Rect::new(-1, -2, 12, 8);
    let bounds = Rect::new(0, 0, 80, 40);
    for (occupied, expected) in [
        (
            [Rect::new(20, 10, 20, 8), Rect::new(40, 10, 20, 8)],
            Rect::new(60, 10, 12, 8),
        ),
        (
            [Rect::new(21, 10, 59, 8), Rect::new(0, 0, 0, 0)],
            Rect::new(9, 10, 12, 8),
        ),
        (
            [Rect::new(0, 10, 80, 8), Rect::new(0, 0, 0, 0)],
            Rect::new(21, 18, 12, 8),
        ),
    ] {
        let obstacles = occupied.map(PlacementObstacle::Rect);
        assert_eq!(
            place(&region(extent, bounds, &obstacles), (22, 12)),
            Some(expected)
        );
    }
    let above = [PlacementObstacle::Rect(Rect::new(0, 10, 20, 10))];
    assert_eq!(
        place(
            &region(Rect::new(0, 0, 4, 4), Rect::new(0, 0, 20, 20), &above),
            (5, 10),
        ),
        Some(Rect::new(5, 6, 4, 4))
    );
    let full = [PlacementObstacle::Rect(Rect::new(0, 0, 20, 20))];
    assert_eq!(
        place(
            &region(Rect::new(0, 0, 4, 4), Rect::new(0, 0, 20, 20), &full),
            (5, 10),
        ),
        None
    );
}

#[test]
fn placement_finds_clear_rows_between_raster_events() {
    for (bounds, extent, preferred, lines, expected) in [
        (
            Rect::new(0, 0, 100, 100),
            Rect::new(0, 0, 4, 4),
            (8, 8),
            [((0, 0), (99, 99)), ((0, 0), (0, 0))],
            Rect::new(12, 8, 4, 4),
        ),
        (
            Rect::new(0, 0, 20, 20),
            Rect::new(0, 0, 4, 4),
            (5, 5),
            [((0, 6), (19, 6)), ((0, 0), (0, 0))],
            Rect::new(5, 7, 4, 4),
        ),
        (
            Rect::new(0, 0, 5, 5),
            Rect::new(0, 0, 3, 1),
            (0, 0),
            [((2, 0), (6, 4)), ((6, 0), (2, 4))],
            Rect::new(0, 1, 3, 1),
        ),
        (
            Rect::new(2, 0, 9, 32),
            Rect::new(0, 0, 3, 2),
            (2, 13),
            [((9, 9), (3, 38)), ((1, 30), (7, 2))],
            Rect::new(5, 14, 3, 2),
        ),
    ] {
        let obstacles: Vec<_> = lines
            .into_iter()
            .filter(|(start, end)| start != end)
            .map(|(start, end)| PlacementObstacle::Line(RasterLine::new(start, end)))
            .collect();
        assert_eq!(
            place(&region(extent, bounds, &obstacles), preferred),
            Some(expected)
        );
    }

    let height = i32::MAX;
    let line = [PlacementObstacle::Line(RasterLine::new(
        (0, 0),
        (0, height - 1),
    ))];
    assert_eq!(
        place(
            &region(Rect::new(0, 0, 1, 1), Rect::new(0, 0, 1, height), &line),
            (0, 0),
        ),
        None
    );
}

#[test]
fn placement_matches_ordered_membership_enumeration() {
    for (start, end) in [
        ((0, 0), (4, 2)),
        ((4, 2), (0, 0)),
        ((2, -2), (2, 4)),
        ((-2, 2), (4, 2)),
        ((2, 2), (2, 2)),
        ((-2, -2), (4, 4)),
        ((4, 4), (-2, -2)),
        ((-2, 4), (4, -2)),
        ((4, -2), (-2, 4)),
    ] {
        for width in 1..=2 {
            for height in 1..=2 {
                let obstacles = [
                    PlacementObstacle::Line(RasterLine::new(start, end)),
                    PlacementObstacle::Rect(Rect::new(0, -1, 2, 2)),
                ];
                let value = region(
                    Rect::new(-1, 1, width, height),
                    Rect::new(-2, -2, 7, 7),
                    &obstacles,
                );
                for x in -4..=6 {
                    for y in -4..=6 {
                        assert_eq!(place(&value, (x, y)), brute_place(&value, (x, y)));
                    }
                }
            }
        }
    }

    let max_bearing = region(Rect::new(-1, 0, 1, 1), Rect::new(i32::MAX, 0, 1, 1), &[]);
    assert_eq!(place(&max_bearing, (i32::MAX, 0)), None);
    let min_bearing = region(Rect::new(1, 0, 1, 1), Rect::new(i32::MIN, 0, 2, 1), &[]);
    assert_eq!(
        place(&min_bearing, (i32::MIN, 0)),
        Some(Rect::new(i32::MIN + 1, 0, 1, 1))
    );
}
