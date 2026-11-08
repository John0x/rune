//! Nice linear steps: 1, 2 or 5 × 10ᵏ, after d3-array's `ticks`, `tickStep` and scale `nice`.

use super::MAX_TICKS;

/// The largest count used; nice steps give at most about 1.4 ticks per count, so the result
/// stays under [`MAX_TICKS`].
const MAX_COUNT: usize = MAX_TICKS / 2;

const E10: f64 = 7.0710678118654755; // √50
const E5: f64 = 3.1622776601683795; // √10
const E2: f64 = std::f64::consts::SQRT_2;

/// A run of ticks `i1..=i2` on a grid. A positive `inc` is the step itself; a negative `inc`
/// means the step is `1 / -inc`, so a tick is `i / -inc`: dividing by an exact integer keeps
/// `0.1 * 3` from showing up as `0.30000000000000004`.
#[derive(Clone, Copy, Debug)]
struct Spec {
    i1: f64,
    i2: f64,
    inc: f64,
}

impl Spec {
    fn value(self, i: f64) -> f64 {
        if self.inc < 0.0 { i / -self.inc } else { i * self.inc }
    }

    fn step(self) -> f64 {
        if self.inc < 0.0 { 1.0 / -self.inc } else { self.inc }
    }
}

/// Expects `start <= stop`.
fn spec(start: f64, stop: f64, count: f64) -> Option<Spec> {
    let step = (stop - start) / count.max(0.0);
    if !(step.is_finite() && step > 0.0) {
        return None;
    }
    let power = step.log10().floor();
    let error = step / 10f64.powf(power);
    let factor = if error >= E10 {
        10.0
    } else if error >= E5 {
        5.0
    } else if error >= E2 {
        2.0
    } else {
        1.0
    };
    let s = if power < 0.0 {
        let inc = 10f64.powf(-power) / factor;
        let (mut i1, mut i2) = ((start * inc).round(), (stop * inc).round());
        if i1 / inc < start {
            i1 += 1.0;
        }
        if i2 / inc > stop {
            i2 -= 1.0;
        }
        Spec { i1, i2, inc: -inc }
    } else {
        let inc = 10f64.powf(power) * factor;
        let (mut i1, mut i2) = ((start / inc).round(), (stop / inc).round());
        if i1 * inc < start {
            i1 += 1.0;
        }
        if i2 * inc > stop {
            i2 -= 1.0;
        }
        Spec { i1, i2, inc }
    };
    if !(s.inc.is_finite() && s.i1.is_finite() && s.i2.is_finite()) {
        return None;
    }
    if s.i2 < s.i1 && (0.5..2.0).contains(&count) {
        return spec(start, stop, count * 2.0);
    }
    Some(s)
}

fn usable(start: f64, stop: f64, count: usize) -> bool {
    count > 0 && start.is_finite() && stop.is_finite()
}

/// About `count` evenly spaced, nicely rounded values within `[start, stop]` (inclusive), in the
/// domain's direction: descending when `stop < start`.
///
/// Returns `[start]` when `start == stop`, and nothing when `count` is 0 or a bound isn't finite.
pub fn ticks(start: f64, stop: f64, count: usize) -> Vec<f64> {
    if !usable(start, stop, count) {
        return Vec::new();
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (lo, hi) = if reverse { (stop, start) } else { (start, stop) };
    let Some(s) = spec(lo, hi, count.min(MAX_COUNT) as f64) else { return Vec::new() };
    if s.i2 < s.i1 || s.i2 - s.i1 >= MAX_TICKS as f64 - 1.0 {
        return Vec::new();
    }
    let n = (s.i2 - s.i1) as usize + 1;
    let mut out: Vec<f64> = (0..n).map(|i| s.value(s.i1 + i as f64)).collect();
    if reverse {
        out.reverse();
    }
    out
}

/// The step between the values [`ticks`] returns for the same arguments, negative when
/// `stop < start`, and 0 when there is no step (equal bounds, `count` 0, or a bound not finite).
pub fn tick_step(start: f64, stop: f64, count: usize) -> f64 {
    if !usable(start, stop, count) || start == stop {
        return 0.0;
    }
    let reverse = stop < start;
    let (lo, hi) = if reverse { (stop, start) } else { (start, stop) };
    match spec(lo, hi, count.min(MAX_COUNT) as f64) {
        Some(s) if reverse => -s.step(),
        Some(s) => s.step(),
        None => 0.0,
    }
}

/// Extends `(start, stop)` outward to the tick step for `count` ticks, so the domain begins and
/// ends on a tick, as d3's `scale.nice(count)`: `(0.13, 0.97)` becomes `(0.1, 1.0)`.
///
/// Keeps the domain's direction. Equal bounds, non-finite bounds and `count` 0 are returned as
/// they are.
pub fn nice(start: f64, stop: f64, count: usize) -> (f64, f64) {
    if !usable(start, stop, count) || start == stop {
        return (start, stop);
    }
    let reverse = stop < start;
    let (mut lo, mut hi) = if reverse { (stop, start) } else { (start, stop) };
    let count = count.min(MAX_COUNT) as f64;
    let mut previous = None;
    for _ in 0..10 {
        let Some(s) = spec(lo, hi, count) else { break };
        if previous == Some(s.inc) {
            break;
        }
        if s.inc > 0.0 {
            lo = (lo / s.inc).floor() * s.inc;
            hi = (hi / s.inc).ceil() * s.inc;
        } else {
            let k = -s.inc;
            lo = (lo * k).floor() / k;
            hi = (hi * k).ceil() / k;
        }
        previous = Some(s.inc);
    }
    if reverse { (hi, lo) } else { (lo, hi) }
}

#[cfg(test)]
#[path = "step_tests.rs"]
mod tests;
