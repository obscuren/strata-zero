//! The game in its own window instead of a terminal (`strata --window`, the `window`
//! feature): a ratatui backend that paints the cell grid into pixels, driven by a winit
//! loop that runs the same `Runner` frame as the terminal does, and presented through
//! the GPU with the screen effects chosen in Settings.

pub mod backend;
#[cfg(all(windows, feature = "app"))]
pub mod fatal;
pub mod font;
pub mod gpu;
pub mod keys;
pub mod platform;
pub mod png;
pub mod present;
pub mod raster;
pub mod shapes;

use crate::app::{AppState, FRAME, MIN_H, MIN_W, Runner, Screen, ScreenFx};
use anyhow::{Context, Result};
use backend::{Damage, PixelBackend};
use crossterm::event::KeyEvent;
use font::Fonts;
use keys::{Input, Shortcut};
use platform::Platform;
use present::{Presenter, Shown};
use ratatui::Terminal;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};

/// Text height in logical pixels at zoom 1.
const TEXT_PX: f32 = 15.0;
/// The grid the window opens with, clamped to the screen.
const OPEN: (u16, u16) = (120, 40);
/// The frame `STRATA_WINDOW_SHOT` writes: 1.5 s in, after the title has typed out.
const SHOT_FRAME: u64 = 45;
/// How far Cmd+= and Cmd+- step the text size, and its bounds.
const ZOOM_STEP: f32 = 1.1;
const ZOOM_RANGE: (f32, f32) = (0.6, 2.5);

/// Whether this is the desktop app (the `app` feature: the Mac `.app`, `Strata Zero.exe`,
/// the Linux `strata-zero`): it always opens the window, and keeps its files in the
/// player's app folder.
pub const IS_APP: bool = cfg!(feature = "app");

/// Runs the game in a window until it quits. The desktop app first makes its app folder
/// (`Platform::app_dir`) the working folder, so `cities/` is saved there.
pub fn run(screen: Box<dyn Screen>, mut st: AppState) -> Result<()> {
    if let Some(fx) = std::env::var("STRATA_SCREEN_FX")
        .ok()
        .and_then(|s| ScreenFx::parse(&s))
    {
        st.screen_fx = fx;
    }
    if IS_APP {
        let dir = Platform::NATIVE
            .app_dir(&platform::process_env)
            .context("can't tell where to keep the game's files: set APPDATA (Windows) or HOME")?;
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        std::env::set_current_dir(&dir).with_context(|| format!("opening {}", dir.display()))?;
    }
    let fonts = Fonts::load();
    let event_loop = EventLoop::new()?;
    let mut app = App {
        runner: Runner::new(screen, st),
        fonts: Some(fonts),
        live: None,
        keys: Vec::new(),
        mods: ModifiersState::empty(),
        zoom: 1.0,
        regridded: false,
        damage: None,
        shown_fx: None,
        hidden: false,
        next: Instant::now(),
        frames: 0,
        shot: std::env::var_os("STRATA_WINDOW_SHOT").map(PathBuf::from),
        error: None,
    };
    event_loop.run_app(&mut app)?;
    app.error.map_or(Ok(()), Err)
}

/// The window, what presents into it and the terminal drawing for it, once it exists.
struct Live {
    window: Arc<Window>,
    presenter: Presenter,
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
    /// What the backend painted that the screen hasn't shown yet.
    damage: Option<Damage>,
    /// The screen effects last presented, to present again when Settings changes them.
    shown_fx: Option<ScreenFx>,
    /// The window can't be seen: frames still run and gather damage, but nothing is
    /// presented until it can be seen again.
    hidden: bool,
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
        // scale is known, as cell sizes don't scale exactly (a face 9 px wide at 15 px
        // can be 19 px wide at 30).
        let cell = fonts.metrics(TEXT_PX);
        let attrs = Window::default_attributes()
            .with_title("Strata Zero")
            .with_inner_size(LogicalSize::new(
                (cell.w * OPEN.0 as usize) as f64,
                (cell.h * OPEN.1 as usize) as f64,
            ));
        let attrs = with_platform_look(attrs);
        let window = Arc::new(el.create_window(attrs)?);
        let cell = fonts.metrics(TEXT_PX * window.scale_factor() as f32);
        fit(&window, cell, true);
        let presenter = Presenter::new(window.clone())?;
        // Settings offers the screen effects only where they can be drawn.
        self.runner.st.effects = presenter.has_effects();
        let px = self.text_px(&window);
        let inner = window.inner_size();
        let backend = PixelBackend::new(fonts, px, inner.width as usize, inner.height as usize);
        self.live = Some(Live {
            window,
            presenter,
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
        let painted = live.term.backend_mut().take_damage();
        self.damage = Damage::join(self.damage, painted);
        let fx = self.runner.st.screen_fx;
        if !self.hidden && (self.damage.is_some() || self.shown_fx != Some(fx)) {
            live.window.request_redraw();
        }
        if self.frames == SHOT_FRAME
            && let Some(path) = &self.shot
        {
            let scale = live.window.scale_factor() as f32;
            let frame = live.term.backend().frame();
            // A debugging aid: a failed shot is a note, not the end of the game.
            let shot = live
                .presenter
                .capture(frame, fx, scale)
                .and_then(|(w, h, rgba)| png::write_rgba(path, w, h, &rgba));
            if let Err(e) = shot {
                eprintln!("strata: STRATA_WINDOW_SHOT: {e:#}");
            }
        }
        Ok(())
    }

    fn present(&mut self) -> Result<()> {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        // A resize repaints before the next frame does; show that too.
        let painted = live.term.backend_mut().take_damage();
        let damage = Damage::join(self.damage.take(), painted);
        let fx = self.runner.st.screen_fx;
        let scale = live.window.scale_factor() as f32;
        let frame = live.term.backend().frame();
        match live.presenter.present(frame, damage, fx, scale)? {
            Shown::Yes => self.shown_fx = Some(fx),
            Shown::Retry => live.window.request_redraw(),
            Shown::Hidden => self.hidden = true,
        }
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

/// The window's icon and name where the platform takes them from the window: on Windows
/// the app's icon resource, for the title bar and the taskbar; on Linux the name
/// `strata-zero` (the Wayland app id and the X11 class, which tie the window to
/// `strata-zero.desktop`) and the icon for X11. The Mac takes both from the `.app`.
fn with_platform_look(attrs: WindowAttributes) -> WindowAttributes {
    #[cfg(all(windows, feature = "app"))]
    let attrs = {
        use winit::platform::windows::{IconExtWindows, WindowAttributesExtWindows};
        use winit::window::Icon;
        let icon = Icon::from_resource(1, None).ok();
        attrs.with_window_icon(icon.clone()).with_taskbar_icon(icon)
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let attrs = {
        use winit::platform::wayland::WindowAttributesExtWayland;
        // Sets the name X11 reads too.
        WindowAttributesExtWayland::with_name(attrs, "strata-zero", "strata-zero")
            .with_window_icon(linux_icon())
    };
    attrs
}

/// The PC icon (`linux/strata-zero.png`), for an X11 window.
#[cfg(all(unix, not(target_os = "macos")))]
fn linux_icon() -> Option<winit::window::Icon> {
    let png = include_bytes!("../../linux/strata-zero.png");
    let mut reader = ::png::Decoder::new(std::io::Cursor::new(&png[..]))
        .read_info()
        .ok()?;
    let mut rgba = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut rgba).ok()?;
    rgba.truncate(info.buffer_size());
    winit::window::Icon::from_rgba(rgba, info.width, info.height).ok()
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
            WindowEvent::Occluded(hidden) => {
                self.hidden = hidden;
                if let Some(live) = self.live.as_ref().filter(|_| !hidden) {
                    live.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                let pressed = event.state == ElementState::Pressed;
                let bare = event.key_without_modifiers();
                let input = keys::translate(
                    pressed,
                    &event.logical_key,
                    &bare,
                    self.mods,
                    Platform::NATIVE,
                );
                match input {
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
    use std::path::Path;

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

    /// The app icons, to `SHOT_DIR`:
    /// - `icon.png` (1024 × 1024, inset as macOS icons are drawn), which
    ///   `scripts/macos-app.sh --icon` turns into `macos/AppIcon.icns`;
    /// - the PC icon, on a fuller square, as `icon.ico` (256 down to 16 pixels) and
    ///   `strata-zero.png` (256), which `scripts/windows-app.sh --icon` and
    ///   `scripts/linux-app.sh --icon` copy to `windows/AppIcon.ico` and
    ///   `linux/strata-zero.png`.
    #[test]
    #[ignore]
    fn app_icon() {
        let Ok(dir) = std::env::var("SHOT_DIR") else {
            return;
        };
        let dir = Path::new(&dir);
        let mac = icon_rgba(100.0, 180.0, 10.0);
        png::write_rgba(&dir.join("icon.png"), ICON, ICON, &mac).unwrap();
        let pc = icon_rgba(24.0, 160.0, 14.0);
        let sizes: Vec<(u32, Vec<u8>)> = [256, 64, 48, 32, 24, 16]
            .into_iter()
            .map(|s| (s as u32, png::downsample(&pc, ICON as usize, s)))
            .collect();
        png::write_ico(&dir.join("icon.ico"), &sizes).unwrap();
        png::write_rgba(&dir.join("strata-zero.png"), 256, 256, &sizes[0].1).unwrap();
    }

    /// The side of the icon art, in pixels.
    const ICON: u32 = 1024;

    /// The icon art, `ICON` pixels a side: "SZ" in the title's block letters on a
    /// rounded square of the game's background, `inset` from the edges, with corners of
    /// `radius` and a neon edge `edge` wide.
    fn icon_rgba(inset: f32, radius: f32, edge: f32) -> Vec<u8> {
        use crate::render::Rgb;
        use crate::ui::logo::{self, Logo};
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::widgets::Widget;
        let mut buf = Buffer::empty(Rect::new(0, 0, logo::width("SZ"), logo::ROWS));
        let logo = Logo {
            text: "SZ",
            tick: 0,
            accent: Rgb(255, 0, 200),
            steady: true,
        };
        logo.render(buf.area, &mut buf);
        let mut canvas = raster::Canvas::new(Fonts::load(), 80.0, 0, 0);
        canvas.paint_buffer(&buf);
        let art = &canvas.frame;

        const SIZE: usize = ICON as usize;
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
        rgba
    }
}
