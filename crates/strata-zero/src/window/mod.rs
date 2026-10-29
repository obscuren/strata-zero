//! The game in its own window instead of a terminal (`strata --window`, the `window`
//! feature): a ratatui backend that paints the cell grid into pixels, driven by a winit
//! loop that runs the same `Runner` frame as the terminal does.

pub mod backend;
pub mod font;
pub mod keys;
pub mod png;
pub mod raster;
pub mod shapes;

use crate::app::{AppState, FRAME, MIN_H, MIN_W, Runner, Screen};
use anyhow::{Context, Result};
use backend::PixelBackend;
use crossterm::event::KeyEvent;
use font::Fonts;
use keys::{Input, Shortcut};
use ratatui::Terminal;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Fullscreen, Window, WindowId};

/// Text height in logical pixels at zoom 1.
const TEXT_PX: f32 = 15.0;
/// The grid the window opens with, clamped to the screen.
const OPEN: (u16, u16) = (120, 40);
/// The frame `STRATA_WINDOW_SHOT` writes: 1.5 s in, after the title has typed out.
const SHOT_FRAME: u64 = 45;
/// How far Cmd+= and Cmd+- step the text size, and its bounds.
const ZOOM_STEP: f32 = 1.1;
const ZOOM_RANGE: (f32, f32) = (0.6, 2.5);

/// Runs the game in a window until it quits. Started from inside an `.app`, the working
/// folder becomes `app_support_dir()` first, so `cities/` is saved there.
pub fn run(screen: Box<dyn Screen>, st: AppState) -> Result<()> {
    if in_bundle() {
        let dir = app_support_dir();
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        std::env::set_current_dir(&dir)?;
    }
    let fonts = Fonts::load()?;
    let event_loop = EventLoop::new()?;
    let mut app = App {
        runner: Runner::new(screen, st),
        fonts: Some(fonts),
        live: None,
        keys: Vec::new(),
        mods: ModifiersState::empty(),
        zoom: 1.0,
        regridded: false,
        next: Instant::now(),
        frames: 0,
        shot: std::env::var_os("STRATA_WINDOW_SHOT").map(PathBuf::from),
        error: None,
    };
    event_loop.run_app(&mut app)?;
    app.error.map_or(Ok(()), Err)
}

/// Whether this executable sits inside a macOS `.app` bundle.
pub fn in_bundle() -> bool {
    std::env::current_exe().is_ok_and(|p| is_bundle_path(&p))
}

fn is_bundle_path(p: &Path) -> bool {
    p.to_string_lossy().contains(".app/Contents/MacOS/")
}

/// Where a bundled game keeps its files: `~/Library/Application Support/Strata Zero`.
pub fn app_support_dir() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    Path::new(&home).join("Library/Application Support/Strata Zero")
}

/// The window, its surface and the terminal drawing into it, once the window exists.
struct Live {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    /// The surface's size in pixels, to resize it only when the frame changes size.
    surface_size: (usize, usize),
    term: Terminal<PixelBackend>,
}

struct App {
    runner: Runner,
    /// Handed to the backend when the window is made.
    fonts: Option<Fonts>,
    live: Option<Live>,
    /// Keys pressed since the last frame.
    keys: Vec<KeyEvent>,
    mods: ModifiersState,
    zoom: f32,
    /// The grid was rebuilt (a resize, a new scale or zoom): the next frame clears the
    /// terminal so ratatui resends every cell.
    regridded: bool,
    /// When the next frame is due.
    next: Instant,
    frames: u64,
    shot: Option<PathBuf>,
    error: Option<anyhow::Error>,
}

impl App {
    fn open(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let fonts = self.fonts.take().context("the window is made once")?;
        // A first guess in logical pixels; `fit` sets the exact sizes once the window's
        // scale is known, as cell sizes don't scale exactly (SF Mono is 9 px wide at
        // 15 px and 19 px at 30).
        let cell = fonts.metrics(TEXT_PX);
        let attrs = Window::default_attributes()
            .with_title("Strata Zero")
            .with_inner_size(LogicalSize::new(
                (cell.w * OPEN.0 as usize) as f64,
                (cell.h * OPEN.1 as usize) as f64,
            ));
        let window = Rc::new(el.create_window(attrs)?);
        let cell = fonts.metrics(TEXT_PX * window.scale_factor() as f32);
        fit(&window, cell, true);
        let context = softbuffer::Context::new(window.clone()).map_err(sb)?;
        let surface = softbuffer::Surface::new(&context, window.clone()).map_err(sb)?;
        let px = self.text_px(&window);
        let inner = window.inner_size();
        let backend = PixelBackend::new(fonts, px, inner.width as usize, inner.height as usize);
        self.live = Some(Live {
            window,
            surface,
            surface_size: (0, 0),
            term: Terminal::new(backend)?,
        });
        Ok(())
    }

    /// The text height in physical pixels for `window`'s scale and the zoom.
    fn text_px(&self, window: &Window) -> f32 {
        TEXT_PX * window.scale_factor() as f32 * self.zoom
    }

    /// One frame: the screens update with the keys gathered since the last, and draw.
    fn tick(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        if std::mem::take(&mut self.regridded) {
            live.term.clear()?;
        }
        let keys = std::mem::take(&mut self.keys);
        if !self.runner.frame(&mut live.term, &keys)? {
            el.exit();
        }
        self.frames += 1;
        if live.term.backend_mut().take_dirty() {
            live.window.request_redraw();
        }
        if self.frames == SHOT_FRAME
            && let Some(path) = &self.shot
        {
            png::write_frame(path, live.term.backend().frame())?;
        }
        Ok(())
    }

    fn present(&mut self) -> Result<()> {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        let frame = live.term.backend().frame();
        let (Some(w), Some(h)) = (
            NonZeroU32::new(frame.w as u32),
            NonZeroU32::new(frame.h as u32),
        ) else {
            return Ok(());
        };
        if live.surface_size != (frame.w, frame.h) {
            live.surface.resize(w, h).map_err(sb)?;
            live.surface_size = (frame.w, frame.h);
        }
        let mut buf = live.surface.buffer_mut().map_err(sb)?;
        buf.copy_from_slice(&frame.px);
        buf.present().map_err(sb)?;
        Ok(())
    }

    fn shortcut(&mut self, el: &ActiveEventLoop, s: Shortcut) {
        let Some(live) = self.live.as_ref() else {
            return;
        };
        self.zoom = match s {
            Shortcut::Quit => return el.exit(),
            Shortcut::Fullscreen => {
                let full = live.window.fullscreen().is_some();
                live.window
                    .set_fullscreen((!full).then_some(Fullscreen::Borderless(None)));
                return;
            }
            Shortcut::Bigger => self.zoom * ZOOM_STEP,
            Shortcut::Smaller => self.zoom / ZOOM_STEP,
            Shortcut::ResetSize => 1.0,
        }
        .clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
        self.rescale();
    }

    /// The text size again from the window's scale and the zoom.
    fn rescale(&mut self) {
        let Some(live) = self.live.as_ref() else {
            return;
        };
        let px = self.text_px(&live.window);
        let live = self.live.as_mut().expect("checked above");
        if live.term.backend().px() != px {
            live.term.backend_mut().set_px(px);
            live.window.request_redraw();
            self.regridded = true;
        }
    }

    fn fail(&mut self, el: &ActiveEventLoop, e: anyhow::Error) {
        self.error.get_or_insert(e);
        el.exit();
    }
}

/// The window's minimum, 100×30 cells of `cell` (the cell at the window's scale and
/// zoom 1), and when `open` its size, 120×40 cells within 90% of its screen.
fn fit(window: &Window, cell: font::CellMetrics, open: bool) {
    let cells = |cols: u16, rows: u16| {
        PhysicalSize::new(
            (cell.w * cols as usize) as u32,
            (cell.h * rows as usize) as u32,
        )
    };
    window.set_min_inner_size(Some(cells(MIN_W, MIN_H)));
    if open {
        let mut size = cells(OPEN.0, OPEN.1);
        if let Some(m) = window.current_monitor() {
            size.width = size.width.min(m.size().width * 9 / 10);
            size.height = size.height.min(m.size().height * 9 / 10);
        }
        let _ = window.request_inner_size(size);
    }
}

/// softbuffer's error as anyhow's (it is not `Sync`).
fn sb(e: softbuffer::SoftBufferError) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.live.is_none()
            && let Err(e) = self.open(el)
        {
            self.fail(el, e);
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(live) = self.live.as_mut() {
                    let backend = live.term.backend_mut();
                    backend.resize(size.width as usize, size.height as usize);
                    live.window.request_redraw();
                    self.regridded = true;
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(live) = self.live.as_ref() {
                    let cell = live
                        .term
                        .backend()
                        .metrics_at(TEXT_PX * scale_factor as f32);
                    fit(&live.window, cell, false);
                }
                self.rescale();
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                let pressed = event.state == ElementState::Pressed;
                let bare = event.key_without_modifiers();
                match keys::translate(pressed, &event.logical_key, &bare, self.mods) {
                    Input::Game(k) => self.keys.push(k),
                    Input::Window(s) => self.shortcut(el, s),
                    Input::Nothing => {}
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.present() {
                    self.fail(el, e);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next {
            if let Err(e) = self.tick(el) {
                self.fail(el, e);
            }
            self.next += FRAME;
            if self.next < now {
                self.next = now + FRAME;
            }
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_bundle_detects_app_paths() {
        assert!(is_bundle_path(Path::new(
            "/Applications/Strata Zero.app/Contents/MacOS/strata"
        )));
        assert!(!is_bundle_path(Path::new("target/smoke/release/strata")));
        assert!(!is_bundle_path(Path::new("/Users/x/my.app.notes/strata")));
    }

    /// The title screen as the window paints it, to `SHOT_DIR/title.png`. The street, the
    /// deck and the ATTRIBUTES sheet come out of the shot harness (`screens::shots`) as
    /// PNGs too when this feature is on.
    #[test]
    #[ignore]
    fn window_shots() {
        let Ok(dir) = std::env::var("SHOT_DIR") else {
            return;
        };
        let mut st = AppState::new();
        let mut title = crate::screens::initial();
        for _ in 0..90 {
            st.tick += 1;
            title.update(&mut st, &[]);
        }
        let buf = crate::screens::shots::draw_stack(&st, &[&title], (120, 40));
        png::write_buffer(&dir, "title", &buf).unwrap();
    }

    /// The app icon, to `SHOT_DIR/icon.png` (1024 × 1024): "SZ" in the title's block
    /// letters on a rounded square of the game's background with a neon edge.
    /// `scripts/macos-app.sh --icon` turns it into `macos/AppIcon.icns`.
    #[test]
    #[ignore]
    fn app_icon() {
        use crate::render::Rgb;
        use crate::ui::logo::{self, Logo};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::widgets::Widget;
        let Ok(dir) = std::env::var("SHOT_DIR") else {
            return;
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, logo::width("SZ"), logo::ROWS));
        let logo = Logo {
            text: "SZ",
            tick: 0,
            accent: Rgb(255, 0, 200),
            steady: true,
        };
        logo.render(buf.area, &mut buf);
        let mut canvas = raster::Canvas::new(Fonts::load().unwrap(), 80.0, 0, 0);
        canvas.paint_buffer(&buf);
        let art = &canvas.frame;

        // A rounded square 824 wide in a 1024 canvas, as macOS icons are drawn.
        const SIZE: usize = 1024;
        let (inset, radius, edge) = (100.0, 180.0, 10.0);
        let mut rgba = vec![0u8; SIZE * SIZE * 4];
        let (ox, oy) = ((SIZE - art.w) / 2, (SIZE - art.h) / 2);
        for y in 0..SIZE {
            for x in 0..SIZE {
                // Signed distance to the rounded square's outline, negative inside.
                let half = SIZE as f32 / 2.0 - inset;
                let (dx, dy) = (
                    (x as f32 + 0.5 - SIZE as f32 / 2.0).abs() - (half - radius),
                    (y as f32 + 0.5 - SIZE as f32 / 2.0).abs() - (half - radius),
                );
                let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
                let d = outside + dx.max(dy).min(0.0) - radius;
                let alpha = (0.5 - d).clamp(0.0, 1.0);
                if alpha == 0.0 {
                    continue;
                }
                let mut c = raster::BG;
                if (x as isize - ox as isize) >= 0
                    && x - ox < art.w
                    && (y as isize - oy as isize) >= 0
                    && y - oy < art.h
                {
                    c = art.px[(y - oy) * art.w + (x - ox)];
                }
                if d > -edge {
                    c = 0x00e5ff;
                }
                let i = (y * SIZE + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[
                    (c >> 16) as u8,
                    (c >> 8) as u8,
                    c as u8,
                    (alpha * 255.0) as u8,
                ]);
            }
        }
        let path = Path::new(&dir).join("icon.png");
        let file = std::fs::File::create(path).unwrap();
        let mut enc = ::png::Encoder::new(std::io::BufWriter::new(file), SIZE as u32, SIZE as u32);
        enc.set_color(::png::ColorType::Rgba);
        enc.set_depth(::png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    }
}
