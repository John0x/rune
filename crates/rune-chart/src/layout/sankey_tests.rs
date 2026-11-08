use super::*;

const EPS: f32 = 1e-3;

fn options() -> SankeyOptions {
    SankeyOptions::new([600.0, 400.0])
}

/// An energy-flow-like graph: three sources, two hubs, four sinks, with a link that skips a
/// column.
fn graph() -> (usize, Vec<SankeyLink>) {
    let l = SankeyLink::new;
    (
        9,
        vec![
            l(0, 3, 30.0),
            l(0, 4, 10.0),
            l(1, 3, 15.0),
            l(2, 4, 25.0),
            l(2, 8, 5.0),
            l(3, 5, 20.0),
            l(3, 6, 25.0),
            l(4, 6, 5.0),
            l(4, 7, 30.0),
        ],
    )
}

fn check_invariants(layout: &SankeyLayout, links: &[SankeyLink], o: &SankeyOptions) {
    let (top, bottom) = (o.origin[1], o.origin[1] + o.size[1]);
    let (left, right) = (o.origin[0], o.origin[0] + o.size[0]);
    for (i, n) in layout.nodes.iter().enumerate() {
        assert!(n.y0 >= top - EPS && n.y1 <= bottom + EPS, "node {i} inside: {n:?}");
        assert!(n.x0 >= left - EPS && n.x1 <= right + EPS, "node {i} inside: {n:?}");
        assert!(n.y1 >= n.y0 && n.x1 >= n.x0);
    }
    // Nodes in a column never overlap, and keep the padding between them.
    let columns = layout.nodes.iter().map(|n| n.layer).max().map_or(0, |m| m + 1);
    for c in 0..columns {
        let mut col: Vec<&SankeyNode> = layout.nodes.iter().filter(|n| n.layer == c).collect();
        col.sort_by(|a, b| a.y0.total_cmp(&b.y0));
        for w in col.windows(2) {
            assert!(w[1].y0 >= w[0].y1 + o.node_padding.min(400.0) - EPS * 10.0, "column {c}: {:?}", w);
        }
    }
    // Flow is conserved: a node is as tall as its larger side, and links fill it from the top.
    let ky = layout.links.iter().find(|l| l.value > 0.0).map(|l| f64::from(l.width) / l.value).unwrap();
    for (i, n) in layout.nodes.iter().enumerate() {
        let inflow: f64 = links.iter().zip(&layout.links).filter(|(l, _)| l.target == i).map(|(_, p)| p.value).sum();
        let outflow: f64 = links.iter().zip(&layout.links).filter(|(l, _)| l.source == i).map(|(_, p)| p.value).sum();
        assert_eq!(n.value, inflow.max(outflow), "node {i}");
        assert!((f64::from(n.y1 - n.y0) - n.value * ky).abs() < 1e-2, "node {i} height");
        for (side, flow) in [(true, outflow), (false, inflow)] {
            let ls: Vec<&SankeyLinkLayout> = layout
                .links
                .iter()
                .filter(|l| if side { l.source == i } else { l.target == i })
                .filter(|l| l.value > 0.0)
                .collect();
            let width: f32 = ls.iter().map(|l| l.width).sum();
            assert!((f64::from(width) - flow * ky).abs() < 1e-2, "node {i} link widths");
            for l in &ls {
                let y = if side { l.y0 } else { l.y1 };
                assert!(y - l.width / 2.0 >= n.y0 - EPS && y + l.width / 2.0 <= n.y1 + EPS, "link inside node {i}");
            }
            // Bands leaving one side don't overlap each other.
            let mut spans: Vec<(f32, f32)> = ls
                .iter()
                .map(|l| if side { l.y0 } else { l.y1 })
                .zip(ls.iter().map(|l| l.width))
                .map(|(y, w)| (y - w / 2.0, y + w / 2.0))
                .collect();
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            assert!(spans.windows(2).all(|w| w[1].0 >= w[0].1 - EPS));
        }
    }
    for l in &layout.links {
        if l.value > 0.0 {
            assert_eq!(l.x0, layout.nodes[l.source].x1);
            assert_eq!(l.x1, layout.nodes[l.target].x0);
        }
    }
}

#[test]
fn a_chain_runs_left_to_right() {
    let links = [SankeyLink::new(0, 1, 5.0), SankeyLink::new(1, 2, 5.0)];
    let s = sankey(3, &links, &options());
    assert_eq!(
        s.nodes.iter().map(|n| (n.depth, n.height, n.layer)).collect::<Vec<_>>(),
        [(0, 2, 0), (1, 1, 1), (2, 0, 2)]
    );
    assert_eq!(s.nodes.iter().map(|n| n.x0).collect::<Vec<_>>(), [0.0, 288.0, 576.0]);
    assert_eq!(s.nodes[2].x1, 600.0);
    assert!((s.nodes[0].y1 - s.nodes[0].y0 - 400.0).abs() < EPS, "a lone column fills the height");
    assert_eq!(s.links[0].width, s.links[1].width);
    check_invariants(&s, &links, &options());
}

#[test]
fn invariants_hold_for_a_real_graph() {
    let (n, links) = graph();
    for align in [SankeyAlign::Justify, SankeyAlign::Left, SankeyAlign::Right, SankeyAlign::Center] {
        for iterations in [0, 1, 6, 32] {
            let o = SankeyOptions { align, iterations, origin: [20.0, 10.0], ..options() };
            let s = sankey(n, &links, &o);
            check_invariants(&s, &links, &o);
        }
    }
}

#[test]
fn the_fullest_column_fills_the_height() {
    let (n, links) = graph();
    let o = options();
    let s = sankey(n, &links, &o);
    let ky = f64::from(s.links[0].width) / s.links[0].value;
    let fullest = (0..3)
        .map(|c| {
            let col: Vec<&SankeyNode> = s.nodes.iter().filter(|n| n.layer == c).collect();
            col.iter().map(|n| n.value).sum::<f64>() * ky + (col.len() - 1) as f64 * f64::from(o.node_padding)
        })
        .fold(0.0, f64::max);
    assert!((fullest - 400.0).abs() < 1e-2, "{fullest}");
}

#[test]
fn alignment_moves_sources_and_sinks() {
    let (n, links) = graph();
    let layer = |align| {
        sankey(n, &links, &SankeyOptions { align, ..options() }).nodes.iter().map(|n| n.layer).collect::<Vec<_>>()
    };
    assert_eq!(layer(SankeyAlign::Justify), [0, 0, 0, 1, 1, 2, 2, 2, 2], "sinks go last");
    assert_eq!(layer(SankeyAlign::Left), [0, 0, 0, 1, 1, 2, 2, 2, 1], "a sink stays at its depth");
    assert_eq!(layer(SankeyAlign::Right), [0, 0, 0, 1, 1, 2, 2, 2, 2]);
    let late = [SankeyLink::new(0, 1, 1.0), SankeyLink::new(1, 2, 1.0), SankeyLink::new(3, 2, 1.0)];
    let right = sankey(4, &late, &SankeyOptions { align: SankeyAlign::Right, ..options() });
    assert_eq!(right.nodes[3].layer, 1, "right alignment moves a short source next to its target");
    let center = sankey(4, &late, &SankeyOptions { align: SankeyAlign::Center, ..options() });
    assert_eq!(center.nodes[3].layer, 1);
    let left = sankey(4, &late, &SankeyOptions { align: SankeyAlign::Left, ..options() });
    assert_eq!(left.nodes[3].layer, 0);
}

#[test]
fn relaxation_untangles_crossings() {
    // Two independent flows listed so that their initial order crosses.
    let links = [SankeyLink::new(0, 3, 10.0), SankeyLink::new(1, 2, 10.0)];
    let s = sankey(4, &links, &SankeyOptions { align: SankeyAlign::Left, ..options() });
    let sources_down = s.nodes[0].y0 < s.nodes[1].y0;
    let targets_down = s.nodes[3].y0 < s.nodes[2].y0;
    assert_eq!(sources_down, targets_down, "the flows no longer cross: {:?}", s.nodes);
}

#[test]
fn bad_links_are_left_out() {
    let links = [
        SankeyLink::new(0, 1, 4.0),
        SankeyLink::new(0, 9, 4.0),
        SankeyLink::new(1, 1, 4.0),
        SankeyLink::new(1, 2, f64::NAN),
        SankeyLink::new(1, 2, -3.0),
        SankeyLink::new(1, 2, 2.0),
    ];
    let s = sankey(3, &links, &options());
    assert_eq!(s.links.len(), 6);
    for i in [1, 2] {
        assert_eq!((s.links[i].value, s.links[i].width, s.links[i].x0), (0.0, 0.0, 0.0));
    }
    assert_eq!((s.links[3].value, s.links[3].width), (0.0, 0.0));
    assert_eq!(s.links[3].x0, s.nodes[1].x1, "a zero link is still placed");
    assert_eq!(s.nodes[1].value, 4.0);
    assert!(s.links[5].width > 0.0);
}

#[test]
fn cycles_and_odd_sizes_stay_finite() {
    let cycle = [SankeyLink::new(0, 1, 3.0), SankeyLink::new(1, 0, 1.0), SankeyLink::new(1, 2, 2.0)];
    for o in [
        options(),
        SankeyOptions::new([0.0, 0.0]),
        SankeyOptions::new([f32::NAN, -5.0]),
        SankeyOptions { node_width: 1e6, node_padding: 1e6, ..options() },
    ] {
        let s = sankey(3, &cycle, &o);
        assert!(s.nodes.iter().all(|n| [n.x0, n.x1, n.y0, n.y1].iter().all(|v| v.is_finite())));
        assert!(s.links.iter().all(|l| [l.x0, l.x1, l.y0, l.y1, l.width].iter().all(|v| v.is_finite())));
    }
    assert_eq!(sankey(0, &[], &options()), SankeyLayout::default());
    let lonely = sankey(3, &[], &options());
    assert_eq!(lonely.nodes.len(), 3);
    assert!(lonely.nodes.iter().all(|n| n.y0.is_finite() && n.y1 == n.y0 && n.layer == 0));
}

#[test]
fn the_layout_is_deterministic() {
    let (n, links) = graph();
    assert_eq!(sankey(n, &links, &options()), sankey(n, &links, &options()));
}

#[test]
fn links_draw_as_curves_and_bands() {
    let (n, links) = graph();
    let s = sankey(n, &links, &options());
    let l = s.links[0];
    let path = l.path();
    assert_eq!((path.from, path.to), ([l.x0, l.y0], [l.x1, l.y1]));
    assert_eq!((path.c1[1], path.c2[1]), (l.y0, l.y1), "leaves and enters horizontally");
    let band = l.band(0.5);
    let half = l.width / 2.0;
    assert_eq!(band[0], [l.x0, l.y0 - half]);
    assert_eq!(*band.last().unwrap(), [l.x0, l.y0 + half]);
    assert!(band.contains(&[l.x1, l.y1 - half]) && band.contains(&[l.x1, l.y1 + half]));
    let zero = SankeyLinkLayout { width: 0.0, ..l };
    assert!(zero.band(0.5).is_empty());
}
