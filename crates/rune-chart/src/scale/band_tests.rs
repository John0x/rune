use super::*;

#[test]
fn bands_split_the_range() {
    let b = Band::new(4, (0.0, 100.0));
    assert_eq!((b.count(), b.range()), (4, (0.0, 100.0)));
    assert_eq!((b.step(), b.bandwidth()), (25.0, 25.0));
    assert_eq!((0..4).map(|i| b.position(i).unwrap()).collect::<Vec<_>>(), [0.0, 25.0, 50.0, 75.0]);
    assert_eq!(b.center(1), Some(37.5));
    assert_eq!(b.position(4), None);
}

#[test]
fn padding_leaves_gaps_and_centers_the_bands() {
    let b = Band::new(4, (0.0, 100.0)).padding_inner(0.2).padding_outer(0.1);
    assert_eq!(b.step(), 25.0);
    assert_eq!(b.bandwidth(), 20.0);
    assert_eq!((0..4).map(|i| b.position(i).unwrap()).collect::<Vec<_>>(), [2.5, 27.5, 52.5, 77.5]);
    let start = Band::new(4, (0.0, 100.0)).padding(0.2).align(0.0);
    assert_eq!(start.position(0), Some(0.0));
    let outer = Band::new(2, (0.0, 100.0)).padding_outer(1.0);
    assert_eq!(outer.step(), 25.0);
    assert_eq!(outer.position(0), Some(25.0));
}

#[test]
fn positions_find_their_band_including_half_the_gap() {
    let b = Band::new(4, (0.0, 100.0)).padding_inner(0.2).padding_outer(0.1);
    assert_eq!(b.index_at(0.0), Some(0));
    assert_eq!(b.index_at(24.9), Some(0));
    assert_eq!(b.index_at(25.1), Some(1));
    assert_eq!(b.index_at(99.9), Some(3));
    assert_eq!(b.index_at(100.0), None);
    assert_eq!(b.index_at(-0.1), None);
    assert_eq!(b.index_at(f32::NAN), None);
    for i in 0..4 {
        assert_eq!(b.index_at(b.center(i).unwrap()), Some(i));
    }
}

#[test]
fn reversed_ranges_start_at_the_first_bound() {
    let b = Band::new(3, (90.0, 0.0));
    assert_eq!(b.position(0), Some(60.0));
    assert_eq!(b.position(2), Some(0.0));
    assert_eq!(b.index_at(80.0), Some(0));
    assert_eq!(b.index_at(10.0), Some(2));
}

#[test]
fn odd_inputs_are_safe() {
    let empty = Band::new(0, (0.0, 100.0));
    assert_eq!(empty.position(0), None);
    assert_eq!(empty.index_at(50.0), None);
    let flat = Band::new(3, (50.0, 50.0));
    assert_eq!(flat.bandwidth(), 0.0);
    assert_eq!(flat.index_at(50.0), None);
    let nan = Band::new(3, (f32::NAN, 10.0));
    assert_eq!(nan.index_at(5.0), None);
    let wild = Band::new(3, (0.0, 30.0)).padding_inner(f32::NAN).padding_outer(-4.0).align(9.0);
    assert_eq!(wild.step(), 10.0);
    assert_eq!(wild.position(0), Some(0.0));
}
