//! Points at (x, y) pairs, each a dot, optionally sized and coloured by its values.

use std::collections::HashSet;
use std::rc::Rc;

use rune::{Color, Shape};

use super::plot::{Frame, HoverPoint, Plot, TooltipRow};
use crate::data::Series;
use crate::decimate::visible_range;
use crate::hit::nearest_point;

/// How near the pointer must come to a point, in points, to hover it.
const REACH: f32 = 12.0;

/// A dot at each of a set of (x, y) points, in any order. Hovering shows the point nearest the
/// pointer (within 12 points); the crosshair doesn't snap to them.
///
/// ```
/// use rune_chart::Scatter;
/// let trades = Scatter::new("Trades", [(1.0, 20.0), (0.5, 18.0), (2.0, 22.5)]).radius(4.0);
/// ```
#[derive(Clone)]
pub struct Scatter {
    name: String,
    /// The points, sorted by x so the visible ones are a range.
    points: Series,
    color: Option<Color>,
    radius: f32,
    size_by: Option<Rc<dyn Fn(f64, f64) -> f32>>,
    color_by: Option<Rc<dyn Fn(f64, f64) -> Color>>,
}

impl Scatter {
    /// Dots at `points`, `(x, y)` each, in any order; a point with a `NaN` or infinite value is
    /// left out.
    pub fn new(name: impl Into<String>, points: impl IntoIterator<Item = (f64, f64)>) -> Scatter {
        let points = Series::from_unsorted(points.into_iter().filter(|p| p.1.is_finite()));
        Scatter { name: name.into(), points, color: None, radius: 3.0, size_by: None, color_by: None }
    }

    pub fn color(mut self, color: Color) -> Scatter {
        self.color = Some(color);
        self
    }

    /// The dots' radius in points (3 by default).
    pub fn radius(mut self, radius: f32) -> Scatter {
        self.radius = radius.max(0.0);
        self
    }

    /// Each dot's radius in points from its x and y, instead of one radius for all.
    pub fn size_by(mut self, f: impl Fn(f64, f64) -> f32 + 'static) -> Scatter {
        self.size_by = Some(Rc::new(f));
        self
    }

    /// Each dot's colour from its x and y; the legend keeps the plot's colour.
    pub fn color_by(mut self, f: impl Fn(f64, f64) -> Color + 'static) -> Scatter {
        self.color_by = Some(Rc::new(f));
        self
    }

    /// The points, sorted by x.
    pub fn points(&self) -> &Series {
        &self.points
    }

    fn radius_at(&self, x: f64, y: f64) -> f32 {
        self.size_by.as_ref().map_or(self.radius, |f| f(x, y)).max(0.0)
    }

    fn color_at(&self, x: f64, y: f64) -> Color {
        self.color_by.as_ref().map_or_else(|| self.color(), |f| f(x, y))
    }
}

impl Plot for Scatter {
    fn name(&self) -> &str {
        &self.name
    }

    fn color(&self) -> Color {
        self.color.unwrap_or(Color::WHITE)
    }

    fn set_default_color(&mut self, color: Color) {
        self.color.get_or_insert(color);
    }

    /// The points' x extent with 3% to spare on each side, so the outermost dots aren't cut.
    fn x_extent(&self) -> Option<(f64, f64)> {
        let (a, b) = self.points.x_extent()?;
        let pad = (b - a) * 0.03;
        Some((a - pad, b + pad))
    }

    fn y_extent(&self, x: (f64, f64)) -> Option<(f64, f64)> {
        self.points.y_extent_in(visible_range(self.points.x(), x))
    }

    fn draw(&self, frame: &Frame, mut shape: Shape) -> Shape {
        let (x, y) = (self.points.x(), self.points.y());
        let range = visible_range(x, frame.viewport.x);
        // With more points than pixels across, one dot per pixel is all that shows.
        let mut taken: Option<HashSet<(i32, i32)>> = (range.len() > frame.columns() * 4).then(HashSet::new);
        for i in range {
            let p = frame.px(x[i], y[i]);
            if let Some(taken) = &mut taken
                && !taken.insert((p[0] as i32, p[1] as i32))
            {
                continue;
            }
            shape = shape.dot(p, self.radius_at(x[i], y[i]), self.color_at(x[i], y[i]));
        }
        shape
    }

    fn hover(&self, frame: &Frame, _: f64, pointer: [f32; 2]) -> Option<HoverPoint> {
        let (x, y) = (self.points.x(), self.points.y());
        // Only points within reach of the pointer can be nearest.
        let reach = |px: f32| frame.x.invert(px);
        let range = visible_range(x, (reach(pointer[0] - REACH), reach(pointer[0] + REACH)));
        let i = range.start + nearest_point(&x[range.clone()], &y[range], |a, b| frame.px(a, b), pointer, REACH)?;
        let (px, py) = (x[i], y[i]);
        // The tooltip's title reads the point's x.
        Some(HoverPoint {
            x: px,
            dot: Some(frame.px(px, py)),
            row: TooltipRow { color: self.color_at(px, py), label: self.name.clone(), value: (frame.format_y)(py) },
        })
    }

    fn follows_pointer(&self) -> bool {
        true
    }
}
