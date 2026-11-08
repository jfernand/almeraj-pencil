//! Dashed lines (AlMeraj et al. 2009, §3.6).
//!
//! The study found three schemes. Type 1: single dashes with gaps about as long as a dash. Type 2:
//! pairs of dashes, a short gap (M-gap) inside a pair and a long one (G-gap, 50–70 pixels) between
//! pairs, the second dash pressed a little harder. Type 3: the same with groups of three. Dash
//! lengths grow with the line's length (Table 2).

use crate::path::{Curve, PathParams, Point};
use crate::rng::Rng;

/// A dashing scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dashes {
    /// Single dashes, evenly spaced.
    Single,
    /// Pairs of dashes.
    Double,
    /// Groups of three dashes.
    Triple,
}

impl Dashes {
    /// Table 2: the range of dash lengths, in pixels at 100 ppi, for a line `length` pixels long.
    fn dash_range(self, length: f32) -> (f32, f32) {
        let band = if length <= 200.0 {
            0
        } else if length <= 400.0 {
            1
        } else {
            2
        };
        let table = match self {
            Dashes::Single => [(15.0, 25.0), (20.0, 30.0), (25.0, 45.0)],
            Dashes::Double => [(15.0, 25.0), (25.0, 30.0), (35.0, 40.0)],
            Dashes::Triple => [(10.0, 20.0), (20.0, 30.0), (30.0, 45.0)],
        };
        table[band]
    }

    fn group(self) -> usize {
        match self {
            Dashes::Single => 1,
            Dashes::Double => 2,
            Dashes::Triple => 3,
        }
    }
}

/// Splits a path into dashes. Returns each dash's polyline and the pressure to draw it with.
pub fn dashes(path: &[Point], scheme: Dashes, params: &PathParams, rng: &mut Rng) -> Vec<(Vec<Point>, f32)> {
    if path.len() < 2 {
        return Vec::new();
    }
    let curve = Curve::new(path);
    let scale = params.ppi / 100.0;
    let (lo, hi) = scheme.dash_range(curve.length / scale);
    let mut out = Vec::new();
    let mut s = 0.0;
    let mut index = 0;
    while s < curve.length {
        let dash = (lo + (hi - lo) * rng.unit()) * scale;
        let end = (s + dash).min(curve.length);
        let steps = ((end - s) / params.spacing.max(1e-3)).ceil().max(1.0) as usize;
        let points = (0..=steps).map(|k| curve.at(s + (end - s) * k as f32 / steps as f32).0).collect();
        let position = index % scheme.group();
        // Later dashes of a group are pressed a little harder.
        out.push((points, 1.0 + 0.08 * position as f32));
        index += 1;
        let gap = if scheme == Dashes::Single {
            dash * (0.9 + 0.2 * rng.unit())
        } else if position + 1 < scheme.group() {
            // M-gap: short.
            dash * (0.3 + 0.1 * rng.unit())
        } else {
            // G-gap: 50–70 pixels.
            (50.0 + 20.0 * rng.unit()) * scale
        };
        s = end + gap;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashes_cover_part_of_the_line_in_order() {
        let path: Vec<Point> = (0..=600).map(|x| [x as f32, 0.0]).collect();
        let params = PathParams::default();
        for scheme in [Dashes::Single, Dashes::Double, Dashes::Triple] {
            let d = dashes(&path, scheme, &params, &mut Rng::new(4));
            assert!(d.len() >= 5, "{scheme:?}");
            let starts: Vec<f32> = d.iter().map(|(p, _)| p[0][0]).collect();
            assert!(starts.windows(2).all(|w| w[0] < w[1]));
            let drawn: f32 = d.iter().map(|(p, _)| crate::path::length(p)).sum();
            assert!(drawn < 600.0 && drawn > 150.0, "{scheme:?}: {drawn}");
        }
    }
}
