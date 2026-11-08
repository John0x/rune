//! The band scale, for categories.

/// Splits a pixel range into `count` equal bands, one per category, with padding between bands
/// (`inner`) and at the ends (`outer`), as d3's `scaleBand`.
///
/// Paddings are fractions of the step (band plus gap): `inner` 0.2 leaves a fifth of each step
/// empty between bars. Category 0 sits at `range.0`, so a reversed range lists categories from
/// the other end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    count: usize,
    range: (f32, f32),
    inner: f32,
    outer: f32,
    align: f32,
}

impl Band {
    /// `count` bands across `range`, without padding.
    pub fn new(count: usize, range: (f32, f32)) -> Band {
        Band { count, range, inner: 0.0, outer: 0.0, align: 0.5 }
    }

    /// The same inner and outer padding.
    pub fn padding(self, padding: f32) -> Band {
        self.padding_inner(padding).padding_outer(padding)
    }

    /// The gap between bands, as a fraction of the step, clamped to `[0, 1]`.
    pub fn padding_inner(self, inner: f32) -> Band {
        Band { inner: clamp01(inner), ..self }
    }

    /// The space before the first and after the last band, in steps, at least 0.
    pub fn padding_outer(self, outer: f32) -> Band {
        Band { outer: if outer.is_finite() { outer.max(0.0) } else { 0.0 }, ..self }
    }

    /// Where the bands sit when outer space is left over: 0 at the start, 0.5 centered (the
    /// default), 1 at the end.
    pub fn align(self, align: f32) -> Band {
        Band { align: clamp01(align), ..self }
    }

    /// The number of bands.
    pub fn count(&self) -> usize {
        self.count
    }

    /// The pixel bounds.
    pub fn range(&self) -> (f32, f32) {
        self.range
    }

    /// The distance from one band's start to the next one's, always positive.
    pub fn step(&self) -> f32 {
        let (lo, hi) = self.bounds();
        let n = self.count as f32;
        (hi - lo) / (n - self.inner + self.outer * 2.0).max(1.0)
    }

    /// The width of a band.
    pub fn bandwidth(&self) -> f32 {
        self.step() * (1.0 - self.inner)
    }

    /// The low-pixel edge of band `index` (its left, or its top in a vertical layout), or `None`
    /// past the last band.
    pub fn position(&self, index: usize) -> Option<f32> {
        if index >= self.count {
            return None;
        }
        let (step, start) = (self.step(), self.start());
        let slot = if self.reversed() { self.count - 1 - index } else { index };
        Some(start + step * slot as f32)
    }

    /// The middle of band `index`.
    pub fn center(&self, index: usize) -> Option<f32> {
        self.position(index).map(|p| p + self.bandwidth() / 2.0)
    }

    /// The band whose step holds pixel `px` (the band plus half the gap on each side), or `None`
    /// outside every band.
    pub fn index_at(&self, px: f32) -> Option<usize> {
        let step = self.step();
        if self.count == 0 || step.is_nan() || step <= 0.0 || !px.is_finite() {
            return None;
        }
        let gap = step - self.bandwidth();
        let slot = ((px - self.start() + gap / 2.0) / step).floor();
        if !(0.0..self.count as f32).contains(&slot) {
            return None;
        }
        let slot = slot as usize;
        Some(if self.reversed() { self.count - 1 - slot } else { slot })
    }

    fn reversed(&self) -> bool {
        self.range.1 < self.range.0
    }

    fn bounds(&self) -> (f32, f32) {
        let (a, b) = self.range;
        if b < a { (b, a) } else { (a, b) }
    }

    /// The low-pixel edge of the first slot.
    fn start(&self) -> f32 {
        let (lo, hi) = self.bounds();
        let step = self.step();
        lo + (hi - lo - step * (self.count as f32 - self.inner)) * self.align
    }
}

fn clamp01(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
}

#[cfg(test)]
#[path = "band_tests.rs"]
mod tests;
