//! The window on the GPU: the frame the CPU painted, uploaded as a texture where it
//! changed, and drawn to the target through the screen effects in `effects.wgsl`.

use super::backend::Damage;
use super::raster::Frame;
use crate::app::ScreenFx;
use anyhow::{Context, Result};

/// How strong each effect is; zero is off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub glow: f32,
    pub scan: f32,
    pub vignette: f32,
    pub curve: f32,
}

impl Params {
    pub fn of(fx: ScreenFx) -> Params {
        match fx {
            ScreenFx::Off => Params {
                glow: 0.0,
                scan: 0.0,
                vignette: 0.0,
                curve: 0.0,
            },
            ScreenFx::Soft => Params {
                glow: 0.6,
                scan: 0.0,
                vignette: 0.25,
                curve: 0.0,
            },
            ScreenFx::Crt => Params {
                glow: 0.9,
                scan: 0.25,
                vignette: 0.45,
                curve: 0.03,
            },
        }
    }
}

/// The glow texture's format: float, so the glow adds light without banding.
const GLOW: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// The target's format (the surface's, or the capture's).
    format: wgpu::TextureFormat,
    glow_pipe: wgpu::RenderPipeline,
    comp_pipe: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    tex: Option<Textures>,
}

/// The textures sized to one frame size and glow factor, with their bind groups.
struct Textures {
    w: u32,
    h: u32,
    down: u32,
    frame: wgpu::Texture,
    glow_view: wgpu::TextureView,
    glow_bind: wgpu::BindGroup,
    comp_bind: wgpu::BindGroup,
}

impl Gpu {
    /// The pipelines for drawing to targets of `format`.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Gpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("effects"),
            source: wgpu::ShaderSource::Wgsl(include_str!("effects.wgsl").into()),
        });
        let pipe = |entry: &str, target: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let glow_pipe = pipe("fs_glow", GLOW);
        let comp_pipe = pipe("fs", format);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("effects"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("effects"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Gpu {
            device,
            queue,
            format,
            glow_pipe,
            comp_pipe,
            sampler,
            uniforms,
            tex: None,
        }
    }

    /// A GPU with no window, drawing to `Bgra8UnormSrgb` (for tests and shots); `None`
    /// when this machine has no adapter.
    #[cfg(test)]
    pub fn headless() -> Option<Gpu> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        Some(Gpu::new(device, queue, wgpu::TextureFormat::Bgra8UnormSrgb))
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// The frame's texture format: its `0x00RRGGBB` pixels are BGRA bytes in memory, and
    /// sRGB when the target is, so the glow adds light rather than gamma values.
    fn frame_format(&self) -> wgpu::TextureFormat {
        if self.format.is_srgb() {
            wgpu::TextureFormat::Bgra8UnormSrgb
        } else {
            wgpu::TextureFormat::Bgra8Unorm
        }
    }

    /// Copies `damage` of `frame` to the GPU, or all of it when the textures are new.
    pub fn upload(&mut self, frame: &Frame, damage: Option<Damage>, scale: f32) {
        if frame.w == 0 || frame.h == 0 {
            return;
        }
        let down = if scale >= 1.5 { 4 } else { 2 };
        let (w, h) = (frame.w as u32, frame.h as u32);
        let fresh = !self
            .tex
            .as_ref()
            .is_some_and(|t| (t.w, t.h, t.down) == (w, h, down));
        if fresh {
            self.tex = Some(self.textures(w, h, down));
        }
        let all = Damage {
            x0: 0,
            y0: 0,
            x1: frame.w,
            y1: frame.h,
        };
        let Some(d) = (if fresh { Some(all) } else { damage }) else {
            return;
        };
        let (x1, y1) = (d.x1.min(frame.w), d.y1.min(frame.h));
        if d.x0 >= x1 || d.y0 >= y1 {
            return;
        }
        let tex = self.tex.as_ref().expect("made above");
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex.frame,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: d.x0 as u32,
                    y: d.y0 as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&frame.px),
            wgpu::TexelCopyBufferLayout {
                offset: ((d.y0 * frame.w + d.x0) * 4) as u64,
                bytes_per_row: Some(w * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: (x1 - d.x0) as u32,
                height: (y1 - d.y0) as u32,
                depth_or_array_layers: 1,
            },
        );
    }

    fn textures(&self, w: u32, h: u32, down: u32) -> Textures {
        let texture = |label, w, h, format, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let frame = texture(
            "frame",
            w,
            h,
            self.frame_format(),
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let glow = texture(
            "glow",
            w.div_ceil(down),
            h.div_ceil(down),
            GLOW,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let frame_view = frame.create_view(&Default::default());
        let glow_view = glow.create_view(&Default::default());
        let bind = |pipe: &wgpu::RenderPipeline, with_glow: bool| {
            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&frame_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.uniforms.as_entire_binding(),
                },
            ];
            if with_glow {
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&glow_view),
                });
            }
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipe.get_bind_group_layout(0),
                entries: &entries,
            })
        };
        let glow_bind = bind(&self.glow_pipe, false);
        let comp_bind = bind(&self.comp_pipe, true);
        Textures {
            w,
            h,
            down,
            frame,
            glow_view,
            glow_bind,
            comp_bind,
        }
    }

    /// Draws the uploaded frame to `target` through the effects in `fx`, at the window's
    /// `scale`. Nothing before the first upload.
    pub fn render(&self, target: &wgpu::TextureView, fx: Params, scale: f32) {
        let Some(tex) = &self.tex else {
            return;
        };
        let u: [f32; 8] = [
            tex.w as f32,
            tex.h as f32,
            scale,
            fx.glow,
            fx.scan,
            fx.vignette,
            fx.curve,
            tex.down as f32,
        ];
        self.queue
            .write_buffer(&self.uniforms, 0, bytemuck::cast_slice(&u));
        let mut enc = self.device.create_command_encoder(&Default::default());
        let mut pass = |view: &wgpu::TextureView, pipe: &wgpu::RenderPipeline, bind| {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(pipe);
            rp.set_bind_group(0, bind, &[]);
            rp.draw(0..3, 0..1);
        };
        if fx.glow > 0.0 {
            pass(&tex.glow_view, &self.glow_pipe, &tex.glow_bind);
        }
        pass(target, &self.comp_pipe, &tex.comp_bind);
        self.queue.submit([enc.finish()]);
    }

    /// The frame drawn through `fx` into a texture and read back: width, height and RGBA
    /// bytes, as the window would show them.
    pub fn capture(&self, fx: Params, scale: f32) -> Result<(u32, u32, Vec<u8>)> {
        let tex = self.tex.as_ref().context("nothing uploaded yet")?;
        let (w, h) = (tex.w, tex.h);
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.render(&target.create_view(&Default::default()), fx, scale);
        let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: None,
                },
            },
            target.size(),
        );
        self.queue.submit([enc.finish()]);
        buf.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .context("waiting for the capture")?;
        let data = buf
            .get_mapped_range(..)
            .map_err(|e| anyhow::anyhow!("reading the capture: {e:?}"))?;
        let bgra = matches!(
            self.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            for px in data[y * row as usize..][..w as usize * 4].chunks_exact(4) {
                if bgra {
                    out.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    out.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        Ok((w, h, out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::raster::BG;

    fn gpu() -> Option<Gpu> {
        let g = Gpu::headless();
        if g.is_none() {
            eprintln!("skipped: no GPU adapter");
        }
        g
    }

    fn frame(w: usize, h: usize, px: impl Fn(usize, usize) -> u32) -> Frame {
        let mut f = Frame::new(w, h);
        for y in 0..h {
            for x in 0..w {
                f.px[y * w + x] = px(x, y);
            }
        }
        f
    }

    fn at(rgba: &[u8], w: u32, x: usize, y: usize) -> [u8; 3] {
        let i = (y * w as usize + x) * 4;
        [rgba[i], rgba[i + 1], rgba[i + 2]]
    }

    #[test]
    fn off_is_the_frame() {
        let Some(mut g) = gpu() else { return };
        let f = frame(64, 40, |x, y| {
            ((x * 4) as u32) << 16 | ((y * 6) as u32) << 8 | 0x80
        });
        g.upload(&f, None, 1.0);
        let (w, h, rgba) = g.capture(Params::of(ScreenFx::Off), 1.0).unwrap();
        assert_eq!((w, h), (64, 40));
        let worst = rgba
            .iter()
            .zip(f.to_rgba())
            .map(|(a, b)| a.abs_diff(b))
            .max()
            .unwrap();
        assert!(worst <= 1, "Off differs from the frame by {worst}");
    }

    #[test]
    fn damage_uploads_only_its_rectangle() {
        let Some(mut g) = gpu() else { return };
        let mut f = frame(32, 32, |_, _| 0x000000);
        g.upload(&f, None, 1.0);
        f.px[5 * 32 + 5] = 0xff0000;
        f.px[20 * 32 + 20] = 0x00ff00;
        let d = Damage {
            x0: 4,
            y0: 4,
            x1: 8,
            y1: 8,
        };
        g.upload(&f, Some(d), 1.0);
        let (w, _, rgba) = g.capture(Params::of(ScreenFx::Off), 1.0).unwrap();
        assert_eq!(at(&rgba, w, 5, 5), [255, 0, 0], "inside the damage");
        assert_eq!(at(&rgba, w, 20, 20), [0, 0, 0], "outside it, not uploaded");
    }

    #[test]
    fn soft_brightens_around_neon() {
        let Some(mut g) = gpu() else { return };
        let neon = |x: usize, y: usize| (28..36).contains(&x) && (28..36).contains(&y);
        let f = frame(64, 64, |x, y| if neon(x, y) { 0xff00c8 } else { BG });
        g.upload(&f, None, 1.0);
        let lum = |p: [u8; 3]| p.iter().map(|&c| c as u32).sum::<u32>();
        let (w, _, off) = g.capture(Params::of(ScreenFx::Off), 1.0).unwrap();
        let (_, _, soft) = g.capture(Params::of(ScreenFx::Soft), 1.0).unwrap();
        assert!(
            lum(at(&soft, w, 39, 32)) > lum(at(&off, w, 39, 32)) + 6,
            "a halo beside it"
        );
        assert!(
            lum(at(&soft, w, 60, 4)) <= lum(at(&off, w, 60, 4)),
            "none far off"
        );
    }

    #[test]
    fn crt_keeps_the_edge_middles() {
        let Some(mut g) = gpu() else { return };
        let f = frame(64, 40, |x, _| match x {
            0 => 0xff0000,
            63 => 0x00ff00,
            _ => BG,
        });
        g.upload(&f, None, 1.0);
        let (w, _, crt) = g.capture(Params::of(ScreenFx::Crt), 1.0).unwrap();
        let [r, gr, b] = at(&crt, w, 0, 20);
        assert!(
            r > gr && r > b && r > 60,
            "the left column is still there: {r},{gr},{b}"
        );
        let [r, gr, b] = at(&crt, w, 63, 20);
        assert!(gr > r && gr > b && gr > 60, "and the right: {r},{gr},{b}");
        assert_eq!(at(&crt, w, 0, 0), [0, 0, 0], "the corners round off");
    }

    #[test]
    fn crt_draws_scanlines() {
        let Some(mut g) = gpu() else { return };
        let f = frame(64, 64, |_, _| 0x808080);
        g.upload(&f, None, 1.0);
        let (w, _, crt) = g.capture(Params::of(ScreenFx::Crt), 1.0).unwrap();
        let rows: Vec<u8> = (30..34).map(|y| at(&crt, w, 32, y)[0]).collect();
        let (lo, hi) = (rows.iter().min().unwrap(), rows.iter().max().unwrap());
        assert!(hi - lo > 10, "rows alternate light and dark: {rows:?}");
    }
}
