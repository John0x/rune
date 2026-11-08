//! The linear scale.

use super::{Continuous, from_range, lerp, normalize, to_range};
use crate::ticks;

/// A linear map from a data domain to a pixel range.
///
/// ```
/// use rune_chart::scale::Linear;
/// // A y axis 200 px tall, values growing upward.
/// let y = Linear::new((0.0, 100.0), (200.0, 0.0));
/// assert_eq!(y.map(25.0), 150.0);
/// assert_eq!(y.invert(150.0), 25.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Linear {
    domain: (f64, f64),
    range: (f32, f32),
    clamp: bool,
}

impl Linear {
    /// A scale mapping `domain.0` to `range.0` and `domain.1` to `range.1`, unclamped.
    pub fn new(domain: (f64, f64), range: (f32, f32)) -> Linear {
        Linear { domain, range, clamp: false }
    }

    /// With `clamp` on, values outside the domain map to the nearest end of the range, and
    /// [`invert`](Linear::invert) stays inside the domain.
    pub fn clamp(self, clamp: bool) -> Linear {
        Linear { clamp, ..self }
    }

    /// The same scale over another domain.
    pub fn with_domain(self, domain: (f64, f64)) -> Linear {
        Linear { domain, ..self }
    }

    /// The same scale onto another pixel range.
    pub fn with_range(self, range: (f32, f32)) -> Linear {
        Linear { range, ..self }
    }

    /// Whether the scale clamps.
    pub fn is_clamped(&self) -> bool {
        self.clamp
    }

    /// The data bounds.
    pub fn domain(&self) -> (f64, f64) {
        self.domain
    }

    /// The pixel bounds.
    pub fn range(&self) -> (f32, f32) {
        self.range
    }

    /// The pixel position of `value`. A domain with equal bounds maps everything to the middle
    /// of the range.
    pub fn map(&self, value: f64) -> f32 {
        to_range(normalize(value, self.domain.0, self.domain.1), self.range, self.clamp)
    }

    /// The data value at pixel `px`. A range with equal bounds gives the middle of the domain.
    pub fn invert(&self, px: f32) -> f64 {
        lerp(self.domain.0, self.domain.1, from_range(px, self.range, self.clamp))
    }

    /// About `count` nice values inside the domain (see [`ticks::ticks`]).
    pub fn ticks(&self, count: usize) -> Vec<f64> {
        ticks::ticks(self.domain.0, self.domain.1, count)
    }

    /// The step between `ticks(count)`, for formatting them; negative for a descending domain.
    pub fn tick_step(&self, count: usize) -> f64 {
        ticks::tick_step(self.domain.0, self.domain.1, count)
    }

    /// The scale with its domain extended to whole tick steps (see [`ticks::nice`]).
    pub fn nice(self, count: usize) -> Linear {
        let (a, b) = ticks::nice(self.domain.0, self.domain.1, count);
        Linear { domain: (a, b), ..self }
    }
}

impl Continuous for Linear {
    fn domain(&self) -> (f64, f64) {
        self.domain
    }

    fn range(&self) -> (f32, f32) {
        self.range
    }

    fn map(&self, value: f64) -> f32 {
        Linear::map(self, value)
    }

    fn invert(&self, px: f32) -> f64 {
        Linear::invert(self, px)
    }

    fn ticks(&self, count: usize) -> Vec<f64> {
        Linear::ticks(self, count)
    }

    fn format_tick(&self, value: f64, count: usize) -> String {
        ticks::format_number(value, self.tick_step(count))
    }
}

#[cfg(test)]
#[path = "linear_tests.rs"]
mod tests;
