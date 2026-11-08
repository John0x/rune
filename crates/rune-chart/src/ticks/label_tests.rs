use jiff::Timestamp;

use super::*;
use crate::ticks::{TimeInterval, TimeUnit, time_ticks};

fn berlin() -> TimeZone {
    TimeZone::get("Europe/Berlin").unwrap()
}

fn at(s: &str) -> f64 {
    s.parse::<Timestamp>().unwrap().as_second() as f64
}

#[test]
fn labels_show_the_largest_unit_that_starts() {
    let tz = berlin();
    assert_eq!(format_time(at("2026-11-09T06:00:00+01:00"), &tz), "06:00");
    assert_eq!(format_time(at("2026-11-09T06:15:00+01:00"), &tz), "06:15");
    assert_eq!(format_time(at("2026-11-09T06:15:30+01:00"), &tz), ":30");
    assert_eq!(format_time(at("2026-11-09T06:15:30+01:00") + 0.25, &tz), ".250");
    assert_eq!(format_time(at("2026-11-09T00:00:00+01:00"), &tz), "Nov 9", "a Monday shows its month");
    assert_eq!(format_time(at("2026-11-10T00:00:00+01:00"), &tz), "Tue 10");
    assert_eq!(format_time(at("2026-11-01T00:00:00+01:00"), &tz), "Nov");
    assert_eq!(format_time(at("2027-01-01T00:00:00+01:00"), &tz), "2027");
}

#[test]
fn labels_are_local() {
    let t = at("2026-11-09T00:00:00Z");
    assert_eq!(format_time(t, &TimeZone::UTC), "Nov 9");
    assert_eq!(format_time(t, &berlin()), "01:00");
    assert_eq!(format_time(t, &TimeZone::get("America/New_York").unwrap()), "19:00");
}

#[test]
fn axes_read_as_documented() {
    let tz = berlin();
    let hourly = TimeInterval::new(TimeUnit::Hour, 1).ticks(
        at("2026-11-09T22:00:00+01:00"),
        at("2026-11-10T01:00:00+01:00"),
        &tz,
    );
    assert_eq!(hourly.iter().map(|&t| format_time(t, &tz)).collect::<Vec<_>>(), ["22:00", "23:00", "Tue 10", "01:00"]);
    let monthly = TimeInterval::new(TimeUnit::Month, 1).ticks(
        at("2026-11-01T00:00:00+01:00"),
        at("2027-02-01T00:00:00+01:00"),
        &tz,
    );
    assert_eq!(monthly.iter().map(|&t| format_time(t, &tz)).collect::<Vec<_>>(), ["Nov", "Dec", "2027", "Feb"]);
}

#[test]
fn labels_across_the_night_clocks_go_back() {
    let tz = berlin();
    let t = time_ticks(at("2026-10-25T00:00:00+02:00"), at("2026-10-25T04:00:00+01:00"), 5, &tz);
    let labels: Vec<String> = t.iter().map(|&t| format_time(t, &tz)).collect();
    assert_eq!(labels, ["Sun 25", "01:00", "02:00", "02:00", "03:00", "04:00"]);
}

#[test]
fn labels_off_the_calendar_are_numbers() {
    assert_eq!(format_time(f64::NAN, &TimeZone::UTC), "NaN");
    assert_eq!(format_time(1e17, &TimeZone::UTC), "100000000000000000");
    assert_eq!(format_time(-0.5, &TimeZone::UTC), ".500");
}
