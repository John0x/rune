//! A chart's axes: which scale each is, how its values read, and its ticks and labels.

use std::rc::Rc;

use jiff::tz::TimeZone;

use crate::scale::{Continuous, Linear, Log, Time};

#[derive(Clone)]
enum Kind {
    Linear,
    Log,
    Time(Option<TimeZone>),
}

/// An x or y axis: its scale (linear, log or time), an optional fixed domain, its value format
/// and whether it draws grid lines.
#[derive(Clone)]
pub struct Axis {
    kind: Kind,
    pub(crate) format: Option<Rc<dyn Fn(f64) -> String>>,
    pub(crate) domain: Option<(f64, f64)>,
    pub(crate) grid: bool,
    pub(crate) zero: bool,
    pub(crate) visible: bool,
}

impl Axis {
    fn of(kind: Kind) -> Axis {
        Axis { kind, format: None, domain: None, grid: true, zero: false, visible: true }
    }

    /// A linear scale.
    pub fn linear() -> Axis {
        Axis::of(Kind::Linear)
    }

    /// A base-10 log scale (positive values only).
    pub fn log() -> Axis {
        Axis::of(Kind::Log)
    }

    /// Time in Unix seconds, ticked and labelled in the system's time zone.
    pub fn time() -> Axis {
        Axis::of(Kind::Time(None))
    }

    /// Time in Unix seconds, in `tz`.
    pub fn time_in(tz: TimeZone) -> Axis {
        Axis::of(Kind::Time(Some(tz)))
    }

    /// How values read in labels and tooltips (prices, percentages).
    pub fn format(mut self, f: impl Fn(f64) -> String + 'static) -> Axis {
        self.format = Some(Rc::new(f));
        self
    }

    /// A fixed domain instead of one fitted to the data (zoom and pan still move from it).
    pub fn domain(mut self, min: f64, max: f64) -> Axis {
        self.domain = Some((min, max));
        self
    }

    /// Grid lines at the ticks (on by default).
    pub fn grid(mut self, grid: bool) -> Axis {
        self.grid = grid;
        self
    }

    /// Keep zero in the fitted domain (bars and volumes).
    pub fn include_zero(mut self) -> Axis {
        self.zero = true;
        self
    }

    /// Hides the axis' labels (the scale still applies).
    pub fn hidden(mut self) -> Axis {
        self.visible = false;
        self
    }

    pub(crate) fn is_log(&self) -> bool {
        matches!(self.kind, Kind::Log)
    }

    /// The scale from `domain` to `range`.
    pub(crate) fn scale(&self, domain: (f64, f64), range: (f32, f32)) -> Rc<dyn Continuous> {
        match &self.kind {
            Kind::Linear => Rc::new(Linear::new(domain, range)),
            Kind::Log => Rc::new(Log::new(domain, range)),
            Kind::Time(None) => Rc::new(Time::new(domain, range)),
            Kind::Time(Some(tz)) => Rc::new(Time::new(domain, range).with_time_zone(tz.clone())),
        }
    }

    /// How a value reads: the axis' format, or the scale's own for `count` ticks.
    pub(crate) fn formatter(&self, scale: &Rc<dyn Continuous>, count: usize) -> Rc<dyn Fn(f64) -> String> {
        match &self.format {
            Some(f) => f.clone(),
            None => {
                let scale = scale.clone();
                Rc::new(move |v| scale.format_tick(v, count))
            }
        }
    }
}

impl Axis {
    /// How a value reads in a tooltip's title: the axis' format, a full date and time on a time
    /// axis, else the scale's format.
    pub(crate) fn title_formatter(&self, scale: &Rc<dyn Continuous>) -> Rc<dyn Fn(f64) -> String> {
        if self.format.is_some() {
            return self.formatter(scale, 10);
        }
        match &self.kind {
            Kind::Time(tz) => {
                let tz = tz.clone().unwrap_or_else(TimeZone::system);
                Rc::new(move |t| {
                    jiff::Timestamp::from_second(t as i64)
                        .map(|at| at.to_zoned(tz.clone()).strftime("%a %-d %b %Y, %H:%M").to_string())
                        .unwrap_or_default()
                })
            }
            _ => self.formatter(scale, 10),
        }
    }
}

impl Default for Axis {
    fn default() -> Axis {
        Axis::linear()
    }
}
