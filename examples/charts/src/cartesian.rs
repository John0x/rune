//! Gallery sections for the cartesian plots: areas, bars, candles, scatter, rules.

use rune::{El, col};
use rune_chart::data::{Ohlc, Series};
use rune_chart::layout::Curve;
use rune_chart::{
    Area, Axes, Axis, Bars, Candles, Chart, ChartStyle, Line, Rule, Scatter, StackedArea,
};

use crate::data::{START, noise, walk};
use crate::section;

const HOUR: f64 = 3600.0;
const DAY: f64 = 86_400.0;

fn tz() -> jiff::tz::TimeZone {
    jiff::tz::TimeZone::get("Europe/Berlin").unwrap_or(jiff::tz::TimeZone::UTC)
}

fn gold(v: f64) -> String {
    format!("{v:.2}g")
}

/// `series` shifted by a seeded amount between `-low` and `-high` per point.
fn below(series: &Series, seed: u64, low: f64, high: f64) -> Series {
    let y: Vec<f64> = (series.y().iter().enumerate())
        .map(|(i, v)| v - low - noise(seed, i) * (high - low))
        .collect();
    Series::new(series.x().clone(), y).unwrap_or_default()
}

/// `n` daily values around `level`, swinging by `swing`.
fn daily(n: usize, seed: u64, level: f64, swing: f64) -> Series {
    Series::from_fn(n, |i| {
        let wave = (i as f64 * 0.35 + seed as f64).sin() * 0.5;
        (
            START + i as f64 * DAY,
            (level + (noise(seed, i) - 0.5 + wave) * swing).max(0.0),
        )
    })
    .unwrap_or_default()
}

/// `n` hourly candles of a random walk around `level`.
fn candles(n: usize, seed: u64, level: f64) -> Ohlc {
    let mut close = level;
    let rows = (0..n).map(|i| {
        let open = close;
        close = (open + (noise(seed, i) - 0.48) * level * 0.03).max(level * 0.5);
        let high = open.max(close) + noise(seed + 1, i) * level * 0.012;
        let low = open.min(close) - noise(seed + 2, i) * level * 0.012;
        (START + i as f64 * HOUR, open, high, low, close)
    });
    Ohlc::from_rows(rows).unwrap_or_default()
}

fn area() -> El {
    let median = walk(240, 7, 20.0);
    let p25 = below(&median, 5, 0.4, 1.4);
    let blue = ChartStyle::dark().color(0);
    Chart::new("cart-area")
        .x(Axis::time_in(tz()))
        .y(Axis::linear().format(gold))
        .style(ChartStyle::dark())
        .plot(
            Area::band("25th percentile", median.clone(), p25)
                .color(blue)
                .opacity(0.25),
        )
        .plot(Line::new("Median", median).color(blue))
        .plot(
            Area::new("Cheapest", walk(240, 11, 16.0))
                .gradient(true)
                .opacity(0.45),
        )
        .build()
}

fn stacked_area() -> El {
    let layers = [
        ("Weapons", daily(90, 1, 40.0, 30.0)),
        ("Armour", daily(90, 2, 30.0, 24.0)),
        ("Potions", daily(90, 3, 20.0, 16.0)),
    ];
    Chart::new("cart-stacked-area")
        .x(Axis::time_in(tz()))
        .y(Axis::linear().include_zero())
        .plot(StackedArea::new(layers).curve(Curve::MonotoneX))
        .build()
}

const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

fn grouped_bars() -> El {
    let week = |seed: u64, level: f64, swing: f64| {
        Series::from_y((0..7).map(|i| level + (noise(seed, i) - 0.5) * swing))
    };
    Chart::new("cart-grouped-bars")
        .x(Axis::linear().grid(false).format(|v| {
            let i = v.round();
            if (v - i).abs() < 1e-6 && (0.0..7.0).contains(&i) {
                DAYS[i as usize].to_owned()
            } else {
                String::new()
            }
        }))
        .y(Axis::linear().format(gold))
        .plot(Bars::grouped([
            ("Bought", week(4, 120.0, 80.0)),
            ("Sold", week(5, 140.0, 90.0)),
            ("Profit", week(6, 15.0, 90.0)),
        ]))
        .build()
}

fn stacked_bars() -> El {
    Chart::new("cart-stacked-bars")
        .x(Axis::time_in(tz()))
        .plot(Bars::stacked([
            ("Auction house", daily(30, 7, 50.0, 40.0)),
            ("Vendor", daily(30, 8, 25.0, 20.0)),
            ("Trade", daily(30, 9, 12.0, 14.0)),
        ]))
        .build()
}

fn candles_over_volume() -> El {
    let ohlc = candles(160, 21, 20.0);
    let volume = Series::from_fn(ohlc.len(), |i| {
        let swing = (ohlc.close()[i] - ohlc.open()[i]).abs();
        (ohlc.x()[i], 200.0 + noise(23, i) * 600.0 + swing * 900.0)
    })
    .unwrap_or_default();
    let rising = ohlc.clone();
    let style = ChartStyle::dark();
    let (up, down) = (style.up, style.down);
    let price = Chart::new("cart-candles")
        .link("cart-market")
        .gutter(52.0)
        .x(Axis::time_in(tz()).hidden())
        .y(Axis::linear().format(gold))
        .plot(Candles::new("Price", ohlc))
        .build();
    let volume = Chart::new("cart-volume")
        .link("cart-market")
        .gutter(52.0)
        .height(80.0)
        .legend(false)
        .x(Axis::time_in(tz()))
        .y(Axis::linear().include_zero().format(|v| format!("{v:.0}")))
        .plot(Bars::new("Volume", volume).color_by(move |x, _| {
            let i = rising.x().partition_point(|v| *v < x);
            let up_day = i < rising.len() && rising.close()[i] >= rising.open()[i];
            if up_day {
                up.alpha(0.7)
            } else {
                down.alpha(0.7)
            }
        }))
        .build();
    col()
        .key("market")
        .w_full()
        .gap(4.0)
        .child(price)
        .child(volume)
}

fn scatter() -> El {
    let points: Vec<(f64, f64)> = (0..400)
        .map(|i| {
            let x = noise(31, i) * 100.0;
            (x, 10.0 + x * 0.6 + (noise(32, i) - 0.5) * 40.0)
        })
        .collect();
    Chart::new("cart-scatter")
        .x(Axis::linear().format(|v| format!("{v:.0} lvl")))
        .y(Axis::linear().format(gold))
        .zoom_axes(Axes::Both)
        .plot(
            Scatter::new("Trades", points)
                .size_by(|_, y| (2.0 + y as f32 / 25.0).clamp(2.0, 6.0))
                .color_by(|x, _| {
                    let t = (x / 100.0) as f32;
                    rune::oklch(0.72, 0.15, 255.0 - t * 200.0).alpha(0.8)
                }),
        )
        .build()
}

fn rules() -> El {
    Chart::new("cart-rules")
        .x(Axis::time_in(tz()))
        .y(Axis::linear().format(gold))
        .plot(Line::new("Median", walk(300, 13, 20.0)))
        .plot(Rule::y_band("Target range", 19.0, 20.5))
        .plot(Rule::y("7-day median", 19.6).dashed(4.0, 4.0))
        .plot(Rule::x("Patch 1.4", START + 180.0 * HOUR).dashed(2.0, 3.0))
        .build()
}

pub(crate) fn sections() -> Vec<El> {
    vec![
        section("Area: gradient and a percentile band", area()),
        section("Area: stacked", stacked_area()),
        section("Bars: grouped, with negatives", grouped_bars()),
        section("Bars: stacked", stacked_bars()),
        section("Candles over volume (linked x)", candles_over_volume()),
        section("Scatter: sized and coloured by value", scatter()),
        section("Rules: a line, a band and a marker", rules()),
    ]
}
