use super::*;

fn berlin() -> TimeZone {
    TimeZone::get("Europe/Berlin").unwrap()
}

/// Unix seconds of an RFC 3339 time.
fn at(s: &str) -> f64 {
    s.parse::<Timestamp>().unwrap().as_second() as f64
}

/// Ticks written in local time with their offset.
fn local(ticks: &[f64], tz: &TimeZone) -> Vec<String> {
    ticks
        .iter()
        .map(|&t| {
            Timestamp::from_second(t as i64).unwrap().to_zoned(tz.clone()).strftime("%m-%d %H:%M %:z").to_string()
        })
        .collect()
}

#[test]
fn intervals_follow_the_span() {
    let pick = |span: f64, count| TimeInterval::for_span(0.0, span, count);
    assert_eq!(pick(60.0, 6), Some(TimeInterval::new(TimeUnit::Second, 15)));
    assert_eq!(pick(3_600.0, 10), Some(TimeInterval::new(TimeUnit::Minute, 5)));
    assert_eq!(pick(86_400.0, 8), Some(TimeInterval::new(TimeUnit::Hour, 3)));
    assert_eq!(pick(86_400.0 * 7.0, 7), Some(TimeInterval::new(TimeUnit::Day, 1)));
    assert_eq!(pick(86_400.0 * 60.0, 8), Some(TimeInterval::new(TimeUnit::Week, 1)));
    assert_eq!(pick(86_400.0 * 365.0, 4), Some(TimeInterval::new(TimeUnit::Month, 3)));
    assert_eq!(pick(86_400.0 * 365.0 * 3.0, 3), Some(TimeInterval::new(TimeUnit::Year, 1)));
    assert_eq!(pick(86_400.0 * 365.0 * 30.0, 10), Some(TimeInterval::new(TimeUnit::Year, 2)));
    assert_eq!(pick(86_400.0 * 365.0 * 300.0, 6), Some(TimeInterval::new(TimeUnit::Year, 50)));
}

#[test]
fn sub_second_spans_and_bad_input_have_no_interval() {
    assert_eq!(TimeInterval::for_span(0.0, 1.0, 10), None);
    assert_eq!(TimeInterval::for_span(0.0, 100.0, 0), None);
    assert_eq!(TimeInterval::for_span(f64::NAN, 100.0, 5), None);
    assert_eq!(TimeInterval::new(TimeUnit::Hour, 0).step, 1);
}

#[test]
fn hours_skip_the_missing_hour_when_clocks_go_forward() {
    let tz = berlin();
    let t = TimeInterval::new(TimeUnit::Hour, 1).ticks(
        at("2026-03-28T23:00:00+01:00"),
        at("2026-03-29T05:00:00+02:00"),
        &tz,
    );
    assert_eq!(
        local(&t, &tz),
        [
            "03-28 23:00 +01:00",
            "03-29 00:00 +01:00",
            "03-29 01:00 +01:00",
            "03-29 03:00 +02:00",
            "03-29 04:00 +02:00",
            "03-29 05:00 +02:00"
        ]
    );
}

#[test]
fn hours_repeat_the_doubled_hour_when_clocks_go_back() {
    let tz = berlin();
    let t = TimeInterval::new(TimeUnit::Hour, 1).ticks(
        at("2026-10-25T00:00:00+02:00"),
        at("2026-10-25T04:00:00+01:00"),
        &tz,
    );
    assert_eq!(
        local(&t, &tz),
        [
            "10-25 00:00 +02:00",
            "10-25 01:00 +02:00",
            "10-25 02:00 +02:00",
            "10-25 02:00 +01:00",
            "10-25 03:00 +01:00",
            "10-25 04:00 +01:00"
        ]
    );
    assert!(t.windows(2).all(|w| w[1] - w[0] == 3_600.0));
}

#[test]
fn multi_hour_steps_stay_on_local_multiples_across_dst() {
    let tz = berlin();
    let t = TimeInterval::new(TimeUnit::Hour, 3).ticks(
        at("2026-10-24T20:30:00+02:00"),
        at("2026-10-25T09:00:00+01:00"),
        &tz,
    );
    assert_eq!(
        local(&t, &tz),
        ["10-24 21:00 +02:00", "10-25 00:00 +02:00", "10-25 03:00 +01:00", "10-25 06:00 +01:00", "10-25 09:00 +01:00"]
    );
    assert_eq!(t[2] - t[1], 4.0 * 3_600.0, "the doubled hour falls inside one step");
}

#[test]
fn days_start_at_local_midnight_across_dst() {
    let tz = berlin();
    let t = TimeInterval::new(TimeUnit::Day, 1).ticks(
        at("2026-03-27T12:00:00+01:00"),
        at("2026-03-31T00:00:00+02:00"),
        &tz,
    );
    assert_eq!(
        local(&t, &tz),
        ["03-28 00:00 +01:00", "03-29 00:00 +01:00", "03-30 00:00 +02:00", "03-31 00:00 +02:00"]
    );
    assert_eq!(t[2] - t[1], 23.0 * 3_600.0);
    let t = TimeInterval::new(TimeUnit::Day, 1).ticks(
        at("2026-10-25T00:00:00+02:00"),
        at("2026-10-26T00:00:00+01:00"),
        &tz,
    );
    assert_eq!(t[1] - t[0], 25.0 * 3_600.0);
}

#[test]
fn every_other_day_restarts_each_month() {
    let tz = berlin();
    let t = TimeInterval::new(TimeUnit::Day, 2).ticks(
        at("2026-01-28T00:00:00+01:00"),
        at("2026-02-04T00:00:00+01:00"),
        &tz,
    );
    assert_eq!(
        local(&t, &tz),
        ["01-29 00:00 +01:00", "01-31 00:00 +01:00", "02-01 00:00 +01:00", "02-03 00:00 +01:00"]
    );
}

#[test]
fn weeks_start_on_monday() {
    let tz = berlin();
    let t = TimeInterval::new(TimeUnit::Week, 1).ticks(
        at("2026-11-01T00:00:00+01:00"),
        at("2026-11-30T00:00:00+01:00"),
        &tz,
    );
    assert_eq!(
        local(&t, &tz),
        ["11-02 00:00 +01:00", "11-09 00:00 +01:00", "11-16 00:00 +01:00", "11-23 00:00 +01:00", "11-30 00:00 +01:00"]
    );
}

#[test]
fn months_and_quarters_start_on_the_first() {
    let tz = berlin();
    let (a, b) = (at("2026-01-01T00:00:00+01:00"), at("2026-06-01T00:00:00+02:00"));
    let t = TimeInterval::new(TimeUnit::Month, 1).ticks(a, b, &tz);
    assert_eq!(
        local(&t, &tz),
        [
            "01-01 00:00 +01:00",
            "02-01 00:00 +01:00",
            "03-01 00:00 +01:00",
            "04-01 00:00 +02:00",
            "05-01 00:00 +02:00",
            "06-01 00:00 +02:00"
        ]
    );
    let t = TimeInterval::new(TimeUnit::Month, 3).ticks(a, at("2027-01-01T00:00:00+01:00"), &tz);
    assert_eq!(
        local(&t, &tz),
        ["01-01 00:00 +01:00", "04-01 00:00 +02:00", "07-01 00:00 +02:00", "10-01 00:00 +02:00", "01-01 00:00 +01:00"]
    );
}

#[test]
fn years_step_on_multiples() {
    let tz = TimeZone::UTC;
    let t = TimeInterval::new(TimeUnit::Year, 5).ticks(at("2013-06-01T00:00:00Z"), at("2031-01-01T00:00:00Z"), &tz);
    assert_eq!(
        t,
        [
            at("2015-01-01T00:00:00Z"),
            at("2020-01-01T00:00:00Z"),
            at("2025-01-01T00:00:00Z"),
            at("2030-01-01T00:00:00Z")
        ]
    );
}

#[test]
fn minutes_and_seconds_align_to_their_step() {
    let tz = berlin();
    let t = TimeInterval::new(TimeUnit::Minute, 15).ticks(
        at("2026-11-08T10:07:00+01:00"),
        at("2026-11-08T11:07:00+01:00"),
        &tz,
    );
    assert_eq!(
        local(&t, &tz),
        ["11-08 10:15 +01:00", "11-08 10:30 +01:00", "11-08 10:45 +01:00", "11-08 11:00 +01:00"]
    );
    let start = at("2026-11-08T10:00:07+01:00");
    let t = TimeInterval::new(TimeUnit::Second, 15).ticks(start, start + 50.0, &tz);
    assert_eq!(t, [start + 8.0, start + 23.0, start + 38.0]);
}

#[test]
fn floor_and_ceil_land_on_the_calendar() {
    let tz = berlin();
    let day = TimeInterval::new(TimeUnit::Day, 1);
    let noon = at("2026-03-29T12:00:00+02:00");
    assert_eq!(day.floor(noon, &tz), at("2026-03-29T00:00:00+01:00"));
    assert_eq!(day.ceil(noon, &tz), at("2026-03-30T00:00:00+02:00"));
    let midnight = at("2026-03-30T00:00:00+02:00");
    assert_eq!(day.floor(midnight, &tz), midnight);
    assert_eq!(day.ceil(midnight, &tz), midnight);
    let six = TimeInterval::new(TimeUnit::Hour, 6);
    assert_eq!(six.floor(at("2026-11-08T07:30:00+01:00"), &tz), at("2026-11-08T06:00:00+01:00"));
    assert_eq!(six.ceil(at("2026-11-08T07:30:00+01:00"), &tz), at("2026-11-08T12:00:00+01:00"));
    let decade = TimeInterval::new(TimeUnit::Year, 10);
    assert_eq!(decade.floor(at("2026-11-08T07:30:00+01:00"), &tz), at("2020-01-01T00:00:00+01:00"));
    assert_eq!(decade.ceil(at("2026-11-08T07:30:00+01:00"), &tz), at("2030-01-01T00:00:00+01:00"));
    let week = TimeInterval::new(TimeUnit::Week, 1);
    assert_eq!(week.floor(at("2026-11-08T07:30:00+01:00"), &tz), at("2026-11-02T00:00:00+01:00"));
}

#[test]
fn reversed_domains_tick_backward() {
    let tz = TimeZone::UTC;
    let t = TimeInterval::new(TimeUnit::Hour, 1).ticks(7_200.0, 0.0, &tz);
    assert_eq!(t, [7_200.0, 3_600.0, 0.0]);
}

#[test]
fn off_calendar_and_huge_requests_stay_bounded() {
    let tz = TimeZone::UTC;
    let hour = TimeInterval::new(TimeUnit::Hour, 1);
    assert!(hour.ticks(f64::NAN, 10.0, &tz).is_empty());
    assert!(hour.ticks(1e14, 1e14 + 7_200.0, &tz).is_empty());
    assert_eq!(hour.floor(1e14, &tz), 1e14);
    assert_eq!(hour.ceil(f64::INFINITY, &tz), f64::INFINITY);
    let fine = TimeInterval::new(TimeUnit::Second, 1).ticks(0.0, 1e9, &tz);
    assert_eq!(fine.len(), MAX_TICKS);
    assert!(time_ticks(0.0, 1e9, 1_000_000, &tz).len() <= MAX_TICKS);
}

#[test]
fn time_ticks_fall_back_to_numbers_under_a_second() {
    let tz = TimeZone::UTC;
    assert_eq!(time_ticks(0.0, 1.0, 4, &tz), [0.0, 0.2, 0.4, 0.6, 0.8, 1.0]);
    assert_eq!(time_ticks(5.0, 5.0, 4, &tz), [5.0]);
    assert!(time_ticks(0.0, 10.0, 0, &tz).is_empty());
    assert_eq!(time_ticks(0.0, 6.0 * 3_600.0, 6, &tz), [0.0, 3_600.0, 7_200.0, 10_800.0, 14_400.0, 18_000.0, 21_600.0]);
}
