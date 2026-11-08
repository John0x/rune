//! Sankey diagrams, after d3-sankey: nodes in columns by their place in the flow, ordered
//! within a column so links cross little, and links as bands as wide as their value.

use super::curve::Cubic;

/// A flow of `value` from node `source` to node `target` (indices into the nodes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SankeyLink {
    /// The node the flow leaves.
    pub source: usize,
    /// The node the flow enters.
    pub target: usize,
    /// The amount; values that aren't finite and positive count as 0.
    pub value: f64,
}

impl SankeyLink {
    /// A link of `value` from `source` to `target`.
    pub fn new(source: usize, target: usize, value: f64) -> SankeyLink {
        SankeyLink { source, target, value }
    }
}

/// Which column a node goes in, as d3-sankey's alignments.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SankeyAlign {
    /// By the longest path from a source: sources at the left.
    Left,
    /// By the longest path to a sink: sinks at the right.
    Right,
    /// Like `Left`, with each source moved next to its nearest target.
    Center,
    /// Like `Left`, with every sink in the last column (the default).
    #[default]
    Justify,
}

/// How [`sankey`] lays out a diagram.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SankeyOptions {
    /// The top-left corner of the area the diagram fills.
    pub origin: [f32; 2],
    /// The width and height of that area.
    pub size: [f32; 2],
    /// The width of a node.
    pub node_width: f32,
    /// The vertical space between nodes in a column (less when a column can't fit it).
    pub node_padding: f32,
    /// Which column each node goes in.
    pub align: SankeyAlign,
    /// Rounds of relaxation that move nodes toward the nodes they link to; more give fewer
    /// crossings and cost more.
    pub iterations: usize,
}

impl SankeyOptions {
    /// A diagram filling `size` from the origin, with d3-sankey's defaults: 24 px nodes, 8 px
    /// apart, justified, six iterations.
    pub fn new(size: [f32; 2]) -> SankeyOptions {
        SankeyOptions {
            origin: [0.0, 0.0],
            size,
            node_width: 24.0,
            node_padding: 8.0,
            align: SankeyAlign::Justify,
            iterations: 6,
        }
    }
}

/// A placed node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SankeyNode {
    /// The left edge.
    pub x0: f32,
    /// The right edge.
    pub x1: f32,
    /// The top edge.
    pub y0: f32,
    /// The bottom edge.
    pub y1: f32,
    /// The flow through the node: the larger of what comes in and what goes out.
    pub value: f64,
    /// The longest path from a source, in links.
    pub depth: usize,
    /// The longest path to a sink, in links.
    pub height: usize,
    /// The column the node is in, from 0 at the left.
    pub layer: usize,
}

/// A placed link: a band `width` pixels thick from its source's right edge to its target's
/// left edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SankeyLinkLayout {
    /// The source node's index.
    pub source: usize,
    /// The target node's index.
    pub target: usize,
    /// The value, 0 for a link that was left out.
    pub value: f64,
    /// The band's thickness.
    pub width: f32,
    /// The x where the band leaves its source.
    pub x0: f32,
    /// The x where the band enters its target.
    pub x1: f32,
    /// The band's middle where it leaves its source.
    pub y0: f32,
    /// The band's middle where it enters its target.
    pub y1: f32,
}

impl SankeyLinkLayout {
    /// The band's center line: a cubic leaving and entering horizontally, as d3's
    /// `sankeyLinkHorizontal`. Stroke it `width` wide, or fill [`band`](Self::band).
    pub fn path(&self) -> Cubic {
        self.offset(0.0)
    }

    /// The band as a polygon to fill: its top edge, then its bottom edge backward, flattened to
    /// `tolerance` pixels. Thick bands keep their vertical thickness along the curve, as a
    /// stroked d3 link does at its ends. Empty for a link of zero width.
    pub fn band(&self, tolerance: f32) -> Vec<[f32; 2]> {
        if self.width.is_nan() || self.width <= 0.0 {
            return Vec::new();
        }
        let half = self.width / 2.0;
        let (top, bottom) = (self.offset(-half), self.offset(half));
        let mut out = vec![top.from];
        top.flatten_into(&mut out, tolerance);
        let mut lower = vec![bottom.from];
        bottom.flatten_into(&mut lower, tolerance);
        out.extend(lower.into_iter().rev());
        out
    }

    fn offset(&self, dy: f32) -> Cubic {
        let mid = (self.x0 + self.x1) / 2.0;
        let (y0, y1) = (self.y0 + dy, self.y1 + dy);
        Cubic { from: [self.x0, y0], c1: [mid, y0], c2: [mid, y1], to: [self.x1, y1] }
    }
}

/// A laid-out diagram: nodes and links in input order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SankeyLayout {
    /// One per node.
    pub nodes: Vec<SankeyNode>,
    /// One per input link. Links to a node that doesn't exist and links from a node to itself
    /// are left out: their value and width are 0 and they sit at the origin.
    pub links: Vec<SankeyLinkLayout>,
}

#[derive(Clone, Default)]
struct Node {
    value: f64,
    depth: usize,
    height: usize,
    layer: usize,
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    sources: Vec<usize>,
    targets: Vec<usize>,
}

#[derive(Clone, Copy)]
struct Link {
    source: usize,
    target: usize,
    value: f64,
    width: f64,
    y0: f64,
    y1: f64,
}

struct Graph {
    nodes: Vec<Node>,
    links: Vec<Link>,
    /// Indices of the links in the layout.
    used: Vec<usize>,
    padding: f64,
    top: f64,
    bottom: f64,
}

/// Lays out `node_count` nodes joined by `links` in the area of `options`.
///
/// The layout follows d3-sankey: each node's value is the larger of its inflow and outflow;
/// columns come from the longest paths through the graph and `align`; heights share one scale,
/// chosen so the fullest column fits; nodes then relax toward the weighted middle of their
/// neighbours, column by column in both directions, and are pushed apart so they never overlap.
/// Graphs with cycles get a bounded layout, but their backward links run right to left.
pub fn sankey(node_count: usize, links: &[SankeyLink], options: &SankeyOptions) -> SankeyLayout {
    let finite = |v: f32| if v.is_finite() { f64::from(v) } else { 0.0 };
    let (x0, y0) = (finite(options.origin[0]), finite(options.origin[1]));
    let (x1, y1) = (x0 + finite(options.size[0]).max(0.0), y0 + finite(options.size[1]).max(0.0));
    let dx = finite(options.node_width).clamp(0.0, x1 - x0);

    let mut g = Graph {
        nodes: vec![Node::default(); node_count],
        links: Vec::with_capacity(links.len()),
        used: Vec::new(),
        padding: finite(options.node_padding).max(0.0),
        top: y0,
        bottom: y1,
    };
    for (i, l) in links.iter().enumerate() {
        let ok = l.source < node_count && l.target < node_count && l.source != l.target;
        let value = if ok && l.value.is_finite() && l.value > 0.0 { l.value } else { 0.0 };
        g.links.push(Link { source: l.source, target: l.target, value, width: 0.0, y0: 0.0, y1: 0.0 });
        if ok {
            g.nodes[l.source].sources.push(i);
            g.nodes[l.target].targets.push(i);
            g.used.push(i);
        }
    }
    g.values();
    g.depths();
    let columns = g.columns(options.align, x0, x1, dx);
    g.breadths(&columns, options.iterations.min(1_000));
    g.link_breadths();
    g.output(links.len())
}

impl Graph {
    fn values(&mut self) {
        for n in &mut self.nodes {
            let sum = |ls: &[usize]| ls.iter().map(|&l| self.links[l].value).sum::<f64>();
            n.value = sum(&n.sources).max(sum(&n.targets));
        }
    }

    /// Longest paths from sources (`depth`) and to sinks (`height`), bounded by the node count
    /// so cycles end.
    fn depths(&mut self) {
        let n = self.nodes.len();
        for forward in [true, false] {
            let mut current: Vec<usize> = (0..n).collect();
            let mut level = 0;
            while !current.is_empty() && level <= n {
                let mut seen = vec![false; n];
                let mut next = Vec::new();
                for &i in &current {
                    let node = &mut self.nodes[i];
                    let edges = if forward {
                        node.depth = level;
                        &node.sources
                    } else {
                        node.height = level;
                        &node.targets
                    };
                    for &l in edges {
                        let j = if forward { self.links[l].target } else { self.links[l].source };
                        if !seen[j] {
                            seen[j] = true;
                            next.push(j);
                        }
                    }
                }
                current = next;
                level += 1;
            }
        }
    }

    fn columns(&mut self, align: SankeyAlign, x0: f64, x1: f64, dx: f64) -> Vec<Vec<usize>> {
        let count = self.nodes.iter().map(|n| n.depth).max().map_or(0, |d| d + 1);
        let kx = if count > 1 { (x1 - x0 - dx) / (count - 1) as f64 } else { 0.0 };
        let mut columns = vec![Vec::new(); count];
        for i in 0..self.nodes.len() {
            let layer = self.align(i, align, count).min(count.saturating_sub(1));
            let node = &mut self.nodes[i];
            node.layer = layer;
            node.x0 = x0 + layer as f64 * kx;
            node.x1 = node.x0 + dx;
            columns[layer].push(i);
        }
        columns
    }

    fn align(&self, i: usize, align: SankeyAlign, count: usize) -> usize {
        let node = &self.nodes[i];
        match align {
            SankeyAlign::Left => node.depth,
            SankeyAlign::Right => count.saturating_sub(1 + node.height),
            SankeyAlign::Justify if node.sources.is_empty() => count.saturating_sub(1),
            SankeyAlign::Justify => node.depth,
            SankeyAlign::Center if !node.targets.is_empty() => node.depth,
            SankeyAlign::Center => {
                let nearest = node.sources.iter().map(|&l| self.nodes[self.links[l].target].depth).min();
                nearest.map_or(0, |d| d.saturating_sub(1))
            }
        }
    }

    fn breadths(&mut self, columns: &[Vec<usize>], iterations: usize) {
        let tallest = columns.iter().map(Vec::len).max().unwrap_or(0);
        if tallest > 1 {
            self.padding = self.padding.min((self.bottom - self.top) / (tallest - 1) as f64);
        }
        let mut columns = columns.to_vec();
        self.initialize(&columns);
        for i in 0..iterations {
            let alpha = 0.99f64.powi(i as i32);
            let beta = (1.0 - alpha).max((i + 1) as f64 / iterations as f64);
            self.relax(&mut columns, alpha, beta, false);
            self.relax(&mut columns, alpha, beta, true);
        }
    }

    fn initialize(&mut self, columns: &[Vec<usize>]) {
        let py = self.padding;
        let height = self.bottom - self.top;
        let ky = columns
            .iter()
            .filter_map(|c| {
                let sum: f64 = c.iter().map(|&i| self.nodes[i].value).sum();
                (sum > 0.0).then(|| (height - (c.len() - 1) as f64 * py) / sum)
            })
            .fold(f64::INFINITY, f64::min);
        let ky = if ky.is_finite() { ky.max(0.0) } else { 0.0 };
        for &l in &self.used {
            self.links[l].width = self.links[l].value * ky;
        }
        for column in columns {
            let mut y = self.top;
            for &i in column {
                let n = &mut self.nodes[i];
                n.y0 = y;
                n.y1 = y + n.value * ky;
                y = n.y1 + py;
            }
            let spare = (self.bottom - y + py) / (column.len() + 1) as f64;
            for (k, &i) in column.iter().enumerate() {
                let n = &mut self.nodes[i];
                n.y0 += spare * (k + 1) as f64;
                n.y1 += spare * (k + 1) as f64;
            }
            for &i in column {
                self.sort_links(i);
            }
        }
    }

    /// One pass moving each node toward its neighbours: from the sources' side when
    /// `left_to_right`, else from the targets' side.
    fn relax(&mut self, columns: &mut [Vec<usize>], alpha: f64, beta: f64, left_to_right: bool) {
        let order: Vec<usize> = if left_to_right {
            (1..columns.len()).collect()
        } else {
            (0..columns.len().saturating_sub(1)).rev().collect()
        };
        for c in order {
            for &i in &columns[c] {
                let (mut y, mut w) = (0.0, 0.0);
                let edges = if left_to_right { &self.nodes[i].targets } else { &self.nodes[i].sources };
                for &l in edges {
                    let Link { source, target, value, .. } = self.links[l];
                    let v = value * (self.nodes[target].layer as f64 - self.nodes[source].layer as f64);
                    let top =
                        if left_to_right { self.target_top(source, target) } else { self.source_top(source, target) };
                    y += top * v;
                    w += v;
                }
                if w.is_nan() || w <= 0.0 {
                    continue;
                }
                let n = &mut self.nodes[i];
                let dy = (y / w - n.y0) * alpha;
                n.y0 += dy;
                n.y1 += dy;
                self.sort_neighbour_links(i);
            }
            let nodes = &self.nodes;
            columns[c].sort_by(|&a, &b| nodes[a].y0.total_cmp(&nodes[b].y0));
            self.resolve_collisions(&columns[c], beta);
        }
    }

    /// Pushes the nodes of a column apart, out from the middle node, then back inside the
    /// diagram from the bottom and the top.
    fn resolve_collisions(&mut self, column: &[usize], alpha: f64) {
        let Some(&mid) = column.get(column.len() / 2) else { return };
        let i = column.len() / 2;
        let (y0, y1) = (self.nodes[mid].y0, self.nodes[mid].y1);
        self.push_up(column, y0 - self.padding, i, alpha);
        self.push_down(column, y1 + self.padding, i + 1, alpha);
        self.push_up(column, self.bottom, column.len(), alpha);
        self.push_down(column, self.top, 0, alpha);
    }

    /// Moves the nodes before index `end`, last first, up so each ends above `y`.
    fn push_up(&mut self, column: &[usize], mut y: f64, end: usize, alpha: f64) {
        for &i in column[..end].iter().rev() {
            let n = &mut self.nodes[i];
            let dy = (n.y1 - y) * alpha;
            if dy > 1e-6 {
                n.y0 -= dy;
                n.y1 -= dy;
            }
            y = n.y0 - self.padding;
        }
    }

    /// Moves the nodes from index `start` on down so each starts below `y`.
    fn push_down(&mut self, column: &[usize], mut y: f64, start: usize, alpha: f64) {
        for &i in column.iter().skip(start) {
            let n = &mut self.nodes[i];
            let dy = (y - n.y0) * alpha;
            if dy > 1e-6 {
                n.y0 += dy;
                n.y1 += dy;
            }
            y = n.y1 + self.padding;
        }
    }

    /// Where a link from `source` would enter `target` if `source` sat where its links point.
    fn target_top(&self, source: usize, target: usize) -> f64 {
        let s = &self.nodes[source];
        let mut y = s.y0 - (s.sources.len() as f64 - 1.0) * self.padding / 2.0;
        for &l in &s.sources {
            if self.links[l].target == target {
                break;
            }
            y += self.links[l].width + self.padding;
        }
        for &l in &self.nodes[target].targets {
            if self.links[l].source == source {
                break;
            }
            y -= self.links[l].width;
        }
        y
    }

    /// Where a link into `target` would leave `source` if `target` sat where its links point.
    fn source_top(&self, source: usize, target: usize) -> f64 {
        let t = &self.nodes[target];
        let mut y = t.y0 - (t.targets.len() as f64 - 1.0) * self.padding / 2.0;
        for &l in &t.targets {
            if self.links[l].source == source {
                break;
            }
            y += self.links[l].width + self.padding;
        }
        for &l in &self.nodes[source].sources {
            if self.links[l].target == target {
                break;
            }
            y -= self.links[l].width;
        }
        y
    }

    /// Orders a node's outgoing and incoming links (see [`Graph::sort_sources`]).
    fn sort_links(&mut self, i: usize) {
        self.sort_sources(i);
        self.sort_targets(i);
    }

    /// Orders a node's outgoing links by their targets' height, so they leave without crossing.
    fn sort_sources(&mut self, i: usize) {
        let mut ls = std::mem::take(&mut self.nodes[i].sources);
        ls.sort_by(|&a, &b| {
            self.y_of(self.links[a].target).total_cmp(&self.y_of(self.links[b].target)).then(a.cmp(&b))
        });
        self.nodes[i].sources = ls;
    }

    /// Orders a node's incoming links by their sources' height, so they enter without crossing.
    fn sort_targets(&mut self, i: usize) {
        let mut ls = std::mem::take(&mut self.nodes[i].targets);
        ls.sort_by(|&a, &b| {
            self.y_of(self.links[a].source).total_cmp(&self.y_of(self.links[b].source)).then(a.cmp(&b))
        });
        self.nodes[i].targets = ls;
    }

    /// After node `i` moved: re-sorts the outgoing links of its sources and the incoming links of
    /// its targets, whose order depends on where it is.
    fn sort_neighbour_links(&mut self, i: usize) {
        for k in 0..self.nodes[i].targets.len() {
            let source = self.links[self.nodes[i].targets[k]].source;
            self.sort_sources(source);
        }
        for k in 0..self.nodes[i].sources.len() {
            let target = self.links[self.nodes[i].sources[k]].target;
            self.sort_targets(target);
        }
    }

    fn y_of(&self, node: usize) -> f64 {
        self.nodes[node].y0
    }

    fn link_breadths(&mut self) {
        for i in 0..self.nodes.len() {
            self.sort_links(i);
            let (mut y0, mut y1) = (self.nodes[i].y0, self.nodes[i].y0);
            for &l in &self.nodes[i].sources {
                let link = &mut self.links[l];
                link.y0 = y0 + link.width / 2.0;
                y0 += link.width;
            }
            for &l in &self.nodes[i].targets {
                let link = &mut self.links[l];
                link.y1 = y1 + link.width / 2.0;
                y1 += link.width;
            }
        }
    }

    fn output(&self, link_count: usize) -> SankeyLayout {
        let nodes = self
            .nodes
            .iter()
            .map(|n| SankeyNode {
                x0: n.x0 as f32,
                x1: n.x1 as f32,
                y0: n.y0 as f32,
                y1: n.y1 as f32,
                value: n.value,
                depth: n.depth,
                height: n.height,
                layer: n.layer,
            })
            .collect();
        let mut links: Vec<SankeyLinkLayout> = (0..link_count)
            .map(|i| SankeyLinkLayout {
                source: self.links[i].source,
                target: self.links[i].target,
                value: 0.0,
                width: 0.0,
                x0: 0.0,
                x1: 0.0,
                y0: 0.0,
                y1: 0.0,
            })
            .collect();
        for &i in &self.used {
            let l = &self.links[i];
            links[i] = SankeyLinkLayout {
                source: l.source,
                target: l.target,
                value: l.value,
                width: l.width as f32,
                x0: self.nodes[l.source].x1 as f32,
                x1: self.nodes[l.target].x0 as f32,
                y0: l.y0 as f32,
                y1: l.y1 as f32,
            };
        }
        SankeyLayout { nodes, links }
    }
}

#[cfg(test)]
#[path = "sankey_tests.rs"]
mod tests;
