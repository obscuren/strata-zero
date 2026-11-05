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
use std::cell::Cell as Kept;
use std::collections::BTreeSet;

/// The least light on the open ground, at night.
const MOONLIGHT: u8 = 110;

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
    (
        "W",
        "up: from the open ground to the overworld · jacked in, the Net over it",
    ),
    ("space", "jack in at a port in reach: a cabinet or a pump"),
    ("J", "jacked in: jack out onto your body"),
    ("s", "save game"),
    ("?", "this help"),
    ("esc q", "pause menu"),
];

/// The keys while you look up at the Net over the world.
pub const NET_UP_KEYS: &[(&str, &str)] = &[
    ("enter esc W", "back down to where you were on the Net"),
    ("J", "jack out onto your body"),
    ("hjkl yubn ↑↓←→", "nothing: the Net out there is to look at"),
    ("?", "this help"),
];

/// The overworld and the open ground, as a level of play.
pub struct OutsideScreen {
    /// The ground round you while you're on it.
    view: Option<GroundView>,
    /// Its Net while you're jacked in out here.
    net_view: Option<GroundView>,
    /// What you see from where you stand on the ground.
    sight: BTreeSet<Pos>,
    help: bool,
    /// The map area's size as last drawn, so the ground is built for all of it.
    map: Kept<(u16, u16)>,
    /// Jacked in, looking up at the Net over the world (look only).
    pub(crate) net_up: Option<NetUp>,
    /// The world jacked in, drawn from once it's first looked at.
    net_atlas: std::cell::OnceCell<crate::render::overworld::NetAtlas>,
}

/// Looking up at the Net over the world: over which tile, and whether from the city's
/// map (going back down there closes this screen).
pub(crate) struct NetUp {
    pub tile: Pos,
    from_city: bool,
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
            net_view: None,
            sight: BTreeSet::new(),
            help: false,
            // a big window's map until the first frame says otherwise, so even the
            // first frame on the ground has nothing still to build
            map: Kept::new((240, 80)),
            net_up: None,
            net_atlas: Default::default(),
        };
        s.settle(st);
        s
    }

    /// Jacked in on the city's Net: the Net over the world, looking up from the city's
    /// tile you're on (look only; ↵ or esc goes back down to the city's Net).
    pub fn net_from_city(st: &mut AppState) -> OutsideScreen {
        crate::outside::ensure(st);
        let tile = match (st.outside.as_ref(), st.city.as_ref(), st.player.as_ref()) {
            (Some(o), Some(city), Some(p)) => o.tile_of(city, p.pos),
            _ => Pos::new(0, 0),
        };
        OutsideScreen {
            view: None,
            net_view: None,
            sight: BTreeSet::new(),
            help: false,
            map: Kept::new((240, 80)),
            net_up: Some(NetUp {
                tile,
                from_city: true,
            }),
            net_atlas: Default::default(),
        }
    }

    /// The keys while you look up at the Net over the world.
    fn net_up_key(&mut self, st: &mut AppState, k: &KeyEvent) -> ScreenCmd {
        let from_city = self.net_up.as_ref().is_some_and(|u| u.from_city);
        let down = |s: &mut Self| {
            if from_city {
                ScreenCmd::Pop {
                    fx: TransitionKind::Glitch,
                }
            } else {
                s.net_up = None;
                ScreenCmd::Stay
            }
        };
        match k.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('W') | KeyCode::Char('q') => down(self),
            KeyCode::Char('J') if from_city => {
                match crate::net::jack_out(st) {
                    Ok(()) => st.say("jacked out"),
                    Err(why) => st.warn(why),
                }
                down(self)
            }
            KeyCode::Char('J') => {
                if let Some(o) = st.outside.as_mut() {
                    o.net = None;
                }
                self.net_view = None;
                self.net_up = None;
                st.say("jacked out");
                self.settle(st);
                ScreenCmd::Stay
            }
            KeyCode::Char('?') => {
                self.help = true;
                ScreenCmd::Stay
            }
            code => {
                if dir_for(code).is_some() {
                    st.warn("the Net out there can be looked at, not walked");
                }
                ScreenCmd::Stay
            }
        }
    }

    /// The ground view and what you see, for where you are now.
    fn settle(&mut self, st: &mut AppState) {
        let minute = st.minute_of_day();
        let Some(o) = st.outside.as_mut() else {
            return;
        };
        match o.at {
            At::Ground(c) => {
                // all that's drawn and all you might see, built before it's drawn
                let (mw, mh) = self.map.get();
                let view = self.view.get_or_insert_with(GroundView::default);
                view.cover(
                    o.world(),
                    citygen::Rect::new(
                        c.x - mw as i32 / 2,
                        c.y - mh as i32 / 2,
                        mw as i32,
                        mh as i32,
                    ),
                );
                view.cover(o.world(), ground::sight_area(c));
                if let Some(n) = o.net {
                    let nv = self.net_view.get_or_insert_with(GroundView::net);
                    nv.cover(
                        o.world(),
                        citygen::Rect::new(
                            n.x - mw as i32 / 2,
                            n.y - mh as i32 / 2,
                            mw as i32,
                            mh as i32,
                        ),
                    );
                } else {
                    self.net_view = None;
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
        if self.net_up.is_some() {
            return self.net_up_key(st, k);
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                return ScreenCmd::Push {
                    to: Box::new(Pause::new(st, st.tick)),
                    fx: TransitionKind::None,
                };
            }
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('s') => match crate::save::refusal(st) {
                Some(why) => st.warn(why),
                None => {
                    crate::runs::when::save_now(st);
                }
            },
            KeyCode::Tab => {
                // out in the open the MAP is the world's
                st.atlas.zoom = crate::screens::atlas::Zoom::World;
                st.atlas.world.cursor = None;
                let street = citygen::Place::Stratum(StratumKind::Street);
                let (place, pos) = st
                    .player
                    .as_ref()
                    .map_or((street, Pos::new(0, 0)), |p| (p.place, p.pos));
                return ScreenCmd::Push {
                    to: Box::new(
                        crate::screens::deck::Deck::new(st.tick)
                            .focus(place, pos)
                            .restore(st),
                    ),
                    fx: TransitionKind::Glitch,
                };
            }
            KeyCode::Char('1'..='5') => st.warn("abilities don't work out here yet"),
            KeyCode::Char(' ') if matches!(at, Some(At::Ground(_))) && !jacked(st) => {
                self.jack_in(st)
            }
            KeyCode::Char('J') if jacked(st) => {
                if let Some(o) = st.outside.as_mut() {
                    o.net = None;
                }
                self.net_view = None;
                st.say("jacked out");
                self.settle(st);
            }
            // jacked in out here: look up at the Net over the world
            KeyCode::Char('W') if jacked(st) => {
                if let Some(At::Ground(c)) = at {
                    self.net_up = Some(NetUp {
                        tile: Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)),
                        from_city: false,
                    });
                }
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
                if jacked(st) {
                    self.net_step(st, d);
                    return ScreenCmd::Stay;
                }
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

/// Jacked in on the open ground.
fn jacked(st: &AppState) -> bool {
    st.outside.as_ref().is_some_and(|o| o.net.is_some())
}

/// Whether Net cell `p` is within the body's reach: its tile and the eight round it
/// (until how far the Net reaches is decided).
fn in_reach(body: Pos, p: Pos) -> bool {
    let t = |c: Pos| (c.x.div_euclid(TILE), c.y.div_euclid(TILE));
    let ((bx, by), (px, py)) = (t(body), t(p));
    (bx - px).abs() <= 1 && (by - py).abs() <= 1
}

impl OutsideScreen {
    /// Space on the ground: jack in at a port in reach (a POI's cabinet or a pump).
    fn jack_in(&mut self, st: &mut AppState) {
        let Some(At::Ground(body)) = st.outside.as_ref().map(|o| o.at) else {
            return;
        };
        let port = self.view.as_ref().is_some_and(|v| {
            std::iter::once(body).chain(body.neighbors8()).any(|q| {
                v.tile(q).is_some_and(|t| {
                    matches!(
                        t.feature,
                        Some(citygen::Feature::Terminal)
                            | Some(citygen::Feature::Furniture(citygen::Furniture::Vending))
                    )
                })
            })
        });
        if !port {
            st.warn("no port in reach");
            return;
        }
        if let Some(left) = st.net.cooling(st.net.now(st)) {
            st.warn(format!("deck cooling · {left} min"));
            return;
        }
        let Some(o) = st.outside.as_mut() else {
            return;
        };
        let mut nv = GroundView::net();
        nv.cover(o.world(), ground::sight_area(body));
        let start = crate::outside::nearest(body, 8, |p| {
            nv.tile(p)
                .is_some_and(|t| t.is_walkable() && t.terrain != citygen::Terrain::Void)
        });
        let Some(start) = start else {
            st.warn("no signal here");
            return;
        };
        o.net = Some(start);
        self.net_view = Some(nv);
        st.say("jacked in · the open Net");
        self.settle(st);
    }

    /// A step on the open Net, within the body's reach.
    fn net_step(&mut self, st: &mut AppState, d: citygen::Dir) {
        let Some((At::Ground(body), Some(n))) = st.outside.as_ref().map(|o| (o.at, o.net)) else {
            return;
        };
        let to = n.step(d);
        if !in_reach(body, to) {
            st.warn("the signal is too thin out there");
            return;
        }
        // the void between signal is barren but walkable; a construct's wall is not
        let ok = self
            .net_view
            .as_ref()
            .and_then(|v| v.tile(to))
            .is_some_and(|t| t.terrain == citygen::Terrain::Void || t.is_walkable());
        if !ok {
            return;
        }
        if let Some(o) = st.outside.as_mut() {
            o.net = Some(to);
        }
        crate::clock::run(st, Some(sim::WALK_TICKS), crate::clock::PATHS_PER_FRAME);
        self.settle(st);
    }

    /// The open Net round you: all of it in reach seen, past it dim; your body grey on
    /// the ground below.
    fn draw_net(&self, st: &AppState, body: Pos, n: Pos, map: Rect, buf: &mut Buffer) {
        let Some(view) = self.net_view.as_ref() else {
            return;
        };
        let origin = Pos::new(n.x - map.width as i32 / 2, n.y - map.height as i32 / 2);
        for sy in 0..map.height {
            for sx in 0..map.width {
                let p = origin.offset(sx as i32, sy as i32);
                let cell = match view.tile(p) {
                    Some(t) => {
                        let vis = if in_reach(body, p) {
                            Visibility::Visible
                        } else {
                            Visibility::Remembered
                        };
                        let ctx = RenderCtx {
                            style: &st.style,
                            theme: None,
                            stratum: StratumKind::Net,
                            light: 255,
                            tick: st.tick,
                            vis,
                            connector: None,
                            kind: None,
                            wall_mask: wall_mask(view, p, t.terrain),
                            lamps_on: true,
                            sky: None,
                            glow: None,
                        };
                        let mut c = tileset::cell(t, &ctx);
                        if t.terrain == citygen::Terrain::Void {
                            c.bg = crate::render::netlook::VOID;
                        }
                        lighting::apply_vis(c, vis)
                    }
                    None => blank(),
                };
                cell.write(buf, map.x + sx, map.y + sy);
            }
        }
        let (bx, by) = (body.x - origin.x, body.y - origin.y);
        if bx >= 0 && by >= 0 && (bx as u16) < map.width && (by as u16) < map.height {
            buf[(map.x + bx as u16, map.y + by as u16)]
                .set_char('@')
                .set_style(
                    Style::default()
                        .fg(Color::Rgb(110, 120, 130))
                        .add_modifier(Modifier::DIM),
                );
        }
        let (sx, sy) = (n.x - origin.x, n.y - origin.y);
        if sx >= 0 && sy >= 0 && (sx as u16) < map.width && (sy as u16) < map.height {
            buf[(map.x + sx as u16, map.y + sy as u16)]
                .set_char('@')
                .set_style(
                    Style::default()
                        .fg(crate::render::netlook::YOU.to_color())
                        .bg(crate::render::netlook::YOU_GLOW.to_color())
                        .add_modifier(Modifier::BOLD),
                );
        }
    }
}

impl Screen for OutsideScreen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // The city's clock runs out here as it does on the map, but not while you look
        // up at the Net over the world.
        if self.net_up.is_none() {
            crate::clock::run(st, Some(1), crate::clock::PATHS_PER_FRAME);
            // the ground for the view as drawn, and sight as the hour turns
            self.settle(st);
        }
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
        // the whole frame is this screen's, over whatever it was pushed on
        Widget::render(ratatui::widgets::Clear, area, f.buffer_mut());
        let r = layout(area, false);
        self.map.set((r.map.width, r.map.height));
        let buf = f.buffer_mut();
        match (o.at, o.net, self.net_up.as_ref()) {
            (_, _, Some(up)) => {
                let atlas = self
                    .net_atlas
                    .get_or_init(|| crate::render::overworld::NetAtlas::new(o.world()));
                draw_net_world(st, o.world(), atlas, up.tile, r.map, buf);
            }
            (At::Ground(c), Some(n), None) => self.draw_net(st, c, n, r.map, buf),
            (At::Ground(c), None, None) => self.draw_ground(st, o, c, r.map, buf),
            (At::Overworld(t), _, None) => draw_world(st, o.world(), t, r.map, buf),
            (At::City, _, None) => {}
        }
        let up = self.net_up.is_some();
        let place = if up {
            "the Net overworld"
        } else {
            where_label(o)
        };
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
        Paragraph::new(hud::location_line(place, st.clock(), rw)).render(r.band_r[0], buf);
        Paragraph::new(hud::carry_row(
            ch.weight(&st.content),
            ch.carry_cap(&st.content),
            rw,
        ))
        .render(r.band_r[1], buf);
        Paragraph::new(hud::exposure_row("", rw)).render(r.band_r[2], buf);
        // The panel: here, the map, what's near, the log.
        let log_drawn = draw_panel(st, o, up, r.side, buf);
        // The footer.
        let slots = crate::screens::explore::ability_slots(st);
        let footer = hud::Footer {
            place: place.to_string(),
            saved: crate::runs::when::marked(st),
            jacked: o.net.is_some() || up,
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
            // abilities don't work out here yet
            hollow: true,
            budgets: None,
            acting: None,
            prompt: Some((
                String::new(),
                if up {
                    "look only · enter back down · J jack out · ? keys".to_string()
                } else {
                    hint(o).to_string()
                },
            )),
            act: None,
            flash_pools: false,
        };
        for (i, row) in hud::footer_rows(&footer, area.width)
            .into_iter()
            .enumerate()
        {
            Paragraph::new(row).render(r.footer[i], buf);
        }
        // With no room for the LOG pane, the newest entry on the footer's free row.
        if !log_drawn && let Some(e) = st.log.recent(st.tick) {
            let fg = match e.kind {
                crate::messages::Kind::Warn => crate::ui::chrome::MAGENTA,
                crate::messages::Kind::Story => CYAN,
                crate::messages::Kind::Info => TEXT,
            };
            Paragraph::new(Line::from(Span::styled(
                format!(" {}", e.text),
                Style::default().fg(fg),
            )))
            .render(r.footer[2], buf);
        }
        if self.help {
            if self.net_up.is_some() {
                crate::screens::help::draw_keys(f, "KEYS · THE NET OVERWORLD", NET_UP_KEYS);
            } else {
                crate::screens::help::draw_keys(f, "KEYS · OUTSIDE", KEYS);
            }
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
        // by night the moon, never as dark as an unlit street, and the lamps' pools
        let light = citygen::lighting::ambient(StratumKind::Street, minute).max(MOONLIGHT);
        let lamps = view.lamps_near(c, map.width as i32 / 2 + ground::LAMP_REACH as i32);
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
                            light: light.max(ground::lamp_light(&lamps, p)),
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
    if o.net.is_some() {
        return "the Net";
    }
    match o.at {
        At::Overworld(_) => "overworld",
        _ => "open ground",
    }
}

/// The footer's keys for where you are.
fn hint(o: &Outside) -> &'static str {
    if o.net.is_some() {
        return "arrows run · J jack out · esc menu · ? keys";
    }
    match o.at {
        At::Overworld(_) => "arrows walk · enter goes down · esc menu · ? keys",
        _ => "arrows walk · W up to the overworld · esc menu · ? keys",
    }
}

/// The overworld tile you're on (in the city, the one its street there lies in).
fn tile_of(st: &AppState, o: &Outside) -> Pos {
    match o.at {
        At::Overworld(t) => t,
        At::Ground(c) => Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)),
        At::City => match (st.city.as_ref(), st.player.as_ref()) {
            (Some(city), Some(p)) => o.tile_of(city, p.pos),
            _ => Pos::new(0, 0),
        },
    }
}

/// HERE jacked in, looking up: the trunk you're over (or the city's node field, or
/// the dead grid), and that it's for looking.
fn net_here_lines(st: &AppState, o: &Outside) -> Vec<Line<'static>> {
    let w = o.world();
    let t = tile_of(st, o);
    let over = match (
        w.routes.iter().find(|r| r.path.contains(&t)),
        crate::outside::settlement_at(st, t),
    ) {
        (Some(r), _) => format!("{} trunk", r.name),
        (None, Some(s)) => o.name(s),
        (None, None) => "dead grid".to_string(),
    };
    vec![
        Line::from(Span::styled(
            format!(" the Net · {over}"),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            " the Net overworld · look only".to_string(),
            Style::default().fg(DIM),
        )),
    ]
}

/// The Net over the world round `t`, as `draw_world` draws the world, `@` on `t`.
fn draw_net_world(
    st: &AppState,
    world: &Overworld,
    atlas: &crate::render::overworld::NetAtlas,
    t: Pos,
    map: Rect,
    buf: &mut Buffer,
) {
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
                atlas.cell(world, p, &st.style, st.tick)
            } else {
                blank()
            };
            cell.write(buf, map.x + sx, map.y + sy);
        }
    }
    you(buf, map, origin, t);
}

/// HERE: the region (or the city) and the road, then the ground and the hour.
fn here_lines(st: &AppState, o: &Outside) -> Vec<Line<'static>> {
    let w = o.world();
    let t = tile_of(st, o);
    let Some(tile) = w.tile(t) else {
        return Vec::new();
    };
    let place = match (crate::outside::settlement_at(st, t), tile.urban) {
        (Some(s), _) => o.name(s),
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
fn near_lines(st: &AppState, o: &Outside, n: usize) -> Vec<Line<'static>> {
    // the mark from the MAP first, then what's nearest
    let mark = crate::outside::mark_way(st).map(|(name, d, dir)| {
        Line::from(vec![
            Span::styled(" ⌖ ", Style::default().fg(CYAN)),
            Span::styled(name, Style::default().fg(TEXT)),
            Span::styled(format!("  {d} {dir}"), Style::default().fg(DIM)),
        ])
    });
    let n = n - mark.is_some() as usize;
    let mut out: Vec<Line<'static>> = mark.into_iter().collect();
    out.extend(nearest_lines(st, o, n));
    out
}

/// NEAR's places: the nearest cities and places out here, with how far and which way.
fn nearest_lines(st: &AppState, o: &Outside, n: usize) -> Vec<Line<'static>> {
    let w = o.world();
    let t = tile_of(st, o);
    // the city whose tiles you stand on is here, whatever its middle's distance
    let in_city = |i: usize| {
        w.tile(t)
            .and_then(|x| x.urban)
            .is_some_and(|u| u.city as usize == i)
            || crate::outside::settlement_at(st, t)
                .is_some_and(|s| s.town.is_none() && s.city as usize == i)
    };
    let mut places: Vec<(i32, String, Pos)> = w
        .cities
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.clone(), c.centre, in_city(i)))
        .chain(w.pois.iter().map(|p| (p.name.clone(), p.at, p.at == t)))
        .map(|(name, at, here)| {
            let (dx, dy) = (at.x - t.x, at.y - t.y);
            let d = if here {
                0
            } else {
                ((dx * dx + dy * dy) as f32).sqrt().round().max(1.0) as i32
            };
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
pub(crate) fn bearing(dx: i32, dy: i32) -> &'static str {
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

/// The panel: HERE, the map, NEAR and the log, each under a rule; whether the log fit.
fn draw_panel(st: &AppState, o: &Outside, net_up: bool, side: Rect, buf: &mut Buffer) -> bool {
    if side.width == 0 || side.height < 4 {
        return false;
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
    if net_up {
        here.extend(net_here_lines(st, o));
    } else {
        here.extend(here_lines(st, o));
    }
    put(buf, here, &mut y, 3);
    put(buf, vec![rule.clone()], &mut y, 1);
    put(buf, vec![title("MAP")], &mut y, 1);
    let map_rows = 9.min(bottom.saturating_sub(y + 8));
    if map_rows > 0 {
        let pane = Rect::new(side.x + 1, y, w.saturating_sub(2), map_rows);
        match o.at {
            _ if net_up => world_pane(st, o, pane, buf),
            At::Overworld(_) => world_pane(st, o, pane, buf),
            _ => round_pane(st, o, pane, buf),
        }
        y += map_rows;
    }
    put(buf, vec![rule.clone()], &mut y, 1);
    let mut near = vec![title("NEAR")];
    near.extend(near_lines(st, o, 4));
    put(buf, near, &mut y, 5);
    if y + 2 < bottom {
        put(buf, vec![rule], &mut y, 1);
        crate::ui::sidebar::draw_log(&st.log, st.tick, Rect::new(side.x, y, w, bottom - y), buf);
        return true;
    }
    false
}

/// The whole world scaled into the pane, two tiles a row in half blocks, `@` on you.
fn world_pane(st: &AppState, o: &Outside, pane: Rect, buf: &mut Buffer) {
    let w = o.world();
    let (ww, wh) = (w.size.w as i32, w.size.h as i32);
    let (pw, ph) = (pane.width as i32, pane.height as i32 * 2);
    // each half-block pixel the mean of the tiles it covers, so the map reads in areas
    let span = |i: i32, len: i32, n: i32| {
        let a = i * len / n.max(1);
        (a, ((i + 1) * len / n.max(1)).max(a + 1))
    };
    let pixel = |px: i32, py: i32| {
        let ((x0, x1), (y0, y1)) = (span(px, ww, pw), span(py, wh, ph));
        let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
        for ty in y0..y1 {
            for tx in x0..x1 {
                let c = ink(st, w, Pos::new(tx, ty));
                (r, g, b, n) = (r + c.0 as u32, g + c.1 as u32, b + c.2 as u32, n + 1);
            }
        }
        let n = n.max(1);
        Rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
    };
    for y in 0..pane.height as i32 {
        for x in 0..pw {
            let (top, low) = (pixel(x, 2 * y), pixel(x, 2 * y + 1));
            buf[(pane.x + x as u16, pane.y + y as u16)]
                .set_char('▀')
                .set_style(Style::default().fg(top.to_color()).bg(low.to_color()));
        }
    }
    let t = tile_of(st, o);
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
    let t = tile_of(st, o);
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
                    && !crate::outside::in_any_block(&st, *p)
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
        // the street carries on onto the ground, on the city's own screen
        assert!(matches!(m.update(&mut st, &[key(code)]), ScreenCmd::Stay));
        let to = from.step(d);
        assert_eq!(st.player.as_ref().unwrap().pos, to);
        assert!(!city.size.contains(to));
        assert_eq!(st.outside.as_ref().unwrap().at, At::City);
        let c = world_cell(to);
        assert_eq!(
            crate::outside::your_tile(&st),
            Some(Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)))
        );
        assert!(crate::outside::away(&st));
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
    fn the_city_you_stand_in_is_here() {
        let mut st = on_the_world();
        let _ = OutsideScreen::new(&mut st);
        let o = st.outside.as_ref().unwrap();
        let home = o.world().home().name.clone();
        let lines: Vec<String> = near_lines(&st, o, 4)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect();
        assert!(
            lines
                .iter()
                .any(|l| l.contains(&home) && l.contains("here")),
            "{lines:?}"
        );
    }

    #[test]
    fn walking_into_the_city_sees_where_you_stand() {
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
        s.update(&mut st, &[key(code)]);
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.vis().get(p.pos), Some(&Visibility::Visible));
    }

    #[test]
    fn the_ground_is_built_ahead_of_the_view() {
        let (mut st, mut s) = out_there();
        s.update(&mut st, &[key(KeyCode::Enter)]);
        let w = st.outside.as_ref().unwrap().world().clone();
        let (cw, ch) = (TILE * w.size.w as i32, TILE * w.size.h as i32);
        let map = layout(Rect::new(0, 0, 200, 60), false).map;
        for step in 0..80 {
            let At::Ground(c) = st.outside.as_ref().unwrap().at else {
                panic!()
            };
            let (x0, y0) = (c.x - map.width as i32 / 2, c.y - map.height as i32 / 2);
            for y in y0..y0 + map.height as i32 {
                for x in x0..x0 + map.width as i32 {
                    let p = Pos::new(x, y);
                    let inside = x >= 0 && y >= 0 && x < cw && y < ch;
                    assert!(
                        !inside || s.view.as_ref().unwrap().tile(p).is_some(),
                        "step {step}: {p:?} not built"
                    );
                }
            }
            s.update(&mut st, &[key(KeyCode::Char('l'))]);
        }
    }

    #[test]
    fn refusals_show_at_the_smallest_terminal() {
        let (mut st, mut s) = out_there();
        st.outside.as_mut().unwrap().at = At::Overworld(Pos::new(0, 10));
        s.update(&mut st, &[key(KeyCode::Char('h'))]);
        let page = text(&st, &s, 100, 30);
        assert!(page.contains("the world ends here"), "{page}");
    }

    #[test]
    fn tab_opens_the_deck_and_abilities_say_not_here() {
        let (mut st, mut s) = out_there();
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Tab)]),
            ScreenCmd::Push { .. }
        ));
        s.update(&mut st, &[key(KeyCode::Char('1'))]);
        assert!(
            st.log
                .recent(st.tick)
                .is_some_and(|e| e.text.contains("out here"))
        );
    }

    #[test]
    fn a_run_reopened_onto_changed_ground_lands_in_the_city() {
        let (mut st, _) = out_there();
        // the world under a saved run has changed: its cell is now in the sea, or gone
        st.outside.as_mut().unwrap().at = At::Ground(Pos::new(-50, -50));
        crate::runs::snapshot::after_restore(&mut st, Vec::new());
        assert_eq!(st.outside.as_ref().unwrap().at, At::City);
        assert!(
            st.log
                .recent(st.tick)
                .is_some_and(|e| e.text.contains("world"))
        );
    }

    #[test]
    fn w_waits_for_a_scene_or_a_call() {
        let mut st = on_the_world();
        let city = st.city.clone().unwrap();
        let o = st.outside.as_ref().unwrap();
        let cell = o.street_of(&city, Pos::new(o.block(&city).x + 10, o.block(&city).y + 7));
        let p = st.player.as_mut().unwrap();
        p.place = citygen::Place::Stratum(StratumKind::Street);
        p.pos = cell;
        st.missions.scenes.push_back((None, "waking".to_string()));
        assert!(crate::outside::can_leave(&st).is_err());
    }

    /// On the ground beside a truck stop's pump, and the screen.
    fn by_a_pump() -> (AppState, OutsideScreen) {
        use citygen::overworld::PoiKind;
        let mut st = on_the_world();
        let w = st.outside.as_ref().unwrap().world().clone();
        let poi = w
            .pois
            .iter()
            .find(|p| {
                p.kind == PoiKind::TruckStop
                    && (-3..=3).all(|dy| {
                        (-3..=3).all(|dx| !crate::outside::in_any_block(&st, p.at.offset(dx, dy)))
                    })
            })
            .unwrap()
            .clone();
        let o = Pos::new(TILE * (poi.at.x - 3), TILE * (poi.at.y - 3));
        let g = citygen::overworld::detail::region(&w, o.x, o.y, 7 * TILE as u32, 7 * TILE as u32);
        let pump = g
            .iter()
            .find(|(_, t)| {
                t.feature == Some(citygen::Feature::Furniture(citygen::Furniture::Vending))
            })
            .map(|(p, _)| p)
            .unwrap();
        let beside = pump
            .neighbors4()
            .find(|q| g.get(*q).is_some_and(|t| t.is_walkable()))
            .unwrap();
        st.outside.as_mut().unwrap().at = At::Ground(Pos::new(o.x + beside.x, o.y + beside.y));
        let s = OutsideScreen::new(&mut st);
        (st, s)
    }

    #[test]
    fn space_by_a_terminal_jacks_in() {
        let (mut st, mut s) = by_a_pump();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(st.outside.as_ref().unwrap().net.is_some(), "jacked in");
        let page = text(&st, &s, 120, 40);
        assert!(page.contains("the Net"), "{page}");
        // away from any port it says why not
        let (mut st, mut s) = out_there();
        s.update(&mut st, &[key(KeyCode::Enter)]);
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(st.outside.as_ref().unwrap().net.is_none());
        assert!(
            st.log
                .recent(st.tick)
                .is_some_and(|e| e.text.contains("port"))
        );
    }

    #[test]
    fn the_net_out_here_ends_at_the_bodys_reach() {
        let (mut st, mut s) = by_a_pump();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        let At::Ground(body) = st.outside.as_ref().unwrap().at else {
            panic!()
        };
        for _ in 0..60 {
            s.update(&mut st, &[key(KeyCode::Char('l'))]);
        }
        let net = st.outside.as_ref().unwrap().net.unwrap();
        assert!(
            net.x.div_euclid(TILE) - body.x.div_euclid(TILE) <= 1,
            "{net:?} from {body:?}"
        );
        // at the edge of reach, a step further is refused
        let edge = Pos::new(TILE * (body.x.div_euclid(TILE) + 1) + TILE - 1, body.y);
        st.outside.as_mut().unwrap().net = Some(edge);
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(st.outside.as_ref().unwrap().net, Some(edge));
        assert!(
            st.log
                .recent(st.tick)
                .is_some_and(|e| e.text.contains("signal"))
        );
    }

    #[test]
    fn w_jacked_in_out_here_looks_up_at_the_net() {
        let (mut st, mut s) = by_a_pump();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        let (at, net) = {
            let o = st.outside.as_ref().unwrap();
            (o.at, o.net)
        };
        let At::Ground(body) = at else { panic!() };
        s.update(&mut st, &[key(KeyCode::Char('W'))]);
        let up = s.net_up.as_ref().expect("looking up at the Net");
        assert_eq!(
            up.tile,
            Pos::new(body.x.div_euclid(TILE), body.y.div_euclid(TILE))
        );
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains("the Net overworld"), "{text}");
        assert!(
            crate::render::overworld::HEAVY
                .iter()
                .any(|c| text.contains(*c)),
            "a trunk on the map"
        );
        s.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(s.net_up.is_none());
        let o = st.outside.as_ref().unwrap();
        assert_eq!((o.at, o.net), (at, net), "back where you were on the Net");
    }

    #[test]
    fn the_net_overworld_is_look_only() {
        let (mut st, mut s) = by_a_pump();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char('W'))]);
        let (net, tick) = (st.outside.as_ref().unwrap().net, st.clock_tick());
        s.update(&mut st, &[key(KeyCode::Char('l')), key(KeyCode::Char('j'))]);
        assert_eq!(st.outside.as_ref().unwrap().net, net);
        assert!(s.net_up.is_some(), "still looking");
        assert_eq!(st.clock_tick(), tick, "the clock holds while you look");
        assert!(
            st.log
                .iter()
                .any(|e| e.text.contains("looked at, not walked")),
            "it says why"
        );
    }

    #[test]
    fn the_net_overworlds_help_is_its_own() {
        let (mut st, mut s) = by_a_pump();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char('W')), key(KeyCode::Char('?'))]);
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains("THE NET OVERWORLD"), "{text}");
        assert!(
            !text.contains("save game"),
            "the outside's keys aren't these"
        );
    }

    #[test]
    fn j_from_the_net_overworld_jacks_out() {
        let (mut st, mut s) = by_a_pump();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char('W'))]);
        s.update(&mut st, &[key(KeyCode::Char('J'))]);
        assert!(s.net_up.is_none());
        assert!(st.outside.as_ref().unwrap().net.is_none());
    }

    #[test]
    fn the_net_overworld_fits_the_smallest_terminal() {
        let (mut st, mut s) = by_a_pump();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char('W'))]);
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains("look only"), "{text}");
    }

    #[test]
    fn w_jacked_in_in_the_city_looks_up_at_the_net() {
        let (mut st, mut map) = crate::screens::shots::on_the_open_net();
        let pos = st.player.as_ref().unwrap().pos;
        let cmd = map.update(&mut st, &[key(KeyCode::Char('W'))]);
        let ScreenCmd::Push { to: mut up, .. } = cmd else {
            panic!("W on the city's Net looks up");
        };
        // drawn over the city's map, as the stack draws it: nothing of the map shows
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| {
            map.draw(&st, f);
            up.draw(&st, f);
        })
        .unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains("the Net overworld"), "{text}");
        assert!(
            !text.contains("[F1]"),
            "the map's tabs show through: {text}"
        );
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        let p = st.player.as_ref().unwrap();
        assert!(
            p.jacked_in() && p.pos == pos,
            "still jacked in where you were"
        );
    }

    #[test]
    fn the_barren_net_walks_within_reach() {
        let (mut st, mut s) = by_a_pump();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        // stand on the void in reach, beside more void: barren, but a step onto it goes
        let o = st.outside.as_ref().unwrap();
        let (At::Ground(body), Some(n)) = (o.at, o.net) else {
            panic!("jacked in on the ground");
        };
        let view = s.net_view.as_ref().unwrap();
        let void = |p: Pos| {
            in_reach(body, p)
                && view
                    .tile(p)
                    .is_some_and(|t| t.terrain == citygen::Terrain::Void)
        };
        let from = (-12..=12)
            .flat_map(|dy| (-12..=12).map(move |dx| n.offset(dx, dy)))
            .find(|p| void(*p) && void(p.offset(1, 0)))
            .expect("void in reach");
        st.outside.as_mut().unwrap().net = Some(from);
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(st.outside.as_ref().unwrap().net, Some(from.offset(1, 0)));
    }

    #[test]
    fn jacking_out_wakes_on_the_body() {
        let (mut st, mut s) = by_a_pump();
        let at = st.outside.as_ref().unwrap().at;
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        s.update(&mut st, &[key(KeyCode::Char('l'))]);
        s.update(&mut st, &[key(KeyCode::Char('J'))]);
        let o = st.outside.as_ref().unwrap();
        assert!(o.net.is_none());
        assert_eq!(o.at, at);
    }

    #[test]
    fn a_run_saved_jacked_in_reopens_on_its_body() {
        let (mut st, mut s) = by_a_pump();
        s.update(&mut st, &[key(KeyCode::Char(' '))]);
        let at = st.outside.as_ref().unwrap().at;
        let dir = crate::runs::testutil::Dir::new("open-net");
        let st = crate::runs::testutil::running(st);
        let path = crate::runs::file::write(&dir.0, &st).unwrap();
        let mut back = AppState::new();
        let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
        crate::runs::snapshot::restore(&mut back, snap);
        crate::runs::snapshot::after_restore(&mut back, lost);
        let o = back.outside.as_ref().unwrap();
        assert!(o.net.is_none(), "you wake on your body");
        // on the street past the city's edge, where the body was
        let At::Ground(body) = at else {
            panic!("jacked in on the ground")
        };
        assert_eq!(o.at, At::City);
        let origin = o.origin(back.city.as_ref().unwrap());
        let p = back.player.as_ref().unwrap();
        assert_eq!(
            (p.place, p.pos, p.body),
            (
                citygen::Place::Stratum(StratumKind::Street),
                Pos::new(body.x - origin.x, body.y - origin.y),
                None
            )
        );
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
