use jiff::Timestamp;

use super::*;
use crate::ticks::TimeUnit;

fn berlin() -> TimeZone {
    TimeZone::get("Europe/Berlin").unwrap()
}

fn at(s: &str) -> f64 {
    s.parse::<Timestamp>().unwrap().as_second() as f64
}

#[test]
fn maps_like_a_linear_scale() {
    let (a, b) = (at("2026-11-09T00:00:00Z"), at("2026-11-10T00:00:00Z"));
    let s = Time::new((a, b), (0.0, 240.0)).with_time_zone(berlin());
    assert_eq!(s.map(a + 43_200.0), 120.0);
    assert_eq!(s.invert(60.0), a + 21_600.0);
    assert_eq!(s.clone().clamp(true).map(b + 86_400.0), 240.0);
    assert_eq!(s.time_zone(), &berlin());
    assert_eq!(s.clone().with_range((0.0, 10.0)).range(), (0.0, 10.0));
    assert_eq!(s.with_domain((a, a + 1.0)).domain(), (a, a + 1.0));
}

#[test]
fn ticks_are_calendar_aligned_in_the_zone() {
    let (a, b) = (at("2026-11-09T00:30:00+01:00"), at("2026-11-09T12:30:00+01:00"));
    let s = Time::new((a, b), (0.0, 600.0)).with_time_zone(berlin());
    assert_eq!(s.interval(4), Some(TimeInterval::new(TimeUnit::Hour, 3)));
    let labels: Vec<String> = s.ticks(4).iter().map(|&t| s.format(t)).collect();
    assert_eq!(labels, ["03:00", "06:00", "09:00", "12:00"]);
    let utc = s.with_time_zone(TimeZone::UTC);
    let labels: Vec<String> = utc.ticks(4).iter().map(|&t| Continuous::format_tick(&utc, t, 4)).collect();
    assert_eq!(labels, ["Nov 9", "03:00", "06:00", "09:00"]);
}

#[test]
fn nice_extends_to_the_interval() {
    let (a, b) = (at("2026-11-09T13:20:00+01:00"), at("2026-11-12T09:00:00+01:00"));
    let s = Time::new((a, b), (0.0, 600.0)).with_time_zone(berlin()).nice(3);
    assert_eq!(s.domain(), (at("2026-11-09T00:00:00+01:00"), at("2026-11-13T00:00:00+01:00")));
    let reversed = Time::new((b, a), (0.0, 600.0)).with_time_zone(berlin()).nice(3);
    assert_eq!(reversed.domain(), (at("2026-11-13T00:00:00+01:00"), at("2026-11-09T00:00:00+01:00")));
}

#[test]
fn sub_second_spans_tick_and_nice_as_numbers() {
    let s = Time::new((10.13, 10.97), (0.0, 100.0)).with_time_zone(TimeZone::UTC);
    assert_eq!(s.interval(5), None);
    assert_eq!(s.clone().nice(10).domain(), (10.1, 11.0));
    assert_eq!(s.ticks(4), [10.2, 10.4, 10.6, 10.8]);
    assert_eq!(s.format(10.2), ".200");
}

#[test]
fn degenerate_domains_are_safe() {
    let s = Time::new((100.0, 100.0), (0.0, 100.0)).with_time_zone(TimeZone::UTC);
    assert_eq!(s.map(100.0), 50.0);
    assert_eq!(s.ticks(5), [100.0]);
    assert_eq!(s.clone().nice(5).domain(), (100.0, 100.0));
    let nan = Time::new((f64::NAN, 1.0), (0.0, 100.0));
    assert!(nan.ticks(5).is_empty());
}
