//! The world beyond the city in the play screen: the overworld in the atlas look, one
//! cell a tile, and the open ground one level down, with the map's bands, panel and
//! footer. See `2026-11-04-overworld-screen-design.md`.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::outside::ground::{self, GroundView};
use crate::outside::moves::{self, Moved};
use crate::outside::{At, Outside, TILE};
use crate::render::overworld::{ATLAS_BG, atlas_cell};
use crate::render::{Cell, RenderCtx, Rgb, Visibility, lighting, tileset};
use crate::screens::explore::{dir_for, layout};
use crate::screens::pause::Pause;
use crate::ui::chrome::CYAN;
use crate::ui::hud;
use citygen::overworld::{Ground, Overworld, Water};
use citygen::{Pos, StratumKind, Terrain};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use std::collections::BTreeSet;

/// You, drawn over whatever you stand on.
const YOU: Rgb = Rgb(255, 255, 255);
const LABEL: Color = Color::Rgb(120, 130, 145);
const TEXT: Color = Color::Rgb(205, 212, 222);
const DIM: Color = Color::Rgb(110, 118, 130);

/// The keys out here, for `?`.
pub const KEYS: &[(&str, &str)] = &[
    (
        "hjkl yubn ↑↓←→",
        "walk: a tile on the overworld, a step on the ground",
    ),
    (
        "enter",
        "down onto the tile you're on · into the city on its tiles",
    ),
    ("W", "up: from the open ground to the overworld"),
    ("s", "save game"),
    ("?", "this help"),
    ("esc q", "pause menu"),
];

/// The overworld and the open ground, as a level of play.
pub struct OutsideScreen {
    /// The ground round you while you're on it.
    view: Option<GroundView>,
    /// What you see from where you stand on the ground.
    sight: BTreeSet<Pos>,
    help: bool,
}

impl OutsideScreen {
    /// The screen where `st.outside` says you are (the overworld if it says the city).
    pub fn new(st: &mut AppState) -> OutsideScreen {
        if let Some(o) = crate::outside::ensure(st)
            && o.at == At::City
        {
            moves::up(st);
        }
        let mut s = OutsideScreen {
            view: None,
            sight: BTreeSet::new(),
            help: false,
        };
        s.settle(st);
        s
    }

    /// The ground view and what you see, for where you are now.
    fn settle(&mut self, st: &mut AppState) {
        let minute = st.minute_of_day();
        let Some(o) = st.outside.as_mut() else {
            return;
        };
        match o.at {
            At::Ground(c) => {
                match self.view.as_mut() {
                    Some(v) if v.tile(c).is_some() => {
                        v.keep(o.world(), c);
                    }
                    _ => self.view = Some(GroundView::round(o.world(), c)),
                }
                let view = self.view.as_ref().expect("made above");
                self.sight = ground::sight(view, c, minute);
                ground::remember(o, &self.sight);
            }
            _ => {
                self.view = None;
                self.sight.clear();
            }
        }
    }

    /// Back on the map, in the city at `cell`.
    fn into_city(st: &mut AppState, cell: Pos) -> ScreenCmd {
        moves::into_city(st, cell);
        ScreenCmd::Switch {
            to: crate::screens::resume_screen(st),
            fx: TransitionKind::Glitch,
        }
    }

    fn key(&mut self, st: &mut AppState, k: &KeyEvent) -> ScreenCmd {
        let at = st.outside.as_ref().map(|o| o.at);
        if self.help {
            self.help = false;
            return ScreenCmd::Stay;
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                return ScreenCmd::Push {
                    to: Box::new(Pause::new(st, st.tick)),
                    fx: TransitionKind::None,
                };
            }
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('s') => {
                crate::runs::when::save_now(st);
            }
            KeyCode::Enter if matches!(at, Some(At::Overworld(_))) => match moves::down(st) {
                Ok(Moved::City(cell)) => return Self::into_city(st, cell),
                Ok(_) => self.settle(st),
                Err(why) => st.warn(why),
            },
            KeyCode::Char('W') if matches!(at, Some(At::Ground(_))) => {
                moves::up(st);
                self.settle(st);
            }
            code => {
                let Some(d) = dir_for(code) else {
                    return ScreenCmd::Stay;
                };
                let moved = match (at, self.view.as_mut()) {
                    (Some(At::Overworld(_)), _) => moves::world_step(st, d),
                    (Some(At::Ground(_)), Some(view)) => ground::ground_step(st, view, d),
                    _ => return ScreenCmd::Stay,
                };
                match moved {
                    Moved::City(cell) => return Self::into_city(st, cell),
                    Moved::Blocked(why) => st.warn(why),
                    Moved::Stepped => self.settle(st),
                }
            }
        }
        ScreenCmd::Stay
    }
}

impl Screen for OutsideScreen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // The city's clock runs out here as it does on the map.
        crate::clock::run(st, Some(1), crate::clock::PATHS_PER_FRAME);
        for k in keys {
            let cmd = self.key(st, k);
            if !matches!(cmd, ScreenCmd::Stay) {
                return cmd;
            }
        }
        crate::runs::when::frame(st);
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let Some(o) = st.outside.as_ref() else {
            return;
        };
        if area.width < crate::app::MIN_W || area.height < crate::app::MIN_H {
            crate::app::draw_too_small(f);
            return;
        }
        let r = layout(area, false);
        let buf = f.buffer_mut();
        match o.at {
            At::Ground(c) => self.draw_ground(st, o, c, r.map, buf),
            At::Overworld(t) => draw_world(st, o.world(), t, r.map, buf),
            At::City => {}
        }
        // The rules.
        for y in area.y..r.bottom_rule.y {
            if y != r.top_rule.y {
                buf[(r.vrule_x, y)].set_char('│').set_style(hud::RULE_STYLE);
            }
        }
        let cross = r.vrule_x - area.x;
        Paragraph::new(hud::rule_row(area.width, cross, '┿')).render(r.top_rule, buf);
        Paragraph::new(hud::rule_row(area.width, cross, '┷')).render(r.bottom_rule, buf);
        // The top band: you, and where.
        let ch = &st.character;
        let lw = r.band_l[0].width;
        let rw = r.band_r[0].width;
        let status = hud::status_line(
            &hud::Status {
                dev: st.dev,
                handle: &ch.handle,
                level: ch.level,
                credits: ch.credits,
                seed: o.params.seed,
                fps: None,
                mail: crate::mail_mark::unread(st),
            },
            lw,
        );
        Paragraph::new(status).render(r.band_l[0], buf);
        let hp_max = ch.hp_max(&st.content);
        let rows = [
            hud::bar_row(
                "HP",
                ch.hp,
                hp_max,
                hud::hp_colour(ch.hp, hp_max, st.tick),
                lw,
            ),
            hud::bar_row("FOCUS", ch.focus, ch.focus_max(&st.content), CYAN, lw),
            hud::bar_row(
                "XP",
                ch.xp,
                game::Character::xp_to_next(ch.level),
                hud::DIM_AMBER,
                lw,
            ),
        ];
        for (i, row) in rows.into_iter().enumerate() {
            Paragraph::new(row).render(r.band_l[i + 1], buf);
        }
        Paragraph::new(hud::location_line(where_label(o), st.clock(), rw)).render(r.band_r[0], buf);
        Paragraph::new(hud::carry_row(
            ch.weight(&st.content),
            ch.carry_cap(&st.content),
            rw,
        ))
        .render(r.band_r[1], buf);
        Paragraph::new(hud::exposure_row("", rw)).render(r.band_r[2], buf);
        // The panel: here, the map, what's near, the log.
        draw_panel(st, o, r.side, buf);
        // The footer.
        let slots = crate::screens::explore::ability_slots(st);
        let footer = hud::Footer {
            place: where_label(o).to_string(),
            saved: crate::runs::when::marked(st),
            jacked: false,
            camera: false,
            overburdened: st.character.overburdened(&st.content),
            trace: None,
            hunted: false,
            engaged: false,
            turn: None,
            run: None,
            ice: Vec::new(),
            slots_label: "ABILITIES",
            slots: &slots,
            hollow: false,
            budgets: None,
            acting: None,
            prompt: Some((String::new(), hint(o).to_string())),
            act: None,
            flash_pools: false,
        };
        for (i, row) in hud::footer_rows(&footer, area.width)
            .into_iter()
            .enumerate()
        {
            Paragraph::new(row).render(r.footer[i], buf);
        }
        if self.help {
            crate::screens::help::draw_keys(f, "KEYS · OUTSIDE", KEYS);
        }
    }
}

impl OutsideScreen {
    /// The open ground round you: what you see lit by the hour, what you saw dim.
    fn draw_ground(&self, st: &AppState, o: &Outside, c: Pos, map: Rect, buf: &mut Buffer) {
        let Some(view) = self.view.as_ref() else {
            return;
        };
        let minute = st.minute_of_day();
        let origin = Pos::new(c.x - map.width as i32 / 2, c.y - map.height as i32 / 2);
        let light = citygen::lighting::ambient(StratumKind::Street, minute);
        for sy in 0..map.height {
            for sx in 0..map.width {
                let p = origin.offset(sx as i32, sy as i32);
                let vis = if self.sight.contains(&p) {
                    Visibility::Visible
                } else if ground::remembered(o, p) {
                    Visibility::Remembered
                } else {
                    Visibility::Unseen
                };
                let cell = match view.tile(p) {
                    Some(t) if vis != Visibility::Unseen => {
                        let ctx = RenderCtx {
                            style: &st.style,
                            theme: None,
                            stratum: StratumKind::Street,
                            light,
                            tick: st.tick,
                            vis,
                            connector: None,
                            kind: None,
                            wall_mask: wall_mask(view, p, t.terrain),
                            lamps_on: true,
                            sky: None,
                            glow: None,
                        };
                        lighting::apply_vis(tileset::cell(t, &ctx), vis)
                    }
                    _ => blank(),
                };
                cell.write(buf, map.x + sx, map.y + sy);
            }
        }
        you(buf, map, origin, c);
    }
}

/// The overworld round tile `t` in the atlas look, one cell a tile.
fn draw_world(st: &AppState, world: &Overworld, t: Pos, map: Rect, buf: &mut Buffer) {
    // centred on you, held inside the world where the world is bigger than the view
    let fit = |at: i32, view: u16, len: u32| {
        let (view, len) = (view as i32, len as i32);
        if len > view {
            (at - view / 2).clamp(0, len - view)
        } else {
            (len - view) / 2
        }
    };
    let origin = Pos::new(
        fit(t.x, map.width, world.size.w),
        fit(t.y, map.height, world.size.h),
    );
    for sy in 0..map.height {
        for sx in 0..map.width {
            let p = origin.offset(sx as i32, sy as i32);
            let cell = if world.tile(p).is_some() {
                atlas_cell(world, p, &st.style)
            } else {
                blank()
            };
            cell.write(buf, map.x + sx, map.y + sy);
        }
    }
    you(buf, map, origin, t);
}

/// `@` on `at`, if it is in the view.
fn you(buf: &mut Buffer, map: Rect, origin: Pos, at: Pos) {
    let (sx, sy) = (at.x - origin.x, at.y - origin.y);
    if sx >= 0 && sy >= 0 && (sx as u16) < map.width && (sy as u16) < map.height {
        buf[(map.x + sx as u16, map.y + sy as u16)]
            .set_char('@')
            .set_style(
                Style::default()
                    .fg(YOU.to_color())
                    .add_modifier(Modifier::BOLD),
            );
    }
}

fn blank() -> Cell {
    Cell {
        ch: ' ',
        fg: ATLAS_BG,
        bg: ATLAS_BG,
        bold: false,
        dim: false,
    }
}

/// Which neighbours of `p` join its terrain, for the glyphs drawn by their neighbours
/// (a bridge's deck, its rails).
fn wall_mask(view: &GroundView, p: Pos, terrain: Terrain) -> u8 {
    use crate::render::tileset::{MASK_E, MASK_N, MASK_S, MASK_W, connects};
    [
        ((0, -1), MASK_N),
        ((1, 0), MASK_E),
        ((0, 1), MASK_S),
        ((-1, 0), MASK_W),
    ]
    .into_iter()
    .filter(|((dx, dy), _)| {
        view.tile(p.offset(*dx, *dy))
            .is_some_and(|n| connects(terrain, n.terrain))
    })
    .fold(0, |m, (_, bit)| m | bit)
}

/// The place word for the bands and the footer.
fn where_label(o: &Outside) -> &'static str {
    match o.at {
        At::Overworld(_) => "overworld",
        _ => "open ground",
    }
}

/// The footer's keys for where you are.
fn hint(o: &Outside) -> &'static str {
    match o.at {
        At::Overworld(_) => "arrows walk · enter goes down · esc menu · ? keys",
        _ => "arrows walk · W up to the overworld · esc menu · ? keys",
    }
}

/// The overworld tile you're on.
fn tile_of(o: &Outside) -> Pos {
    match o.at {
        At::Overworld(t) => t,
        At::Ground(c) => Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)),
        At::City => Pos::new(0, 0),
    }
}

/// HERE: the region (or the city) and the road, then the ground and the hour.
fn here_lines(st: &AppState, o: &Outside) -> Vec<Line<'static>> {
    let w = o.world();
    let t = tile_of(o);
    let Some(tile) = w.tile(t) else {
        return Vec::new();
    };
    let place = match (st.city.as_ref(), tile.urban) {
        (Some(city), _) if o.block(city).contains(t) => w.home().name.clone(),
        (_, Some(u)) => w
            .cities
            .get(u.city as usize)
            .map_or("the edge of town".to_string(), |c| c.name.clone()),
        _ => w
            .regions
            .iter()
            .min_by_key(|r| (r.at.x - t.x).pow(2) + (4 * (r.at.y - t.y).pow(2)))
            .map_or("the open ground".to_string(), |r| r.name.clone()),
    };
    let road = w
        .routes
        .iter()
        .find(|r| r.path.contains(&t))
        .map(|r| format!(" · {}", r.name))
        .unwrap_or_default();
    let ground = match (tile.water, tile.ground) {
        (Water::River, _) if tile.road != 0 => "a bridge",
        (Water::River, _) => "the river",
        (_, Ground::Flats) => "flats",
        (_, Ground::Scrub) => "scrub",
        (_, Ground::Fields) => "fields",
        (_, Ground::Salt) => "salt pan",
        (_, Ground::Spill) => "the spill",
        (_, Ground::Hills) => "hills",
        (_, Ground::Mountains | Ground::Peak) => "mountains",
    };
    let hour = if citygen::lighting::daylight(st.minute_of_day()) > 0.25 {
        "day"
    } else {
        "night"
    };
    vec![
        Line::from(Span::styled(
            format!(" {place}{road}"),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(" {} · {ground} · {hour}", where_label(o)),
            Style::default().fg(DIM),
        )),
    ]
}

/// NEAR: the nearest cities and places out here, with how far and which way.
fn near_lines(o: &Outside, n: usize) -> Vec<Line<'static>> {
    let w = o.world();
    let t = tile_of(o);
    let mut places: Vec<(i32, String, Pos)> = w
        .cities
        .iter()
        .map(|c| (c.name.clone(), c.centre))
        .chain(w.pois.iter().map(|p| (p.name.clone(), p.at)))
        .map(|(name, at)| {
            let (dx, dy) = (at.x - t.x, at.y - t.y);
            let d = ((dx * dx + dy * dy) as f32).sqrt().round() as i32;
            (d, name, at)
        })
        .collect();
    places.sort();
    places
        .into_iter()
        .take(n)
        .map(|(d, name, at)| {
            let way = if d == 0 {
                "here".to_string()
            } else {
                format!("{d} {}", bearing(at.x - t.x, at.y - t.y))
            };
            Line::from(vec![
                Span::styled(format!(" {name}"), Style::default().fg(TEXT)),
                Span::styled(format!("  {way}"), Style::default().fg(DIM)),
            ])
        })
        .collect()
}

/// The compass point from you toward `(dx, dy)`.
fn bearing(dx: i32, dy: i32) -> &'static str {
    let (ax, ay) = (dx.abs(), dy.abs());
    match (dx.signum(), dy.signum()) {
        _ if ax > 2 * ay => {
            if dx > 0 {
                "E"
            } else {
                "W"
            }
        }
        _ if ay > 2 * ax => {
            if dy > 0 {
                "S"
            } else {
                "N"
            }
        }
        (1, 1) => "SE",
        (1, _) => "NE",
        (_, 1) => "SW",
        _ => "NW",
    }
}

/// The panel: HERE, the map, NEAR and the log, each under a rule.
fn draw_panel(st: &AppState, o: &Outside, side: Rect, buf: &mut Buffer) {
    if side.width == 0 || side.height < 4 {
        return;
    }
    let w = side.width;
    let mut y = side.y;
    let bottom = side.bottom();
    let title = |text: &str| {
        Line::from(Span::styled(
            format!(" {text}"),
            Style::default().fg(LABEL).add_modifier(Modifier::BOLD),
        ))
    };
    let rule = Line::from(Span::styled(
        "─".repeat(w as usize),
        Style::default().fg(Color::Rgb(40, 46, 56)),
    ));
    let put = |buf: &mut Buffer, lines: Vec<Line<'static>>, y: &mut u16, rows: u16| {
        let rows = rows.min(bottom.saturating_sub(*y));
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(Rect::new(side.x, *y, w, rows), buf);
        *y += rows;
    };
    let mut here = vec![title("HERE")];
    here.extend(here_lines(st, o));
    put(buf, here, &mut y, 3);
    put(buf, vec![rule.clone()], &mut y, 1);
    put(buf, vec![title("MAP")], &mut y, 1);
    let map_rows = 9.min(bottom.saturating_sub(y + 8));
    if map_rows > 0 {
        let pane = Rect::new(side.x + 1, y, w.saturating_sub(2), map_rows);
        match o.at {
            At::Overworld(_) => world_pane(st, o, pane, buf),
            _ => round_pane(st, o, pane, buf),
        }
        y += map_rows;
    }
    put(buf, vec![rule.clone()], &mut y, 1);
    let mut near = vec![title("NEAR")];
    near.extend(near_lines(o, 4));
    put(buf, near, &mut y, 5);
    if y + 2 < bottom {
        put(buf, vec![rule], &mut y, 1);
        crate::ui::sidebar::draw_log(&st.log, st.tick, Rect::new(side.x, y, w, bottom - y), buf);
    }
}

/// The whole world scaled into the pane, two tiles a row in half blocks, `@` on you.
fn world_pane(st: &AppState, o: &Outside, pane: Rect, buf: &mut Buffer) {
    let w = o.world();
    let (ww, wh) = (w.size.w as i32, w.size.h as i32);
    let (pw, ph) = (pane.width as i32, pane.height as i32 * 2);
    let sample = |px: i32, py: i32| Pos::new(px * ww / pw.max(1), py * wh / ph.max(1));
    for y in 0..pane.height as i32 {
        for x in 0..pw {
            let (top, low) = (
                ink(st, w, sample(x, 2 * y)),
                ink(st, w, sample(x, 2 * y + 1)),
            );
            buf[(pane.x + x as u16, pane.y + y as u16)]
                .set_char('▀')
                .set_style(Style::default().fg(top.to_color()).bg(low.to_color()));
        }
    }
    let t = tile_of(o);
    let (x, y) = (t.x * pw / ww.max(1), t.y * ph / wh.max(1) / 2);
    if x < pw && y < pane.height as i32 {
        buf[(pane.x + x as u16, pane.y + y as u16)]
            .set_char('@')
            .set_style(
                Style::default()
                    .fg(YOU.to_color())
                    .add_modifier(Modifier::BOLD),
            );
    }
}

/// The colour a tile reads as on the world map: its glyph's, where it has one.
fn ink(st: &AppState, w: &Overworld, p: Pos) -> Rgb {
    let c = atlas_cell(w, p, &st.style);
    if c.ch == ' ' { c.bg } else { c.fg.scale(0.8) }
}

/// The overworld round you in the atlas look, one cell a tile, `@` on your tile.
fn round_pane(st: &AppState, o: &Outside, pane: Rect, buf: &mut Buffer) {
    let w = o.world();
    let t = tile_of(o);
    let origin = Pos::new(t.x - pane.width as i32 / 2, t.y - pane.height as i32 / 2);
    for y in 0..pane.height {
        for x in 0..pane.width {
            let p = origin.offset(x as i32, y as i32);
            let cell = if w.tile(p).is_some() {
                atlas_cell(w, p, &st.style)
            } else {
                blank()
            };
            cell.write(buf, pane.x + x, pane.y + y);
        }
    }
    you(buf, pane, origin, t);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outside::moves::step_cost;
    use crate::outside::moves::tests::on_the_world;
    use crate::screens::testutil::{buffer_text, key};
    use citygen::Dir;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn text(st: &AppState, s: &OutsideScreen, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| s.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    /// Up out of the city onto the overworld, then onto a tile outside the block.
    fn out_there() -> (AppState, OutsideScreen) {
        let mut st = on_the_world();
        let mut s = OutsideScreen::new(&mut st);
        let o = st.outside.as_ref().unwrap();
        let b = o.block(st.city.as_ref().unwrap());
        let w = o.world().clone();
        let t = w
            .tiles
            .iter()
            .map(|(p, _)| p)
            .find(|p| {
                !b.contains(*p)
                    && w.tile(*p)
                        .is_some_and(|t| t.road != 0 && t.water == Water::None)
            })
            .unwrap();
        st.outside.as_mut().unwrap().at = At::Overworld(t);
        s.settle(&mut st);
        (st, s)
    }

    #[test]
    fn going_up_from_the_city_lands_on_its_block() {
        let mut st = on_the_world();
        let _ = OutsideScreen::new(&mut st);
        let o = st.outside.as_ref().unwrap();
        let At::Overworld(t) = o.at else {
            panic!("on the overworld")
        };
        assert!(o.block(st.city.as_ref().unwrap()).contains(t));
    }

    #[test]
    fn the_overworld_draws_in_the_play_screen() {
        let (st, s) = out_there();
        let page = text(&st, &s, 120, 40);
        assert!(page.contains("on foot · overworld"), "{page}");
        assert!(page.contains('@'));
        assert!(page.contains("HERE") && page.contains("NEAR"));
        let w = st.outside.as_ref().unwrap().world();
        let named = w.cities.iter().any(|c| page.contains(&c.name))
            || w.pois.iter().any(|p| page.contains(&p.name));
        assert!(named, "a city or place named in NEAR: {page}");
        assert!(page.contains("enter goes down"));
    }

    #[test]
    fn it_fits_the_smallest_terminal() {
        let (mut st, mut s) = out_there();
        for (w, h) in [(100, 30), (120, 40), (60, 20)] {
            let page = text(&st, &s, w, h);
            assert!(page.contains('@') || w < 100, "{w}x{h}");
        }
        s.update(&mut st, &[key(KeyCode::Enter)]);
        let page = text(&st, &s, 100, 30);
        assert!(page.contains('@') && page.contains("open ground"), "{page}");
    }

    #[test]
    fn enter_goes_down_and_w_comes_up() {
        let (mut st, mut s) = out_there();
        let At::Overworld(t) = st.outside.as_ref().unwrap().at else {
            panic!()
        };
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(matches!(st.outside.as_ref().unwrap().at, At::Ground(_)));
        let page = text(&st, &s, 120, 40);
        assert!(page.contains("on foot · open ground"), "{page}");
        assert!(page.contains("W up to the overworld"));
        s.update(&mut st, &[key(KeyCode::Char('W'))]);
        assert_eq!(st.outside.as_ref().unwrap().at, At::Overworld(t));
    }

    #[test]
    fn walking_into_the_city_switches_to_the_map() {
        let mut st = on_the_world();
        let mut s = OutsideScreen::new(&mut st);
        let o = st.outside.as_ref().unwrap();
        let b = o.block(st.city.as_ref().unwrap());
        let w = o.world().clone();
        let (from, d) = (b.y..b.y + b.h)
            .flat_map(|y| {
                [
                    (Pos::new(b.x - 1, y), Dir::E),
                    (Pos::new(b.x + b.w, y), Dir::W),
                ]
            })
            .find(|(p, d)| step_cost(&w, *p).is_some() && step_cost(&w, p.step(*d)).is_some())
            .unwrap();
        st.outside.as_mut().unwrap().at = At::Overworld(from);
        let code = if d == Dir::E {
            KeyCode::Char('l')
        } else {
            KeyCode::Char('h')
        };
        assert!(matches!(
            s.update(&mut st, &[key(code)]),
            ScreenCmd::Switch { .. }
        ));
        assert_eq!(st.outside.as_ref().unwrap().at, At::City);
    }

    fn map(st: &mut AppState) -> crate::screens::explore::MapScreen {
        crate::screens::explore::MapScreen::walking(st)
    }

    #[test]
    fn w_on_the_open_street_goes_up_to_the_world() {
        let mut st = on_the_world();
        let city = st.city.clone().unwrap();
        let o = st.outside.as_ref().unwrap();
        let cell = o.street_of(&city, Pos::new(o.block(&city).x + 10, o.block(&city).y + 7));
        let p = st.player.as_mut().unwrap();
        p.place = citygen::Place::Stratum(StratumKind::Street);
        p.pos = cell;
        let mut m = map(&mut st);
        assert!(matches!(
            m.update(&mut st, &[key(KeyCode::Char('W'))]),
            ScreenCmd::Switch { .. }
        ));
        let o = st.outside.as_ref().unwrap();
        assert_eq!(o.at, At::Overworld(o.tile_of(&city, cell)));
    }

    #[test]
    fn w_indoors_says_why_not() {
        let mut st = on_the_world();
        let city = st.city.clone().unwrap();
        let b = city.buildings[0].id;
        let p = st.player.as_mut().unwrap();
        p.place = citygen::Place::Floor {
            building: b,
            level: 0,
        };
        let mut m = map(&mut st);
        assert!(matches!(
            m.update(&mut st, &[key(KeyCode::Char('W'))]),
            ScreenCmd::Stay
        ));
        assert_eq!(st.outside.as_ref().unwrap().at, At::City);
        assert!(
            st.log
                .recent(st.tick)
                .is_some_and(|e| e.text.contains("open street"))
        );
    }

    #[test]
    fn the_streets_edge_leads_out() {
        let mut st = on_the_world();
        let city = st.city.clone().unwrap();
        let o = st.outside.as_ref().unwrap();
        let b = o.block(&city);
        let w = o.world().clone();
        let street = |p: Pos| {
            city.tile(StratumKind::Street, p)
                .is_some_and(|t| t.is_walkable() && t.building.is_none())
        };
        let open = |c: Pos| {
            citygen::overworld::detail::region(&w, c.x, c.y, 1, 1)[Pos::new(0, 0)].is_walkable()
        };
        let (cw, ch) = (city.size.w as i32, city.size.h as i32);
        let world_cell = |q: Pos| Pos::new(TILE * b.x + q.x, TILE * b.y + q.y);
        let edges = (0..ch)
            .flat_map(|y| [(Pos::new(0, y), Dir::W), (Pos::new(cw - 1, y), Dir::E)])
            .chain((0..cw).flat_map(|x| [(Pos::new(x, 0), Dir::N), (Pos::new(x, ch - 1), Dir::S)]));
        let (from, d) = edges
            .into_iter()
            .find(|(q, d)| street(*q) && open(world_cell(q.step(*d))))
            .expect("a street out of the city");
        let p = st.player.as_mut().unwrap();
        p.place = citygen::Place::Stratum(StratumKind::Street);
        p.pos = from;
        let mut m = map(&mut st);
        let code = match d {
            Dir::W => KeyCode::Char('h'),
            Dir::E => KeyCode::Char('l'),
            Dir::N => KeyCode::Char('k'),
            _ => KeyCode::Char('j'),
        };
        assert!(matches!(
            m.update(&mut st, &[key(code)]),
            ScreenCmd::Switch { .. }
        ));
        assert_eq!(
            st.outside.as_ref().unwrap().at,
            At::Ground(world_cell(from.step(d)))
        );
    }

    #[test]
    fn a_run_saved_outside_reopens_outside() {
        let (mut st, _) = out_there();
        let s = crate::screens::resume_screen(&mut st);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("on foot · overworld"));
        st.outside.as_mut().unwrap().at = At::City;
        let s = crate::screens::resume_screen(&mut st);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(!buffer_text(t.backend().buffer()).contains("on foot · overworld"));
    }

    #[test]
    fn the_help_lists_w() {
        let mut t = Terminal::new(TestBackend::new(120, 60)).unwrap();
        t.draw(|f| crate::screens::help::draw(f, crate::screens::explore::Mode::Walk, false))
            .unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("up to the overworld"));
    }

    #[test]
    fn esc_opens_the_pause_menu() {
        let (mut st, mut s) = out_there();
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Push { .. }
        ));
    }

    #[test]
    fn a_closed_step_says_why() {
        let (mut st, mut s) = out_there();
        let w = st.outside.as_ref().unwrap().world().clone();
        let edge = Pos::new(0, 10);
        assert!(step_cost(&w, edge).is_some() || step_cost(&w, edge).is_none());
        st.outside.as_mut().unwrap().at = At::Overworld(Pos::new(0, 10));
        s.update(&mut st, &[key(KeyCode::Char('h'))]);
        assert!(
            st.log
                .recent(st.tick)
                .is_some_and(|e| e.text.contains("world ends")),
            "{:?}",
            st.log.recent(st.tick).map(|e| e.text.clone())
        );
    }
}
