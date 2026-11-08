//! [`Chart`]: series on an x and a y axis (lines, areas, bars, candles, points, rules) with grid
//! lines, tick labels, a legend, a crosshair and tooltip, and zoom and pan.
//!
//! Interaction, with the pointer over the plot area:
//! - hover: a crosshair snapped to the nearest data x, a dot on each series and a tooltip;
//! - pinch, or ⌘/Ctrl + wheel (any wheel with [`Chart::wheel_zoom`]), zooms at the pointer;
//!   a sideways swipe pans;
//! - drag pans; shift-drag zooms to the box drawn; option-drag selects an x range
//!   ([`Chart::selectable`], read with [`selection`]); a double click resets.
//!
//! A chart keeps its zoom, hover and hidden series itself ([`rune::component_state`]), under its
//! key, so the app stores nothing. Charts given one [`Chart::link`] share their x zoom and
//! crosshair (a price chart over its volume).

mod area;
mod axis;
mod bars;
mod candles;
mod geometry;
mod line;
mod memory;
mod plot;
mod rule;
mod scatter;
mod stacked;
mod view;

use std::rc::Rc;

use rune::El;

pub use area::Area;
pub use axis::Axis;
pub use bars::Bars;
pub use candles::Candles;
pub use line::Line;
pub use plot::Frame;
pub use plot::HoverPoint;
pub use plot::Plot;
pub use plot::PlotLabel;
pub use plot::TooltipRow;
pub use rule::Rule;
pub use scatter::Scatter;
pub use stacked::StackedArea;

use crate::style::ChartStyle;
use crate::viewport::Axes;
use memory::XMemory;

/// A chart of [`Plot`]s on an x and a y [`Axis`].
pub struct Chart {
    key: String,
    x: Axis,
    y: Axis,
    plots: Vec<Box<dyn Plot>>,
    style: ChartStyle,
    height: f32,
    legend: bool,
    link: Option<String>,
    axes: Axes,
    wheel_zoom: bool,
    select: bool,
    x_domain: Option<(f64, f64)>,
    title: Option<Rc<dyn Fn(f64) -> String>>,
    gutter: Option<f32>,
}

impl Chart {
    /// A chart keyed `key`: its state lives under it, so give each chart on a page its own.
    pub fn new(key: impl Into<String>) -> Chart {
        Chart {
            key: key.into(),
            x: Axis::linear(),
            y: Axis::linear(),
            plots: Vec::new(),
            style: ChartStyle::default(),
            height: 220.0,
            legend: true,
            link: None,
            axes: Axes::X,
            wheel_zoom: false,
            select: false,
            x_domain: None,
            title: None,
            gutter: None,
        }
    }

    pub fn x(mut self, axis: Axis) -> Chart {
        self.x = axis;
        self
    }

    pub fn y(mut self, axis: Axis) -> Chart {
        self.y = axis;
        self
    }

    /// Adds a plot: a [`Line`], an [`Area`], a [`StackedArea`], [`Bars`], [`Candles`], a
    /// [`Scatter`] or a [`Rule`].
    pub fn plot(mut self, plot: impl Plot + 'static) -> Chart {
        self.plots.push(Box::new(plot));
        self
    }

    pub fn style(mut self, style: ChartStyle) -> Chart {
        self.style = style;
        self
    }

    /// The plot area's height in points (220 by default); labels and legend add to it.
    pub fn height(mut self, height: f32) -> Chart {
        self.height = height;
        self
    }

    /// Whether to show a legend (with more than one named series; on by default).
    pub fn legend(mut self, legend: bool) -> Chart {
        self.legend = legend;
        self
    }

    /// Shares x zoom, pan and crosshair with every chart linked to `group`.
    pub fn link(mut self, group: impl Into<String>) -> Chart {
        self.link = Some(group.into());
        self
    }

    /// Which axes zoom and pan move: x (default, y fits what's in view), y or both.
    pub fn zoom_axes(mut self, axes: Axes) -> Chart {
        self.axes = axes;
        self
    }

    /// Lets a plain wheel zoom (for a chart that fills a window); otherwise it scrolls the page
    /// and ⌘ or Ctrl + wheel zooms.
    pub fn wheel_zoom(mut self, on: bool) -> Chart {
        self.wheel_zoom = on;
        self
    }

    /// Lets option-drag select an x range, read with [`selection`].
    pub fn selectable(mut self, on: bool) -> Chart {
        self.select = on;
        self
    }

    /// Shows this x domain whenever it differs from the last one given (range buttons: 24 h,
    /// 7 d, all); zoom and pan move on from it. `None` follows the data.
    pub fn x_domain(mut self, domain: Option<(f64, f64)>) -> Chart {
        self.x_domain = domain;
        self
    }

    /// How the tooltip's title reads the hovered x (the x axis' format by default).
    pub fn tooltip_title(mut self, f: impl Fn(f64) -> String + 'static) -> Chart {
        self.title = Some(Rc::new(f));
        self
    }

    /// The y labels' column width in points, instead of the widest label's: give charts linked
    /// one above the other one width so their plot areas line up.
    pub fn gutter(mut self, width: f32) -> Chart {
        self.gutter = Some(width.max(0.0));
        self
    }

    pub fn build(self) -> El {
        view::build(self)
    }
}

fn x_state(key_or_link: &str) -> Rc<XMemory> {
    rune::component_state::<XMemory>(&format!("rune-chart-x/{key_or_link}"))
}

/// The x range option-dragged on the chart keyed (or linked as) `key`, read while building a
/// view.
pub fn selection(key: &str) -> Option<(f64, f64)> {
    x_state(key).selection.get()
}

/// The data x under the crosshair of the chart keyed (or linked as) `key`, read while building
/// a view.
pub fn hovered(key: &str) -> Option<f64> {
    x_state(key).hover.get()
}

#[cfg(test)]
#[path = "chart/plots_tests.rs"]
mod plots_tests;
