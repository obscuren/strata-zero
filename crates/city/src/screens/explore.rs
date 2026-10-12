//! The map screen: free-camera Explore mode and the Walk mode with the `@`.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::minimap::Minimap;
use crate::render::overlay::Digital;
use crate::render::traffic::Traffic;
use crate::render::viewport::{Camera, MapView, Overlay, tile_to_screen};
use crate::render::weather::Weather;
use crate::screens::configure::Configure;
use crate::screens::files::Files;
use crate::screens::generating::Generating;
use crate::screens::help;
use crate::screens::title::Title;
use crate::ui::panel::{boxed, inspector_lines, legend, status_bar, strata_selector};
use crate::walker::Player;
use citygen::rng::hash64;
use citygen::{Dir, Pos, Rect as TileRect, StratumKind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use std::cell::Cell;
use std::time::Instant;

const PANEL_W: u16 = 34;
const MINIMAP_W: u16 = 26;
const MINIMAP_H: u16 = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Explore,
    Walk,
}

/// Frames per second over the last second.
pub struct FpsMeter {
    last: Instant,
    frames: u32,
    pub fps: f32,
}

impl Default for FpsMeter {
    fn default() -> Self {
        FpsMeter {
            last: Instant::now(),
            frames: 0,
            fps: 0.0,
        }
    }
}

impl FpsMeter {
    pub fn frame(&mut self) {
        self.frames += 1;
        let elapsed = self.last.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            self.fps = self.frames as f32 / elapsed;
            self.frames = 0;
            self.last = Instant::now();
        }
    }
}

pub struct MapScreen {
    pub mode: Mode,
    pub stratum: StratumKind,
    pub cam: Camera,
    pub show_inspector: bool,
    pub show_minimap: bool,
    pub show_legend: bool,
    pub digital: bool,
    pub help: bool,
    traffic: Traffic,
    fps: FpsMeter,
    cam_ready: bool,
    /// The map area of the last frame, so `update` knows the view size.
    last_map: Cell<Rect>,
}

fn dir_for(code: KeyCode) -> Option<Dir> {
    Some(match code {
        KeyCode::Up | KeyCode::Char('k') => Dir::N,
        KeyCode::Down | KeyCode::Char('j') => Dir::S,
        KeyCode::Left | KeyCode::Char('h') => Dir::W,
        KeyCode::Right | KeyCode::Char('l') => Dir::E,
        KeyCode::Char('y') => Dir::NW,
        KeyCode::Char('u') => Dir::NE,
        KeyCode::Char('b') => Dir::SW,
        KeyCode::Char('n') => Dir::SE,
        _ => return None,
    })
}

impl MapScreen {
    pub fn explore(_tick: u64) -> Self {
        MapScreen {
            mode: Mode::Explore,
            stratum: StratumKind::Street,
            cam: Camera::default(),
            show_inspector: true,
            show_minimap: true,
            show_legend: false,
            digital: false,
            help: false,
            traffic: Traffic::default(),
            fps: FpsMeter::default(),
            cam_ready: false,
            last_map: Cell::new(Rect::new(0, 0, 100, 28)),
        }
    }

    /// The tile the inspector describes: the player in Walk mode, the view centre otherwise.
    fn cursor(&self, st: &AppState) -> Pos {
        match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) => p.pos,
            _ => Pos::new(self.cam.tx, self.cam.ty),
        }
    }

    fn layout(&self, area: Rect) -> (Rect, Option<Rect>, Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(1)])
            .split(area);
        let status = rows[1];
        if self.show_inspector && rows[0].width > PANEL_W + 20 {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(20), Constraint::Length(PANEL_W)])
                .split(rows[0]);
            (cols[0], Some(cols[1]), status)
        } else {
            (rows[0], None, status)
        }
    }

    fn change_stratum(&mut self, delta: i32) {
        let i = self.stratum.index() as i32 + delta;
        if let Some(k) = StratumKind::from_index(i.clamp(0, 3) as usize) {
            self.stratum = k;
        }
    }

    fn enter_walk(&mut self, st: &mut AppState) {
        let Some(city) = st.city.as_ref() else { return };
        if st.player.is_none() {
            st.player = Some(Player::spawn(city, hash64(city.seed, st.tick)));
        }
        if let Some(p) = &st.player {
            self.stratum = p.stratum;
            self.cam.snap(p.pos);
        }
        self.mode = Mode::Walk;
    }

    fn handle_common(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        match k.code {
            KeyCode::Char('i') => self.show_inspector = !self.show_inspector,
            KeyCode::Char('m') => self.show_minimap = !self.show_minimap,
            KeyCode::Char('e') => self.show_legend = !self.show_legend,
            KeyCode::Char('d') => self.digital = !self.digital,
            KeyCode::Char('?') => self.help = !self.help,
            KeyCode::Char('s') => {
                let name = st.city.as_ref().map(|c| c.name.clone())?;
                return Some(ScreenCmd::Switch {
                    to: Box::new(Files::save(st.tick, &name)),
                    fx: TransitionKind::None,
                });
            }
            KeyCode::Char('q') => {
                return Some(ScreenCmd::Switch {
                    to: Box::new(Title::new(st.tick)),
                    fx: TransitionKind::SlideUp,
                });
            }
            KeyCode::Esc => {
                if self.help {
                    self.help = false;
                } else {
                    return Some(ScreenCmd::Switch {
                        to: Box::new(Title::new(st.tick)),
                        fx: TransitionKind::SlideUp,
                    });
                }
            }
            _ => return None,
        }
        Some(ScreenCmd::Stay)
    }

    fn handle_explore(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        let fast = k.modifiers.contains(KeyModifiers::SHIFT)
            || matches!(k.code, KeyCode::Char('H' | 'J' | 'K' | 'L'));
        let step = if fast { 8 } else { 1 };
        let code = match k.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        };
        match code {
            _ if dir_for(code).is_some()
                && !matches!(code, KeyCode::Char('y' | 'u' | 'b' | 'n')) =>
            {
                let (dx, dy) = dir_for(code).expect("checked").delta();
                self.cam.pan(dx * step, dy * step);
            }
            KeyCode::Char('[') => self.change_stratum(-1),
            KeyCode::Char(']') => self.change_stratum(1),
            KeyCode::Char('w') => self.enter_walk(st),
            KeyCode::Char('r') => {
                let params = st.params.clone().with_seed(hash64(st.params.seed, st.tick));
                return Some(match Generating::new(params, st.tick) {
                    Ok(next) => ScreenCmd::Switch {
                        to: Box::new(next),
                        fx: TransitionKind::Glitch,
                    },
                    Err(e) => {
                        st.set_status(e.to_string(), 90);
                        ScreenCmd::Stay
                    }
                });
            }
            KeyCode::Char('g') => {
                return Some(ScreenCmd::Switch {
                    to: Box::new(Configure::new(st.params.clone(), st.tick)),
                    fx: TransitionKind::SlideUp,
                });
            }
            _ => return None,
        }
        Some(ScreenCmd::Stay)
    }

    fn handle_walk(&mut self, st: &mut AppState, k: &KeyEvent) -> Option<ScreenCmd> {
        let city = st.city.as_ref()?;
        let player = st.player.as_mut()?;
        match k.code {
            KeyCode::Tab => self.mode = Mode::Explore,
            KeyCode::Char('<') | KeyCode::Char('>') => {
                let up = k.code == KeyCode::Char('>');
                match player.use_connector(city, up) {
                    Some(dest) => {
                        self.stratum = dest;
                        player.update_fov(city);
                        st.set_status(format!("now on {}", dest.label()), 60);
                    }
                    None => st.set_status(
                        if up {
                            "nothing leads up from here"
                        } else {
                            "nothing leads down from here"
                        },
                        60,
                    ),
                }
            }
            KeyCode::Char('.') => player.update_fov(city),
            code => {
                let dir = dir_for(code)?;
                if player.try_move(city, dir) {
                    player.update_fov(city);
                }
            }
        }
        Some(ScreenCmd::Stay)
    }
}

impl Screen for MapScreen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        self.fps.frame();
        let Some(size) = st.city.as_ref().map(|c| c.size) else {
            if keys
                .iter()
                .any(|k| matches!(k.code, KeyCode::Char('q') | KeyCode::Esc))
            {
                return ScreenCmd::Switch {
                    to: Box::new(Title::new(st.tick)),
                    fx: TransitionKind::SlideUp,
                };
            }
            return ScreenCmd::Stay;
        };
        if !self.cam_ready {
            self.cam.snap(size.center());
            self.cam_ready = true;
        }
        for k in keys {
            if let Some(cmd) = self.handle_common(st, k) {
                if !matches!(cmd, ScreenCmd::Stay) {
                    return cmd;
                }
                continue;
            }
            let cmd = match self.mode {
                Mode::Explore => self.handle_explore(st, k),
                Mode::Walk => self.handle_walk(st, k),
            };
            if let Some(cmd) = cmd
                && !matches!(cmd, ScreenCmd::Stay)
            {
                return cmd;
            }
        }
        let map = self.last_map.get();
        if let (Mode::Walk, Some(p)) = (self.mode, &st.player) {
            self.cam.follow(p.pos, map.width, map.height);
        }
        self.cam.clamp(size, map.width, map.height);
        self.cam.update();
        if let Some(city) = st.city.as_ref() {
            let origin = self.cam.origin(map.width, map.height);
            let view = TileRect::new(origin.x, origin.y, map.width as i32, map.height as i32);
            self.traffic.update(city, self.stratum, view, st.tick);
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let Some(city) = st.city.as_ref() else {
            f.render_widget(
                Paragraph::new("no city loaded · q back").alignment(Alignment::Center),
                Rect::new(area.x, area.y + area.height / 2, area.width, 1),
            );
            return;
        };
        let (map, panel, status) = self.layout(area);
        self.last_map.set(map);
        let origin = self.cam.origin(map.width, map.height);
        let cursor = self.cursor(st);
        let focus = if self.digital {
            Digital::node_under(city, self.stratum, cursor)
        } else {
            None
        };
        let digital = Digital {
            enabled: self.digital,
            focus,
        };
        let overlays: [&dyn Overlay; 3] = [&Weather, &self.traffic, &digital];
        let walking = self.mode == Mode::Walk
            && st
                .player
                .as_ref()
                .is_some_and(|p| p.stratum == self.stratum);
        let vis = if walking {
            st.player.as_ref().map(|p| p.vis())
        } else {
            None
        };
        MapView {
            city,
            stratum: self.stratum,
            origin,
            tick: st.tick,
            style: &st.style,
            vis,
            overlays: &overlays,
            cursor: if self.mode == Mode::Explore {
                Some(cursor)
            } else {
                None
            },
        }
        .render(map, f.buffer_mut());

        if let Some(p) = &st.player
            && p.stratum == self.stratum
            && let Some((sx, sy)) = tile_to_screen(map, origin, p.pos)
        {
            f.buffer_mut()[(sx, sy)].set_char('@').set_style(
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .add_modifier(Modifier::BOLD),
            );
        }

        if self.show_minimap && map.width > MINIMAP_W + 4 && map.height > MINIMAP_H + 4 {
            let mm = Rect::new(
                map.right() - MINIMAP_W - 1,
                map.bottom() - MINIMAP_H - 1,
                MINIMAP_W,
                MINIMAP_H,
            );
            let block = boxed("map");
            let inner = block.inner(mm);
            f.render_widget(block, mm);
            Minimap {
                city,
                stratum: self.stratum,
                view: TileRect::new(origin.x, origin.y, map.width as i32, map.height as i32),
                player: st
                    .player
                    .as_ref()
                    .filter(|p| p.stratum == self.stratum)
                    .map(|p| p.pos),
                style: &st.style,
            }
            .render(inner, f.buffer_mut());
        }

        if let Some(panel) = panel {
            let block = boxed(if self.show_legend {
                "LEGEND"
            } else {
                "INSPECT"
            });
            let inner = block.inner(panel);
            f.render_widget(block, panel);
            let mut lines: Vec<Line> = strata_selector(self.stratum, st.tick);
            lines.push(Line::from(""));
            if self.show_legend {
                lines.extend(legend(self.stratum));
            } else {
                let node = focus.map(|id| city.net.node(id));
                lines.extend(inspector_lines(city, self.stratum, cursor, node));
            }
            f.render_widget(
                Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }),
                inner,
            );
        }

        let hints = match self.mode {
            Mode::Explore => "hjkl pan · [] level · w walk · d net · e legend · s save · ? keys",
            Mode::Walk => "hjkl move · <> stairs · tab camera · d net · ? keys",
        };
        f.render_widget(
            Paragraph::new(status_bar(
                Some(city),
                self.stratum,
                self.fps.fps,
                hints,
                st.status_text(),
            )),
            status,
        );

        if self.help {
            help::draw(f, self.mode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset, StratumKind};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> AppState {
        let mut st = AppState::new();
        st.city = Some(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    #[test]
    fn explore_and_walk_round_trip() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("Street"), "{text}");
        s.update(&mut st, &[key(KeyCode::Char(']'))]);
        assert_eq!(s.stratum, StratumKind::Skyways);
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        assert_eq!(s.stratum, StratumKind::Undercity);
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        assert_eq!(s.stratum, StratumKind::Undercity);
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        assert!(st.player.is_some());
        assert_eq!(s.mode, Mode::Walk);
        assert_eq!(s.stratum, StratumKind::Street);
        for d in ['h', 'j', 'k', 'l', 'y', 'u', 'b', 'n'] {
            st.tick += 1;
            s.update(&mut st, &[key(KeyCode::Char(d))]);
        }
        s.update(&mut st, &[key(KeyCode::Char('.'))]);
        s.update(&mut st, &[key(KeyCode::Char('<'))]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains('@'));
        s.update(
            &mut st,
            &[
                key(KeyCode::Char('d')),
                key(KeyCode::Char('m')),
                key(KeyCode::Char('i')),
            ],
        );
        assert!(s.digital && !s.show_minimap && !s.show_inspector);
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("move"));
        s.update(&mut st, &[key(KeyCode::Esc)]);
        assert!(!s.help);
        s.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(s.mode, Mode::Explore);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('s'))]),
            ScreenCmd::Switch { .. }
        ));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('q'))]),
            ScreenCmd::Switch { .. }
        ));
    }

    #[test]
    fn explore_pans_and_switches_to_other_screens() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let before = (s.cam.tx, s.cam.ty);
        s.update(&mut st, &[key(KeyCode::Right), key(KeyCode::Down)]);
        assert_ne!((s.cam.tx, s.cam.ty), before);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('r'))]),
            ScreenCmd::Switch { .. }
        ));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('g'))]),
            ScreenCmd::Switch { .. }
        ));
        let mut empty = AppState::new();
        let mut t2 = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t2.draw(|f| s.draw(&empty, f)).unwrap();
        assert!(matches!(
            s.update(&mut empty, &[key(KeyCode::Char('w'))]),
            ScreenCmd::Stay
        ));
    }
}
