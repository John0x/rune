//! Radar (spider) chart geometry: one axis per variable around a circle.

use std::f64::consts::TAU;

use super::polar;
use crate::scale::Linear;

/// The angles of `count` evenly spread axes, the first at `start_angle` (0 is twelve o'clock),
/// clockwise.
pub fn radar_angles(count: usize, start_angle: f64) -> Vec<f64> {
    (0..count).map(|i| start_angle + TAU * i as f64 / count as f64).collect()
}

/// The polygon of one series: value `i` on axis `i`, at the distance `radius` maps it to (a
/// scale from the value domain onto `(0, outer radius)`). Values that aren't finite, and
/// distances below zero, sit at the center.
pub fn radar_points(center: [f32; 2], values: &[f64], radius: &Linear, start_angle: f64) -> Vec<[f32; 2]> {
    let angles = radar_angles(values.len(), start_angle);
    values
        .iter()
        .zip(angles)
        .map(|(&v, a)| {
            let r = if v.is_finite() { radius.map(v) } else { 0.0 };
            polar(center, a, if r.is_finite() { r.max(0.0) } else { 0.0 })
        })
        .collect()
}

/// A ring of the grid: the polygon through every axis at `radius` pixels.
pub fn radar_ring(center: [f32; 2], count: usize, radius: f32, start_angle: f64) -> Vec<[f32; 2]> {
    radar_angles(count, start_angle).into_iter().map(|a| polar(center, a, radius)).collect()
}

#[cfg(test)]
#[path = "radar_tests.rs"]
mod tests;
