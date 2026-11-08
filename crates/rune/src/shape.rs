//! Vector shapes painted inside an element: strokes (polylines) and fills (simple polygons), in
//! the element's own points with the origin at its top-left corner. For waveforms, meters,
//! sparklines, charts and other drawing that changes every frame.
//!
//! Shapes render in the uber-quad pass without the glyph atlas: a stroke is one capsule or box
//! per segment (`KIND_SEGMENT`), a fill is triangulated into triangles (`KIND_TRIANGLE`) whose
//! polygon-boundary edges are antialiased and whose interior edges are hard, so fills have no
//! seams. A fill is triangulated once, when it is added, and the triangles stay with the shape:
//! a shape kept across frames (`memo`, a stored `Shape`) costs no triangulation to draw again.
//! [`Shape::area`] triangulates the band between two polylines in linear time, for charts with
//! thousands of points. Translucent strokes blend twice where neighbouring segments overlap at a
//! joint; draw translucent areas as fills. An element's opacity doesn't: while it is between 0
//! and 1, a shape of more than one piece paints through a layer that takes it once.

use std::rc::Rc;

use crate::color::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineCap {
    /// Segments end at their points.
    Butt,
    /// Half-discs past each end; also rounds every joint of a polyline.
    #[default]
    Round,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Op {
    Stroke {
        points: Vec<[f32; 2]>,
        closed: bool,
        width: f32,
        cap: LineCap,
        color: Color,
    },
    Fill {
        points: Vec<[f32; 2]>,
        /// The colour, or the gradient's start colour.
        color: Color,
        gradient: Option<Gradient>,
        mesh: Mesh,
    },
}

/// A fill's triangles: vertex indices into its points and a mask of the edges on its outline
/// (a→b, b→c, c→a as bits 0, 1, 2). Made from the points, so two fills with equal points have
/// equal meshes and comparing them compares nothing.
#[derive(Clone, Debug, Default)]
pub(crate) struct Mesh(pub(crate) Rc<[([usize; 3], u32)]>);

impl PartialEq for Mesh {
    fn eq(&self, _: &Mesh) -> bool {
        true
    }
}

/// Which way a [`Gradient`] runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientAxis {
    /// Down the shape: `from` and `to` are y coordinates.
    Vertical,
    /// Across the shape: `from` and `to` are x coordinates.
    Horizontal,
}

/// A linear gradient for a fill: `start` at `from` to `end` at `to` along `axis`, in the shape's
/// coordinates, each colour held beyond its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    pub axis: GradientAxis,
    pub from: f32,
    pub to: f32,
    pub start: Color,
    pub end: Color,
}

impl Gradient {
    /// `top` at y `from` to `bottom` at y `to`: an area under a line fading toward its baseline.
    pub fn vertical(from: f32, top: Color, to: f32, bottom: Color) -> Gradient {
        Gradient { axis: GradientAxis::Vertical, from, to, start: top, end: bottom }
    }

    /// `left` at x `from` to `right` at x `to`.
    pub fn horizontal(from: f32, left: Color, to: f32, right: Color) -> Gradient {
        Gradient { axis: GradientAxis::Horizontal, from, to, start: left, end: right }
    }

    fn faded(self, a: f32) -> Gradient {
        Gradient { start: Color { a: self.start.a * a, ..self.start }, end: Color { a: self.end.a * a, ..self.end }, ..self }
    }
}

/// A list of strokes and fills, painted in order over the element's fill and under its text.
///
/// ```
/// use rune::{Color, Shape, canvas};
/// let wave: Vec<[f32; 2]> = (0..64).map(|i| [i as f32 * 4.0, 20.0 + (i as f32 * 0.3).sin() * 16.0]).collect();
/// let meter = Shape::new()
///     .fill(vec![[0.0, 40.0], [120.0, 40.0], [120.0, 36.0], [0.0, 36.0]], Color::WHITE)
///     .stroke(wave, 1.5, Color::WHITE);
/// let _el = canvas(meter).size(256.0, 40.0);
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shape {
    pub(crate) ops: Vec<Op>,
    /// Coordinates span this box and stretch to the element's size (SVG `viewBox` with
    /// `preserveAspectRatio="none"`); stroke widths stay in points.
    pub(crate) view_box: Option<[f32; 2]>,
}

impl Shape {
    pub fn new() -> Shape {
        Shape::default()
    }

    /// An open polyline through `points`, `width` points wide, with round caps and joints.
    pub fn stroke(self, points: impl Into<Vec<[f32; 2]>>, width: f32, color: Color) -> Shape {
        self.stroke_with(points, false, width, LineCap::Round, color)
    }

    /// A polyline; `closed` joins the last point back to the first.
    pub fn stroke_with(
        mut self,
        points: impl Into<Vec<[f32; 2]>>,
        closed: bool,
        width: f32,
        cap: LineCap,
        color: Color,
    ) -> Shape {
        let points = points.into();
        if points.len() >= 2 && width > 0.0 {
            self.ops.push(Op::Stroke {
                points,
                closed,
                width,
                cap,
                color,
            });
        }
        self
    }

    /// A straight line from `a` to `b`.
    pub fn line(self, a: [f32; 2], b: [f32; 2], width: f32, cap: LineCap, color: Color) -> Shape {
        self.stroke_with(vec![a, b], false, width, cap, color)
    }

    /// The inside of the simple polygon through `points` (either winding; not self-intersecting).
    pub fn fill(self, points: impl Into<Vec<[f32; 2]>>, color: Color) -> Shape {
        self.push_fill(points.into(), color, None)
    }

    /// [`Self::fill`] with a linear gradient.
    pub fn fill_gradient(self, points: impl Into<Vec<[f32; 2]>>, gradient: Gradient) -> Shape {
        self.push_fill(points.into(), gradient.start, Some(gradient))
    }

    fn push_fill(mut self, points: Vec<[f32; 2]>, color: Color, gradient: Option<Gradient>) -> Shape {
        if points.len() >= 3 {
            let mesh = Mesh(triangulate(&points).into());
            self.ops.push(Op::Fill { points, color, gradient, mesh });
        }
        self
    }

    /// The band between two polylines that run left to right, `upper` above `lower` (an area
    /// under a chart's line, down to a baseline or to another line), as one fill triangulated in
    /// linear time. Each line is sampled at the other's points too (held level past its ends),
    /// so the fill's points are those of both lines. The band's outline is antialiased; where
    /// the lines cross, the band folds over itself, so draw crossing lines' bands separately.
    pub fn area(self, upper: impl Into<Vec<[f32; 2]>>, lower: impl Into<Vec<[f32; 2]>>, color: Color) -> Shape {
        self.push_area(upper.into(), lower.into(), color, None)
    }

    /// [`Self::area`] with a linear gradient.
    pub fn area_gradient(self, upper: impl Into<Vec<[f32; 2]>>, lower: impl Into<Vec<[f32; 2]>>, gradient: Gradient) -> Shape {
        self.push_area(upper.into(), lower.into(), gradient.start, Some(gradient))
    }

    fn push_area(mut self, upper: Vec<[f32; 2]>, lower: Vec<[f32; 2]>, color: Color, gradient: Option<Gradient>) -> Shape {
        if upper.len() < 2 || lower.len() < 2 {
            let mut points = upper;
            points.extend(lower.into_iter().rev());
            return self.push_fill(points, color, gradient);
        }
        let (points, tris) = band(&upper, &lower);
        self.ops.push(Op::Fill { points, color, gradient, mesh: Mesh(tris.into()) });
        self
    }

    /// A dashed polyline: `dash` units of line, then `gap` units of none, along its length.
    pub fn stroke_dashed(mut self, points: impl Into<Vec<[f32; 2]>>, width: f32, dash: f32, gap: f32, cap: LineCap, color: Color) -> Shape {
        let points = points.into();
        if dash <= 0.0 || gap <= 0.0 {
            return self.stroke_with(points, false, width, cap, color);
        }
        let length: f32 = points.windows(2).map(|w| dist(w[0], w[1])).sum();
        let mut at = 0.0;
        while at < length {
            self = self.stroke_with(slice(&points, at, (at + dash).min(length)), false, width, cap, color);
            at += dash + gap;
        }
        self
    }

    /// A round dot of `radius` points at `center` (a chart's marker), round however the shape is
    /// stretched.
    pub fn dot(self, center: [f32; 2], radius: f32, color: Color) -> Shape {
        self.stroke_with(vec![center, center], false, radius * 2.0, LineCap::Round, color)
    }

    /// Strokes every subpath of `path`, closed ones back to their start.
    pub fn stroke_path(mut self, path: &Path, width: f32, cap: LineCap, color: Color) -> Shape {
        for sub in &path.subpaths {
            self = self.stroke_with(sub.points.clone(), sub.closed, width, cap, color);
        }
        self
    }

    /// Fills every subpath of `path` on its own (no holes between subpaths).
    pub fn fill_path(mut self, path: &Path, color: Color) -> Shape {
        for sub in &path.subpaths {
            self = self.fill(sub.points.clone(), color);
        }
        self
    }

    /// Treats coordinates as spanning `width` × `height` and stretches them to whatever size the
    /// element lays out at, so a shape can fill a flexible element (a waveform in a growing row).
    /// Stroke widths stay in points.
    pub fn view_box(mut self, width: f32, height: f32) -> Shape {
        self.view_box = (width > 0.0 && height > 0.0).then_some([width, height]);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Whether it draws more than one piece, so two can overlap: a stroke of two or more
    /// segments (they meet at a joint), or more than one stroke or fill. Faded, overlapping
    /// pieces blend twice where they meet, so a fade paints such a shape through a layer.
    pub(crate) fn overlaps(&self) -> bool {
        let pieces = |op: &Op| match op {
            Op::Stroke { points, closed, .. } => if *closed { points.len() } else { points.len() - 1 },
            Op::Fill { .. } => 1,
        };
        self.ops.iter().map(pieces).sum::<usize>() > 1
    }

    /// This shape drawn on to `t` (0 to 1), as SVG's `pathLength` trim draws a line: its strokes
    /// trace in order along their combined length, the one in progress cut where the drawing has
    /// got to, and each fill fades in over the last [`DRAW_RAMP`] points of line before it (an
    /// arrowhead lands as its line arrives). With no strokes, the fills fade in together.
    /// [`Transition::draw`](crate::Transition::draw) animates it.
    pub fn drawn(&self, t: f32) -> Shape {
        self.trimmed(0.0, if t.is_nan() { 1.0 } else { t })
    }

    /// The part of this shape from `start` to `end` (0 to 1) of its strokes' combined length, as
    /// Core Animation's `strokeStart` and `strokeEnd` trim a path: each stroke keeps the stretch
    /// of it inside that span, with its own caps at both cut ends, and each fill fades in as `end`
    /// reaches it, as under [`Self::drawn`]. Nothing of the strokes draws when `end` is at or
    /// before `start`. [`El::live_trim`](crate::El::live_trim) moves it every frame.
    pub fn trimmed(&self, start: f32, end: f32) -> Shape {
        let unit = |v: f32, nan: f32| if v.is_nan() { nan } else { v.clamp(0.0, 1.0) };
        let (start, t) = (unit(start, 0.0), unit(end, 1.0));
        if start <= 0.0 && t >= 1.0 {
            return self.clone();
        }
        let len = |points: &[[f32; 2]], closed: bool| {
            let open: f32 = points.windows(2).map(|w| dist(w[0], w[1])).sum();
            open + if closed && points.len() > 2 { dist(points[points.len() - 1], points[0]) } else { 0.0 }
        };
        let total: f32 = self.ops.iter().map(|op| if let Op::Stroke { points, closed, .. } = op { len(points, *closed) } else { 0.0 }).sum();
        let (from, at) = (start * total, t * total);
        let mut done = 0.0;
        let mut ops = Vec::new();
        for op in &self.ops {
            match op {
                Op::Stroke { points, closed, width, cap, color } => {
                    let l = len(points, *closed);
                    let (lo, hi) = (from.max(done), at.min(done + l));
                    if lo <= done && hi >= done + l {
                        ops.push(op.clone());
                    } else if hi > lo {
                        let mut ring = points.clone();
                        if *closed {
                            ring.push(points[0]);
                        }
                        let cut = slice(&ring, lo - done, hi - done);
                        if cut.len() >= 2 {
                            ops.push(Op::Stroke { points: cut, closed: false, width: *width, cap: *cap, color: *color });
                        }
                    }
                    done += l;
                }
                Op::Fill { color, gradient, .. } => {
                    let a = if total <= 0.0 {
                        t
                    } else if done <= 0.0 {
                        1.0
                    } else {
                        let ramp = DRAW_RAMP.min(done);
                        ((at - (done - ramp)) / ramp).clamp(0.0, 1.0)
                    };
                    if a > 0.0 {
                        let mut faded = op.clone();
                        if let Op::Fill { color: c, gradient: g, .. } = &mut faded {
                            *c = Color { a: color.a * a, ..*color };
                            *g = gradient.map(|g| g.faded(a));
                        }
                        ops.push(faded);
                    }
                }
            }
        }
        Shape { ops, view_box: self.view_box }
    }
}

/// How many points of line a fill fades in over as [`Shape::drawn`] reaches it.
pub const DRAW_RAMP: f32 = 8.0;

/// The polyline `points` from `from` to `to` along its length (`from` < `to`).
fn slice(points: &[[f32; 2]], from: f32, to: f32) -> Vec<[f32; 2]> {
    let lerp = |a: [f32; 2], b: [f32; 2], at: f32, d: f32| {
        let k = if d > 0.0 { (at / d).clamp(0.0, 1.0) } else { 0.0 };
        [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k]
    };
    let mut out = Vec::new();
    let mut done = 0.0;
    for w in points.windows(2) {
        let d = dist(w[0], w[1]);
        if out.is_empty() && from < done + d {
            out.push(lerp(w[0], w[1], from - done, d));
        }
        if !out.is_empty() {
            if to <= done + d {
                out.push(lerp(w[0], w[1], to - done, d));
                return out;
            }
            out.push(w[1]);
        }
        done += d;
    }
    out
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Subpath {
    points: Vec<[f32; 2]>,
    closed: bool,
}

/// A path built from lines and Bézier curves, flattened to polylines for [`Shape`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    subpaths: Vec<Subpath>,
}

/// Flattening tolerance, in points: curves stay within this of their true position.
const TOLERANCE: f32 = 0.1;

impl Path {
    pub fn new() -> Path {
        Path::default()
    }

    fn last(&self) -> [f32; 2] {
        self.subpaths
            .last()
            .and_then(|s| s.points.last().copied())
            .unwrap_or([0.0, 0.0])
    }

    fn current(&mut self) -> &mut Subpath {
        if self.subpaths.last().is_none_or(|s| s.closed) {
            let start = self.last();
            self.subpaths.push(Subpath {
                points: vec![start],
                closed: false,
            });
        }
        self.subpaths.last_mut().expect("pushed above")
    }

    pub fn move_to(mut self, p: [f32; 2]) -> Path {
        match self.subpaths.last_mut() {
            Some(s) if s.points.len() == 1 && !s.closed => s.points[0] = p,
            _ => self.subpaths.push(Subpath {
                points: vec![p],
                closed: false,
            }),
        }
        self
    }

    pub fn line_to(mut self, p: [f32; 2]) -> Path {
        self.current().points.push(p);
        self
    }

    pub fn quad_to(mut self, c: [f32; 2], p: [f32; 2]) -> Path {
        let a = self.last();
        let n = segments(dist(a, c) + dist(c, p));
        let points = &mut self.current().points;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            points.push([
                u * u * a[0] + 2.0 * u * t * c[0] + t * t * p[0],
                u * u * a[1] + 2.0 * u * t * c[1] + t * t * p[1],
            ]);
        }
        self
    }

    pub fn cubic_to(mut self, c1: [f32; 2], c2: [f32; 2], p: [f32; 2]) -> Path {
        let a = self.last();
        let n = segments(dist(a, c1) + dist(c1, c2) + dist(c2, p));
        let points = &mut self.current().points;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            points.push([
                b0 * a[0] + b1 * c1[0] + b2 * c2[0] + b3 * p[0],
                b0 * a[1] + b1 * c1[1] + b2 * c2[1] + b3 * p[1],
            ]);
        }
        self
    }

    pub fn close(mut self) -> Path {
        if let Some(s) = self.subpaths.last_mut() {
            s.closed = true;
        }
        self
    }
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

/// Segments for a curve whose control polygon is `length` long.
fn segments(length: f32) -> usize {
    ((length / TOLERANCE).sqrt() * 0.5).ceil().clamp(1.0, 256.0) as usize
}

/// Ear-clips a simple polygon. Each triangle is three vertex indices and a mask of which of its
/// edges (a→b, b→c, c→a as bits 0, 1, 2) lie on the polygon's boundary.
pub(crate) fn triangulate(points: &[[f32; 2]]) -> Vec<([usize; 3], u32)> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f32 = (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let ccw = area > 0.0;
    let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        let v = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if ccw { v } else { -v }
    };
    let boundary = |i: usize, j: usize| (i + 1) % n == j || (j + 1) % n == i;
    let mut ring: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity(n - 2);
    let mut misses = 0;
    let mut i = 0;
    while ring.len() > 3 {
        let m = ring.len();
        let (ia, ib, ic) = (ring[(i + m - 1) % m], ring[i % m], ring[(i + 1) % m]);
        let (a, b, c) = (points[ia], points[ib], points[ic]);
        let convex = cross(a, b, c) > 0.0;
        let empty = convex
            && ring.iter().all(|&k| {
                k == ia || k == ib || k == ic || {
                    let p = points[k];
                    !(cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0)
                }
            });
        // Degenerate (collinear) vertices are dropped once no ear is left.
        if empty || (misses >= m && cross(a, b, c) == 0.0) {
            if empty {
                out.push(([ia, ib, ic], mask(boundary, ia, ib, ic)));
            }
            ring.remove(i % m);
            misses = 0;
            i %= ring.len();
        } else {
            misses += 1;
            if misses > 2 * m {
                // Not simple: fall back to a fan over what is left.
                for k in 1..ring.len() - 1 {
                    let (x, y, z) = (ring[0], ring[k], ring[k + 1]);
                    out.push(([x, y, z], mask(boundary, x, y, z)));
                }
                return out;
            }
            i = (i + 1) % m;
        }
    }
    let (a, b, c) = (ring[0], ring[1], ring[2]);
    out.push(([a, b, c], mask(boundary, a, b, c)));
    out
}

/// The band between `upper` and `lower` (both left to right, two points or more) as a ring and
/// its triangles. Each line is sampled at every point of both, so the band is a run of
/// vertical-sided strips, two triangles each, which are convex wherever `upper` is above
/// `lower`: linear in the points, however wavy the lines or sparse the baseline. The ring is the
/// sampled `upper` then the sampled `lower` reversed; the lines and the band's two ends are its
/// outline.
pub(crate) fn band(upper: &[[f32; 2]], lower: &[[f32; 2]]) -> (Vec<[f32; 2]>, Vec<([usize; 3], u32)>) {
    // `line`'s height at `x`, from its segment ending at `next`, the first point not yet taken
    // (held level beyond its ends).
    let at = |line: &[[f32; 2]], next: usize, x: f32| -> f32 {
        if next == 0 {
            return line[0][1];
        }
        let Some(&b) = line.get(next) else { return line[line.len() - 1][1] };
        let a = line[next - 1];
        let dx = b[0] - a[0];
        if dx.abs() < 1e-6 { a[1] } else { a[1] + (b[1] - a[1]) * ((x - a[0]) / dx).clamp(0.0, 1.0) }
    };
    let (mut top, mut bottom) = (Vec::with_capacity(upper.len() + lower.len()), Vec::with_capacity(upper.len() + lower.len()));
    let (mut i, mut j) = (0, 0);
    while i < upper.len() || j < lower.len() {
        let (u, l) = (upper.get(i), lower.get(j));
        match (u, l) {
            (Some(u), Some(l)) if u[0] == l[0] => {
                (top.push(*u), bottom.push(*l));
                (i, j) = (i + 1, j + 1);
            }
            (Some(u), l) if l.is_none_or(|l| u[0] < l[0]) => {
                (top.push(*u), bottom.push([u[0], at(lower, j, u[0])]));
                i += 1;
            }
            (_, Some(l)) => {
                (top.push([l[0], at(upper, i, l[0])]), bottom.push(*l));
                j += 1;
            }
            _ => unreachable!("one line has a point left"),
        }
    }
    let k = top.len();
    let ring = 2 * k;
    let low = |s: usize| k + (k - 1 - s);
    let boundary = |a: usize, b: usize| (a + 1) % ring == b || (b + 1) % ring == a;
    let mut tris = Vec::with_capacity(ring - 2);
    for t in 0..k - 1 {
        for [a, b, c] in [[t, t + 1, low(t + 1)], [t, low(t + 1), low(t)]] {
            tris.push(([a, b, c], mask(boundary, a, b, c)));
        }
    }
    top.extend(bottom.into_iter().rev());
    (top, tris)
}

fn mask(boundary: impl Fn(usize, usize) -> bool, a: usize, b: usize, c: usize) -> u32 {
    (boundary(a, b) as u32) | ((boundary(b, c) as u32) << 1) | ((boundary(c, a) as u32) << 2)
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
