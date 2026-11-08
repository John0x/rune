//! Heat-map cells.

use std::sync::Arc;

use super::DataError;

/// A heat map's values: `columns` x bands by `rows` y bands, each cell a value or empty.
/// Cells are stored row by row: cell `(column, row)` is at `row * columns + column`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grid {
    columns: usize,
    rows: usize,
    values: Arc<[Option<f64>]>,
}

impl Grid {
    /// A grid from its cells, row by row; there must be `columns * rows` of them.
    pub fn new(columns: usize, rows: usize, values: impl Into<Arc<[Option<f64>]>>) -> Result<Grid, DataError> {
        let values = values.into();
        let expected = columns.saturating_mul(rows);
        if values.len() != expected {
            return Err(DataError::LengthMismatch { expected, found: values.len() });
        }
        Ok(Grid { columns, rows, values })
    }

    /// A grid with each cell from `cell(column, row)`.
    pub fn from_fn(columns: usize, rows: usize, mut cell: impl FnMut(usize, usize) -> Option<f64>) -> Grid {
        let values = (0..rows).flat_map(|r| (0..columns).map(move |c| (c, r))).map(|(c, r)| cell(c, r)).collect();
        Grid { columns, rows, values }
    }

    /// The number of x bands.
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// The number of y bands.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// All cells, row by row.
    pub fn values(&self) -> &Arc<[Option<f64>]> {
        &self.values
    }

    /// The value of a cell; `None` when it is empty, `NaN`, or outside the grid.
    pub fn get(&self, column: usize, row: usize) -> Option<f64> {
        if column >= self.columns || row >= self.rows {
            return None;
        }
        self.values[row * self.columns + column].filter(|v| !v.is_nan())
    }

    /// The smallest and largest finite value, for the color scale.
    pub fn extent(&self) -> Option<(f64, f64)> {
        self.values.iter().flatten().copied().filter(|v| v.is_finite()).fold(None, |acc, v| match acc {
            None => Some((v, v)),
            Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
        })
    }
}
