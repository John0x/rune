//! Calendar intervals for time ticks, from one second to years, aligned in a time zone.
//!
//! Times are Unix seconds as `f64`. Intervals under a day step in absolute time and keep the
//! instants whose local field is a multiple of the step (so an hourly axis shows 02:00 twice on
//! the night clocks go back, and skips it when they go forward); days, weeks, months and years
//! step on the calendar and start at local midnight, so they stay put across offset changes.

use jiff::Timestamp;
use jiff::ToSpan;
use jiff::civil::{Date, DateTime, Time};
use jiff::tz::TimeZone;

use super::{MAX_TICKS, step};

/// The calendar unit of a [`TimeInterval`]. Weeks start on Monday.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimeUnit {
    /// Whole seconds.
    Second,
    /// Whole minutes.
    Minute,
    /// Whole hours.
    Hour,
    /// Local midnights.
    Day,
    /// Local midnights on Mondays.
    Week,
    /// Local midnights on the first of a month.
    Month,
    /// Local midnights on January 1.
    Year,
}

impl TimeUnit {
    /// The unit's nominal length in seconds (a month counts 30 days, a year 365), for choosing
    /// an interval; real lengths vary with the calendar and daylight saving.
    pub fn approx_seconds(self) -> f64 {
        match self {
            TimeUnit::Second => 1.0,
            TimeUnit::Minute => 60.0,
            TimeUnit::Hour => 3_600.0,
            TimeUnit::Day => 86_400.0,
            TimeUnit::Week => 604_800.0,
            TimeUnit::Month => 2_592_000.0,
            TimeUnit::Year => 31_536_000.0,
        }
    }
}

/// A tick interval: every `step` units, aligned so the unit's field is a multiple of `step`
/// (minutes 0, 15, 30, 45; hours 0, 6, 12, 18; days of the month 1, 3, 5, …; months January,
/// April, July, October; years divisible by `step`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimeInterval {
    /// The calendar unit.
    pub unit: TimeUnit,
    /// How many units between ticks, at least 1.
    pub step: u32,
}

/// The intervals a time axis picks from, shortest first; past the last, whole years with a nice
/// step (2, 5, 10, …).
const INTERVALS: [TimeInterval; 18] = [
    TimeInterval::new(TimeUnit::Second, 1),
    TimeInterval::new(TimeUnit::Second, 5),
    TimeInterval::new(TimeUnit::Second, 15),
    TimeInterval::new(TimeUnit::Second, 30),
    TimeInterval::new(TimeUnit::Minute, 1),
    TimeInterval::new(TimeUnit::Minute, 5),
    TimeInterval::new(TimeUnit::Minute, 15),
    TimeInterval::new(TimeUnit::Minute, 30),
    TimeInterval::new(TimeUnit::Hour, 1),
    TimeInterval::new(TimeUnit::Hour, 3),
    TimeInterval::new(TimeUnit::Hour, 6),
    TimeInterval::new(TimeUnit::Hour, 12),
    TimeInterval::new(TimeUnit::Day, 1),
    TimeInterval::new(TimeUnit::Day, 2),
    TimeInterval::new(TimeUnit::Week, 1),
    TimeInterval::new(TimeUnit::Month, 1),
    TimeInterval::new(TimeUnit::Month, 3),
    TimeInterval::new(TimeUnit::Year, 1),
];

/// Bounds the walk over units, so an interval far too fine for its span gives up instead of
/// looping for long.
const MAX_STEPS: usize = 200_000;

impl TimeInterval {
    /// Every `step` units; a `step` of 0 counts as 1.
    pub const fn new(unit: TimeUnit, step: u32) -> TimeInterval {
        TimeInterval { unit, step: if step == 0 { 1 } else { step } }
    }

    /// The interval's nominal length in seconds.
    pub fn approx_seconds(self) -> f64 {
        self.unit.approx_seconds() * f64::from(self.step)
    }

    /// The interval that gives about `count` ticks between `start` and `stop`, as d3's time
    /// scale picks it: the listed interval nearest (by ratio) to the span divided by `count`.
    ///
    /// `None` when that is under a second (use [`step::ticks`](super::ticks) on the seconds), and
    /// when the bounds aren't finite or `count` is 0.
    pub fn for_span(start: f64, stop: f64, count: usize) -> Option<TimeInterval> {
        if count == 0 || !start.is_finite() || !stop.is_finite() {
            return None;
        }
        let target = (stop - start).abs() / count as f64;
        let i = INTERVALS.partition_point(|iv| iv.approx_seconds() <= target);
        if i == 0 {
            return None;
        }
        if i == INTERVALS.len() {
            let year = TimeUnit::Year.approx_seconds();
            let years = step::tick_step(start / year, stop / year, count).abs().round();
            return Some(TimeInterval::new(TimeUnit::Year, years.clamp(1.0, 1e6) as u32));
        }
        let (a, b) = (INTERVALS[i - 1], INTERVALS[i]);
        Some(if target / a.approx_seconds() < b.approx_seconds() / target { a } else { b })
    }

    /// The latest aligned instant at or before `t`. Returns `t` unchanged when it can't be
    /// placed on the calendar (not finite, or beyond the years -9999 to 9999).
    pub fn floor(self, t: f64, tz: &TimeZone) -> f64 {
        let Some(mut u) = self.floor_unit(t, tz) else { return t };
        for _ in 0..=self.step.min(1_000) {
            if self.aligned(u, tz) {
                return u;
            }
            match self.floor_unit(u - 1.0, tz) {
                Some(prev) if prev < u => u = prev,
                _ => break,
            }
        }
        t
    }

    /// The earliest aligned instant at or after `t`. Returns `t` unchanged when it can't be
    /// placed on the calendar.
    pub fn ceil(self, t: f64, tz: &TimeZone) -> f64 {
        let f = self.floor(t, tz);
        if f >= t {
            return f;
        }
        let mut u = f;
        for _ in 0..=self.step.min(1_000) {
            match self.next_unit(u, tz) {
                Some(next) if next > u => u = next,
                _ => return t,
            }
            if self.aligned(u, tz) {
                return u;
            }
        }
        t
    }

    /// The aligned instants within `[start, stop]` (inclusive), in the domain's direction.
    /// At most [`MAX_TICKS`](super::MAX_TICKS).
    pub fn ticks(self, start: f64, stop: f64, tz: &TimeZone) -> Vec<f64> {
        if !start.is_finite() || !stop.is_finite() {
            return Vec::new();
        }
        let reverse = stop < start;
        let (lo, hi) = if reverse { (stop, start) } else { (start, stop) };
        let mut out = Vec::new();
        let Some(mut t) = self.floor_unit(lo, tz) else { return out };
        for _ in 0..MAX_STEPS {
            if t > hi || out.len() >= MAX_TICKS {
                break;
            }
            if t >= lo && self.aligned(t, tz) {
                out.push(t);
            }
            match self.next_unit(t, tz) {
                Some(next) if next > t => t = next,
                _ => break,
            }
        }
        if reverse {
            out.reverse();
        }
        out
    }

    /// The start of the unit holding `t` (for years, of the `step`-aligned year).
    fn floor_unit(self, t: f64, tz: &TimeZone) -> Option<f64> {
        let s = t.floor();
        let dt = civil(s, tz)?;
        let date = dt.date();
        match self.unit {
            TimeUnit::Second => Some(s),
            TimeUnit::Minute => Some(s - f64::from(dt.second())),
            TimeUnit::Hour => Some(s - f64::from(dt.minute()) * 60.0 - f64::from(dt.second())),
            TimeUnit::Day => midnight(date, tz),
            TimeUnit::Week => {
                let back = i64::from(date.weekday().to_monday_zero_offset());
                midnight(date.checked_sub(back.days()).ok()?, tz)
            }
            TimeUnit::Month => midnight(date.first_of_month(), tz),
            TimeUnit::Year => {
                let step = i64::from(self.step);
                let year = i64::from(date.year()).div_euclid(step) * step;
                midnight(Date::new(i16::try_from(year).ok()?, 1, 1).ok()?, tz)
            }
        }
    }

    /// The start of the next unit after a unit start `t` (for years, `step` years on).
    fn next_unit(self, t: f64, tz: &TimeZone) -> Option<f64> {
        let date = || civil(t, tz).map(|dt| dt.date());
        match self.unit {
            TimeUnit::Second => Some(t + 1.0),
            TimeUnit::Minute => Some(t + 60.0),
            TimeUnit::Hour => Some(t + 3_600.0),
            TimeUnit::Day => midnight(date()?.tomorrow().ok()?, tz),
            TimeUnit::Week => midnight(date()?.checked_add(7.days()).ok()?, tz),
            TimeUnit::Month => midnight(date()?.first_of_month().checked_add(1.month()).ok()?, tz),
            TimeUnit::Year => {
                let year = i64::from(date()?.year()) + i64::from(self.step);
                midnight(Date::new(i16::try_from(year).ok()?, 1, 1).ok()?, tz)
            }
        }
    }

    fn aligned(self, t: f64, tz: &TimeZone) -> bool {
        let Some(dt) = civil(t, tz) else { return false };
        let step = i64::from(self.step);
        let field = match self.unit {
            TimeUnit::Second => i64::from(dt.second()),
            TimeUnit::Minute => i64::from(dt.minute()),
            TimeUnit::Hour => i64::from(dt.hour()),
            TimeUnit::Day => i64::from(dt.day()) - 1,
            TimeUnit::Week => (epoch_day(dt.date()) - 4).div_euclid(7),
            TimeUnit::Month => i64::from(dt.month()) - 1,
            TimeUnit::Year => i64::from(dt.year()),
        };
        field.rem_euclid(step) == 0
    }
}

/// About `count` time ticks between `start` and `stop` (Unix seconds), aligned to the calendar
/// in `tz`, in the domain's direction. Spans too short for whole seconds get plain number ticks
/// on the seconds (`0.25`, `0.5`, …).
pub fn time_ticks(start: f64, stop: f64, count: usize, tz: &TimeZone) -> Vec<f64> {
    if start == stop && start.is_finite() && count > 0 {
        return vec![start];
    }
    match TimeInterval::for_span(start, stop, count) {
        Some(interval) => interval.ticks(start, stop, tz),
        None => step::ticks(start, stop, count),
    }
}

/// The local date and time of the whole second `t`, if it's on the calendar.
pub(crate) fn civil(t: f64, tz: &TimeZone) -> Option<DateTime> {
    if !t.is_finite() || t.abs() > 1e15 {
        return None;
    }
    let ts = Timestamp::from_second(t.floor() as i64).ok()?;
    Some(tz.to_datetime(ts))
}

/// The first instant of `date` in `tz`: midnight, or the end of a gap that skips midnight.
fn midnight(date: Date, tz: &TimeZone) -> Option<f64> {
    let ts = tz.to_ambiguous_timestamp(date.to_datetime(Time::midnight())).compatible().ok()?;
    Some(ts.as_second() as f64)
}

/// Days since 1970-01-01 (a Thursday) in the proleptic Gregorian calendar.
fn epoch_day(date: Date) -> i64 {
    let (m, d) = (i64::from(date.month()), i64::from(date.day()));
    let y = i64::from(date.year()) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
#[path = "interval_tests.rs"]
mod tests;
