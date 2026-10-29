//! Getting the frame on screen: through the GPU with the screen effects, or, where no GPU
//! will do (a VM, an odd driver), through softbuffer as the CPU painted it.

use super::backend::Damage;
use super::gpu::{Gpu, Params};
use super::raster::Frame;
use crate::app::ScreenFx;
use anyhow::{Context, Result, bail};
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::window::Window;

pub enum Presenter {
    Gpu(Box<GpuPresenter>),
    Cpu(CpuPresenter),
}

impl Presenter {
    /// The GPU when one answers for `window`, else softbuffer, with a note on stderr.
    pub fn new(window: Arc<Window>) -> Result<Presenter> {
        match GpuPresenter::new(window.clone()) {
            Ok(p) => Ok(Presenter::Gpu(Box::new(p))),
            Err(e) => {
                eprintln!("strata: no GPU ({e:#}); drawing without screen effects");
                Ok(Presenter::Cpu(CpuPresenter::new(window)?))
            }
        }
    }

    /// Shows `frame`, of which `damage` changed since the last present, through `fx`.
    /// False when the surface wasn't ready and the frame should be presented again.
    pub fn present(
        &mut self,
        frame: &Frame,
        damage: Option<Damage>,
        fx: ScreenFx,
        scale: f32,
    ) -> Result<bool> {
        match self {
            Presenter::Gpu(p) => p.present(frame, damage, fx, scale),
            Presenter::Cpu(p) => p.present(frame),
        }
    }

    /// `frame` as the window shows it, RGBA: through the effects on the GPU.
    pub fn capture(
        &mut self,
        frame: &Frame,
        fx: ScreenFx,
        scale: f32,
    ) -> Result<(u32, u32, Vec<u8>)> {
        match self {
            Presenter::Gpu(p) => {
                p.gpu.upload(frame, Some(all(frame)), scale);
                p.gpu.capture(Params::of(fx), scale)
            }
            Presenter::Cpu(_) => Ok((frame.w as u32, frame.h as u32, frame.to_rgba())),
        }
    }
}

fn all(frame: &Frame) -> Damage {
    Damage {
        x0: 0,
        y0: 0,
        x1: frame.w,
        y1: frame.h,
    }
}

pub struct GpuPresenter {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
}

impl GpuPresenter {
    fn new(window: Arc<Window>) -> Result<GpuPresenter> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let caps = surface.get_capabilities(&adapter);
        // An sRGB surface, so the effects work in linear light; else whatever there is.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or(caps.formats.first().copied())
            .context("the surface offers no format")?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        Ok(GpuPresenter {
            surface,
            config,
            gpu: Gpu::new(device, queue, format),
        })
    }

    fn present(
        &mut self,
        frame: &Frame,
        damage: Option<Damage>,
        fx: ScreenFx,
        scale: f32,
    ) -> Result<bool> {
        let (w, h) = (frame.w as u32, frame.h as u32);
        if w == 0 || h == 0 {
            return Ok(true);
        }
        if (self.config.width, self.config.height) != (w, h) {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(self.gpu.device(), &self.config);
        }
        // Uploaded whatever happens below, so a skipped present loses nothing.
        self.gpu.upload(frame, damage, scale);
        let (texture, stale) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => (t, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => (t, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(self.gpu.device(), &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => bail!("the surface failed validation"),
        };
        let view = texture.texture.create_view(&Default::default());
        self.gpu.render(&view, Params::of(fx), scale);
        self.gpu.queue().present(texture);
        if stale {
            self.surface.configure(self.gpu.device(), &self.config);
        }
        Ok(true)
    }
}

pub struct CpuPresenter {
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    /// The surface's size in pixels, to resize it only when the frame changes size.
    size: (usize, usize),
}

impl CpuPresenter {
    fn new(window: Arc<Window>) -> Result<CpuPresenter> {
        let context = softbuffer::Context::new(window.clone()).map_err(sb)?;
        let surface = softbuffer::Surface::new(&context, window).map_err(sb)?;
        Ok(CpuPresenter {
            surface,
            size: (0, 0),
        })
    }

    fn present(&mut self, frame: &Frame) -> Result<bool> {
        let (Some(w), Some(h)) = (
            NonZeroU32::new(frame.w as u32),
            NonZeroU32::new(frame.h as u32),
        ) else {
            return Ok(true);
        };
        if self.size != (frame.w, frame.h) {
            self.surface.resize(w, h).map_err(sb)?;
            self.size = (frame.w, frame.h);
        }
        let mut buf = self.surface.buffer_mut().map_err(sb)?;
        buf.copy_from_slice(&frame.px);
        buf.present().map_err(sb)?;
        Ok(true)
    }
}

/// softbuffer's error as anyhow's (it is not `Sync`).
fn sb(e: softbuffer::SoftBufferError) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}
