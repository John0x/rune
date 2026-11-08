//! Chart data as columns behind `Arc`s, so cloning a series into a view costs a reference count,
//! not a copy.
//!
//! x columns are sorted ascending (equal neighbours allowed) and finite, which lets the chart
//! find the visible range and the point under the pointer by binary search. A `NaN` y is a gap:
//! lines break there.

mod extent;
mod grid;
mod ohlc;
mod series;

pub use extent::extent;
pub use extent::extent_in;
pub use extent::union;
pub use grid::Grid;
pub use ohlc::Ohlc;
pub use series::Series;

use std::fmt;

/// Why data was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataError {
    /// Two columns that must be equally long are not.
    LengthMismatch {
        /// The x column's (or the grid's expected) length.
        expected: usize,
        /// The length found.
        found: usize,
    },
    /// The x column goes down at this index.
    Unsorted {
        /// The first index whose x is below the one before.
        index: usize,
    },
    /// An x value is `NaN` or infinite.
    NonFiniteX {
        /// The index of the value.
        index: usize,
    },
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::LengthMismatch { expected, found } => {
                write!(f, "column has {found} values, expected {expected}")
            }
            DataError::Unsorted { index } => write!(f, "x is not ascending at index {index}"),
            DataError::NonFiniteX { index } => write!(f, "x is not finite at index {index}"),
        }
    }
}

impl std::error::Error for DataError {}

/// Checks that `x` is finite and ascending.
pub(crate) fn check_x(x: &[f64]) -> Result<(), DataError> {
    for (i, v) in x.iter().enumerate() {
        if !v.is_finite() {
            return Err(DataError::NonFiniteX { index: i });
        }
        if i > 0 && *v < x[i - 1] {
            return Err(DataError::Unsorted { index: i });
        }
    }
    Ok(())
}

/// Checks that a column is as long as x.
pub(crate) fn check_len(expected: usize, column: &[f64]) -> Result<(), DataError> {
    if column.len() == expected { Ok(()) } else { Err(DataError::LengthMismatch { expected, found: column.len() }) }
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
