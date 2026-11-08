use super::*;

#[test]
fn spread_leaves_room_alone() {
    let mut ys = [10.0, 40.0, 90.0];
    spread(&mut ys, 14.0, 0.0, 100.0);
    assert_eq!(ys, [10.0, 40.0, 90.0]);
}

#[test]
fn spread_pushes_crowded_labels_apart() {
    let mut ys = [50.0, 52.0, 53.0];
    spread(&mut ys, 10.0, 0.0, 200.0);
    assert_eq!(ys, [50.0, 60.0, 70.0]);
}

#[test]
fn spread_keeps_labels_inside() {
    let mut ys = [90.0, 95.0, 98.0];
    spread(&mut ys, 10.0, 5.0, 100.0);
    assert_eq!(ys, [80.0, 90.0, 100.0]);
    let mut ys = [0.0, 1.0];
    spread(&mut ys, 10.0, 5.0, 100.0);
    assert_eq!(ys, [5.0, 15.0]);
}

#[test]
fn spread_overflows_downward_when_it_must() {
    let mut ys = [10.0, 10.0, 10.0];
    spread(&mut ys, 10.0, 0.0, 15.0);
    assert_eq!(ys, [0.0, 10.0, 20.0]);
    spread(&mut [], 10.0, 0.0, 15.0);
}

#[test]
fn shares_read_as_percent() {
    assert_eq!(percent(0.32), "32%");
    assert_eq!(percent(0.004), "0.4%");
    assert_eq!(percent(0.0), "0%");
    assert_eq!(percent(1.0), "100%");
}

#[test]
fn labels_on_a_side_never_overlap() {
    // Many thin slices on the right: their label ends must step down by a line at least.
    let values = [1.0, 1.0, 1.0, 1.0, 1.0, 40.0];
    let layout = pie(&values, &PieOptions::default());
    let slices: Vec<Drawn> = layout
        .iter()
        .map(|s| Drawn {
            index: s.index,
            color: Color::WHITE,
            value: s.value,
            start: s.start,
            end: s.end,
            pad: 0.0,
            share: 0.0,
        })
        .collect();
    let lines = label_lines(&slices, [200.0, 100.0], 80.0, 200.0, 14.0);
    let mut right: Vec<f32> =
        lines.iter().filter(|l| l.1 == LabelSide::Right).filter_map(|l| l.0.map(|p| p[2][1])).collect();
    right.sort_by(f32::total_cmp);
    assert!(right.len() >= 5);
    assert!(right.windows(2).all(|w| w[1] - w[0] >= 14.0 - 1e-3), "{right:?}");
    // Every label end sits in the column beside the pie.
    for (points, side) in lines.iter().filter_map(|l| l.0.map(|p| (p, l.1))) {
        let column = if side == LabelSide::Right { 200.0 + 80.0 + ELBOW + RUN } else { 200.0 - 80.0 - ELBOW - RUN };
        assert_eq!(points[2][0], column);
    }
}
