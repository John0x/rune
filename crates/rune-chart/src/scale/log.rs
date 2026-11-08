//! The base-10 logarithmic scale.

use super::{Continuous, from_range, lerp, normalize, to_range};
use crate::ticks;

/// A base-10 logarithmic map from a positive data domain to a pixel range: each decade takes the
/// same number of pixels.
///
/// The domain must be positive. A domain bound that is zero, negative or not finite is replaced
/// by a valid one (the other bound divided or multiplied by 10, or `(1, 10)`), so the scale always
/// works. Values at or below zero map to `NaN`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Log {
    domain: (f64, f64),
    range: (f32, f32),
    clamp: bool,
}

fn valid(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

fn positive(domain: (f64, f64)) -> (f64, f64) {
    match (valid(domain.0), valid(domain.1)) {
        (true, true) => domain,
        (true, false) => (domain.0, domain.0 * 10.0),
        (false, true) => (domain.1 / 10.0, domain.1),
        (false, false) => (1.0, 10.0),
    }
}

impl Log {
    /// A scale mapping `domain.0` to `range.0` and `domain.1` to `range.1` on a log axis.
    pub fn new(domain: (f64, f64), range: (f32, f32)) -> Log {
        Log { domain: positive(domain), range, clamp: false }
    }

    /// With `clamp` on, values outside the domain map to the nearest end of the range.
    pub fn clamp(self, clamp: bool) -> Log {
        Log { clamp, ..self }
    }

    /// The same scale over another domain (made positive as in [`Log::new`]).
    pub fn with_domain(self, domain: (f64, f64)) -> Log {
        Log { domain: positive(domain), ..self }
    }

    /// The same scale onto another pixel range.
    pub fn with_range(self, range: (f32, f32)) -> Log {
        Log { range, ..self }
    }

    /// The data bounds, always positive.
    pub fn domain(&self) -> (f64, f64) {
        self.domain
    }

    /// The pixel bounds.
    pub fn range(&self) -> (f32, f32) {
        self.range
    }

    /// The pixel position of `value`; `NaN` for values at or below zero.
    pub fn map(&self, value: f64) -> f32 {
        if value <= 0.0 {
            return f32::NAN;
        }
        let t = normalize(value.log10(), self.domain.0.log10(), self.domain.1.log10());
        to_range(t, self.range, self.clamp)
    }

    /// The data value at pixel `px`.
    pub fn invert(&self, px: f32) -> f64 {
        let t = from_range(px, self.range, self.clamp);
        10f64.powf(lerp(self.domain.0.log10(), self.domain.1.log10(), t))
    }

    /// Ticks on powers of ten and, when few decades are visible, their multiples (1, 2, … 9 ×
    /// 10ᵏ), as d3's log scale. When the domain spans more decades than `count`, only some
    /// powers of ten. In the domain's direction.
    pub fn ticks(&self, count: usize) -> Vec<f64> {
        let (a, b) = self.domain;
        let reverse = b < a;
        let (u, v) = if reverse { (b, a) } else { (a, b) };
        if count == 0 {
            return Vec::new();
        }
        if u == v {
            return vec![u];
        }
        let (i, j) = (u.log10(), v.log10());
        let n = count as f64;
        let mut out = Vec::new();
        if j - i < n {
            let (i, j) = (i.floor() as i32, j.ceil() as i32);
            'decades: for e in i..=j {
                for k in 1..10 {
                    let t = if e < 0 { f64::from(k) / 10f64.powi(-e) } else { f64::from(k) * 10f64.powi(e) };
                    if t < u {
                        continue;
                    }
                    if t > v || out.len() >= ticks::MAX_TICKS {
                        break 'decades;
                    }
                    out.push(t);
                }
            }
            if out.len() * 2 < count {
                out = ticks::ticks(u, v, count);
            }
        } else {
            out = ticks::ticks(i, j, count.min((j - i) as usize)).into_iter().map(|e| 10f64.powf(e)).collect();
        }
        if reverse {
            out.reverse();
        }
        out
    }

    /// The scale with its domain extended to whole powers of ten.
    pub fn nice(self) -> Log {
        let (a, b) = self.domain;
        let f = |v: f64, up: bool| {
            let e = v.log10();
            10f64.powf(if up { e.ceil() } else { e.floor() })
        };
        let domain = if a <= b { (f(a, false), f(b, true)) } else { (f(a, true), f(b, false)) };
        Log { domain, ..self }
    }
}

impl Continuous for Log {
    fn domain(&self) -> (f64, f64) {
        self.domain
    }

    fn range(&self) -> (f32, f32) {
        self.range
    }

    fn map(&self, value: f64) -> f32 {
        Log::map(self, value)
    }

    fn invert(&self, px: f32) -> f64 {
        Log::invert(self, px)
    }

    fn ticks(&self, count: usize) -> Vec<f64> {
        Log::ticks(self, count)
    }

    /// Each tick with the decimals it needs itself (`0.02`, `5`, `3000`), compact from a
    /// thousand up (`3k`, `20M`).
    fn format_tick(&self, value: f64, _count: usize) -> String {
        let magnitude = 10f64.powf(value.abs().log10().floor());
        ticks::format_compact(value, magnitude.clamp(f64::MIN_POSITIVE, 1.0))
    }
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
