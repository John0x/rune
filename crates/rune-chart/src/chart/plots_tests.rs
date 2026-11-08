use std::collections::BTreeSet;
use std::rc::Rc;

use rune::Shape;

use super::*;
use crate::data::{Ohlc, Series};
use crate::scale::Linear;
use crate::viewport::Viewport;

/// A 100 × 100 point frame over x 0..10 and y 0..10.
fn frame() -> Frame {
    frame_over((0.0, 10.0), (0.0, 10.0), 100.0)
}

fn frame_over(x: (f64, f64), y: (f64, f64), width: f32) -> Frame {
    Frame {
        size: [width, 100.0],
        viewport: Viewport { x, y },
        x: Rc::new(Linear::new(x, (0.0, width))),
        y: Rc::new(Linear::new(y, (100.0, 0.0))),
        format_x: Rc::new(|v| format!("{v}")),
        format_y: Rc::new(|v| format!("{v}")),
        hover: None,
    }
}

fn prepared<P: Plot>(mut plot: P) -> P {
    plot.set_default_color(rune::Color::WHITE);
    plot.prepare(&ChartStyle::dark(), 0, &BTreeSet::new());
    plot
}

fn empty_plots() -> Vec<Box<dyn Plot>> {
    let empty = Series::default;
    vec![
        Box::new(Area::new("a", empty())),
        Box::new(Area::band("b", empty(), empty())),
        Box::new(StackedArea::new(Vec::<(String, Series)>::new())),
        Box::new(StackedArea::new([("s", empty())])),
        Box::new(Bars::new("b", empty())),
        Box::new(Bars::stacked([("s", empty()), ("t", empty())])),
        Box::new(Candles::new("c", Ohlc::default())),
        Box::new(Scatter::new("p", [])),
    ]
}

#[test]
fn empty_data_draws_nothing_and_has_no_extent() {
    for mut plot in empty_plots() {
        plot.prepare(&ChartStyle::dark(), 0, &BTreeSet::new());
        assert!(plot.draw(&frame(), Shape::new()).is_empty(), "{}", plot.name());
        assert_eq!(plot.x_extent(), None);
        assert_eq!(plot.y_extent((0.0, 10.0)), None);
        assert!(plot.hover_points(&frame(), 1.0, [10.0, 10.0]).is_empty());
        assert_eq!(plot.snap(1.0), None);
    }
}

#[test]
fn gaps_are_skipped() {
    let nan = Series::from_y([f64::NAN, f64::NAN]);
    assert!(prepared(Area::new("a", nan.clone())).draw(&frame(), Shape::new()).is_empty());
    assert!(prepared(Bars::new("b", nan.clone())).draw(&frame(), Shape::new()).is_empty());
    assert!(prepared(Scatter::new("p", [(1.0, f64::NAN)])).points().is_empty());
    let gap = Series::from_y([1.0, f64::NAN, 3.0]);
    let bars = prepared(Bars::new("b", gap));
    assert!(bars.hover_points(&frame(), 1.0, [0.0, 0.0]).is_empty(), "no row for a gap");
    assert_eq!(bars.hover_points(&frame(), 2.0, [0.0, 0.0]).len(), 1);
}

#[test]
fn bars_fit_zero_and_pad_x_by_half_a_bar() {
    let bars = prepared(Bars::new("b", Series::from_y([3.0, -2.0, 5.0])));
    assert_eq!(bars.y_extent((0.0, 2.0)), Some((-2.0, 5.0)));
    assert_eq!(bars.x_extent(), Some((-0.5, 2.5)));
    let up = prepared(Bars::new("b", Series::from_y([3.0, 4.0])));
    assert_eq!(up.y_extent((0.0, 1.0)), Some((0.0, 4.0)));
    let stacked = prepared(Bars::stacked([("a", Series::from_y([1.0, 2.0])), ("b", Series::from_y([3.0, -1.0]))]));
    assert_eq!(stacked.y_extent((0.0, 1.0)), Some((-1.0, 4.0)));
    assert_eq!(stacked.legend().len(), 2);
    let rows = stacked.hover_points(&frame(), 0.0, [0.0, 0.0]);
    let labels: Vec<_> = rows.iter().map(|p| p.row.label.as_str()).collect();
    assert_eq!(labels, ["b", "a"], "the top series first");
}

#[test]
fn hidden_series_restack() {
    let mut stacked = Bars::stacked([("a", Series::from_y([1.0])), ("b", Series::from_y([3.0]))]);
    stacked.prepare(&ChartStyle::dark(), 0, &BTreeSet::from(["a".to_owned()]));
    assert_eq!(stacked.y_extent((0.0, 0.0)), Some((0.0, 3.0)));
    let mut area = StackedArea::new([("a", Series::from_y([1.0, 1.0])), ("b", Series::from_y([3.0, 2.0]))]);
    assert_eq!(prepared(area.clone()).y_extent((0.0, 1.0)), Some((0.0, 4.0)));
    area.prepare(&ChartStyle::dark(), 0, &BTreeSet::from(["a".to_owned()]));
    assert_eq!(area.y_extent((0.0, 1.0)), Some((0.0, 3.0)));
    assert_eq!(area.hover_points(&frame(), 0.0, [0.0, 0.0]).len(), 1);
}

#[test]
fn many_bars_and_candles_merge_per_column() {
    let n = 50_000;
    let series = Series::from_fn(n, |i| (i as f64, (i % 7) as f64)).unwrap();
    let wide = frame_over((0.0, n as f64), (0.0, 10.0), 400.0);
    assert!(!prepared(Bars::new("b", series.clone())).draw(&wide, Shape::new()).is_empty());
    let stacked = prepared(Bars::stacked([("a", series.clone()), ("b", series)]));
    assert!(!stacked.draw(&wide, Shape::new()).is_empty());
    let ohlc = Ohlc::from_rows((0..n).map(|i| (i as f64, 2.0, 4.0, 1.0, 3.0))).unwrap();
    assert!(!prepared(Candles::new("c", ohlc)).draw(&wide, Shape::new()).is_empty());
}

#[test]
fn candles_show_four_rows_in_their_tone() {
    let ohlc = Ohlc::from_rows([(0.0, 2.0, 4.0, 1.0, 3.0), (1.0, 3.0, 3.5, 1.0, 1.5)]).unwrap();
    let candles = prepared(Candles::new("c", ohlc));
    let style = ChartStyle::dark();
    let rising = candles.hover_points(&frame(), 0.0, [0.0, 0.0]);
    let rows: Vec<_> = rising.iter().map(|p| (p.row.label.as_str(), p.row.value.as_str())).collect();
    assert_eq!(rows, [("Open", "2"), ("High", "4"), ("Low", "1"), ("Close", "3")]);
    assert!(rising.iter().all(|p| p.row.color == style.up && p.dot.is_none()));
    assert!(candles.hover_points(&frame(), 1.0, [0.0, 0.0]).iter().all(|p| p.row.color == style.down));
    assert_eq!(candles.y_extent((0.0, 0.0)), Some((1.0, 4.0)));
}

#[test]
fn band_hover_reads_lower_to_upper() {
    let band = prepared(Area::band("b", Series::from_y([5.0, 6.0]), Series::from_y([2.0, f64::NAN])));
    let point = band.hover(&frame(), 0.0, [0.0, 0.0]).unwrap();
    assert_eq!(point.row.value, "2 – 5");
    assert!(band.hover(&frame(), 1.0, [0.0, 0.0]).is_none(), "a gap in either series is a gap");
    assert_eq!(band.y_extent((0.0, 1.0)), Some((2.0, 5.0)), "the masked gap leaves the extent too");
}

#[test]
fn scatter_hovers_the_nearest_point_within_reach() {
    let scatter = prepared(Scatter::new("p", [(5.0, 5.0), (1.0, 1.0), (5.5, 5.0)]));
    assert_eq!(scatter.points().x().as_ref(), [1.0, 5.0, 5.5], "sorted by x");
    assert!(scatter.follows_pointer());
    assert_eq!(scatter.snap(5.0), None);
    let near = scatter.hover(&frame(), 0.0, [52.0, 50.0]).unwrap();
    assert_eq!((near.x, near.dot), (5.0, Some([50.0, 50.0])));
    assert!(scatter.hover(&frame(), 0.0, [90.0, 90.0]).is_none());
}

#[test]
fn rules_fit_y_only_and_stay_out_of_the_legend() {
    let rule = prepared(Rule::y("median", 7.0));
    assert_eq!((rule.x_extent(), rule.y_extent((0.0, 1.0))), (None, Some((7.0, 7.0))));
    assert_eq!(prepared(Rule::y("m", 7.0).fit(false)).y_extent((0.0, 1.0)), None);
    assert_eq!(prepared(Rule::y_band("t", 4.0, 2.0)).y_extent((0.0, 1.0)), Some((2.0, 4.0)));
    assert_eq!(prepared(Rule::x("release", 3.0)).y_extent((0.0, 1.0)), None);
    assert!(rule.legend().is_empty());
    assert_eq!(prepared(Rule::y("m", 7.0).in_legend(true)).legend().len(), 1);
    assert!(rule.hover_points(&frame(), 7.0, [0.0, 30.0]).is_empty());
    let labels = rule.labels(&frame());
    assert_eq!((labels.len(), labels[0].anchor), (1, [1.0, 1.0]));
    assert!(prepared(Rule::y("m", 7.0).label(false)).labels(&frame()).is_empty());
    assert!(!rule.draw(&frame(), Shape::new()).is_empty());
}
