//! Geometry for chart kinds beyond plain lines and bars: stacking, pie and donut arcs, radar
//! polygons, sankey diagrams, and smooth curves.
//!
//! Angles are radians clockwise from twelve o'clock (as d3), and pixel points are `[x, y]` with
//! y growing downward.

use std::f64::consts::PI;

mod arc;
mod curve;
mod pie;
mod radar;
mod sankey;
mod stack;

pub use arc::LabelSide;
pub use arc::LeaderLine;
pub use arc::arc_centroid;
pub use arc::arc_polygon;
pub use arc::leader_line;
pub use curve::Cubic;
pub use curve::Curve;
pub use curve::StepPosition;
pub use curve::flatten;
pub use curve::monotone_x;
pub use curve::split_gaps;
pub use curve::step;
pub use pie::PieOptions;
pub use pie::PieSlice;
pub use pie::PieSort;
pub use pie::pie;
pub use radar::radar_angles;
pub use radar::radar_points;
pub use radar::radar_ring;
pub use sankey::SankeyAlign;
pub use sankey::SankeyLayout;
pub use sankey::SankeyLink;
pub use sankey::SankeyLinkLayout;
pub use sankey::SankeyNode;
pub use sankey::SankeyOptions;
pub use sankey::sankey;
pub use stack::StackOffset;
pub use stack::StackOrder;
pub use stack::stack;

/// The point `radius` pixels from `center` at `angle` (clockwise from twelve o'clock).
pub fn polar(center: [f32; 2], angle: f64, radius: f32) -> [f32; 2] {
    let r = f64::from(radius);
    [(f64::from(center[0]) + angle.sin() * r) as f32, (f64::from(center[1]) - angle.cos() * r) as f32]
}

/// The angle of `point` seen from `center`, clockwise from twelve o'clock, in `(-π, π]`; 0 at
/// the center itself.
pub fn angle_at(center: [f32; 2], point: [f32; 2]) -> f64 {
    let (dx, dy) = (f64::from(point[0] - center[0]), f64::from(point[1] - center[1]));
    if dx == 0.0 && dy == 0.0 {
        return 0.0;
    }
    let a = dx.atan2(-dy);
    if a == -PI { PI } else { a }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
