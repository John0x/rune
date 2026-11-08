//! The pure core of Rune's charts: everything a chart computes before it draws, with no window,
//! no theme and no allocation per frame beyond what the result needs.
//!
//! - [`scale`]: continuous scales ([`scale::Linear`], [`scale::Log`], [`scale::Time`]) that map
//!   data values to pixels and back, and [`scale::Band`] for categories.
//! - [`ticks`]: nice tick values (1, 2, 5 × 10ᵏ), number labels, and calendar-aligned time ticks
//!   with labels that show what changed (`06:00`, `Mon 8`, `Nov`, `2027`).
//! - [`data`]: columnar series ([`data::Series`], [`data::Ohlc`], [`data::Grid`]) behind `Arc`s, so
//!   a chart never copies its data per frame, with `NaN` as a gap.
//! - [`viewport`]: the visible x and y domains and the zoom, pan and fit operations on them.
//! - [`decimate`]: the visible index range of a sorted column, and min/max-per-pixel decimation.
//! - [`hit`]: what lies under the pointer: the nearest x, the nearest point, a band, a heat-map
//!   cell, a pie slice or a radar axis.
//! - [`layout`]: stacking, pie and donut arcs, radar polygons, sankey diagrams and smooth curves.
//!
//! Data values are `f64`; pixel positions are `f32`, and pixel points are `[f32; 2]` (`[x, y]`,
//! y growing downward), the same shape `rune::Shape` takes. Angles are radians measured clockwise
//! from twelve o'clock, as in d3.
//!
//! Every function is deterministic and total: empty data, `NaN`, zero spans and single points give
//! an empty or degenerate result, never a panic.

mod chart;
mod heatmap;
mod overlay;
mod pie;
mod radar;
mod sankey;
mod style;
mod widget;

pub mod data;
pub mod decimate;
pub mod hit;
pub mod layout;
pub mod scale;
pub mod ticks;
pub mod viewport;

pub use chart::Area;
pub use chart::Axis;
pub use chart::Bars;
pub use chart::Candles;
pub use chart::Chart;
pub use chart::Frame;
pub use chart::HoverPoint;
pub use chart::Line;
pub use chart::Plot;
pub use chart::PlotLabel;
pub use chart::Rule;
pub use chart::Scatter;
pub use chart::StackedArea;
pub use chart::TooltipRow;
pub use chart::hovered;
pub use chart::selection;
pub use heatmap::HeatmapChart;
pub use pie::PieChart;
pub use radar::RadarChart;
pub use sankey::SankeyChart;
pub use style::ChartStyle;
pub use viewport::Axes;
