//! The map screen: free-camera Explore mode and the Walk mode with the `@`.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::minimap::Minimap;
use crate::render::overlay::Digital;
use crate::render::traffic::Traffic;
use crate::render::viewport::{Camera, MapView, Overlay, tile_to_screen};
use crate::render::weather::Weather;
use crate::screens::configure::Configure;
use crate::screens::deck::Deck;
use crate::screens::files::Files;
use crate::screens::generating::Generating;
use crate::screens::help;
use crate::screens::pause::Pause;
use crate::screens::title::Title;
use crate::ui::panel::{boxed, inspector_lines, legend, status_bar, strata_selector};
use crate::walker::Player;
use citygen::rng::hash64;
use citygen::{City, Dir, Pos, Rect as TileRect, StratumKind};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use std::cell::Cell;
use std::time::Instant;

const PANEL_W: u16 = 34;
const MINIMAP_H: u16 = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Explore,
    Walk,
}

/// A gap longer than this between frames means the map was not being updated (a
/// menu was open); the meter restarts its window instead of counting the gap.
const FPS_GAP: f32 = 0.5;

/// Frames per second over the last second.
pub struct FpsMeter {
    /// Start of the current measuring window.
    last: Instant,
    /// The previous frame, to spot gaps.
    prev: Instant,
    frames: u32,
    pub fps: f32,
}

impl Default for FpsMeter {
    fn default() -> Self {
        FpsMeter::new(Instant::now())
    }
}

impl FpsMeter {
    pub fn new(now: Instant) -> Self {
        FpsMeter {
            last: now,
            prev: now,
            frames: 0,
            fps: 0.0,
        }
    }

    pub fn frame(&mut self) {
        self.frame_at(Instant::now());
    }

    pub fn frame_at(&mut self, now: Instant) {
        let gap = now.saturating_duration_since(self.prev).as_secs_f32();
        self.prev = now;
        if gap > FPS_GAP {
            self.last = now;
            self.frames = 0;
            return;
        }
        self.frames += 1;
        let elapsed = now.saturating_duration_since(self.last).as_secs_f32();
        if elapsed >= 1.0 {
            self.fps = self.frames as f32 / elapsed;
            self.frames = 0;
            self.last = now;
        }
    }
}

pub struct MapScreen {
    pub mode: Mode,
    pub stratum: StratumKind,
    pub cam: Camera,
    /// The tile under inspection in Explore mode; the camera follows it.
    pub cursor: Pos,
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
            cursor: Pos::new(0, 0),
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

    /// The tile the inspector describes: the player in Walk mode, the cursor otherwise.
    fn focus_pos(&self, st: &AppState) -> Pos {
        match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) => p.pos,
            _ => self.cursor,
        }
    }

    fn layout(&self, area: Rect, dev: bool) -> (Rect, Option<Rect>, Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(1)])
            .split(area);
        let status = rows[1];
        if self.show_inspector && dev && rows[0].width > PANEL_W + 20 {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(20), Constraint::Length(PANEL_W)])
                .split(rows[0]);
            (cols[0], Some(cols[1]), status)
        } else {
            (rows[0], None, status)
        }
    }

    fn change_stratum(&mut self, st: &AppState, delta: i32) {
        let max = st
            .city
            .as_ref()
            .map(|c| c.strata.len() as i32 - 1)
            .unwrap_or(3);
        let i = (self.stratum.index() as i32 + delta).clamp(0, max);
        if let Some(k) = StratumKind::from_index(i as usize) {
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
            KeyCode::Char('`') => {
                st.dev = !st.dev;
                st.set_status(
                    if st.dev {
                        "dev mode on"
                    } else {
                        "dev mode off"
                    },
                    60,
                );
            }
            KeyCode::Char('i') if st.dev => self.show_inspector = !self.show_inspector,
            KeyCode::Char('m') => self.show_minimap = !self.show_minimap,
            KeyCode::Char('e') => self.show_legend = !self.show_legend,
            KeyCode::Char('d') => self.digital = !self.digital,
            KeyCode::Char('?') => self.help = !self.help,
            KeyCode::Tab => {
                return Some(ScreenCmd::Push {
                    to: Box::new(Deck::new(st.tick)),
                    fx: TransitionKind::Glitch,
                });
            }
            KeyCode::Char('s') => {
                let name = st.city.as_ref().map(|c| c.name.clone())?;
                return Some(ScreenCmd::Push {
                    to: Box::new(Files::save(st.tick, &name)),
                    fx: TransitionKind::None,
                });
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.help {
                    self.help = false;
                } else {
                    return Some(ScreenCmd::Push {
                        to: Box::new(Pause::new(st.tick)),
                        fx: TransitionKind::None,
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
                let size = st.city.as_ref().map(|c| c.size)?;
                self.cursor = Pos::new(
                    (self.cursor.x + dx * step).clamp(0, size.w as i32 - 1),
                    (self.cursor.y + dy * step).clamp(0, size.h as i32 - 1),
                );
            }
            KeyCode::Char('[') => self.change_stratum(st, -1),
            KeyCode::Char(']') => self.change_stratum(st, 1),
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
        let dev = st.dev;
        let city = st.city.as_ref()?;
        let player = st.player.as_mut()?;
        match k.code {
            KeyCode::Char('c') if dev => {
                self.cursor = player.pos;
                self.mode = Mode::Explore;
            }
            KeyCode::Enter => {
                if player.jacked_in() {
                    match player.jack_out(city) {
                        Ok(()) => {
                            self.stratum = player.stratum;
                            self.cam.snap(player.pos);
                            st.set_status("jacked out", 60);
                            return Some(ScreenCmd::Flash(TransitionKind::Glitch));
                        }
                        Err(e) => st.set_status(e, 60),
                    }
                } else {
                    match player.jack_in(city) {
                        Ok(port) => {
                            self.stratum = StratumKind::Net;
                            self.cam.snap(port);
                            let where_ = port_name(city, port);
                            st.set_status(format!("jacked in at {where_}"), 90);
                            return Some(ScreenCmd::Flash(TransitionKind::Glitch));
                        }
                        Err(e) => st.set_status(e, 60),
                    }
                }
            }
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

/// What a port is called in the status line: its building, else its district.
fn port_name(city: &City, port: Pos) -> String {
    let tile = city.tile(StratumKind::Street, port);
    tile.and_then(|t| t.building)
        .and_then(|b| city.building_opt(b))
        .and_then(|b| b.name.clone())
        .or_else(|| {
            tile.and_then(|t| t.district)
                .and_then(|d| city.district_opt(d))
                .map(|d| d.name.clone())
        })
        .unwrap_or_else(|| "the Net".to_string())
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
            self.cursor = size.center();
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
        if !st.dev && self.mode == Mode::Explore {
            // Without dev mode there is no free camera: the player is always on foot.
            self.enter_walk(st);
        }
        let map = self.last_map.get();
        match (self.mode, &st.player) {
            (Mode::Walk, Some(p)) => self.cam.follow(p.pos, map.width, map.height),
            _ => self.cam.follow(self.cursor, map.width, map.height),
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
        let (map, panel, status) = self.layout(area, st.dev);
        self.last_map.set(map);
        let origin = self.cam.origin(map.width, map.height);
        let cursor = self.focus_pos(st);
        let focus = if self.digital || self.stratum == StratumKind::Net {
            Digital::node_under(city, self.stratum, cursor)
        } else {
            None
        };
        let digital = Digital {
            enabled: self.digital,
            focus,
        };
        let mut overlays: Vec<&dyn Overlay> = Vec::new();
        if st.rain {
            overlays.push(&Weather);
        }
        overlays.push(&self.traffic);
        overlays.push(&digital);
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

        if let Some(p) = &st.player {
            if let Some((bk, bp)) = p.body
                && bk == self.stratum
                && let Some((sx, sy)) = tile_to_screen(map, origin, bp)
            {
                f.buffer_mut()[(sx, sy)].set_char('@').set_style(
                    Style::default()
                        .fg(Color::Rgb(110, 120, 130))
                        .add_modifier(Modifier::DIM),
                );
            }
            if p.stratum == self.stratum
                && let Some((sx, sy)) = tile_to_screen(map, origin, p.pos)
            {
                let fg = if p.stratum == StratumKind::Net {
                    Color::Rgb(0, 255, 200)
                } else {
                    Color::Rgb(255, 255, 255)
                };
                f.buffer_mut()[(sx, sy)]
                    .set_char('@')
                    .set_style(Style::default().fg(fg).add_modifier(Modifier::BOLD));
            }
        }

        if let Some(panel) = panel {
            let block = boxed(if self.show_legend {
                "LEGEND"
            } else {
                "INSPECT"
            });
            let inner = block.inner(panel);
            f.render_widget(block, panel);
            // The minimap takes the lower part of the panel when there is room for it.
            let (text_area, map_area) = if self.show_minimap && inner.height > MINIMAP_H + 8 {
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(5), Constraint::Length(MINIMAP_H + 2)])
                    .split(inner);
                (rows[0], Some(rows[1]))
            } else {
                (inner, None)
            };
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
                text_area,
            );
            if let Some(mm) = map_area {
                let block = boxed("map");
                let mi = block.inner(mm);
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
                .render(mi, f.buffer_mut());
            }
        }

        let jacked = st.player.as_ref().is_some_and(|p| p.jacked_in());
        let cam = if st.dev { " · c camera" } else { "" };
        let hints = match (self.mode, jacked) {
            (Mode::Explore, _) => {
                "hjkl pan · [] level · w walk · tab deck · d overlay · s save · ? keys".to_string()
            }
            (Mode::Walk, true) => {
                format!(
                    "⚡ jacked in · hjkl move · enter jack out · tab deck · d overlay{cam} · ? keys"
                )
            }
            (Mode::Walk, false) => {
                format!("hjkl move · <> stairs · enter jack in · tab deck{cam} · ? keys")
            }
        };
        f.render_widget(
            Paragraph::new(status_bar(
                Some(city),
                self.stratum,
                st.show_fps.then_some(self.fps.fps),
                &hints,
                st.status_text(),
                st.dev,
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
        // Saving pushes the save screen over the live map; the map keeps its state.
        s.stratum = StratumKind::Undercity;
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('s'))]),
            ScreenCmd::Push { .. }
        ));
        assert_eq!((s.mode, s.stratum), (Mode::Walk, StratumKind::Undercity));
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Explore);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('q'))]),
            ScreenCmd::Push { .. }
        ));
    }

    #[test]
    fn explore_pans_and_switches_to_other_screens() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[]);
        let before = s.cursor;
        s.update(&mut st, &[key(KeyCode::Right), key(KeyCode::Down)]);
        assert_eq!(s.cursor, Pos::new(before.x + 1, before.y + 1));
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

    #[test]
    fn explore_cursor_reaches_the_map_corners_without_showing_void() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        s.update(&mut st, &[]);
        let map = s.last_map.get();
        for _ in 0..40 {
            s.update(&mut st, &[key(KeyCode::Char('H')), key(KeyCode::Char('K'))]);
        }
        assert_eq!(s.cursor, Pos::new(0, 0));
        for _ in 0..30 {
            s.update(&mut st, &[]);
        }
        let o = s.cam.origin(map.width, map.height);
        assert!(o.x >= 0 && o.y >= 0, "origin {o:?}");
        for _ in 0..40 {
            s.update(&mut st, &[key(KeyCode::Char('L')), key(KeyCode::Char('J'))]);
        }
        assert_eq!(s.cursor, Pos::new(119, 59));
        for _ in 0..30 {
            s.update(&mut st, &[]);
        }
        let o = s.cam.origin(map.width, map.height);
        assert!(
            o.x + map.width as i32 <= 120 && o.y + map.height as i32 <= 60,
            "origin {o:?}"
        );
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("119,59"), "{text}");
    }

    #[test]
    fn enter_jacks_in_and_the_net_is_the_fifth_stratum() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        for _ in 0..4 {
            s.update(&mut st, &[key(KeyCode::Char(']'))]);
        }
        assert_eq!(s.stratum, StratumKind::Net);
        s.update(&mut st, &[key(KeyCode::Char(']'))]);
        assert_eq!(s.stratum, StratumKind::Net);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("▌ Net"), "{text}");
        assert!(
            text.contains('═') || text.contains('┃') || text.contains('━'),
            "{text}"
        );

        // Walk up to a public port and jack in.
        s.update(&mut st, &[key(KeyCode::Char('['))]);
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        let city = st.city.as_ref().unwrap();
        let jack = city
            .connectors
            .iter()
            .find(|k| k.kind == citygen::ConnectorKind::Jack && k.public)
            .unwrap()
            .pos;
        let stand = jack
            .neighbors8()
            .find(|n| city.walkable(StratumKind::Street, *n))
            .unwrap();
        {
            let p = st.player.as_mut().unwrap();
            p.pos = stand;
            p.stratum = StratumKind::Street;
        }
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Flash(TransitionKind::Glitch)
        ));
        let p = st.player.as_ref().unwrap();
        assert!(p.jacked_in());
        assert_eq!(s.stratum, StratumKind::Net);
        assert_eq!(p.pos, jack);
        st.tick += 500;
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("jack out"), "{text}");
        assert!(text.contains("jacked in"), "{text}");
        assert!(text.contains("d overlay"), "{text}");
        // The body shows on the Street while the avatar is in the Net.
        s.mode = Mode::Explore;
        s.stratum = StratumKind::Street;
        s.cursor = stand;
        s.cam.snap(stand);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let map = s.last_map.get();
        let origin = s.cam.origin(map.width, map.height);
        let (sx, sy) = tile_to_screen(map, origin, stand).unwrap();
        assert_eq!(t.backend().buffer()[(sx, sy)].symbol(), "@");
        s.mode = Mode::Walk;
        s.stratum = StratumKind::Net;
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Flash(TransitionKind::Glitch)
        ));
        let p = st.player.as_ref().unwrap();
        assert!(!p.jacked_in());
        assert_eq!((p.stratum, p.pos), (StratumKind::Street, stand));
        assert_eq!(s.stratum, StratumKind::Street);
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("jack in"));
    }

    #[test]
    fn dev_mode_off_locks_the_player_into_walk_mode() {
        let mut st = state();
        st.dev = false;
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        s.update(&mut st, &[]);
        assert_eq!(s.mode, Mode::Walk);
        assert!(st.player.is_some());
        s.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(s.mode, Mode::Walk, "tab must not open the free camera");
        s.update(&mut st, &[key(KeyCode::Char('i'))]);
        st.tick += 500;
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(!text.contains("INSPECT"), "{text}");
        assert!(!text.contains("c camera"), "{text}");
        assert!(!text.contains("DEV"), "{text}");
        // The backtick flips dev mode at runtime; the camera and inspector come back.
        s.update(&mut st, &[key(KeyCode::Char('`'))]);
        assert!(st.dev);
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Explore);
        st.tick += 500;
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("INSPECT") && text.contains("DEV"), "{text}");
        // Flipping it off again drops straight back into the walk.
        s.update(&mut st, &[key(KeyCode::Char('`'))]);
        assert!(!st.dev);
        assert_eq!(s.mode, Mode::Walk);
    }

    #[test]
    fn minimap_lives_in_the_lower_part_of_the_inspector() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        s.update(&mut st, &[]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        let panel_x = 120 - PANEL_W as usize;
        let rows: Vec<(usize, usize)> = text
            .lines()
            .enumerate()
            .filter_map(|(y, l)| l.find("map").map(|x| (y, x)))
            .collect();
        assert!(!rows.is_empty(), "no minimap box: {text}");
        assert!(
            rows.iter().all(|(_, x)| *x >= panel_x),
            "minimap drawn outside the panel: {rows:?}"
        );
        assert!(
            rows.iter().all(|(y, _)| *y > 20),
            "minimap not in the lower part: {rows:?}"
        );
        s.update(&mut st, &[key(KeyCode::Char('m'))]);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(!buffer_text(t.backend().buffer()).contains("map"));
    }

    #[test]
    fn rain_and_fps_follow_the_settings() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        st.tick = 100;
        s.update(&mut st, &[]);
        let rain: Vec<char> = st.style.glyphs.rain.clone();
        let count = |t: &Terminal<TestBackend>| {
            buffer_text(t.backend().buffer())
                .chars()
                .filter(|c| rain.contains(c))
                .count()
        };
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(count(&t) > 0, "no rain on the street");
        assert!(buffer_text(t.backend().buffer()).contains("fps"));
        st.rain = false;
        st.show_fps = false;
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert_eq!(count(&t), 0);
        assert!(!buffer_text(t.backend().buffer()).contains("fps"));
    }

    #[test]
    fn esc_closes_help_before_pausing() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        assert!(s.help);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert!(!s.help);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Push { .. }
        ));
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('q'))]),
            ScreenCmd::Push { .. }
        ));
    }

    #[test]
    fn tab_opens_the_deck_and_c_is_the_dev_camera() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Tab)]),
            ScreenCmd::Push {
                fx: TransitionKind::Glitch,
                ..
            }
        ));
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        assert_eq!(s.mode, Mode::Walk);
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Explore, "c returns to the camera in dev mode");
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        st.dev = false;
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Walk, "no camera without dev mode");
        assert!(
            matches!(
                s.update(&mut st, &[key(KeyCode::Tab)]),
                ScreenCmd::Push { .. }
            ),
            "the deck opens in play too"
        );
        st.tick += 500;
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("tab deck") && !text.contains("c camera"),
            "{text}"
        );
    }

    #[test]
    fn fps_meter_ignores_the_gap_while_a_menu_is_open() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let mut m = FpsMeter::new(t0);
        for i in 1..=60u64 {
            m.frame_at(t0 + Duration::from_millis(i * 1000 / 60 + 1));
        }
        assert!((m.fps - 60.0).abs() < 2.0, "{}", m.fps);
        // Three seconds under a menu, then the first frame back on the map.
        m.frame_at(t0 + Duration::from_millis(4001));
        assert!((m.fps - 60.0).abs() < 2.0, "after the gap: {}", m.fps);
        // A fresh second of frames measures normally again.
        for i in 1..=60u64 {
            m.frame_at(t0 + Duration::from_millis(4001 + i * 1000 / 60 + 1));
        }
        assert!((m.fps - 60.0).abs() < 2.0, "{}", m.fps);
    }

    #[test]
    fn explore_hints_mention_save() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        let status = text.lines().last().unwrap();
        assert!(
            status.contains("s save") && status.contains("? keys"),
            "{status:?}"
        );
    }
}
