//! The path of a hand-drawn line (AlMeraj et al. 2009, §3.3).
//!
//! The hand moves from start to end along the minimum-jerk trajectory of Flash and Hogan (1985),
//!
//! ```text
//! x(t) = x₀ + (x_f − x₀)(10τ³ − 15τ⁴ + 6τ⁵),   τ = t / t_f,   t ∈ [0, t_f],   t_f = 2,
//! ```
//!
//! sampled every `Δt` (Table 1: fewer samples for short lines, which look straighter). Each
//! sample is pushed sideways by a random *squiggle* `D ∈ [−s, s]` pixels (the paper uses `s = 5`),
//! and a Catmull–Rom spline through the samples gives the drawn path (centripetal here; see
//! [`catmull_rom`]).
//!
//! The paper only draws straight lines, and leaves curves to future work (its Figure 21). [`along`]
//! extends the same construction to any curve: the hand's progress along the curve's arc length
//! follows the minimum-jerk profile, and the squiggle pushes along the curve's normal.

use crate::rng::Rng;

/// A point in pixels.
pub type Point = [f32; 2];

/// How a path is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathParams {
    /// Largest sideways displacement of a control point, in pixels. The paper's 5 was chosen for
    /// lines drawn at about 100 pixels per inch.
    pub squiggle: f32,
    /// Pixels per inch of the drawing, which scales the line lengths of Table 1 (given at about
    /// 100 ppi): a line of 300 pixels at 200 ppi is a short line.
    pub ppi: f32,
    /// Keep the end points where they were asked for (the study's participants were told to); when
    /// false they are displaced like the others.
    pub fixed_ends: bool,
    /// Distance between the points of the output polyline, in pixels.
    pub spacing: f32,
}

impl Default for PathParams {
    fn default() -> Self {
        Self { squiggle: 5.0, ppi: 100.0, fixed_ends: true, spacing: 0.5 }
    }
}

/// Duration of the movement, `t_f` (the paper found 2 empirically).
pub const DURATION: f32 = 2.0;

/// The minimum-jerk position profile, from 0 at `τ = 0` to 1 at `τ = 1`.
pub fn min_jerk(tau: f32) -> f32 {
    let t3 = tau * tau * tau;
    t3 * (10.0 - 15.0 * tau + 6.0 * tau * tau)
}

/// Table 1: the time step between control points for a line `length` pixels long at `ppi`.
pub fn time_step(length: f32, ppi: f32) -> f32 {
    let length = length * 100.0 / ppi;
    if length <= 200.0 {
        0.5
    } else if length <= 400.0 {
        0.3
    } else {
        0.2
    }
}

/// The times of the control points: `0, Δt, 2Δt, …` and the end of the movement.
fn times(dt: f32) -> Vec<f32> {
    let mut t: Vec<f32> = (0..).map(|i| i as f32 * dt).take_while(|&t| t < DURATION - 1e-3).collect();
    t.push(DURATION);
    t
}

/// The control points of a hand-drawn straight line from `a` to `b`.
pub fn line_controls(a: Point, b: Point, params: &PathParams, rng: &mut Rng) -> Vec<Point> {
    along_controls(&[a, b], params, rng)
}

/// A hand-drawn straight line from `a` to `b`, as a polyline.
pub fn line(a: Point, b: Point, params: &PathParams, rng: &mut Rng) -> Vec<Point> {
    catmull_rom(&line_controls(a, b, params, rng), params.spacing)
}

/// The control points of a hand-drawn rendition of the polyline `curve`.
///
/// # Panics
/// If `curve` has fewer than two points.
pub fn along_controls(curve: &[Point], params: &PathParams, rng: &mut Rng) -> Vec<Point> {
    let curve = Curve::new(curve);
    let dt = time_step(curve.length, params.ppi);
    let ts = times(dt);
    let last = ts.len() - 1;
    ts.iter()
        .enumerate()
        .map(|(i, &t)| {
            let s = min_jerk(t / DURATION) * curve.length;
            let (p, tangent) = curve.at(s);
            let pinned = params.fixed_ends && (i == 0 || i == last);
            // D: uniform in [−squiggle, squiggle], approximately normal to the path. The
            // minimum-jerk profile crowds control points near the ends, where a full squiggle
            // would bend every tip into a hook; there it is limited to a slope of about 1:4.
            let room = if params.fixed_ends { s.min(curve.length - s) / 4.0 } else { f32::INFINITY };
            let d = if pinned { 0.0 } else { (2.0 * rng.unit() - 1.0) * params.squiggle.min(room) };
            [p[0] - tangent[1] * d, p[1] + tangent[0] * d]
        })
        .collect()
}

/// A hand-drawn rendition of the polyline `curve`.
pub fn along(curve: &[Point], params: &PathParams, rng: &mut Rng) -> Vec<Point> {
    catmull_rom(&along_controls(curve, params, rng), params.spacing)
}

/// A Catmull–Rom spline through `controls`, sampled about every `spacing` pixels. The first and
/// last segments use reflected phantom points.
///
/// The paper uses the uniform spline. The minimum-jerk profile spaces control points unevenly
/// (crowded at the ends), and on a curve the uniform spline then overshoots into small loops near
/// the tips; this is the centripetal parameterisation (Barry–Goldman, `α = ½`), which passes
/// through the same points without overshoot. On a straight line the two agree closely.
pub fn catmull_rom(controls: &[Point], spacing: f32) -> Vec<Point> {
    let n = controls.len();
    if n < 2 {
        return controls.to_vec();
    }
    let at = |i: isize| -> Point {
        if i < 0 {
            let (a, b) = (controls[0], controls[1]);
            [2.0 * a[0] - b[0], 2.0 * a[1] - b[1]]
        } else if i as usize >= n {
            let (a, b) = (controls[n - 1], controls[n - 2]);
            [2.0 * a[0] - b[0], 2.0 * a[1] - b[1]]
        } else {
            controls[i as usize]
        }
    };
    let knot = |a: Point, b: Point| dist(a, b).sqrt().max(1e-4);
    let lerp = |a: Point, b: Point, ta: f32, tb: f32, t: f32| -> Point {
        let w = if tb > ta { (t - ta) / (tb - ta) } else { 0.0 };
        [a[0] + (b[0] - a[0]) * w, a[1] + (b[1] - a[1]) * w]
    };
    let mut out = vec![controls[0]];
    for i in 0..n as isize - 1 {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let t0 = 0.0;
        let t1 = t0 + knot(p0, p1);
        let t2 = t1 + knot(p1, p2);
        let t3 = t2 + knot(p2, p3);
        let steps = (dist(p1, p2) / spacing.max(1e-3)).ceil().max(1.0) as usize;
        for k in 1..=steps {
            let t = t1 + (t2 - t1) * k as f32 / steps as f32;
            let a1 = lerp(p0, p1, t0, t1, t);
            let a2 = lerp(p1, p2, t1, t2, t);
            let a3 = lerp(p2, p3, t2, t3, t);
            let b1 = lerp(a1, a2, t0, t2, t);
            let b2 = lerp(a2, a3, t1, t3, t);
            out.push(lerp(b1, b2, t1, t2, t));
        }
    }
    out
}

/// Distance between two points.
pub fn dist(a: Point, b: Point) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Length of a polyline.
pub fn length(points: &[Point]) -> f32 {
    points.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// A polyline parameterised by arc length.
pub(crate) struct Curve<'a> {
    points: &'a [Point],
    /// Arc length at each point.
    cumulative: Vec<f32>,
    pub length: f32,
}

impl<'a> Curve<'a> {
    pub fn new(points: &'a [Point]) -> Self {
        assert!(points.len() >= 2, "a curve needs two points");
        let mut cumulative = vec![0.0];
        for w in points.windows(2) {
            cumulative.push(cumulative.last().unwrap() + dist(w[0], w[1]));
        }
        let length = *cumulative.last().unwrap();
        Self { points, cumulative, length }
    }

    /// The point at arc length `s` and the unit tangent there.
    pub fn at(&self, s: f32) -> (Point, Point) {
        let s = s.clamp(0.0, self.length);
        let i = self.cumulative.partition_point(|&c| c <= s).clamp(1, self.points.len() - 1);
        let (a, b) = (self.points[i - 1], self.points[i]);
        let seg = self.cumulative[i] - self.cumulative[i - 1];
        let f = if seg > 0.0 { (s - self.cumulative[i - 1]) / seg } else { 0.0 };
        let tangent = if seg > 0.0 { [(b[0] - a[0]) / seg, (b[1] - a[1]) / seg] } else { [1.0, 0.0] };
        ([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f], tangent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_jerk_is_smooth_and_symmetric() {
        assert_eq!(min_jerk(0.0), 0.0);
        assert!((min_jerk(1.0) - 1.0).abs() < 1e-6);
        assert!((min_jerk(0.5) - 0.5).abs() < 1e-6);
        assert!((min_jerk(0.2) + min_jerk(0.8) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn table_one() {
        assert_eq!(time_step(150.0, 100.0), 0.5);
        assert_eq!(time_step(300.0, 100.0), 0.3);
        assert_eq!(time_step(500.0, 100.0), 0.2);
        // Twice the resolution, the same physical line.
        assert_eq!(time_step(300.0, 200.0), 0.5);
        assert_eq!(times(0.5), vec![0.0, 0.5, 1.0, 1.5, 2.0]);
        assert_eq!(times(0.3).len(), 8);
    }

    #[test]
    fn a_line_without_squiggle_is_straight_and_reaches_its_ends() {
        let params = PathParams { squiggle: 0.0, ..PathParams::default() };
        let path = line([10.0, 20.0], [310.0, 20.0], &params, &mut Rng::new(1));
        assert_eq!(path[0], [10.0, 20.0]);
        assert!(dist(*path.last().unwrap(), [310.0, 20.0]) < 1e-3);
        assert!(path.iter().all(|p| (p[1] - 20.0).abs() < 1e-3));
    }

    #[test]
    fn squiggle_stays_within_bounds() {
        let params = PathParams::default();
        let controls = line_controls([0.0, 0.0], [500.0, 0.0], &params, &mut Rng::new(3));
        assert_eq!(controls.len(), 11);
        assert!(controls.iter().all(|p| p[1].abs() <= 5.0));
        assert!(controls.iter().any(|p| p[1].abs() > 0.5));
        assert_eq!(controls[0], [0.0, 0.0]);
    }

    #[test]
    fn along_follows_a_curve() {
        let arc: Vec<Point> = (0..=64)
            .map(|i| {
                let a = std::f32::consts::PI * i as f32 / 64.0;
                [100.0 * a.cos(), 100.0 * a.sin()]
            })
            .collect();
        let params = PathParams { squiggle: 2.0, ..PathParams::default() };
        let path = along(&arc, &params, &mut Rng::new(5));
        // Every point stays near the circle of radius 100.
        assert!(path.iter().all(|p| (p[0].hypot(p[1]) - 100.0).abs() < 4.0));
    }
}
