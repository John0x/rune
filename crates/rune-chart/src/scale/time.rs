//! The time scale: linear over Unix seconds, with calendar ticks in a time zone.

use jiff::tz::TimeZone;

use super::{Continuous, Linear};
use crate::ticks::{self, TimeInterval};

/// A linear map from times (Unix seconds as `f64`) to pixels, whose ticks fall on calendar
/// boundaries (whole minutes, hours, local midnights, months, years) in a time zone.
///
/// The zone is the system's by default; [`with_time_zone`](Time::with_time_zone) sets another,
/// which keeps tests and server-rendered charts deterministic.
#[derive(Clone, Debug, PartialEq)]
pub struct Time {
    linear: Linear,
    tz: TimeZone,
}

impl Time {
    /// A time scale in the system's time zone.
    pub fn new(domain: (f64, f64), range: (f32, f32)) -> Time {
        Time { linear: Linear::new(domain, range), tz: TimeZone::system() }
    }

    /// The same scale with ticks and labels in `tz`.
    pub fn with_time_zone(self, tz: TimeZone) -> Time {
        Time { tz, ..self }
    }

    /// With `clamp` on, times outside the domain map to the nearest end of the range.
    pub fn clamp(self, clamp: bool) -> Time {
        Time { linear: self.linear.clamp(clamp), ..self }
    }

    /// The same scale over another domain.
    pub fn with_domain(self, domain: (f64, f64)) -> Time {
        Time { linear: self.linear.with_domain(domain), ..self }
    }

    /// The same scale onto another pixel range.
    pub fn with_range(self, range: (f32, f32)) -> Time {
        Time { linear: self.linear.with_range(range), ..self }
    }

    /// The time zone ticks and labels use.
    pub fn time_zone(&self) -> &TimeZone {
        &self.tz
    }

    /// The domain in Unix seconds.
    pub fn domain(&self) -> (f64, f64) {
        self.linear.domain()
    }

    /// The pixel bounds.
    pub fn range(&self) -> (f32, f32) {
        self.linear.range()
    }

    /// The pixel position of time `t`.
    pub fn map(&self, t: f64) -> f32 {
        self.linear.map(t)
    }

    /// The time at pixel `px`.
    pub fn invert(&self, px: f32) -> f64 {
        self.linear.invert(px)
    }

    /// The interval `ticks(count)` steps by, or `None` for spans under a second (number ticks).
    pub fn interval(&self, count: usize) -> Option<TimeInterval> {
        let (a, b) = self.domain();
        TimeInterval::for_span(a, b, count)
    }

    /// About `count` calendar-aligned times inside the domain (see [`ticks::time_ticks`]).
    pub fn ticks(&self, count: usize) -> Vec<f64> {
        let (a, b) = self.domain();
        ticks::time_ticks(a, b, count, &self.tz)
    }

    /// The scale with its domain extended to the tick interval for `count` ticks: a span of a
    /// few days starting at 13:20 becomes one starting at midnight.
    pub fn nice(self, count: usize) -> Time {
        let (a, b) = self.domain();
        let Some(interval) = TimeInterval::for_span(a, b, count) else {
            return Time { linear: self.linear.nice(count), ..self };
        };
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let (lo, hi) = (interval.floor(lo, &self.tz), interval.ceil(hi, &self.tz));
        let domain = if a <= b { (lo, hi) } else { (hi, lo) };
        Time { linear: self.linear.with_domain(domain), ..self }
    }

    /// The multi-scale label of a time (see [`ticks::format_time`]).
    pub fn format(&self, t: f64) -> String {
        ticks::format_time(t, &self.tz)
    }
}

impl Continuous for Time {
    fn domain(&self) -> (f64, f64) {
        Time::domain(self)
    }

    fn range(&self) -> (f32, f32) {
        Time::range(self)
    }

    fn map(&self, value: f64) -> f32 {
        Time::map(self, value)
    }

    fn invert(&self, px: f32) -> f64 {
        Time::invert(self, px)
    }

    fn ticks(&self, count: usize) -> Vec<f64> {
        Time::ticks(self, count)
    }

    fn format_tick(&self, value: f64, _count: usize) -> String {
        self.format(value)
    }
}

#[cfg(test)]
#[path = "time_tests.rs"]
mod tests;
