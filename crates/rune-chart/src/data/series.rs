//! A line or scatter series: an x column and a y column.

use std::ops::Range;
use std::sync::Arc;

use super::{DataError, check_len, check_x, extent_in};

/// Points as two equally long columns: `x` ascending and finite, `y` with `NaN` for gaps.
///
/// Cloning is cheap (two reference counts).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Series {
    x: Arc<[f64]>,
    y: Arc<[f64]>,
}

impl Series {
    /// A series from its columns, checking that they are equally long and that `x` is finite and
    /// ascending.
    pub fn new(x: impl Into<Arc<[f64]>>, y: impl Into<Arc<[f64]>>) -> Result<Series, DataError> {
        let (x, y) = (x.into(), y.into());
        check_len(x.len(), &y)?;
        check_x(&x)?;
        Ok(Series { x, y })
    }

    /// A series from `(x, y)` points, checked as in [`Series::new`].
    pub fn from_points(points: impl IntoIterator<Item = (f64, f64)>) -> Result<Series, DataError> {
        let (x, y): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
        Series::new(x, y)
    }

    /// A series of `len` points from `point(i) -> (x, y)`, checked as in [`Series::new`].
    pub fn from_fn(len: usize, point: impl FnMut(usize) -> (f64, f64)) -> Result<Series, DataError> {
        Series::from_points((0..len).map(point))
    }

    /// A series over x = 0, 1, 2, … (bars by index, sparklines).
    pub fn from_y(y: impl IntoIterator<Item = f64>) -> Series {
        let y: Arc<[f64]> = y.into_iter().collect();
        let x: Arc<[f64]> = (0..y.len()).map(|i| i as f64).collect();
        Series { x, y }
    }

    /// A series from points in any order: sorted by x (stably), with points whose x isn't finite
    /// dropped.
    pub fn from_unsorted(points: impl IntoIterator<Item = (f64, f64)>) -> Series {
        let mut points: Vec<(f64, f64)> = points.into_iter().filter(|p| p.0.is_finite()).collect();
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (x, y): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
        Series { x: x.into(), y: y.into() }
    }

    /// The x column, ascending.
    pub fn x(&self) -> &Arc<[f64]> {
        &self.x
    }

    /// The y column; `NaN` is a gap.
    pub fn y(&self) -> &Arc<[f64]> {
        &self.y
    }

    /// The number of points.
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// Whether there are no points.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// Point `i`, or `None` past the end.
    pub fn point(&self, i: usize) -> Option<(f64, f64)> {
        Some((*self.x.get(i)?, *self.y.get(i)?))
    }

    /// The first and last x, or `None` when empty.
    pub fn x_extent(&self) -> Option<(f64, f64)> {
        Some((*self.x.first()?, *self.x.last()?))
    }

    /// The smallest and largest y, gaps skipped.
    pub fn y_extent(&self) -> Option<(f64, f64)> {
        extent_in(&self.y, 0..self.len())
    }

    /// The smallest and largest y over the points in `range`.
    pub fn y_extent_in(&self, range: Range<usize>) -> Option<(f64, f64)> {
        extent_in(&self.y, range)
    }
}
