//! Deterministic sample data for the gallery.

use rune_chart::data::Series;

/// Start of the samples: 2026-11-01 00:00 UTC.
pub(crate) const START: f64 = 1_793_491_200.0;

/// A seeded pseudo-random number in [0, 1).
pub(crate) fn noise(seed: u64, i: usize) -> f64 {
    let mut x =
        seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (i as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// `n` hourly points of a random walk around `level`.
pub(crate) fn walk(n: usize, seed: u64, level: f64) -> Series {
    let mut v = level;
    Series::from_fn(n, |i| {
        v = (v + (noise(seed, i) - 0.5) * level * 0.04).max(level * 0.5);
        (START + i as f64 * 3600.0, v)
    })
    .unwrap_or_else(|_| Series::from_y([]))
}

/// Sales per hour (columns 0–23) and weekday (rows Monday–Sunday): quiet nights, an evening
/// peak that runs later at the weekend, and a few hours without data.
pub(crate) fn week_sales() -> rune_chart::data::Grid {
    rune_chart::data::Grid::from_fn(24, 7, |hour, day| {
        let i = day * 24 + hour;
        if noise(41, i) < 0.03 {
            return None;
        }
        let weekend = day >= 5;
        let peak = if weekend { 21.0 } else { 19.5 };
        let evening = (-((hour as f64 - peak) / 3.0).powi(2)).exp();
        let midday = 0.45 * (-((hour as f64 - 13.0) / 2.5).powi(2)).exp();
        let base = if weekend { 1.25 } else { 1.0 };
        Some(((8.0 + 120.0 * (evening + midday) * base) * (0.85 + 0.3 * noise(43, i))).round())
    })
}
