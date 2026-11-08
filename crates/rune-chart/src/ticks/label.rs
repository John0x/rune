//! Time tick labels that show what changed, as d3's default multi-scale time format, in 24-hour
//! time and English names.

use jiff::civil::Weekday;
use jiff::tz::TimeZone;

use super::interval::civil;

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// A label for the time tick `t` (Unix seconds) in `tz`, by the largest unit it starts:
///
/// - `.250` within a second, `:30` within a minute;
/// - `06:00` or `06:15` within a day;
/// - `Nov 9` on a Monday, `Tue 10` on another day;
/// - `Nov` on the first of a month, `2027` on January 1.
///
/// So an hourly axis reads `22:00 23:00 Tue 10 01:00`, and a monthly one `Nov Dec 2027 Feb`.
/// A time off the calendar is written as its number of seconds.
pub fn format_time(t: f64, tz: &TimeZone) -> String {
    if !t.is_finite() {
        return format!("{t}");
    }
    let ms = (t * 1000.0).round();
    let (seconds, millis) = ((ms / 1000.0).floor(), ms.rem_euclid(1000.0));
    if millis != 0.0 {
        return format!(".{millis:03}");
    }
    let Some(dt) = civil(seconds, tz) else { return format!("{t}") };
    if dt.second() != 0 {
        format!(":{:02}", dt.second())
    } else if dt.hour() != 0 || dt.minute() != 0 {
        format!("{:02}:{:02}", dt.hour(), dt.minute())
    } else if dt.day() != 1 {
        let month = MONTHS[(dt.month() - 1) as usize];
        if dt.weekday() == Weekday::Monday {
            format!("{month} {}", dt.day())
        } else {
            format!("{} {}", weekday(dt.weekday()), dt.day())
        }
    } else if dt.month() != 1 {
        MONTHS[(dt.month() - 1) as usize].to_owned()
    } else {
        dt.year().to_string()
    }
}

fn weekday(w: Weekday) -> &'static str {
    match w {
        Weekday::Monday => "Mon",
        Weekday::Tuesday => "Tue",
        Weekday::Wednesday => "Wed",
        Weekday::Thursday => "Thu",
        Weekday::Friday => "Fri",
        Weekday::Saturday => "Sat",
        Weekday::Sunday => "Sun",
    }
}

#[cfg(test)]
#[path = "label_tests.rs"]
mod tests;
