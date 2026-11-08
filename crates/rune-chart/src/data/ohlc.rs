//! Candles: open, high, low and close per x.

use std::ops::Range;
use std::sync::Arc;

use super::{DataError, check_len, check_x, extent_in, union};

/// Open-high-low-close columns over an ascending x, for candlestick charts. A `NaN` in a row
/// marks a missing candle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ohlc {
    x: Arc<[f64]>,
    open: Arc<[f64]>,
    high: Arc<[f64]>,
    low: Arc<[f64]>,
    close: Arc<[f64]>,
}

impl Ohlc {
    /// Candles from their columns, checking that all are as long as `x` and that `x` is finite
    /// and ascending.
    pub fn new(
        x: impl Into<Arc<[f64]>>,
        open: impl Into<Arc<[f64]>>,
        high: impl Into<Arc<[f64]>>,
        low: impl Into<Arc<[f64]>>,
        close: impl Into<Arc<[f64]>>,
    ) -> Result<Ohlc, DataError> {
        let o = Ohlc { x: x.into(), open: open.into(), high: high.into(), low: low.into(), close: close.into() };
        for column in [&o.open, &o.high, &o.low, &o.close] {
            check_len(o.x.len(), column)?;
        }
        check_x(&o.x)?;
        Ok(o)
    }

    /// Candles from `(x, open, high, low, close)` rows, checked as in [`Ohlc::new`].
    pub fn from_rows(rows: impl IntoIterator<Item = (f64, f64, f64, f64, f64)>) -> Result<Ohlc, DataError> {
        let mut cols: [Vec<f64>; 5] = Default::default();
        for (x, o, h, l, c) in rows {
            for (col, v) in cols.iter_mut().zip([x, o, h, l, c]) {
                col.push(v);
            }
        }
        let [x, o, h, l, c] = cols;
        Ohlc::new(x, o, h, l, c)
    }

    /// The x column, ascending.
    pub fn x(&self) -> &Arc<[f64]> {
        &self.x
    }

    /// The opening values.
    pub fn open(&self) -> &Arc<[f64]> {
        &self.open
    }

    /// The highs.
    pub fn high(&self) -> &Arc<[f64]> {
        &self.high
    }

    /// The lows.
    pub fn low(&self) -> &Arc<[f64]> {
        &self.low
    }

    /// The closing values.
    pub fn close(&self) -> &Arc<[f64]> {
        &self.close
    }

    /// The number of candles.
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// Whether there are no candles.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// Candle `i` as `(x, open, high, low, close)`, or `None` past the end.
    pub fn row(&self, i: usize) -> Option<(f64, f64, f64, f64, f64)> {
        Some((*self.x.get(i)?, self.open[i], self.high[i], self.low[i], self.close[i]))
    }

    /// The first and last x, or `None` when empty.
    pub fn x_extent(&self) -> Option<(f64, f64)> {
        Some((*self.x.first()?, *self.x.last()?))
    }

    /// The lowest low and highest high over the candles in `range` (all four columns count, so a
    /// row missing its high or low still shows).
    pub fn y_extent_in(&self, range: Range<usize>) -> Option<(f64, f64)> {
        [&self.low, &self.high, &self.open, &self.close]
            .into_iter()
            .fold(None, |acc, col| union(acc, extent_in(col, range.clone())))
    }

    /// The lowest and highest value of all candles.
    pub fn y_extent(&self) -> Option<(f64, f64)> {
        self.y_extent_in(0..self.len())
    }
}
