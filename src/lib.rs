//! Hand-drawn pencil lines: a Rust implementation of Zainab AlMeraj, Brian Wyvill, Tobias Isenberg,
//! Amy A. Gooch and Richard Guy, *Automatically Mimicking Unique Hand-Drawn Pencil Lines*
//! (Computers & Graphics 2009), and of AlMeraj's thesis *Mimicking Human-Drawn Pencil Lines*
//! (University of Victoria, 2008).
//!
//! A line is a **path** and a **texture**:
//!
//! - [`path`](mod@path): the hand moves along the minimum-jerk trajectory of Flash and Hogan (1985), with
//!   a random sideways *squiggle* at each control point and a Catmull–Rom spline through them.
//!   Extended here from straight lines to any curve ([`path::along`]).
//! - [`line`](mod@line): graphite fills the line with greys from the pencil's histograms, falling off across
//!   it like a bell; a grey-level co-occurrence matrix measured from a real line then reorders
//!   them into a pencil-like texture, and a 3 × 3 Gaussian finishes it. The width wanders along
//!   the line.
//! - [`profile`]: the statistics of a pencil grade, measured from a scanned line
//!   ([`Profile::from_sample`]) or built in ([`Grade`]).
//! - [`dash`]: the paper's three dashing schemes.
//!
//! Every line is different, and the same seed always draws the same one. No dependencies.
//!

#![warn(missing_docs)]

pub mod path;
pub mod profile;
pub mod rng;

pub use path::{PathParams, Point};
pub use profile::{Grade, Profile};
pub use rng::Rng;
