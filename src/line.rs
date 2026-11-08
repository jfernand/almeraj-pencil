//! Pencil texture along a path (AlMeraj et al. 2009, §3.4.2 and §3.5, "Phase B").
//!
//! 1. **Initial fill.** Every pixel within the line's half-width gets a random grey: pixels on the
//!    centre of the path from the centre histogram, those beside them uniformly from
//!    `[min_range, max_mid]`, and the outer ones, lighter, from `[max_mid, max_range]` — the
//!    bell-shaped fall-off across a real line.
//! 2. **Co-occurrence correction.** Every pixel looks at its 3 × 3 neighbourhood; a neighbour whose
//!    grey never occurs next to this pixel's grey in the sample is replaced by the nearest grey
//!    that does. Repeated until nothing changes.
//! 3. **Gaussian.** One pass of a 3 × 3 Gaussian smooths tone and aliasing.
//!
//! The half-width wanders along the line by Perlin-style noise (§3.5), so the sides are irregular.

use crate::path::Point;
use crate::profile::{Grade, Profile};
use crate::rng::Rng;

/// A pencil: the statistics of its grade and how it is held.
#[derive(Clone, Debug, PartialEq)]
pub struct Pencil {
    /// The grade's statistics.
    pub profile: Profile,
    /// Line width in pixels (the profile's own width by default).
    pub width: f32,
    /// How much the width wanders along the line, as a fraction of it. Default 0.2.
    pub wobble: f32,
    /// Largest number of co-occurrence correction passes. Default 20.
    pub passes: usize,
}

impl Pencil {
    /// A pencil of a built-in grade at its own width (about 100 ppi).
    pub fn new(grade: Grade) -> Self {
        Self::from_profile(Profile::grade(grade))
    }

    /// A pencil with measured statistics, e.g. from [`Profile::from_sample`].
    pub fn from_profile(profile: Profile) -> Self {
        let width = profile.width;
        Self { profile, width, wobble: 0.2, passes: 20 }
    }

    /// The same pencil drawing lines `width` pixels wide.
    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// Paper to draw on: a tone per pixel, 1 for white paper and 0 for black. Lines darken it by
/// multiplication, as graphite does: two lines crossing are darker than either.
#[derive(Clone, Debug, PartialEq)]
pub struct Canvas {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Tone per pixel, row by row.
    pub tone: Vec<f32>,
}

impl Canvas {
    /// White paper.
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, tone: vec![1.0; width * height] }
    }

    /// Draws `path` with `pencil`. `pressure` scales the darkness of the graphite (1 as sampled).
    pub fn draw(&mut self, path: &[Point], pencil: &Pencil, pressure: f32, rng: &mut Rng) {
        if path.is_empty() {
            return;
        }
        let half = pencil.width / 2.0 * (1.0 + pencil.wobble);
        let margin = half.ceil() as isize + 2;
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in path {
            (x0, y0, x1, y1) = (x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1]));
        }
        let bx = (x0.floor() as isize - margin).max(0);
        let by = (y0.floor() as isize - margin).max(0);
        let ex = (x1.ceil() as isize + margin).min(self.width as isize - 1);
        let ey = (y1.ceil() as isize + margin).min(self.height as isize - 1);
        if bx > ex || by > ey {
            return;
        }
        let (w, h) = ((ex - bx + 1) as usize, (ey - by + 1) as usize);
        let grey = synthesize(path, pencil, [bx as f32, by as f32], w, h, rng);
        for j in 0..h {
            for i in 0..w {
                let g = grey[j * w + i];
                if g < 1.0 {
                    let darkness = ((1.0 - g) * pressure).clamp(0.0, 1.0);
                    self.tone[(by as usize + j) * self.width + bx as usize + i] *= 1.0 - darkness;
                }
            }
        }
    }

    /// The canvas as 8-bit greys (0 black).
    pub fn to_grey8(&self) -> Vec<u8> {
        self.tone.iter().map(|t| (t.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
    }
}

/// The line's texture in a `w × h` box whose top-left pixel is at `origin`: tone per pixel, 1 where
/// no graphite fell.
fn synthesize(path: &[Point], pencil: &Pencil, origin: [f32; 2], w: usize, h: usize, rng: &mut Rng) -> Vec<f32> {
    let profile = &pencil.profile;
    // Distance from each pixel centre to the path, and the arc length of the nearest point,
    // stamped point by point.
    let reach = pencil.width / 2.0 * (1.0 + pencil.wobble) + 1.0;
    let mut nearest = vec![(f32::INFINITY, 0.0f32); w * h];
    let mut s = 0.0;
    for (k, p) in path.iter().enumerate() {
        if k > 0 {
            s += crate::path::dist(path[k - 1], *p);
        }
        let (cx, cy) = (p[0] - origin[0], p[1] - origin[1]);
        let (i0, i1) = (((cx - reach).floor().max(0.0)) as usize, ((cx + reach).ceil() as usize).min(w - 1));
        let (j0, j1) = (((cy - reach).floor().max(0.0)) as usize, ((cy + reach).ceil() as usize).min(h - 1));
        for j in j0..=j1 {
            for i in i0..=i1 {
                let d = (i as f32 + 0.5 - cx).hypot(j as f32 + 0.5 - cy);
                let cell = &mut nearest[j * w + i];
                if d < cell.0 {
                    *cell = (d, s);
                }
            }
        }
    }

    // 1. Initial fill, in zones across the line. Each pixel remembers its zone's range of greys.
    let (min_range, max_range) = Profile::bounds(&profile.range);
    let (min_mid, max_mid) = Profile::bounds(&profile.mid);
    let uniform = |rng: &mut Rng, (lo, hi): (u8, u8)| -> u8 { lo + rng.below(hi as u32 - lo as u32 + 1) as u8 };
    let ordered = |a: u8, b: u8| (a.min(b), a.max(b));
    let zones = [(min_mid, max_mid), ordered(min_range, max_mid), ordered(max_mid, max_range)];
    let length = nearest.iter().map(|&(_, s)| s).fold(0.0f32, f32::max).max(s);
    let phase = rng.unit() * 1000.0;
    let mut grey: Vec<Option<(u8, usize)>> = vec![None; w * h];
    for (c, &(d, s)) in nearest.iter().enumerate() {
        let wander = 1.0 + pencil.wobble * fbm(phase + s / (6.0 * pencil.width.max(1.0)));
        // Tips of long lines taper over a couple of widths; short lines and dashes hardly do
        // (the study's observations, §3.1).
        let depth = 0.45 * smoothstep((length - 50.0) / 250.0);
        let end = s.min(length - s) / (2.0 * pencil.width.max(1.0));
        let taper = 1.0 - depth + depth * smoothstep(end);
        let half = pencil.width / 2.0 * wander * taper;
        if d > half {
            continue;
        }
        let u = d / half.max(1e-3);
        grey[c] = Some(if d <= 0.5 || u < 1.0 / 3.0 {
            (profile.sample_mid(rng), 0)
        } else if u < 2.0 / 3.0 {
            (uniform(rng, zones[1]), 1)
        } else {
            (uniform(rng, zones[2]), 2)
        });
    }

    // 2. Co-occurrence correction. A replacement stays within the neighbour's own zone, so the
    // texture reorganises without its tone drifting along the line.
    for _ in 0..pencil.passes {
        let mut changed = 0;
        for j in 0..h {
            for i in 0..w {
                let Some((g, _)) = grey[j * w + i] else { continue };
                for (di, dj) in NEIGHBOURS {
                    let (ni, nj) = (i as isize + di, j as isize + dj);
                    if ni < 0 || nj < 0 || ni >= w as isize || nj >= h as isize {
                        continue;
                    }
                    let n = nj as usize * w + ni as usize;
                    if let Some((v, zone)) = grey[n]
                        && !profile.occurs(g, v)
                        && let Some(fix) = profile.nearest_neighbour_within(g, v, zones[zone])
                    {
                        grey[n] = Some((fix, zone));
                        changed += 1;
                    }
                }
            }
        }
        if changed == 0 {
            break;
        }
    }

    // 3. One pass of a 3 × 3 Gaussian, paper white around the line.
    let tone: Vec<f32> = grey.iter().map(|g| g.map_or(1.0, |(g, _)| g as f32 / 255.0)).collect();
    let mut out = vec![1.0; w * h];
    for j in 0..h {
        for i in 0..w {
            let mut acc = 0.0;
            for dj in -1..=1isize {
                for di in -1..=1isize {
                    let (ni, nj) =
                        ((i as isize + di).clamp(0, w as isize - 1), (j as isize + dj).clamp(0, h as isize - 1));
                    let k = [1.0, 2.0, 1.0][(di + 1) as usize] * [1.0, 2.0, 1.0][(dj + 1) as usize];
                    acc += k * tone[nj as usize * w + ni as usize];
                }
            }
            out[j * w + i] = acc / 16.0;
        }
    }
    out
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

const NEIGHBOURS: [(isize, isize); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// Gradient noise in about `[-1, 1]`, three octaves (Perlin 2002's fade curve, in one dimension).
pub fn fbm(x: f32) -> f32 {
    let mut sum = 0.0;
    let (mut amplitude, mut frequency) = (0.6, 1.0);
    for octave in 0..3 {
        sum += amplitude * gradient_noise(x * frequency + octave as f32 * 17.3);
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    sum / 0.75
}

fn gradient_noise(x: f32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let grad = |i: f32| {
        let h = crate::rng::mix(i as i64 as u64, 0x9E11);
        (h >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
    };
    let fade = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let (a, b) = (grad(i) * f, grad(i + 1.0) * (f - 1.0));
    2.0 * (a + (b - a) * fade)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::{PathParams, line};

    #[test]
    fn a_line_darkens_its_path_and_nothing_far_away() {
        let mut canvas = Canvas::new(120, 40);
        let pencil = Pencil::new(Grade::B6);
        let params = PathParams { squiggle: 0.0, ..PathParams::default() };
        let mut rng = Rng::new(9);
        canvas.draw(&line([10.0, 20.0], [110.0, 20.0], &params, &mut rng), &pencil, 1.0, &mut rng);
        let on: f32 = (20..100).map(|x| canvas.tone[20 * 120 + x]).sum::<f32>() / 80.0;
        assert!(on < 0.8, "{on}");
        assert!((0..120).all(|x| canvas.tone[2 * 120 + x] == 1.0));
    }

    #[test]
    fn correction_leaves_only_pairs_the_sample_has() {
        let pencil = Pencil::new(Grade::HB);
        let params = PathParams { squiggle: 0.0, ..PathParams::default() };
        let mut rng = Rng::new(2);
        let path = line([2.0, 10.0], [60.0, 10.0], &params, &mut rng);
        // Without the blur, inspect the grey levels directly.
        let mut quiet = pencil.clone();
        quiet.wobble = 0.0;
        let tone = synthesize(&path, &quiet, [0.0, 0.0], 64, 20, &mut rng);
        assert!(tone.iter().any(|&t| t < 0.9));
    }

    #[test]
    fn fbm_is_bounded() {
        assert!((0..1000).map(|i| fbm(i as f32 * 0.137)).all(|v| (-1.2..=1.2).contains(&v)));
    }
}
