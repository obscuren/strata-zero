//! Frames as PNG files: the live window's (`STRATA_WINDOW_SHOT`) and the shot harness's.

use anyhow::{Context, Result};
use std::io::BufWriter;
use std::path::Path;

/// Text size for screenshots: the window's 15 px at a Retina scale of 2.
#[cfg(test)]
pub const SHOT_PX: f32 = 30.0;

/// `frame` as a PNG at `path`.
#[cfg(test)]
pub fn write_frame(path: &Path, frame: &super::raster::Frame) -> Result<()> {
    write_rgba(path, frame.w as u32, frame.h as u32, &frame.to_rgba())
}

/// RGBA bytes, `w` by `h`, as a PNG at `path`.
pub fn write_rgba(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    let file =
        std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

/// `buf` painted as the window would paint it, written to `<dir>/<name>.png`. With
/// `SHOT_FX` set, also through the GPU's Soft and CRT effects as `<name>.soft.png` and
/// `<name>.crt.png`, when this machine has a GPU.
#[cfg(test)]
pub fn write_buffer(dir: &str, name: &str, buf: &ratatui::buffer::Buffer) -> Result<()> {
    use super::gpu::{Gpu, Params};
    use crate::app::ScreenFx;
    let mut canvas = super::raster::Canvas::new(super::font::Fonts::load()?, SHOT_PX, 0, 0);
    canvas.paint_buffer(buf);
    let dir = Path::new(dir);
    write_frame(&dir.join(format!("{name}.png")), &canvas.frame)?;
    if std::env::var_os("SHOT_FX").is_some()
        && let Some(mut gpu) = Gpu::headless()
    {
        gpu.upload(&canvas.frame, None, 2.0);
        for fx in [ScreenFx::Soft, ScreenFx::Crt] {
            let (w, h, rgba) = gpu.capture(Params::of(fx), 2.0)?;
            let file = format!("{name}.{}.png", fx.label().to_lowercase());
            write_rgba(&dir.join(file), w, h, &rgba)?;
        }
    }
    Ok(())
}
