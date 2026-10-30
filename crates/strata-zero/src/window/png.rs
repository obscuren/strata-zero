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
    let mut canvas = super::raster::Canvas::new(super::font::Fonts::load(), SHOT_PX, 0, 0);
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

/// A square RGBA image `from` pixels a side, scaled down to `to`: each pixel the area
/// average of the pixels it covers, colour weighted by alpha so transparent pixels don't
/// darken the edges.
#[cfg(test)]
pub fn downsample(rgba: &[u8], from: usize, to: usize) -> Vec<u8> {
    // For each destination row (or column), the source rows it covers and how much of
    // each.
    let scale = from as f64 / to as f64;
    let spans: Vec<Vec<(usize, f64)>> = (0..to)
        .map(|o| {
            let (a, b) = (o as f64 * scale, (o + 1) as f64 * scale);
            (a.floor() as usize..(b.ceil() as usize).min(from))
                .map(|s| (s, b.min(s as f64 + 1.0) - a.max(s as f64)))
                .collect()
        })
        .collect();
    let mut out = vec![0u8; to * to * 4];
    for (oy, ys) in spans.iter().enumerate() {
        for (ox, xs) in spans.iter().enumerate() {
            let (mut sum, mut area) = ([0.0f64; 4], 0.0);
            for &(y, wy) in ys {
                for &(x, wx) in xs {
                    let p = &rgba[(y * from + x) * 4..][..4];
                    let (w, a) = (wx * wy, p[3] as f64 / 255.0);
                    for c in 0..3 {
                        sum[c] += p[c] as f64 * a * w;
                    }
                    sum[3] += a * w;
                    area += w;
                }
            }
            let o = &mut out[(oy * to + ox) * 4..][..4];
            if sum[3] > 0.0 {
                for c in 0..3 {
                    o[c] = (sum[c] / sum[3]).round() as u8;
                }
            }
            o[3] = (sum[3] / area * 255.0).round() as u8;
        }
    }
    out
}

/// Square RGBA images (`side`, pixels) as a Windows `.ico` at `path`, each stored as a
/// PNG, which Windows reads at every size.
#[cfg(test)]
pub fn write_ico(path: &Path, images: &[(u32, Vec<u8>)]) -> Result<()> {
    let mut pngs = Vec::new();
    for (side, rgba) in images {
        let mut bytes = Vec::new();
        let mut enc = png::Encoder::new(&mut bytes, *side, *side);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(rgba)?;
        pngs.push(bytes);
    }
    // ICONDIR, then an ICONDIRENTRY per image, then the images.
    let mut out = vec![0, 0, 1, 0];
    out.extend((images.len() as u16).to_le_bytes());
    let mut at = 6 + 16 * images.len();
    for ((side, _), png) in images.iter().zip(&pngs) {
        // A side of 256 is written as 0.
        let side = if *side >= 256 { 0 } else { *side as u8 };
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes()); // colour planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((png.len() as u32).to_le_bytes());
        out.extend((at as u32).to_le_bytes());
        at += png.len();
    }
    for png in &pngs {
        out.extend(png);
    }
    std::fs::write(path, out).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ico_lists_each_image() {
        let dir = std::env::temp_dir().join(format!("strata-ico-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.ico");
        let images = [16u32, 256].map(|s| (s, vec![255u8; (s * s * 4) as usize]));
        write_ico(&path, &images).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(bytes[..6], [0, 0, 1, 0, 2, 0]);
        for (i, width) in [16u8, 0].into_iter().enumerate() {
            let entry = &bytes[6 + 16 * i..22 + 16 * i];
            assert_eq!((entry[0], entry[1]), (width, width));
            let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let at = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            assert_eq!(bytes[at..at + 4], [0x89, b'P', b'N', b'G']);
            assert!(at + size <= bytes.len());
        }
    }

    #[test]
    fn downsample_averages() {
        // Left half red, right half blue, opaque.
        let mut img = Vec::new();
        for _ in 0..4 {
            for x in 0..4 {
                img.extend(if x < 2 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                });
            }
        }
        let small = downsample(&img, 4, 2);
        assert_eq!(small[..8], [255, 0, 0, 255, 0, 0, 255, 255]);

        // Half transparent green stays green.
        let img: Vec<u8> = [0, 200, 0, 128].repeat(16);
        let small = downsample(&img, 4, 2);
        assert_eq!(small[1], 200);
        assert!(small[3].abs_diff(128) <= 1, "{}", small[3]);

        // Transparent pixels don't darken an opaque neighbour.
        let img: Vec<u8> = [[200, 200, 200, 255], [0, 0, 0, 0]].concat().repeat(2);
        let small = downsample(&img, 2, 1);
        assert_eq!(small[..3], [200, 200, 200]);
        assert!(small[3].abs_diff(128) <= 1);
    }
}
