//! How a chart looks: its colours, its series palette and its type. Build one from the app's
//! theme, or start from [`ChartStyle::dark`] or [`ChartStyle::light`].

use rune::{Color, TextStyle, oklch};

/// A chart's colours and type.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartStyle {
    /// Axis labels and the legend's names.
    pub text: Color,
    /// Tick labels, quieter than `text`.
    pub muted: Color,
    /// Grid lines.
    pub grid: Color,
    /// The axis lines.
    pub axis: Color,
    /// The crosshair.
    pub crosshair: Color,
    /// The tooltip's background, border and text.
    pub tooltip: Color,
    pub tooltip_border: Color,
    pub tooltip_text: Color,
    /// The rectangle a box zoom draws, and a selected range.
    pub selection: Color,
    /// Series colours, in order; a series without its own colour takes the next.
    pub palette: Vec<Color>,
    /// A rising and a falling candle (or any up/down pair).
    pub up: Color,
    pub down: Color,
    /// Tick labels, the legend and the tooltip.
    pub label: TextStyle,
}

/// Series hues, spread so neighbours never look alike.
const HUES: [f32; 8] = [255.0, 30.0, 150.0, 75.0, 305.0, 195.0, 350.0, 110.0];

impl ChartStyle {
    /// For a dark background.
    pub fn dark() -> ChartStyle {
        ChartStyle {
            text: oklch(0.92, 0.0, 0.0),
            muted: oklch(0.68, 0.0, 0.0),
            grid: oklch(0.32, 0.0, 0.0),
            axis: oklch(0.45, 0.0, 0.0),
            crosshair: oklch(0.75, 0.0, 0.0),
            tooltip: oklch(0.22, 0.0, 0.0),
            tooltip_border: oklch(0.36, 0.0, 0.0),
            tooltip_text: oklch(0.94, 0.0, 0.0),
            selection: oklch(0.7, 0.12, 255.0).alpha(0.22),
            palette: HUES.iter().map(|&h| oklch(0.72, 0.15, h)).collect(),
            up: oklch(0.72, 0.17, 150.0),
            down: oklch(0.66, 0.2, 25.0),
            label: TextStyle::new(11.0, 400.0).lh(14.0),
        }
    }

    /// For a light background.
    pub fn light() -> ChartStyle {
        ChartStyle {
            text: oklch(0.25, 0.0, 0.0),
            muted: oklch(0.5, 0.0, 0.0),
            grid: oklch(0.9, 0.0, 0.0),
            axis: oklch(0.75, 0.0, 0.0),
            crosshair: oklch(0.45, 0.0, 0.0),
            tooltip: oklch(0.99, 0.0, 0.0),
            tooltip_border: oklch(0.86, 0.0, 0.0),
            tooltip_text: oklch(0.2, 0.0, 0.0),
            selection: oklch(0.55, 0.15, 255.0).alpha(0.16),
            palette: HUES.iter().map(|&h| oklch(0.58, 0.16, h)).collect(),
            up: oklch(0.6, 0.16, 150.0),
            down: oklch(0.58, 0.2, 25.0),
            label: TextStyle::new(11.0, 400.0).lh(14.0),
        }
    }

    /// The palette's colour for series `index`.
    pub fn color(&self, index: usize) -> Color {
        if self.palette.is_empty() { self.text } else { self.palette[index % self.palette.len()] }
    }
}

impl Default for ChartStyle {
    fn default() -> ChartStyle {
        ChartStyle::dark()
    }
}
