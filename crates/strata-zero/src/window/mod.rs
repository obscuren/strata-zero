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
use winit::dpi::LogicalSize;
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
    /// When the next frame is due.
    next: Instant,
    frames: u64,
    shot: Option<PathBuf>,
    error: Option<anyhow::Error>,
}

impl App {
    fn open(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let fonts = self.fonts.take().context("the window is made once")?;
        let cell = fonts.metrics(TEXT_PX);
        let cells = |cols: u16, rows: u16| {
            LogicalSize::new(
                (cell.w * cols as usize) as f64,
                (cell.h * rows as usize) as f64,
            )
        };
        let mut size = cells(OPEN.0, OPEN.1);
        if let Some(m) = el.primary_monitor() {
            let screen = m.size().to_logical::<f64>(m.scale_factor());
            size.width = size.width.min(screen.width * 0.9);
            size.height = size.height.min(screen.height * 0.9);
        }
        let attrs = Window::default_attributes()
            .with_title("Strata Zero")
            .with_inner_size(size)
            .with_min_inner_size(cells(MIN_W, MIN_H));
        let window = Rc::new(el.create_window(attrs)?);
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
        }
    }

    fn fail(&mut self, el: &ActiveEventLoop, e: anyhow::Error) {
        self.error.get_or_insert(e);
        el.exit();
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
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => self.rescale(),
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
}
