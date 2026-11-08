//! Draws a line in every grade, straight and curved and dashed, and writes `out/lines.png`.
//!
//!     cargo run --release --example lines

use std::fs::{self, File};
use std::io::BufWriter;

use almeraj_pencil::{Canvas, Dashes, Grade, PathParams, Pencil, Point, Rng, dashes, path};

fn main() -> std::io::Result<()> {
    let (w, row) = (900usize, 48usize);
    let rows = Grade::ALL.len() + 4;
    let mut canvas = Canvas::new(w, row * rows);
    let params = PathParams::default();
    let mut rng = Rng::new(2009);
    for (k, &grade) in Grade::ALL.iter().enumerate() {
        let pencil = Pencil::new(grade);
        let y = (k * row + row / 2) as f32;
        // A long line, and a short one beside it.
        let long = path::line([20.0, y], [600.0, y], &params, &mut rng);
        canvas.draw(&long, &pencil, 1.0, &mut rng);
        let short = path::line([640.0, y], [860.0, y + 4.0], &params, &mut rng);
        canvas.draw(&short, &pencil, 1.0, &mut rng);
    }
    let pencil = Pencil::new(Grade::B);
    // Dashed: the three schemes.
    for (k, scheme) in [Dashes::Single, Dashes::Double, Dashes::Triple].into_iter().enumerate() {
        let y = ((Grade::ALL.len() + k) * row + row / 2) as f32;
        let stroke = path::line([20.0, y], [860.0, y], &params, &mut rng);
        for (dash, pressure) in dashes(&stroke, scheme, &params, &mut rng) {
            canvas.draw(&dash, &pencil, pressure, &mut rng);
        }
    }
    // A curve: a wave across the last row.
    let y = ((rows - 1) * row + row / 2) as f32;
    let wave: Vec<Point> = (0..=400).map(|i| [20.0 + i as f32 * 2.1, y + 12.0 * (i as f32 / 40.0).sin()]).collect();
    let curved = path::along(&wave, &params, &mut rng);
    canvas.draw(&curved, &Pencil::new(Grade::B3), 1.0, &mut rng);

    fs::create_dir_all("out")?;
    let file = BufWriter::new(File::create("out/lines.png")?);
    let mut encoder = png::Encoder::new(file, w as u32, (row * rows) as u32);
    encoder.set_color(png::ColorType::Grayscale);
    encoder
        .write_header()
        .map_err(std::io::Error::other)?
        .write_image_data(&canvas.to_grey8())
        .map_err(std::io::Error::other)?;
    println!("wrote out/lines.png");
    Ok(())
}
