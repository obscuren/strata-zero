//! Frames as PNG files: the live window's (`STRATA_WINDOW_SHOT`) and the shot harness's.

use super::font::Fonts;
use super::raster::{Canvas, Frame};
use anyhow::{Context, Result};
use ratatui::buffer::Buffer;
use std::io::BufWriter;
use std::path::Path;

/// Text size for screenshots: the window's 15 px at a Retina scale of 2.
pub const SHOT_PX: f32 = 30.0;

/// `frame` as a PNG at `path`.
pub fn write_frame(path: &Path, frame: &Frame) -> Result<()> {
    let file =
        std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), frame.w as u32, frame.h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&frame.to_rgba())?;
    Ok(())
}

/// `buf` painted as the window would paint it, written to `<dir>/<name>.png`.
pub fn write_buffer(dir: &str, name: &str, buf: &Buffer) -> Result<()> {
    let mut canvas = Canvas::new(Fonts::load()?, SHOT_PX, 0, 0);
    canvas.paint_buffer(buf);
    write_frame(&Path::new(dir).join(format!("{name}.png")), &canvas.frame)
}
