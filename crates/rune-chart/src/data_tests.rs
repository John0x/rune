use super::*;

#[test]
fn series_check_their_columns() {
    let s = Series::new(vec![0.0, 1.0, 1.0, 3.0], vec![5.0, f64::NAN, 2.0, 7.0]).unwrap();
    assert_eq!(s.len(), 4);
    assert_eq!(s.point(2), Some((1.0, 2.0)));
    assert_eq!(s.point(4), None);
    assert_eq!(Series::new(vec![0.0, 1.0], vec![1.0]), Err(DataError::LengthMismatch { expected: 2, found: 1 }));
    assert_eq!(Series::new(vec![0.0, 2.0, 1.0], vec![1.0; 3]), Err(DataError::Unsorted { index: 2 }));
    assert_eq!(Series::new(vec![0.0, f64::NAN], vec![1.0; 2]), Err(DataError::NonFiniteX { index: 1 }));
    assert_eq!(Series::new(vec![f64::INFINITY], vec![1.0]), Err(DataError::NonFiniteX { index: 0 }));
    assert!(Series::new(Vec::new(), Vec::new()).unwrap().is_empty());
}

#[test]
fn series_build_from_points_closures_and_values() {
    let s = Series::from_points([(0.0, 1.0), (2.0, 3.0)]).unwrap();
    assert_eq!((&s.x()[..], &s.y()[..]), (&[0.0, 2.0][..], &[1.0, 3.0][..]));
    let f = Series::from_fn(3, |i| (i as f64 * 10.0, (i * i) as f64)).unwrap();
    assert_eq!(&f.y()[..], [0.0, 1.0, 4.0]);
    assert!(Series::from_fn(2, |i| (-(i as f64), 0.0)).is_err());
    let y = Series::from_y([4.0, 5.0, 6.0]);
    assert_eq!(&y.x()[..], [0.0, 1.0, 2.0]);
    let u = Series::from_unsorted([(3.0, 30.0), (f64::NAN, 0.0), (1.0, 10.0), (2.0, 20.0), (1.0, 11.0)]);
    assert_eq!(&u.x()[..], [1.0, 1.0, 2.0, 3.0]);
    assert_eq!(&u.y()[..], [10.0, 11.0, 20.0, 30.0], "a stable sort keeps equal x in input order");
}

#[test]
fn series_share_their_columns() {
    let s = Series::from_y((0..1000).map(f64::from));
    let t = s.clone();
    assert!(std::sync::Arc::ptr_eq(s.y(), t.y()));
}

#[test]
fn extents_skip_gaps() {
    assert_eq!(extent(&[3.0, f64::NAN, -1.0, 7.0, f64::INFINITY]), Some((-1.0, 7.0)));
    assert_eq!(extent(&[]), None);
    assert_eq!(extent(&[f64::NAN, f64::NAN]), None);
    assert_eq!(extent_in(&[1.0, 9.0, 3.0, 4.0], 2..10), Some((3.0, 4.0)));
    let (start, end) = (5, 1);
    assert_eq!(extent_in(&[1.0, 9.0], start..end), None);
    assert_eq!(union(Some((0.0, 1.0)), Some((-2.0, 0.5))), Some((-2.0, 1.0)));
    assert_eq!(union(None, Some((1.0, 2.0))), Some((1.0, 2.0)));
    assert_eq!(union(None, None), None);
    let s = Series::new(vec![0.0, 1.0, 2.0], vec![f64::NAN, 4.0, -2.0]).unwrap();
    assert_eq!(s.x_extent(), Some((0.0, 2.0)));
    assert_eq!(s.y_extent(), Some((-2.0, 4.0)));
    assert_eq!(s.y_extent_in(0..2), Some((4.0, 4.0)));
    assert_eq!(Series::default().x_extent(), None);
}

#[test]
fn candles_check_and_measure() {
    let o = Ohlc::from_rows([
        (0.0, 10.0, 12.0, 9.0, 11.0),
        (1.0, 11.0, 15.0, 10.5, 14.0),
        (2.0, f64::NAN, f64::NAN, f64::NAN, f64::NAN),
    ])
    .unwrap();
    assert_eq!(o.len(), 3);
    assert_eq!(o.row(1), Some((1.0, 11.0, 15.0, 10.5, 14.0)));
    assert_eq!(o.row(3), None);
    assert_eq!(o.y_extent(), Some((9.0, 15.0)));
    assert_eq!(o.y_extent_in(1..2), Some((10.5, 15.0)));
    assert_eq!(o.x_extent(), Some((0.0, 2.0)));
    assert_eq!(
        (&o.open()[..1], &o.high()[..1], &o.low()[..1], &o.close()[..1]),
        (&[10.0][..], &[12.0][..], &[9.0][..], &[11.0][..])
    );
    assert_eq!(o.x().len(), 3);
    assert!(!o.is_empty());
    assert_eq!(
        Ohlc::new(vec![0.0, 1.0], vec![1.0; 2], vec![1.0; 2], vec![1.0], vec![1.0; 2]),
        Err(DataError::LengthMismatch { expected: 2, found: 1 })
    );
    assert_eq!(
        Ohlc::from_rows([(1.0, 0.0, 0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 0.0, 0.0)]),
        Err(DataError::Unsorted { index: 1 })
    );
}

#[test]
fn grids_hold_cells_row_by_row() {
    let g = Grid::from_fn(3, 2, |c, r| if c == 1 && r == 1 { None } else { Some((r * 10 + c) as f64) });
    assert_eq!((g.columns(), g.rows()), (3, 2));
    assert_eq!(g.get(2, 1), Some(12.0));
    assert_eq!(g.get(1, 1), None);
    assert_eq!(g.get(3, 0), None);
    assert_eq!(g.get(0, 2), None);
    assert_eq!(g.extent(), Some((0.0, 12.0)));
    assert_eq!(g.values().len(), 6);
    let nan = Grid::new(1, 2, vec![Some(f64::NAN), Some(1.0)]).unwrap();
    assert_eq!(nan.get(0, 0), None);
    assert_eq!(nan.extent(), Some((1.0, 1.0)));
    assert_eq!(Grid::new(2, 2, vec![None; 3]), Err(DataError::LengthMismatch { expected: 4, found: 3 }));
    assert_eq!(Grid::default().extent(), None);
}

#[test]
fn errors_explain_themselves() {
    assert_eq!(DataError::LengthMismatch { expected: 3, found: 2 }.to_string(), "column has 2 values, expected 3");
    assert_eq!(DataError::Unsorted { index: 4 }.to_string(), "x is not ascending at index 4");
    assert_eq!(DataError::NonFiniteX { index: 0 }.to_string(), "x is not finite at index 0");
}
