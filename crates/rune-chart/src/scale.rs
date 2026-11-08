//! Scales: from data values to pixel positions and back.
//!
//! The continuous scales ([`Linear`], [`Log`], [`Time`]) map a domain `(f64, f64)` onto a pixel
//! range `(f32, f32)`. Either may run backwards: a y axis usually maps its domain onto
//! `(bottom, top)` so values grow upward. A domain with equal bounds maps every value to the
//! middle of the range instead of dividing by zero. With [`clamp`](Linear::clamp) on, values
//! outside the domain map to the nearest end of the range.
//!
//! [`Band`] splits a pixel range into equal bands for categories (bars, heat-map rows).

mod band;
mod linear;
mod log;
mod time;

pub use band::Band;
pub use linear::Linear;
pub use log::Log;
pub use time::Time;

/// What every continuous scale offers, so axes and hit tests can take any of them.
pub trait Continuous {
    /// The data bounds, in the order given (`.0` maps to the range's `.0`).
    fn domain(&self) -> (f64, f64);

    /// The pixel bounds, in the order given.
    fn range(&self) -> (f32, f32);

    /// The pixel position of a data value. `NaN` maps to `NaN`.
    fn map(&self, value: f64) -> f32;

    /// The data value at a pixel position: the inverse of [`map`](Continuous::map).
    fn invert(&self, px: f32) -> f64;

    /// About `count` nicely rounded values inside the domain, in the domain's direction.
    fn ticks(&self, count: usize) -> Vec<f64>;

    /// A label for a tick from `ticks(count)`, with the precision the ticks need.
    fn format_tick(&self, value: f64, count: usize) -> String;
}

/// Where `value` lies between `a` and `b`, as a fraction (0 at `a`, 1 at `b`), or 0.5 when
/// `a == b`.
pub(crate) fn normalize(value: f64, a: f64, b: f64) -> f64 {
    let span = b - a;
    if span == 0.0 { if value.is_nan() { f64::NAN } else { 0.5 } } else { (value - a) / span }
}

/// The point at fraction `t` from `a` to `b`.
pub(crate) fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Maps a fraction of the domain onto the pixel range, clamping it to `[0, 1]` when asked.
pub(crate) fn to_range(t: f64, range: (f32, f32), clamp: bool) -> f32 {
    let t = if clamp && !t.is_nan() { t.clamp(0.0, 1.0) } else { t };
    lerp(f64::from(range.0), f64::from(range.1), t) as f32
}

/// The fraction of the pixel range at `px`, clamped to `[0, 1]` when asked.
pub(crate) fn from_range(px: f32, range: (f32, f32), clamp: bool) -> f64 {
    let t = normalize(f64::from(px), f64::from(range.0), f64::from(range.1));
    if clamp && !t.is_nan() { t.clamp(0.0, 1.0) } else { t }
}
