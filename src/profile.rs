//! The statistics of a pencil (AlMeraj et al. 2009, §3.4.1, "Phase A").
//!
//! A pencil is described by what a scan of one of its lines shows:
//!
//! - the grey levels along the centre of the line, as a histogram over `[min_mid, max_mid]`;
//! - the grey levels of the whole line (white paper excluded), over `[min_range, max_range]`;
//! - the grey-level co-occurrence matrix (CCM) of neighbouring pixels along the line, recorded as
//!   which pairs `(i, j)` occur at all;
//! - the line's width.
//!
//! [`Profile::from_sample`] measures these from a scan of a roughly vertical line on white paper,
//! as the paper does. The paper does not publish its measurements, so the built-in
//! [`Grade`]s are measured the same way from procedurally drawn reference lines; scans of real
//! pencils can replace them.

use crate::rng::{Rng, mix};

/// Grey levels at or above this count as white paper, not graphite.
pub const WHITE: u8 = 250;

/// Pairs within this many grey levels of a measured pair count as measured too.
pub const TOLERANCE: i32 = 2;

/// What a pencil leaves on paper.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// Histogram of the grey levels along the centre of the line.
    pub mid: [u32; 256],
    /// Histogram of the grey levels of the whole line.
    pub range: [u32; 256],
    /// `ccm[i]` bit `j`: grey `j` occurs next to grey `i` along the line.
    pub ccm: Vec<[u64; 4]>,
    /// Mean width of the line in pixels, at the resolution of the sample.
    pub width: f32,
}

impl Profile {
    /// Measures a pencil from a greyscale scan (`0` black, `255` white) of one line drawn roughly
    /// vertically, `width × height` pixels row by row.
    ///
    /// # Panics
    /// If `grey` does not hold `width × height` values or shows no graphite.
    pub fn from_sample(width: usize, height: usize, grey: &[u8]) -> Self {
        assert_eq!(grey.len(), width * height, "expected width × height grey levels");
        let mut profile = Profile { mid: [0; 256], range: [0; 256], ccm: vec![[0; 4]; 256], width: 0.0 };
        let (mut rows, mut covered) = (0usize, 0usize);
        for y in 0..height {
            let row = &grey[y * width..(y + 1) * width];
            let line: Vec<usize> = (0..width).filter(|&x| row[x] < WHITE).collect();
            if line.is_empty() {
                continue;
            }
            // The approximate centre: the graphite's centroid in this row.
            let total: f32 = line.iter().map(|&x| (255 - row[x]) as f32).sum();
            let centre = (line.iter().map(|&x| x as f32 * (255 - row[x]) as f32).sum::<f32>() / total).round();
            profile.mid[row[centre as usize] as usize] += 1;
            for &x in &line {
                profile.range[row[x] as usize] += 1;
            }
            rows += 1;
            covered += line.len();
        }
        assert!(rows > 0, "the sample shows no graphite");
        // Neighbours in the positive y direction: offset (Δx, Δy) = (0, +1).
        for y in 0..height - 1 {
            for x in 0..width {
                let (i, j) = (grey[y * width + x], grey[(y + 1) * width + x]);
                if i < WHITE && j < WHITE {
                    // A sample of a few thousand pairs leaves most of the 256 × 256 cells empty
                    // by chance; neighbouring levels are recorded too.
                    for di in -TOLERANCE..=TOLERANCE {
                        for dj in -TOLERANCE..=TOLERANCE {
                            let (a, b) = (i as i32 + di, j as i32 + dj);
                            if (0..WHITE as i32).contains(&a) && (0..WHITE as i32).contains(&b) {
                                profile.set(a as u8, b as u8);
                            }
                        }
                    }
                }
            }
        }
        profile.width = covered as f32 / rows as f32;
        profile
    }

    /// Records that grey `j` may follow grey `i`.
    pub fn set(&mut self, i: u8, j: u8) {
        self.ccm[i as usize][j as usize / 64] |= 1 << (j % 64);
    }

    /// Whether grey `j` occurs next to grey `i` in the sample.
    #[inline]
    pub fn occurs(&self, i: u8, j: u8) -> bool {
        self.ccm[i as usize][j as usize / 64] >> (j % 64) & 1 == 1
    }

    /// The grey nearest to `j` that occurs next to `i`, if any does.
    pub fn nearest_neighbour(&self, i: u8, j: u8) -> Option<u8> {
        (0..=255u16)
            .flat_map(|d| [j as i32 - d as i32, j as i32 + d as i32])
            .find_map(|g| (0..256).contains(&g).then_some(g as u8).filter(|&g| self.occurs(i, g)))
    }

    /// The grey nearest to `j` within `[lo, hi]` that occurs next to `i`, if any does.
    pub fn nearest_neighbour_within(&self, i: u8, j: u8, (lo, hi): (u8, u8)) -> Option<u8> {
        let j = j.clamp(lo, hi);
        (0..=(hi - lo) as i32)
            .flat_map(|d| [j as i32 - d, j as i32 + d])
            .find_map(|g| (lo as i32..=hi as i32).contains(&g).then_some(g as u8).filter(|&g| self.occurs(i, g)))
    }

    /// `[min, max]` of the levels in a histogram.
    pub fn bounds(histogram: &[u32; 256]) -> (u8, u8) {
        let min = histogram.iter().position(|&c| c > 0).unwrap_or(0) as u8;
        let max = histogram.iter().rposition(|&c| c > 0).unwrap_or(255) as u8;
        (min, max)
    }

    /// A grey level drawn from the centre histogram.
    pub fn sample_mid(&self, rng: &mut Rng) -> u8 {
        let total: u32 = self.mid.iter().sum();
        let mut pick = rng.below(total.max(1));
        for (g, &c) in self.mid.iter().enumerate() {
            if pick < c {
                return g as u8;
            }
            pick -= c;
        }
        Self::bounds(&self.mid).0
    }

    /// The built-in profile of a pencil grade.
    pub fn grade(grade: Grade) -> Self {
        let (w, h, grey) = grade.reference();
        Self::from_sample(w, h, &grey)
    }
}

/// Graphite pencil grades, hard to soft: the eight the paper scanned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum Grade {
    H2,
    H,
    F,
    HB,
    B,
    B3,
    B6,
    B8,
}

impl Grade {
    /// All grades, hard to soft.
    pub const ALL: [Grade; 8] = [Grade::H2, Grade::H, Grade::F, Grade::HB, Grade::B, Grade::B3, Grade::B6, Grade::B8];

    /// The grade's name as printed on a pencil.
    pub fn name(self) -> &'static str {
        match self {
            Grade::H2 => "2H",
            Grade::H => "H",
            Grade::F => "F",
            Grade::HB => "HB",
            Grade::B => "B",
            Grade::B3 => "3B",
            Grade::B6 => "6B",
            Grade::B8 => "8B",
        }
    }

    /// Line width in pixels at about 100 ppi, darkest graphite (0 none, 1 black), and how much
    /// the paper's tooth breaks the deposit up (0 smooth, 1 grainy).
    fn character(self) -> (f32, f32, f32) {
        match self {
            Grade::H2 => (2.6, 0.30, 0.75),
            Grade::H => (2.8, 0.36, 0.7),
            Grade::F => (3.0, 0.42, 0.65),
            Grade::HB => (3.2, 0.50, 0.6),
            Grade::B => (3.5, 0.58, 0.55),
            Grade::B3 => (4.0, 0.68, 0.5),
            Grade::B6 => (4.8, 0.78, 0.45),
            Grade::B8 => (5.4, 0.85, 0.4),
        }
    }

    /// A reference line for the grade: a vertical stroke, 2000 pixels long, at about 100 ppi.
    ///
    /// Graphite is deposited across the line with the bell-shaped profile the paper observed
    /// (its Figure 6), its pressure wanders slowly along the line, and it catches on the tooth of
    /// the paper: on a high fibre more graphite, in a pit less.
    pub fn reference(self) -> (usize, usize, Vec<u8>) {
        let (width, dark, grain) = self.character();
        let (w, h) = (32usize, 2000usize);
        let seed = mix(0xA1_3E2A, self as u64);
        let mut rng = Rng::new(seed);
        // Paper tooth: value noise at two scales, fixed per grade.
        let tooth = noise_field(w, h, &mut rng);
        let pressure: Vec<f32> = {
            let phases = [rng.unit() * 6.3, rng.unit() * 6.3];
            (0..h)
                .map(|y| 0.88 + 0.08 * (y as f32 / 70.0 + phases[0]).sin() + 0.04 * (y as f32 / 23.0 + phases[1]).sin())
                .collect()
        };
        let centre = w as f32 / 2.0;
        let sigma = width / 2.5;
        let grey = (0..w * h)
            .map(|c| {
                let (x, y) = ((c % w) as f32 + 0.5, c / w);
                let d = x - centre;
                let bell = (-(d * d) / (2.0 * sigma * sigma)).exp();
                // Graphite catches more on high tooth; the rest is the paper showing through.
                let catch = (1.0 - grain) + grain * (1.6 * tooth[c] - 0.3).clamp(0.0, 1.0);
                let deposit = (dark * bell * pressure[y] * catch).clamp(0.0, 1.0);
                let g = 255.0 * (1.0 - deposit);
                // Faint graphite reads as paper.
                if g >= WHITE as f32 { 255 } else { g.round() as u8 }
            })
            .collect();
        (w, h, grey)
    }
}

/// Value noise in `[0, 1]` summed over two scales, for paper tooth.
fn noise_field(w: usize, h: usize, rng: &mut Rng) -> Vec<f32> {
    let octave = |cell: usize, rng: &mut Rng| -> Vec<f32> {
        let (gw, gh) = (w / cell + 2, h / cell + 2);
        let lattice: Vec<f32> = (0..gw * gh).map(|_| rng.unit()).collect();
        (0..w * h)
            .map(|c| {
                let (x, y) = ((c % w) as f32 / cell as f32, (c / w) as f32 / cell as f32);
                let (i, j) = (x as usize, y as usize);
                let (fx, fy) = (smooth(x.fract()), smooth(y.fract()));
                let l = |i: usize, j: usize| lattice[j * gw + i];
                let top = l(i, j) * (1.0 - fx) + l(i + 1, j) * fx;
                let bottom = l(i, j + 1) * (1.0 - fx) + l(i + 1, j + 1) * fx;
                top * (1.0 - fy) + bottom * fy
            })
            .collect()
    };
    let (fine, coarse) = (octave(1, rng), octave(3, rng));
    fine.iter().zip(&coarse).map(|(a, b)| 0.6 * a + 0.4 * b).collect()
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_a_simple_sample() {
        // Three columns: white, a line of 100 and 120 alternating down the rows, white.
        let (w, h) = (3, 6);
        let grey: Vec<u8> =
            (0..w * h).map(|c| if c % w == 1 { if (c / w) % 2 == 0 { 100 } else { 120 } } else { 255 }).collect();
        let p = Profile::from_sample(w, h, &grey);
        assert_eq!(p.width, 1.0);
        assert_eq!(Profile::bounds(&p.mid), (100, 120));
        assert!(p.occurs(100, 120) && p.occurs(120, 100));
        assert!(!p.occurs(100, 100));
        assert_eq!(p.nearest_neighbour(100, 101), Some(118));
        assert_eq!(p.nearest_neighbour(100, 250), Some(122));
        assert_eq!(p.nearest_neighbour_within(100, 101, (119, 130)), Some(119));
    }

    #[test]
    fn softer_grades_are_darker_and_wider() {
        let hard = Profile::grade(Grade::H2);
        let soft = Profile::grade(Grade::B6);
        assert!(Profile::bounds(&soft.mid).0 < Profile::bounds(&hard.mid).0);
        assert!(soft.width > hard.width);
    }
}
