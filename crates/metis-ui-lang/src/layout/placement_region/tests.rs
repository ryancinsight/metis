use super::*;

enum Axis {
    Horizontal,
    Vertical,
}

fn region(extent: Rect, bounds: Rect, obstacles: &[PlacementObstacle]) -> PlacementRegion {
    PlacementRegion::new(extent, bounds, obstacles).expect("bounded placement region")
}
fn assert_extreme_axis(axis: Axis) {
    let rect = |x, y, width, height| match &axis {
        Axis::Horizontal => Rect::new(x, y, width, height),
        Axis::Vertical => Rect::new(y, x, height, width),
    };
    let point = |x, y| match &axis {
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
    assert_extreme_axis(Axis::Horizontal);
    assert_extreme_axis(Axis::Vertical);

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
