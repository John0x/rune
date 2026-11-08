use super::*;

/// A deterministic noisy signal with a few spikes.
fn signal(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let x: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let y = (0..n)
        .map(|i| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let noise = (state % 1000) as f64 / 1000.0;
            match i % 997 {
                0 => 50.0,
                500 => -50.0,
                _ => (i as f64 / 300.0).sin() * 10.0 + noise,
            }
        })
        .collect();
    (x, y)
}

#[test]
fn visible_range_reaches_one_point_past_each_edge() {
    let x: Vec<f64> = (0..10).map(f64::from).collect();
    assert_eq!(visible_range(&x, (2.5, 6.5)), 2..8);
    assert_eq!(visible_range(&x, (2.0, 6.0)), 1..8);
    assert_eq!(visible_range(&x, (6.5, 2.5)), 2..8);
    assert_eq!(visible_range(&x, (-5.0, 100.0)), 0..10);
    assert_eq!(visible_range(&x, (20.0, 30.0)), 9..10);
    assert_eq!(visible_range(&x, (-30.0, -20.0)), 0..1);
    assert_eq!(visible_range(&[], (0.0, 1.0)), 0..0);
    assert_eq!(visible_range(&x, (f64::NAN, 1.0)), 0..0);
    let dup = [0.0, 1.0, 1.0, 1.0, 2.0];
    assert_eq!(visible_range(&dup, (1.0, 1.0)), 0..5);
}

#[test]
fn few_points_pass_through() {
    let (x, y) = signal(100);
    assert_eq!(decimate(&x, &y, 0..100, (0.0, 99.0), 50), (0..100).collect::<Vec<_>>());
    assert_eq!(decimate(&x, &y, 10..20, (0.0, 99.0), 0), (10..20).collect::<Vec<_>>());
    assert_eq!(decimate(&x, &y, 0..100, (5.0, 5.0), 10), (0..100).collect::<Vec<_>>());
    assert_eq!(decimate(&x, &y, 90..500, (0.0, 99.0), 50), (90..100).collect::<Vec<_>>());
    assert!(decimate(&[], &[], 0..10, (0.0, 1.0), 10).is_empty());
}

#[test]
fn decimation_keeps_extremes_and_ends() {
    let n = 50_000;
    let (x, y) = signal(n);
    let columns = 400;
    let kept = decimate(&x, &y, 0..n, (0.0, (n - 1) as f64), columns);
    assert!(kept.len() <= columns * 4 + 4, "{}", kept.len());
    assert!(kept.len() > columns, "{}", kept.len());
    assert!(kept.windows(2).all(|w| w[0] < w[1]), "indices ascend without repeats");
    assert_eq!((kept[0], kept[kept.len() - 1]), (0, n - 1));
    let max = (0..n).max_by(|&a, &b| y[a].total_cmp(&y[b])).unwrap();
    let min = (0..n).min_by(|&a, &b| y[a].total_cmp(&y[b])).unwrap();
    assert!(kept.contains(&max) && kept.contains(&min));
    // Every pixel column keeps its own lowest and highest value.
    let scale = columns as f64 / (n - 1) as f64;
    let mut lo = vec![f64::MAX; columns + 1];
    let mut hi = vec![f64::MIN; columns + 1];
    let mut kept_lo = lo.clone();
    let mut kept_hi = hi.clone();
    for i in 0..n {
        let c = (x[i] * scale).floor() as usize;
        (lo[c], hi[c]) = (lo[c].min(y[i]), hi[c].max(y[i]));
    }
    for &i in &kept {
        let c = (x[i] * scale).floor() as usize;
        (kept_lo[c], kept_hi[c]) = (kept_lo[c].min(y[i]), kept_hi[c].max(y[i]));
    }
    assert_eq!((kept_lo, kept_hi), (lo, hi));
}

#[test]
fn gaps_survive_decimation() {
    let n = 10_000;
    let (x, mut y) = signal(n);
    y[4_000..4_010].fill(f64::NAN);
    y[7_777] = f64::INFINITY;
    let kept = decimate(&x, &y, 0..n, (0.0, (n - 1) as f64), 100);
    for i in (4_000..4_010).chain([7_777]) {
        assert!(kept.contains(&i), "gap {i} kept");
    }
    assert!(kept.contains(&3_999) && kept.contains(&4_010), "the line resumes at both sides of the gap");
    assert!(kept.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn decimating_a_visible_slice() {
    let n = 20_000;
    let (x, y) = signal(n);
    let domain = (5_000.0, 6_000.0);
    let range = visible_range(&x, domain);
    let kept = decimate(&x, &y, range.clone(), domain, 100);
    assert_eq!(kept.first(), Some(&range.start));
    assert_eq!(kept.last(), Some(&(range.end - 1)));
    assert!(kept.len() <= 4 * 102);
}
