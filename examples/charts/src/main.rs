//! The charts gallery: every rune-chart kind with its interactions. `cargo run -p charts`;
//! `cargo run -p charts -- --snap charts.png` renders it headless.

mod cartesian;
mod data;
mod other;

use rune::{AppSpec, Color, Cx, El, col, text};
use rune_chart::{Axis, Chart, ChartStyle, Line};

struct State;

fn bg() -> Color {
    rune::oklch(0.17, 0.005, 260.0)
}

pub(crate) fn section(title: &str, chart: El) -> El {
    col()
        .key(title.to_owned())
        .gap(10.0)
        .padding(16.0)
        .radius(10.0)
        .fill(rune::oklch(0.22, 0.005, 260.0))
        .child(
            text(title.to_owned())
                .font(rune::TextStyle::new(13.0, 600.0))
                .color(Color::WHITE),
        )
        .child(chart)
}

fn view(_: &State, _: &mut Cx<State>) -> El {
    let tz = jiff::tz::TimeZone::get("Europe/Berlin").unwrap_or(jiff::tz::TimeZone::UTC);
    let prices = Chart::new("prices")
        .x(Axis::time_in(tz))
        .y(Axis::linear().format(|v| format!("{v:.2}g")))
        .style(ChartStyle::dark())
        .plot(Line::new("Median", data::walk(400, 7, 20.0)))
        .plot(Line::new("Cheapest", data::walk(400, 11, 17.0)))
        .plot(Line::new("7-day median", data::walk(400, 3, 19.0)).dashed(5.0, 4.0))
        .build();
    col()
        .key("gallery")
        .w_full()
        .h_full()
        .padding(20.0)
        .gap(16.0)
        .fill(bg())
        .scroll(rune::ScrollSpec {
            stick_bottom: false,
        })
        .child(section("Line: time series", prices))
        .children(cartesian::sections())
        .children(other::sections())
}

fn main() {
    rune::run(AppSpec::new("Charts", (980.0, 720.0), || State, view));
}
