//! Gallery sections for pie, radar, sankey and heat map charts.

use rune::{El, oklch};
use rune_chart::{ChartStyle, HeatmapChart, PieChart, RadarChart, SankeyChart};

use crate::data;

fn gold(v: f64) -> String {
    if v >= 1000.0 {
        format!("{:.1}k g", v / 1000.0)
    } else {
        format!("{v:.0} g")
    }
}

pub(crate) fn sections() -> Vec<El> {
    let income = PieChart::new("pie-income")
        .style(ChartStyle::dark())
        .data([
            ("Auctions", 6800.0),
            ("Quests", 4200.0),
            ("Crafting", 2100.0),
            ("Looting", 1500.0),
        ])
        .format(gold)
        .height(220.0)
        .build();

    let spending = PieChart::new("donut-spending")
        .style(ChartStyle::dark())
        .data([
            ("Consumables", 3900.0),
            ("Mounts", 2500.0),
            ("Repairs", 1800.0),
            ("Auction fees", 700.0),
            ("Transmog", 420.0),
            ("Pets", 160.0),
            ("Mail", 40.0),
        ])
        .donut(0.6)
        .pad_angle(0.02)
        .sort(true)
        .labels(true)
        .format(gold)
        .height(240.0)
        .build();

    let classes = RadarChart::new("radar-classes")
        .style(ChartStyle::dark())
        .axes([
            "Damage", "Survival", "Mobility", "Healing", "Control", "Range",
        ])
        .series("Warrior", vec![8.0, 9.0, 4.0, 2.0, 6.0, 2.0])
        .series("Mage", vec![9.5, 3.0, 6.0, 1.0, 8.0, 9.0])
        .series("Druid", vec![6.0, 6.5, 8.0, 8.5, 5.0, 6.0])
        .levels(5)
        .height(280.0)
        .build();

    let economy = SankeyChart::new("sankey-economy")
        .style(ChartStyle::dark())
        .node("Auctions", oklch(0.72, 0.15, 75.0))
        .node("Quests", oklch(0.72, 0.15, 150.0))
        .node("Crafting", oklch(0.72, 0.15, 195.0))
        .node("Looting", oklch(0.72, 0.15, 305.0))
        .node("Gold", oklch(0.82, 0.16, 90.0))
        .node("Consumables", None)
        .node("Mounts", None)
        .node("Repairs", None)
        .node("Auction fees", None)
        .node("Savings", None)
        .node("Flasks", None)
        .node("Food", None)
        .link(0, 4, 6800.0)
        .link(1, 4, 4200.0)
        .link(2, 4, 2100.0)
        .link(3, 4, 1500.0)
        .link(4, 5, 3900.0)
        .link(4, 6, 2500.0)
        .link(4, 7, 1800.0)
        .link(4, 8, 700.0)
        .link(4, 9, 5700.0)
        .link(5, 10, 2600.0)
        .link(5, 11, 1300.0)
        .format(gold)
        .height(320.0)
        .build();

    let week = HeatmapChart::new("heatmap-week")
        .style(ChartStyle::dark())
        .columns((0..24).map(|h| format!("{h:02}")))
        .rows(["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"])
        .grid(data::week_sales())
        .colors(oklch(0.3, 0.04, 75.0), oklch(0.82, 0.16, 75.0))
        .format(|v| format!("{v:.0} sales"))
        .height(196.0)
        .build();

    vec![
        crate::section("Pie: gold by source", income),
        crate::section("Donut: spending, with labels", spending),
        crate::section("Radar: class strengths", classes),
        crate::section("Sankey: where the gold comes from and goes", economy),
        crate::section("Heat map: sales by weekday and hour", week),
    ]
}
