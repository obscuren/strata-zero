//! The world beyond the city in the play screen: the overworld in the atlas look, one
//! cell a tile, with the map's bands, panel and footer; and, jacked in, the Net over the
//! world, walked tile by tile and gone down from onto any tile's Net. The open ground is
//! walked on the city screen. See `2026-11-04-overworld-screen-design.md` and
//! `2026-11-05-streamed-world-design.md`.

use crate::actions::report;
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::outside::At;
use crate::outside::here;
use crate::outside::moves::{self, Moved};
use crate::render::overworld::live::{Hour, LiveAtlas};
use crate::render::overworld::{ATLAS_BG, atlas_cell};
use crate::render::{Cell, Rgb};
use crate::screens::explore::{dir_for, layout};
use crate::screens::pause::Pause;
use crate::ui::chrome::CYAN;
use crate::ui::console::{Console, ConsoleEvent};
use crate::ui::hud;
use crate::ui::sidebar::Section;
use citygen::overworld::Overworld;
use citygen::{Pos, StratumKind};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

/// You, drawn over whatever you stand on.
const YOU: Rgb = Rgb(255, 255, 255);
const LABEL: Color = Color::Rgb(120, 130, 145);
const TEXT: Color = Color::Rgb(205, 212, 222);
const DIM: Color = Color::Rgb(110, 118, 130);

/// The keys on the overworld, for `?`.
pub const KEYS: &[(&str, &str)] = &[
    ("hjkl yubn ↑↓←→", "walk a tile"),
    (
        "enter",
        "down onto the tile you're on: its ground, or into its city",
    ),
    ("s", "save game"),
    ("?", "this help"),
    ("esc q", "pause menu"),
];

/// The keys on the Net over the world.
pub const NET_UP_KEYS: &[(&str, &str)] = &[
    ("hjkl yubn ↑↓←→", "walk a tile, over any ground"),
    (
        "enter",
        "down onto this tile's Net, on its trunk if it has one",
    ),
    ("enter over yours", "back down to where you were, as W"),
    ("W", "back down to where you were on the Net"),
    ("J", "jack out onto your body"),
    ("?", "this help"),
    ("esc q", "pause menu"),
];

/// The dev keys out here, on both overworlds, listed once dev mode is unlocked.
pub const DEV_KEYS: &[(&str, &str)] = &[
    ("F5", "inspect the tile (dev mode)"),
    (":", "dev console (dev mode)"),
    ("`", "dev mode on / off"),
];

/// The overworld as a level of play, or the Net over it.
pub struct OutsideScreen {
    help: bool,
    /// Jacked in, on the Net over the world.
    pub(crate) net_up: Option<NetUp>,
    /// The world jacked in, drawn from once it's first looked at.
    net_atlas: std::cell::OnceCell<crate::render::overworld::NetAtlas>,
    /// The world on foot in motion, drawn from once it's first looked at.
    live_atlas: std::cell::OnceCell<LiveAtlas>,
    /// The dev console while it is open; it takes every key.
    pub(crate) console: Option<Console>,
}

/// On the Net over the world: over which tile, walked from the one your Net self is
/// over. Going down closes the screen, onto that tile's Net or back where you were.
pub(crate) struct NetUp {
    pub tile: Pos,
}

impl OutsideScreen {
    /// The overworld where `st.outside` says you are (up from the street if it says the
    /// city).
    pub fn new(st: &mut AppState) -> OutsideScreen {
        if let Some(o) = crate::outside::ensure(st)
            && o.at == At::City
        {
            moves::up(st);
        }
        OutsideScreen {
            help: false,
            net_up: None,
            net_atlas: Default::default(),
            live_atlas: Default::default(),
            console: None,
        }
    }

    /// Jacked in on the Net, in a city or past its edge: the Net over the world, over the
    /// tile your Net self is over, to walk.
    pub fn net_from_city(st: &mut AppState) -> OutsideScreen {
        crate::outside::ensure(st);
        OutsideScreen {
            help: false,
            net_up: Some(NetUp {
                tile: here::tile(st).unwrap_or(Pos::new(0, 0)),
            }),
            net_atlas: Default::default(),
            live_atlas: Default::default(),
            console: None,
        }
    }

    /// The keys on the Net over the world: the arrows walk it, ↵ goes down onto the
    /// tile's Net (over the tile you're over, where you were), W back down where you
    /// were, J jacks out.
    fn net_up_key(&mut self, st: &mut AppState, k: &KeyEvent) -> ScreenCmd {
        let down = ScreenCmd::Pop {
            fx: TransitionKind::Glitch,
        };
        let Some(up) = self.net_up.as_mut() else {
            return ScreenCmd::Stay;
        };
        match k.code {
            KeyCode::Char('W') => down,
            KeyCode::Esc | KeyCode::Char('q') => ScreenCmd::Push {
                to: Box::new(Pause::new(st, st.tick)),
                fx: TransitionKind::None,
            },
            KeyCode::Enter => match moves::net_down(st, up.tile) {
                Ok(()) => down,
                Err(why) => {
                    st.warn(why);
                    ScreenCmd::Stay
                }
            },
            KeyCode::Char('J') => {
                match crate::net::jack_out(st) {
                    Ok(()) => st.say("jacked out"),
                    Err(why) => st.warn(why),
                }
                down
            }
            KeyCode::Char('?') => {
                self.help = true;
                ScreenCmd::Stay
            }
            code => {
                if let Some(d) = dir_for(code)
                    && let Moved::Blocked(why) = moves::net_world_step(st, &mut up.tile, d)
                {
                    st.warn(why);
                }
                ScreenCmd::Stay
            }
        }
    }

    /// Back on the map, on the city's street at `cell`: in the city, or past its edge on
    /// the open ground.
    fn into_city(st: &mut AppState, cell: Pos) -> ScreenCmd {
        moves::into_city(st, cell);
        ScreenCmd::Switch {
            to: crate::screens::resume_screen(st),
            fx: TransitionKind::Glitch,
        }
    }

    /// The map's dev keys that mean something without a street, on both overworlds:
    /// `` ` `` turns dev mode on or off (once unlocked), `:` opens the console and F5 the
    /// inspector of the tile you're over (in dev mode). Whether the key was one.
    fn dev_key(&mut self, st: &mut AppState, k: &KeyEvent) -> bool {
        match k.code {
            KeyCode::Char('`') if st.dev_unlocked => {
                st.set_dev(!st.dev);
                st.say(if st.dev {
                    "dev mode on"
                } else {
                    "dev mode off"
                });
            }
            KeyCode::Char(':') if st.dev => self.console = Some(Console::default()),
            KeyCode::F(5) if st.dev => st.sidebar.toggle(Section::Inspect),
            _ => return false,
        }
        true
    }

    /// Keys to the open console: a line run (`run_line`), esc closing it.
    fn feed_console(&mut self, st: &mut AppState, keys: &[KeyEvent]) {
        let Some(mut con) = self.console.take() else {
            return;
        };
        for k in keys {
            match con.key(*k, &|head, word| {
                crate::screens::console::complete(st, head, word)
            }) {
                ConsoleEvent::Run(line) => {
                    let result = self.run_line(st, &line);
                    report(st, result);
                }
                ConsoleEvent::Close => return,
                ConsoleEvent::None => {}
            }
        }
        self.console = Some(con);
    }

    /// A console line out here: `tp` over the world (`teleport`); what acts through the
    /// map (a window over it, `target`'s free camera) refused; the rest run as on the
    /// map, on where your body stands.
    fn run_line(&mut self, st: &mut AppState, line: &str) -> Result<String, String> {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.first() == Some(&"tp") {
            return self.teleport(st, &words[1..]);
        }
        if words.first() == Some(&"target") || crate::screens::console::opens(st, line).is_some() {
            return Err("not out here".to_string());
        }
        let street = citygen::Place::Stratum(StratumKind::Street);
        let (place, pos) = st.player.as_ref().map_or((street, Pos::new(0, 0)), |p| {
            p.body.unwrap_or((p.place, p.pos))
        });
        crate::screens::console::run(st, game::At { place, pos }, line)
    }

    /// `tp x y` out here: you onto overworld tile (x, y) if it can be stood on, or the Net
    /// overworld over any tile of the world.
    fn teleport(&mut self, st: &mut AppState, args: &[&str]) -> Result<String, String> {
        let usage = || "tp <x> <y>".to_string();
        let (x, y) = match args {
            [x, y] => (
                x.parse::<i32>().map_err(|_| usage())?,
                y.parse::<i32>().map_err(|_| usage())?,
            ),
            _ => return Err(usage()),
        };
        let t = Pos::new(x, y);
        let o = st
            .outside
            .as_mut()
            .ok_or_else(|| "there is no world out here".to_string())?;
        let w = o.world();
        if w.tile(t).is_none() {
            return Err("the world ends here".to_string());
        }
        if let Some(up) = self.net_up.as_mut() {
            up.tile = t;
            return Ok(format!("{x},{y} · the Net overworld"));
        }
        if moves::step_cost(w, t).is_none() {
            return Err(moves::closed(w, t).to_string());
        }
        o.at = At::Overworld(t);
        crate::outside::reached_mark(st);
        Ok(format!("{x},{y} · overworld"))
    }

    fn key(&mut self, st: &mut AppState, k: &KeyEvent) -> ScreenCmd {
        let at = st.outside.as_ref().map(|o| o.at);
        if self.help {
            self.help = false;
            return ScreenCmd::Stay;
        }
        if self.dev_key(st, k) {
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
            KeyCode::Enter if matches!(at, Some(At::Overworld(_))) => match moves::down(st) {
                Ok(Moved::City(cell)) => return Self::into_city(st, cell),
                Ok(_) => {}
                Err(why) => st.warn(why),
            },
            code => {
                let Some(d) = dir_for(code) else {
                    return ScreenCmd::Stay;
                };
                if !matches!(at, Some(At::Overworld(_))) {
                    return ScreenCmd::Stay;
                }
                match moves::world_step(st, d) {
                    Moved::City(cell) => return Self::into_city(st, cell),
                    Moved::Blocked(why) => st.warn(why),
                    Moved::Stepped => {}
                }
            }
        }
        ScreenCmd::Stay
    }
}

impl Screen for OutsideScreen {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // The city's clock runs out here as it does on the map; on the Net over the
        // world only a step spends time.
        if self.net_up.is_none() {
            crate::clock::run(st, Some(1), crate::clock::PATHS_PER_FRAME);
        }
        for (i, k) in keys.iter().enumerate() {
            // the console takes every key while it is open, the rest of the frame's too
            if self.console.is_some() {
                self.feed_console(st, &keys[i..]);
                break;
            }
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
        let buf = f.buffer_mut();
        match (o.at, self.net_up.as_ref()) {
            (_, Some(up)) => {
                let atlas = self
                    .net_atlas
                    .get_or_init(|| crate::render::overworld::NetAtlas::new(o.world()));
                draw_net_world(st, o.world(), atlas, up.tile, r.map, buf);
            }
            (At::Overworld(t), None) => {
                let atlas = self.live_atlas.get_or_init(|| LiveAtlas::new(o.world()));
                draw_world(st, o.world(), atlas, t, r.map, buf);
            }
            _ => {}
        }
        let up = self.net_up.is_some();
        let place = if up { "the Net overworld" } else { "overworld" };
        // the tile you're over: the Net overworld's, else where you stand
        let over = self
            .net_up
            .as_ref()
            .map(|u| u.tile)
            .or_else(|| here::tile(st));
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
        let log_drawn = draw_panel(st, up, over, r.side, buf);
        // The footer.
        let slots = crate::screens::explore::ability_slots(st);
        let footer = hud::Footer {
            place: place.to_string(),
            saved: crate::runs::when::marked(st),
            jacked: up,
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
            // no line of keys: `?` lists them, as on the map
            prompt: None,
            act: None,
            flash_pools: false,
        };
        let recent = st.log.recent(st.tick);
        for (i, row) in hud::footer_rows(&footer, area.width)
            .into_iter()
            .enumerate()
        {
            let row = match (&self.console, i) {
                (Some(con), 3) => crate::screens::console::line(con, st.tick, recent),
                _ => row,
            };
            Paragraph::new(row).render(r.footer[i], buf);
        }
        // With no room for the LOG pane, the newest entry on the footer's free row.
        if !log_drawn
            && self.console.is_none()
            && let Some(e) = recent
        {
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
            let (title, keys) = match self.net_up {
                Some(_) => ("KEYS · THE NET OVERWORLD", NET_UP_KEYS),
                None => ("KEYS · OUTSIDE", KEYS),
            };
            let dev = if st.dev_unlocked { DEV_KEYS } else { &[] };
            let rows: Vec<(&str, &str)> = keys.iter().chain(dev).copied().collect();
            crate::screens::help::draw_keys(f, title, &rows);
        }
    }
}

/// The overworld round tile `t` in the atlas look, one cell a tile, in motion at the
/// frame tick and the hour.
fn draw_world(
    st: &AppState,
    world: &Overworld,
    atlas: &LiveAtlas,
    t: Pos,
    map: Rect,
    buf: &mut Buffer,
) {
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
    let minute = st.minute_of_day();
    for sy in 0..map.height {
        for sx in 0..map.width {
            let p = origin.offset(sx as i32, sy as i32);
            let cell = if world.tile(p).is_some() {
                atlas.cell(world, p, &st.style, st.tick, minute)
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

/// The panel: HERE, the map, NEAR and the log, each under a rule, for tile `over`;
/// whether the log fit.
fn draw_panel(
    st: &AppState,
    net_up: bool,
    over: Option<Pos>,
    side: Rect,
    buf: &mut Buffer,
) -> bool {
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
        here.extend(
            over.map(|t| here::net_here_lines_at(st, t))
                .unwrap_or_default(),
        );
        here.push(Line::from(Span::styled(
            " the Net overworld".to_string(),
            Style::default().fg(DIM),
        )));
    } else {
        here.extend(here::here_lines(st));
    }
    put(buf, here, &mut y, 3);
    put(buf, vec![rule.clone()], &mut y, 1);
    put(buf, vec![title("MAP")], &mut y, 1);
    let map_rows = 9.min(bottom.saturating_sub(y + 8));
    if map_rows > 0 {
        let pane = Rect::new(side.x + 1, y, w.saturating_sub(2), map_rows);
        // on foot the map darkens with the hour, as the overworld does
        let dim = if net_up {
            1.0
        } else {
            Hour::of(st.minute_of_day()).pick([1.0, 0.75, 0.5])
        };
        world_pane(st, over, dim, pane, buf);
        y += map_rows;
    }
    put(buf, vec![rule.clone()], &mut y, 1);
    let mut near = vec![title("NEAR")];
    near.extend(
        over.map(|t| here::near_lines_at(st, t, 4))
            .unwrap_or_default(),
    );
    put(buf, near, &mut y, 5);
    // dev mode's inspector: the tile you're over
    if st.dev
        && st.sidebar.is_on(Section::Inspect)
        && let Some(t) = over
    {
        let mut lines = vec![title("INSPECT")];
        lines.extend(inspect_lines(st, net_up, t));
        let rows = lines.len() as u16;
        put(buf, vec![rule.clone()], &mut y, 1);
        put(buf, lines, &mut y, rows);
    }
    if y + 2 < bottom {
        put(buf, vec![rule], &mut y, 1);
        crate::ui::sidebar::draw_log(&st.log, st.tick, Rect::new(side.x, y, w, bottom - y), buf);
        return true;
    }
    false
}

/// The dev inspector's lines for overworld tile `t`: where it is, its ground and water,
/// its road, whose it is, what stands on it, and what a step onto it costs.
fn inspect_lines(st: &AppState, net_up: bool, t: Pos) -> Vec<Line<'static>> {
    use crate::ui::panel::kv;
    let Some(o) = st.outside.as_ref() else {
        return Vec::new();
    };
    let w = o.world();
    let layer = if net_up {
        "the Net overworld"
    } else {
        "overworld"
    };
    let mut out = vec![kv(" pos", format!("{},{}  {layer}", t.x, t.y))];
    let Some(tile) = w.tile(t) else {
        out.push(kv(" ground", "past the world's edge"));
        return out;
    };
    let mut ground = format!("{:?}", tile.ground).to_lowercase();
    if tile.water != citygen::overworld::Water::None {
        ground = format!("{ground} · {:?}", tile.water).to_lowercase();
    }
    out.push(kv(" ground", ground));
    let road = w
        .routes
        .iter()
        .find(|r| r.path.contains(&t))
        .map_or("none".to_string(), |r| r.name.clone());
    out.push(kv(" road", road));
    let owner = crate::outside::tile_owner(st, t).map_or("open ground".to_string(), |s| o.name(s));
    out.push(kv(" owner", owner));
    if let Some(p) = w.pois.iter().find(|p| p.at == t) {
        let kind = format!("{:?}", p.kind).to_lowercase();
        out.push(kv(" poi", format!("{} · {kind}", p.name)));
    }
    let cost = moves::step_cost(w, t).map_or("closed".to_string(), |c| format!("{c} ticks"));
    out.push(kv(" step", cost));
    out
}

/// The whole world scaled into the pane, two tiles a row in half blocks, at `dim` of its
/// colour, `@` on tile `over`.
fn world_pane(st: &AppState, over: Option<Pos>, dim: f32, pane: Rect, buf: &mut Buffer) {
    let Some(o) = st.outside.as_ref() else {
        return;
    };
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
        Rgb((r / n) as u8, (g / n) as u8, (b / n) as u8).scale(dim)
    };
    for y in 0..pane.height as i32 {
        for x in 0..pw {
            let (top, low) = (pixel(x, 2 * y), pixel(x, 2 * y + 1));
            buf[(pane.x + x as u16, pane.y + y as u16)]
                .set_char('▀')
                .set_style(Style::default().fg(top.to_color()).bg(low.to_color()));
        }
    }
    let t = over.unwrap_or(Pos::new(0, 0));
    let (x, y) = (t.x * pw / ww.max(1), t.y * ph / wh.max(1) / 2);
    if x >= 0 && y >= 0 && x < pw && y < pane.height as i32 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outside::TILE;
    use crate::outside::moves::step_cost;
    use crate::outside::moves::tests::on_the_world;
    use crate::render::Visibility;
    use crate::screens::explore::MapScreen;
    use crate::screens::testutil::{buffer_text, key};
    use citygen::overworld::Water;
    use citygen::{Dir, Place};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const STREET: Place = Place::Stratum(StratumKind::Street);
    const NET: Place = Place::Stratum(StratumKind::Net);

    fn text(st: &AppState, s: &dyn Screen, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| s.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    fn logged(st: &AppState, needle: &str) -> bool {
        st.log.iter().any(|e| e.text.contains(needle))
    }

    /// Up out of the city onto the overworld, then onto a tile outside the block.
    fn out_there() -> (AppState, OutsideScreen) {
        let mut st = on_the_world();
        let s = OutsideScreen::new(&mut st);
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
        (st, s)
    }

    /// The world cell of the live city's cell `p`.
    fn world_cell(st: &AppState, p: Pos) -> Pos {
        let o = st.outside.as_ref().unwrap();
        o.origin(st.city.as_ref().unwrap()).offset(p.x, p.y)
    }

    /// Up out of the city onto its block on the overworld, the city's clock at `minute`.
    fn up_at(minute: u16) -> (AppState, OutsideScreen) {
        let mut st = AppState::new();
        st.enter_city_at(crate::runs::testutil::story_city(), minute);
        let city = st.city.clone().unwrap();
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        crate::outside::ensure(&mut st);
        let s = OutsideScreen::new(&mut st);
        (st, s)
    }

    /// The map's cells of `s` drawn at 120x40.
    fn map_cells(st: &AppState, s: &dyn Screen) -> Vec<ratatui::buffer::Cell> {
        let (w, h) = (120, 40);
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| s.draw(st, f)).unwrap();
        let map = layout(Rect::new(0, 0, w, h), false).map;
        let buf = t.backend().buffer();
        (map.y..map.bottom())
            .flat_map(|y| (map.x..map.right()).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].clone())
            .collect()
    }

    #[test]
    fn the_overworld_moves_with_the_tick() {
        let (mut st, s) = up_at(22 * 60);
        st.tick = 6000;
        let a = map_cells(&st, &s);
        assert_eq!(a, map_cells(&st, &s), "the same tick twice, the same map");
        st.tick += 2;
        let b = map_cells(&st, &s);
        let moved = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        assert!(moved > 0, "two ticks on, something moves");
        assert!(
            moved * 10 < a.len(),
            "a few cells a tick, not the whole map: {moved} of {}",
            a.len()
        );
    }

    #[test]
    fn the_overworld_reads_the_hour() {
        let (mut day, s) = up_at(13 * 60);
        let (mut night, _) = up_at(22 * 60);
        day.tick = 6000;
        night.tick = 6000;
        let (a, b) = (map_cells(&day, &s), map_cells(&night, &s));
        let differ = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        assert!(
            differ * 2 > a.len(),
            "day and night: {differ} of {}",
            a.len()
        );
    }

    #[test]
    fn the_net_overworld_is_still() {
        // jacked in, the Net over the world is today's: only the packets move, along the
        // open trunks, whatever the tick and the hour
        let (mut day, _) = up_at(13 * 60);
        let (mut night, _) = up_at(22 * 60);
        let up = OutsideScreen::net_from_city(&mut day);
        let _ = OutsideScreen::net_from_city(&mut night);
        day.tick = 6000;
        let a = map_cells(&day, &up);
        let trunk = |c: &ratatui::buffer::Cell| {
            let ch = c.symbol().chars().next().unwrap();
            crate::render::overworld::HEAVY.contains(&ch)
        };
        let still = |st: &AppState| {
            let b = map_cells(st, &up);
            for (x, y) in a.iter().zip(&b).filter(|(x, y)| x != y) {
                assert!(
                    trunk(x) && trunk(y),
                    "only a packet on a trunk moves: {x:?} {y:?}"
                );
            }
        };
        day.tick = 6090;
        still(&day);
        night.tick = 6000;
        still(&night);
        night.tick = 6150;
        still(&night);
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
    }

    #[test]
    fn it_fits_the_smallest_terminal() {
        let (st, s) = out_there();
        for (w, h) in [(100, 30), (120, 40), (60, 20)] {
            let page = text(&st, &s, w, h);
            assert!(page.contains('@') || w < 100, "{w}x{h}");
        }
    }

    #[test]
    fn enter_on_the_overworld_lands_on_the_city_screen() {
        let (mut st, mut s) = out_there();
        let At::Overworld(t) = st.outside.as_ref().unwrap().at else {
            panic!()
        };
        let ScreenCmd::Switch { to: mut map, .. } = s.update(&mut st, &[key(KeyCode::Enter)])
        else {
            panic!("down onto the city screen")
        };
        let o = st.outside.as_ref().unwrap();
        assert_eq!(o.at, At::City);
        let city = st.city.as_ref().unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.body), (STREET, None));
        assert!(
            !city.size.contains(p.pos),
            "off the city's map: {:?}",
            p.pos
        );
        assert!(!crate::outside::stream::live_owns(&st, p.pos));
        // on ground you can stand on, on the tile or its ring
        let landed = o.tile_of(city, p.pos);
        assert!(
            (landed.x - t.x).abs() <= 1 && (landed.y - t.y).abs() <= 1,
            "{t:?} → {landed:?}"
        );
        let c = world_cell(&st, p.pos);
        let cell = citygen::overworld::detail::region(o.world(), c.x, c.y, 1, 1)[Pos::new(0, 0)];
        assert!(cell.is_walkable(), "{:?}", cell.terrain);
        // you see where you've come down
        assert!(p.beyond_sight().binary_search(&p.pos).is_ok());
        map.update(&mut st, &[]);
        let page = text(&st, map.as_ref(), 120, 40);
        assert!(page.contains('@'), "{page}");
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

    fn map(st: &mut AppState) -> MapScreen {
        MapScreen::walking(st)
    }

    #[test]
    fn w_on_the_open_street_goes_up_to_the_world() {
        let mut st = on_the_world();
        let city = st.city.clone().unwrap();
        let o = st.outside.as_ref().unwrap();
        let cell = o.street_of(&city, Pos::new(o.block(&city).x + 10, o.block(&city).y + 7));
        let p = st.player.as_mut().unwrap();
        p.place = STREET;
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
        p.place = STREET;
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
        let page = buffer_text(t.backend().buffer());
        assert!(
            page.contains("the overworld, from the open street"),
            "{page}"
        );
    }

    #[test]
    fn the_outsides_help_is_the_overworlds() {
        let (mut st, mut s) = out_there();
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        let page = text(&st, &s, 120, 40);
        assert!(page.contains("KEYS · OUTSIDE"), "{page}");
        assert!(page.contains("walk a tile") && page.contains("save game"));
        assert!(
            !page.contains("jack in") && !page.contains("jack out"),
            "the ground's keys are the city screen's: {page}"
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
    fn a_run_saved_past_the_edge_on_ground_since_gone_lands_in_the_city() {
        // saved on the street off the city's map, the world there since changed: back
        // on the city's own street, as an old save on the ground comes back
        let mut st = on_the_world();
        let city = st.city.clone().unwrap();
        let origin = st.outside.as_ref().unwrap().origin(&city);
        let gone = Pos::new(-origin.x - 50, -origin.y - 50);
        let p = st.player.as_mut().unwrap();
        p.place = STREET;
        p.pos = gone;
        crate::runs::snapshot::after_restore(&mut st, Vec::new());
        let p = st.player.as_ref().unwrap();
        assert_eq!(st.outside.as_ref().unwrap().at, At::City);
        assert_eq!(p.place, STREET);
        assert!(city.size.contains(p.pos), "{:?}", p.pos);
        assert!(city.walkable(StratumKind::Street, p.pos));
        assert!(logged(&st, "the world out there has changed"));
        // jacked in with the body there: woken, back on the city's street
        let mut st = on_the_world();
        let p = st.player.as_mut().unwrap();
        let net_at = p.pos;
        p.body = Some((STREET, gone));
        p.place = NET;
        p.pos = net_at;
        crate::runs::snapshot::after_restore(&mut st, Vec::new());
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.body), (STREET, None));
        assert!(city.size.contains(p.pos), "{:?}", p.pos);
        // and ground that is still there keeps you on it
        let (mut st, _, _) = by_a_pump();
        let before = st.player.as_ref().map(|p| (p.place, p.pos));
        crate::runs::snapshot::after_restore(&mut st, Vec::new());
        assert_eq!(st.player.as_ref().map(|p| (p.place, p.pos)), before);
    }

    #[test]
    fn w_waits_for_a_scene_or_a_call() {
        let mut st = on_the_world();
        let city = st.city.clone().unwrap();
        let o = st.outside.as_ref().unwrap();
        let cell = o.street_of(&city, Pos::new(o.block(&city).x + 10, o.block(&city).y + 7));
        let p = st.player.as_mut().unwrap();
        p.place = STREET;
        p.pos = cell;
        st.missions.scenes.push_back((None, "waking".to_string()));
        assert!(crate::outside::can_leave(&st).is_err());
    }

    /// On the open ground beside a truck stop's pump, on the city screen: the run, the
    /// screen and where you stand.
    fn by_a_pump() -> (AppState, MapScreen, Pos) {
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
        let origin = st
            .outside
            .as_ref()
            .unwrap()
            .origin(st.city.as_ref().unwrap());
        let at = Pos::new(o.x + beside.x - origin.x, o.y + beside.y - origin.y);
        let p = st.player.as_mut().unwrap();
        p.place = STREET;
        p.pos = at;
        p.body = None;
        let mut m = MapScreen::walking(&mut st);
        m.update(&mut st, &[]);
        (st, m, at)
    }

    /// Jacked in at the pump: the run and the screen.
    fn jacked_by_a_pump() -> (AppState, MapScreen) {
        let (mut st, mut m, _) = by_a_pump();
        m.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(
            st.player.as_ref().unwrap().jacked_in(),
            "jacked in: {:?}",
            st.log.latest().map(|e| e.text.clone())
        );
        (st, m)
    }

    #[test]
    fn space_by_a_ground_terminal_jacks_in() {
        let (mut st, mut m, at) = by_a_pump();
        m.update(&mut st, &[key(KeyCode::Char(' '))]);
        let p = st.player.as_ref().unwrap().clone();
        assert_eq!(p.body, Some((STREET, at)), "the body stays where it stood");
        assert_eq!(p.place, NET);
        assert!(p.pos.chebyshev(at) <= 8, "{:?} from {at:?}", p.pos);
        {
            // on a cell of the Net with signal: not the void between
            let city = st.city.as_ref().unwrap();
            let b = crate::outside::stream::Beyond::of(&st).unwrap();
            let net = crate::render::map::MapRef::stratum_of(city, StratumKind::Net).beyond(&b);
            let t = crate::render::map::TileMap::tile(&net, p.pos).unwrap();
            assert!(t.is_walkable() && t.terrain != citygen::Terrain::Void);
        }
        assert!(logged(&st, "jacked in"));
        m.update(&mut st, &[]);
        let page = text(&st, &m, 120, 40);
        assert!(page.contains("the Net ·"), "HERE over the open Net: {page}");
        // the Net self walks on, out of the truck stop's gate and past where the body's
        // reach used to end
        let body_tile = at.x.div_euclid(TILE);
        for _ in 0..2 {
            m.update(&mut st, &[key(KeyCode::Char('j'))]);
        }
        for _ in 0..40 {
            m.update(&mut st, &[key(KeyCode::Char('l'))]);
        }
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.body), (NET, Some((STREET, at))));
        assert!(
            p.pos.x.div_euclid(TILE) - body_tile > 1,
            "{:?} from {at:?}",
            p.pos
        );
        // away from any port, space is space on the street: Look at least
        let (mut st, mut s) = out_there();
        let ScreenCmd::Switch { to: mut map, .. } = s.update(&mut st, &[key(KeyCode::Enter)])
        else {
            panic!("down onto the city screen")
        };
        map.update(&mut st, &[]);
        let cmd = map.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(!st.player.as_ref().unwrap().jacked_in());
        assert!(
            matches!(cmd, ScreenCmd::Push { .. }),
            "{:?}",
            st.log.latest().map(|e| e.text.clone())
        );
        assert!(!logged(&st, "no port in reach"));
    }

    #[test]
    fn a_net_self_since_gone_wakes_with_the_net_put_away() {
        // saved jacked in on the ground, a run in flight, a mask up and the rig's route
        // open, the Net self where the world has since changed: woken on the body as a
        // jack out wakes you, none of the Net left over
        let (mut st, _) = jacked_by_a_pump();
        let body = st.player.as_ref().unwrap().body.unwrap();
        let origin = st
            .outside
            .as_ref()
            .unwrap()
            .origin(st.city.as_ref().unwrap());
        st.player.as_mut().unwrap().pos = Pos::new(-origin.x - 50, -origin.y - 50);
        st.net.run = Some(game::Run {
            program: game::ItemId::new("crack"),
            target: game::HackTarget::Deck,
            done: 0,
            work: 2,
        });
        st.net.mask = 3;
        st.backdoors.start_route(None);
        crate::runs::snapshot::after_restore(&mut st, Vec::new());
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos, p.body), (body.0, body.1, None));
        assert!(st.net.run.is_none(), "the run went with the jack");
        assert_eq!(st.net.mask, 0, "and the mask");
        assert!(st.backdoors.route.is_none(), "and the rig's route");
        assert!(logged(&st, "jacked out"));
        // the body gone too: back on the city's street, and the same left over
        let (mut st, _) = jacked_by_a_pump();
        let p = st.player.as_mut().unwrap();
        p.body = Some((STREET, Pos::new(-origin.x - 60, -origin.y - 60)));
        st.net.mask = 3;
        st.backdoors.start_route(None);
        crate::runs::snapshot::after_restore(&mut st, Vec::new());
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.body), (STREET, None));
        assert!(st.city.as_ref().unwrap().size.contains(p.pos));
        assert_eq!(st.net.mask, 0);
        assert!(st.backdoors.route.is_none());
    }

    #[test]
    fn a_cooling_deck_wont_jack_in_on_the_ground() {
        let (mut st, mut m, _) = by_a_pump();
        st.net.cooling_until = Some(st.net.now(&st) + 30);
        m.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(!st.player.as_ref().unwrap().jacked_in());
        assert!(logged(&st, "deck cooling"));
    }

    #[test]
    fn w_off_the_map_goes_up() {
        let (mut st, mut m, at) = by_a_pump();
        let c = world_cell(&st, at);
        let ScreenCmd::Switch { to, .. } = m.update(&mut st, &[key(KeyCode::Char('W'))]) else {
            panic!("up from the ground: {:?}", st.log.latest().map(|e| &e.text))
        };
        assert_eq!(
            st.outside.as_ref().unwrap().at,
            At::Overworld(Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)))
        );
        assert!(text(&st, to.as_ref(), 120, 40).contains("on foot · overworld"));
    }

    #[test]
    fn jacking_out_on_the_ground_wakes_on_the_body() {
        let (mut st, mut m) = jacked_by_a_pump();
        let (_, body) = st.player.as_ref().unwrap().body.unwrap();
        m.update(&mut st, &[key(KeyCode::Char('l'))]);
        let ScreenCmd::Push { to: mut ask, .. } = m.update(&mut st, &[key(KeyCode::Char('J'))])
        else {
            panic!("J asks")
        };
        ask.update(&mut st, &[key(KeyCode::Char('y'))]);
        m.update(&mut st, &[]);
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos, p.body), (STREET, body, None));
        assert!(!st.city.as_ref().unwrap().size.contains(p.pos));
        assert!(
            p.beyond_sight().binary_search(&p.pos).is_ok(),
            "you see again"
        );
    }

    #[test]
    fn a_run_saved_jacked_in_on_the_ground_reopens_jacked_in() {
        let (st, _) = jacked_by_a_pump();
        let was = st.player.as_ref().map(|p| (p.place, p.pos, p.body));
        let dir = crate::runs::testutil::Dir::new("open-net");
        let st = crate::runs::testutil::running(st);
        let path = crate::runs::file::write(&dir.0, &st).unwrap();
        let mut back = AppState::new();
        let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
        crate::runs::snapshot::restore(&mut back, snap);
        crate::runs::snapshot::after_restore(&mut back, lost);
        assert_eq!(back.outside.as_ref().unwrap().at, At::City);
        assert_eq!(
            back.player.as_ref().map(|p| (p.place, p.pos, p.body)),
            was,
            "jacked in where you were, the body where it stood"
        );
        let mut map = crate::screens::resume_screen(&mut back);
        map.update(&mut back, &[]);
        assert!(text(&back, map.as_ref(), 120, 40).contains("the Net ·"));
    }

    /// Jacked in on the ground, W: the Net over the world, pushed over the map.
    fn looking_up() -> (AppState, MapScreen, Box<dyn Screen>) {
        let (mut st, mut m) = jacked_by_a_pump();
        let ScreenCmd::Push { to, .. } = m.update(&mut st, &[key(KeyCode::Char('W'))]) else {
            panic!("W on the Net looks up")
        };
        (st, m, to)
    }

    #[test]
    fn w_jacked_in_on_the_ground_looks_up_at_the_net() {
        let (mut st, mut m) = jacked_by_a_pump();
        let was = st.player.as_ref().map(|p| (p.place, p.pos, p.body));
        let n = st.player.as_ref().unwrap().pos;
        let ScreenCmd::Push { to: mut up, .. } = m.update(&mut st, &[key(KeyCode::Char('W'))])
        else {
            panic!("W on the Net looks up")
        };
        let c = world_cell(&st, n);
        let page = text(&st, up.as_ref(), 120, 40);
        assert!(page.contains("the Net overworld"), "{page}");
        assert!(
            crate::render::overworld::HEAVY
                .iter()
                .any(|c| page.contains(*c)),
            "a trunk on the map"
        );
        let o = st.outside.as_ref().unwrap();
        assert_eq!(
            o.tile_of(st.city.as_ref().unwrap(), n),
            Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)),
            "over your Net self's tile"
        );
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        assert_eq!(
            st.player.as_ref().map(|p| (p.place, p.pos, p.body)),
            was,
            "back where you were on the Net"
        );
    }

    /// Jacked in on the ground, W: the Net over the world as W opens it over the map,
    /// to walk.
    fn up_here() -> (AppState, MapScreen, OutsideScreen) {
        let (mut st, mut m) = jacked_by_a_pump();
        assert!(matches!(
            m.update(&mut st, &[key(KeyCode::Char('W'))]),
            ScreenCmd::Push { .. }
        ));
        let up = OutsideScreen::net_from_city(&mut st);
        (st, m, up)
    }

    /// The tile the Net overworld is over.
    fn over(up: &OutsideScreen) -> Pos {
        up.net_up.as_ref().unwrap().tile
    }

    fn put_over(up: &mut OutsideScreen, t: Pos) {
        up.net_up.as_mut().unwrap().tile = t;
    }

    fn code(d: Dir) -> KeyCode {
        KeyCode::Char(match d {
            Dir::N => 'k',
            Dir::S => 'j',
            Dir::E => 'l',
            Dir::W => 'h',
            Dir::NE => 'u',
            Dir::NW => 'y',
            Dir::SE => 'n',
            Dir::SW => 'b',
        })
    }

    /// The Net of the live city's frame, carried on past its edge.
    fn net_tile(st: &AppState, p: Pos) -> Option<citygen::Tile> {
        use crate::render::map::{MapRef, TileMap};
        let city = st.city.as_ref().unwrap();
        let b = crate::outside::stream::Beyond::of(st).unwrap();
        MapRef::stratum_of(city, StratumKind::Net)
            .beyond(&b)
            .tile(p)
            .copied()
    }

    /// Whether a Net self can stand on the live city's frame's cell `p`.
    fn net_walkable(st: &AppState, p: Pos) -> bool {
        use crate::render::map::{MapRef, TileMap};
        let city = st.city.as_ref().unwrap();
        let b = crate::outside::stream::Beyond::of(st).unwrap();
        MapRef::stratum_of(city, StratumKind::Net)
            .beyond(&b)
            .walkable(p)
    }

    #[test]
    fn the_net_overworld_walks_tile_by_tile() {
        let (mut st, _m, mut up) = up_here();
        let from = over(&up);
        let was = st.player.as_ref().map(|p| (p.place, p.pos, p.body));
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Char('l')), key(KeyCode::Char('j'))]),
            ScreenCmd::Stay
        ));
        assert_eq!(over(&up), from.offset(1, 1));
        assert_eq!(
            st.player.as_ref().map(|p| (p.place, p.pos, p.body)),
            was,
            "the Net self waits where it is until you go down"
        );
        assert!(!logged(&st, "looked at, not walked"));
        // every tile inside the world: the Net runs over a lake or a peak too
        let w = st.outside.as_ref().unwrap().world().clone();
        let (shut, d) = w
            .tiles
            .iter()
            .map(|(p, _)| p)
            .filter(|p| step_cost(&w, *p).is_none())
            .find_map(|p| {
                [Dir::N, Dir::E, Dir::S, Dir::W]
                    .into_iter()
                    .find(|d| w.tile(p.step(d.opposite())).is_some())
                    .map(|d| (p, d))
            })
            .expect("a tile you can't walk onto on foot");
        put_over(&mut up, shut.step(d.opposite()));
        up.update(&mut st, &[key(code(d))]);
        assert_eq!(over(&up), shut, "the Net runs over it");
        // not past the world's edge
        put_over(&mut up, Pos::new(0, 5));
        up.update(&mut st, &[key(KeyCode::Char('h'))]);
        assert_eq!(over(&up), Pos::new(0, 5));
        assert!(logged(&st, "the world ends here"));
        // HERE is the tile walked to: over another city's own tiles, that city
        let (t, name) = {
            let o = st.outside.as_ref().unwrap();
            let routes = &o.world().routes;
            o.others()
                .into_iter()
                .filter(|(s, _)| s.town.is_none() && *s != o.here())
                .find_map(|(s, b)| {
                    b.iter()
                        .find(|t| {
                            o.owner(*t) == Some(s) && !routes.iter().any(|r| r.path.contains(t))
                        })
                        .map(|t| (t, o.name(s)))
                })
                .expect("a city's own tile off the roads")
        };
        put_over(&mut up, t);
        let page = text(&st, &up, 120, 40);
        assert!(page.contains(&format!("the Net · {name}")), "{page}");
    }

    #[test]
    fn a_net_overworld_step_costs_a_tiles_walk() {
        let (mut st, _m, mut up) = up_here();
        let before = st.clock_tick();
        up.update(&mut st, &[key(KeyCode::Char('l'))]);
        let walk = TILE as u64 * sim::WALK_TICKS;
        assert_eq!(st.clock_tick() - before, walk);
        // standing still costs nothing, nor does a step refused
        up.update(&mut st, &[]);
        put_over(&mut up, Pos::new(0, 5));
        up.update(&mut st, &[key(KeyCode::Char('h'))]);
        assert_eq!(st.clock_tick() - before, walk);
    }

    #[test]
    fn enter_on_the_net_overworld_lands_on_that_tiles_net() {
        let (mut st, mut m, mut up) = up_here();
        let body = st.player.as_ref().unwrap().body;
        let from = over(&up);
        let w = st.outside.as_ref().unwrap().world().size;
        let d = if from.x < w.w as i32 / 2 {
            KeyCode::Char('l')
        } else {
            KeyCode::Char('h')
        };
        up.update(&mut st, &[key(d), key(d), key(d), key(d)]);
        let t = over(&up);
        assert_eq!((t.x - from.x).abs(), 4);
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        let p = st.player.as_ref().unwrap().clone();
        assert_eq!((p.place, p.body), (NET, body), "the body hasn't moved");
        let c = world_cell(&st, p.pos);
        let landed = Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE));
        assert!(
            (landed.x - t.x).abs() <= 1 && (landed.y - t.y).abs() <= 1,
            "{t:?} → {landed:?}"
        );
        assert!(net_walkable(&st, p.pos), "{:?}", net_tile(&st, p.pos));
        assert!(
            !matches!(
                net_tile(&st, p.pos).map(|t| t.terrain),
                Some(citygen::Terrain::NetFloor | citygen::Terrain::NetGate)
            ),
            "not inside a construct"
        );
        // the map shows you there, on the Net
        m.update(&mut st, &[]);
        let page = text(&st, &m, 120, 40);
        assert!(page.contains("the Net ·"), "{page}");
        assert_eq!(st.player.as_ref().unwrap().pos, p.pos);
    }

    #[test]
    fn landing_prefers_the_trunk() {
        let (mut st, _m, mut up) = up_here();
        let here = over(&up);
        let t = {
            let o = st.outside.as_ref().unwrap();
            let w = o.world();
            w.tiles
                .iter()
                .filter(|(p, t)| *p != here && o.owner(*p).is_none() && t.road != 0)
                .map(|(p, _)| p)
                .min_by_key(|p| p.dist2(here))
                .expect("a trunk out in the open")
        };
        put_over(&mut up, t);
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        let p = st.player.as_ref().unwrap().pos;
        let c = world_cell(&st, p);
        assert_eq!(Pos::new(c.x.div_euclid(TILE), c.y.div_euclid(TILE)), t);
        assert_eq!(
            net_tile(&st, p).map(|t| t.terrain),
            Some(citygen::Terrain::NetLane)
        );
    }

    #[test]
    fn landing_over_another_city_hands_it_over() {
        let (mut st, mut m, mut up) = up_here();
        let (s, t, here) = {
            let o = st.outside.as_ref().unwrap();
            let (s, b) = o
                .others()
                .into_iter()
                .find(|(s, _)| s.town.is_none() && *s != o.here())
                .unwrap();
            let t = b.iter().find(|t| o.owner(*t) == Some(s)).unwrap();
            (s, t, o.here())
        };
        put_over(&mut up, t);
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        let o = st.outside.as_ref().unwrap();
        assert_eq!(o.here(), here, "down on its Net, not yet in");
        assert!(o.held_city(s).is_some(), "built to land on");
        let p = st.player.as_ref().unwrap().pos;
        let b = crate::outside::stream::Beyond::of(&st).unwrap();
        assert!(
            matches!(
                b.elsewhere(p),
                Some(crate::outside::stream::Owner::City(..))
            ),
            "on its own map's Net"
        );
        assert!(net_walkable(&st, p));
        // the map's next frame: the city whose Net you stand on is the live one, your
        // body where it was
        let body = st.player.as_ref().unwrap().body;
        let body_at = body.map(|(_, b)| world_cell(&st, b));
        let at = world_cell(&st, p);
        m.update(&mut st, &[]);
        assert_eq!(st.outside.as_ref().unwrap().here(), s, "handed over");
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, world_cell(&st, p.pos)), (NET, at));
        assert_eq!(p.body.map(|(_, b)| world_cell(&st, b)), body_at);
    }

    #[test]
    fn a_run_saved_after_a_drop_reopens_on_that_net() {
        // dropped onto another city's Net by the Net overworld, far from the body:
        // saved and opened again, jacked in there, the body where it stood
        let (mut st, _m, mut up) = up_here();
        let t = {
            let o = st.outside.as_ref().unwrap();
            let (s, b) = o
                .others()
                .into_iter()
                .find(|(s, _)| s.town.is_none() && *s != o.here())
                .unwrap();
            b.iter().find(|t| o.owner(*t) == Some(s)).unwrap()
        };
        put_over(&mut up, t);
        up.update(&mut st, &[key(KeyCode::Enter)]);
        let was = st.player.as_ref().map(|p| (p.place, p.pos, p.body));
        let dir = crate::runs::testutil::Dir::new("net-drop");
        let st = crate::runs::testutil::running(st);
        let path = crate::runs::file::write(&dir.0, &st).unwrap();
        let mut back = AppState::new();
        let (_, snap, lost) = crate::runs::file::read(&path, &back.book).unwrap();
        crate::runs::snapshot::restore(&mut back, snap);
        crate::runs::snapshot::after_restore(&mut back, lost);
        assert_eq!(back.player.as_ref().map(|p| (p.place, p.pos, p.body)), was);
        // on its Net the city is made again and handed over, you on the Net on the same
        // world cell
        let at = world_cell(&back, back.player.as_ref().unwrap().pos);
        let mut map = crate::screens::resume_screen(&mut back);
        map.update(&mut back, &[]);
        let o = back.outside.as_ref().unwrap();
        assert_eq!(
            Some(o.here()),
            crate::outside::tile_owner(
                &back,
                Pos::new(at.x.div_euclid(TILE), at.y.div_euclid(TILE))
            )
        );
        let p = back.player.as_ref().unwrap();
        assert!(p.jacked_in() && p.place == NET);
        assert_eq!(world_cell(&back, p.pos), at);
        assert!(text(&back, map.as_ref(), 120, 40).contains("the Net"));
    }

    #[test]
    fn a_fight_holds_you_where_you_are() {
        let (mut st, _m, mut up) = up_here();
        st.net.engaged = Some(crate::net::Engagement {
            place: NET,
            spent_at: None,
            boosted: false,
            round: 1,
            order: vec![crate::net::Actor::You],
            turn: 0,
            moves: 1,
            ap: 2,
            phase: crate::net::Phase::Act,
            manual: false,
            acting_since: 0,
        });
        let (from, was, tick) = (
            over(&up),
            st.player.as_ref().map(|p| p.pos),
            st.clock_tick(),
        );
        up.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(over(&up), from, "no walking off in a fight");
        assert_eq!(st.clock_tick(), tick);
        put_over(&mut up, from.offset(3, 0));
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert_eq!(
            st.player.as_ref().map(|p| p.pos),
            was,
            "nor dropping elsewhere"
        );
        assert!(logged(&st, "not in a fight"));
    }

    #[test]
    fn a_ground_jack_in_shows_the_cord_and_the_port() {
        let (mut st, mut m) = jacked_by_a_pump();
        let came = st.player.as_ref().unwrap().pos;
        assert_eq!(
            crate::net::port_of(&st),
            Some(came),
            "the port: where you came onto the Net"
        );
        // out of the truck stop's gate and on
        for _ in 0..2 {
            m.update(&mut st, &[key(KeyCode::Char('j'))]);
        }
        for _ in 0..12 {
            m.update(&mut st, &[key(KeyCode::Char('l'))]);
        }
        let you = st.player.as_ref().unwrap().pos;
        assert!(you.chebyshev(came) > 4, "{you:?} from {came:?}");
        let v = crate::net::cord_view(&st).expect("the cord");
        assert_eq!((v.entry, v.port), (came, Some(came)));
        let view = citygen::Rect::new(you.x - 60, you.y - 20, 120, 40);
        let walk = crate::net::cord_walk_beyond(&st, v.entry, you, view);
        assert_eq!(walk.first(), Some(&came));
        assert_eq!(walk.last().map(|q| q.manhattan(you)), Some(1));
        assert!(walk.windows(2).all(|w| w[0].manhattan(w[1]) == 1));
        // drawn: the port on the map and on the minimap
        m.update(&mut st, &[]);
        let page = text(&st, &m, 120, 40);
        assert!(page.contains('▣'), "the port on the map: {page}");
        assert!(page.contains('▪'), "the port on the minimap: {page}");
    }

    #[test]
    fn the_net_overworlds_help_is_its_own() {
        let (mut st, _m, mut up) = looking_up();
        up.update(&mut st, &[key(KeyCode::Char('?'))]);
        let page = text(&st, up.as_ref(), 120, 40);
        assert!(page.contains("THE NET OVERWORLD"), "{page}");
        assert!(
            !page.contains("save game"),
            "the overworld's keys aren't these"
        );
    }

    #[test]
    fn enter_over_your_own_tile_is_back_where_you_were_and_says_so() {
        // ↵ over the tile your Net self is over already leaves it where it stood, as W
        // does; the help says so beside ↵ onto another tile's trunk
        let (mut st, _m, mut up) = up_here();
        let was = st.player.as_ref().map(|p| (p.place, p.pos, p.body));
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        assert_eq!(st.player.as_ref().map(|p| (p.place, p.pos, p.body)), was);
        up.update(&mut st, &[key(KeyCode::Char('?'))]);
        let page = text(&st, &up, 120, 40);
        assert!(page.contains("on its trunk if it has one"), "{page}");
        let own = page
            .lines()
            .find(|l| l.contains("enter over yours"))
            .unwrap_or_else(|| panic!("↵ over your own tile\n{page}"));
        assert!(own.contains("back down to where you were"), "{own}");
    }

    #[test]
    fn j_from_the_net_overworld_jacks_out() {
        let (mut st, mut m, mut up) = looking_up();
        let (_, body) = st.player.as_ref().unwrap().body.unwrap();
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Char('J'))]),
            ScreenCmd::Pop { .. }
        ));
        m.update(&mut st, &[]);
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos, p.body), (STREET, body, None));
    }

    #[test]
    fn the_net_overworld_fits_the_smallest_terminal() {
        let (st, _m, up) = looking_up();
        let page = text(&st, up.as_ref(), 100, 30);
        assert!(page.contains("the Net overworld"), "{page}");
        assert!(!page.contains("look only"), "{page}");
    }

    #[test]
    fn the_net_overworlds_help_says_it_walks() {
        let (mut st, _m, mut up) = looking_up();
        let page = text(&st, up.as_ref(), 120, 40);
        assert!(!page.contains("look only"), "{page}");
        up.update(&mut st, &[key(KeyCode::Char('?'))]);
        let page = text(&st, up.as_ref(), 120, 40);
        assert!(page.contains("THE NET OVERWORLD"), "{page}");
        assert!(page.contains("walk a tile"), "{page}");
        assert!(!page.contains("to look at"), "{page}");
        // and the map's own help says W on the Net walks it
        let mut t = Terminal::new(TestBackend::new(120, 60)).unwrap();
        t.draw(|f| crate::screens::help::draw(f, crate::screens::explore::Mode::Walk, false))
            .unwrap();
        let page = buffer_text(t.backend().buffer());
        assert!(page.contains("open street or the Net"), "{page}");
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
            up.update(&mut st, &[key(KeyCode::Char('W'))]),
            ScreenCmd::Pop { .. }
        ));
        let p = st.player.as_ref().unwrap();
        assert!(
            p.jacked_in() && p.pos == pos,
            "still jacked in where you were"
        );
    }

    #[test]
    fn esc_opens_the_pause_menu() {
        let (mut st, mut s) = out_there();
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Push { .. }
        ));
        // on the Net overworld too
        let (mut st, _m, mut up) = looking_up();
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Push { .. }
        ));
    }

    #[test]
    fn a_closed_step_says_why() {
        let (mut st, mut s) = out_there();
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

    /// The hints the overworlds' footers once carried.
    const HINTS: [&str; 5] = [
        "arrows walk",
        "enter goes down",
        "J jack out",
        "esc menu",
        "? keys",
    ];

    #[test]
    fn the_overworlds_draw_no_hint_line() {
        // the overworld: no line of keys, `?` still lists them
        let (mut st, mut s) = out_there();
        for (w, h) in [(120, 40), (100, 30)] {
            let page = text(&st, &s, w, h);
            for hint in HINTS {
                assert!(!page.contains(hint), "{hint}: {page}");
            }
        }
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        assert!(text(&st, &s, 120, 40).contains("KEYS · OUTSIDE"));
        // the Net over it, the same
        let (mut st, _m, mut up) = looking_up();
        for (w, h) in [(120, 40), (100, 30)] {
            let page = text(&st, up.as_ref(), w, h);
            for hint in HINTS {
                assert!(!page.contains(hint), "{hint}: {page}");
            }
        }
        up.update(&mut st, &[key(KeyCode::Char('?'))]);
        assert!(text(&st, up.as_ref(), 120, 40).contains("THE NET OVERWORLD"));
    }

    /// `line` typed into the console, then enter.
    fn type_line(st: &mut AppState, s: &mut OutsideScreen, line: &str) {
        let mut keys: Vec<KeyEvent> = line.chars().map(|c| key(KeyCode::Char(c))).collect();
        keys.push(key(KeyCode::Enter));
        s.update(st, &keys);
    }

    /// The tile the screen is over: the Net overworld's, else where you stand.
    fn over_tile(st: &AppState, s: &OutsideScreen) -> Pos {
        match s.net_up.as_ref() {
            Some(u) => u.tile,
            None => match st.outside.as_ref().unwrap().at {
                At::Overworld(t) => t,
                at => panic!("not on the overworld: {at:?}"),
            },
        }
    }

    #[test]
    fn the_overworld_takes_dev_commands() {
        for net in [false, true] {
            let (mut st, mut s) = if net {
                let (st, _m, up) = up_here();
                (st, up)
            } else {
                out_there()
            };
            let w = st.outside.as_ref().unwrap().world().clone();
            let from = over_tile(&st, &s);
            // somewhere to go: a tile you can stand on, three east or so
            let to = (3..12)
                .map(|dx| from.offset(dx, 1))
                .find(|t| step_cost(&w, *t).is_some())
                .expect("ground to stand on nearby");
            // locked, as a player's game starts: not the console, not the inspector
            assert!(!st.dev_unlocked);
            let credits = st.character.credits;
            s.update(&mut st, &[key(KeyCode::Char('`'))]);
            assert!(!st.dev, "dev mode stays off");
            s.update(&mut st, &[key(KeyCode::Char(':'))]);
            assert!(s.console.is_none(), "no console");
            s.update(&mut st, &[key(KeyCode::F(5))]);
            assert!(!text(&st, &s, 120, 40).contains("INSPECT"));
            assert_eq!(over_tile(&st, &s), from);
            s.update(&mut st, &[key(KeyCode::Char('?'))]);
            assert!(!text(&st, &s, 120, 40).contains("dev console"), "{net}");
            s.update(&mut st, &[key(KeyCode::Char('?'))]);
            // unlocked: the console runs a line
            st.unlock_dev();
            s.update(&mut st, &[key(KeyCode::Char(':'))]);
            assert!(s.console.is_some(), "the console opens (net {net})");
            let page = text(&st, &s, 120, 40);
            assert!(page.contains(": ▮"), "drawn on the footer: {page}");
            type_line(&mut st, &mut s, "give 100");
            assert_eq!(st.character.credits, credits + 100, "net {net}");
            // a teleport to a tile of the world
            type_line(&mut st, &mut s, &format!("tp {} {}", to.x, to.y));
            assert_eq!(over_tile(&st, &s), to, "net {net}: {:?}", st.log.latest());
            type_line(&mut st, &mut s, "tp -5 -5");
            assert_eq!(over_tile(&st, &s), to, "off the world: refused");
            // what acts through the map waits for it
            type_line(&mut st, &mut s, "wait");
            assert_eq!(
                st.log.latest().map(|e| e.text.as_str()),
                Some("not out here")
            );
            // esc closes it; the inspector reads the tile you're over
            s.update(&mut st, &[key(KeyCode::Esc)]);
            assert!(s.console.is_none());
            s.update(&mut st, &[key(KeyCode::F(5))]);
            let page = text(&st, &s, 140, 50);
            assert!(page.contains("INSPECT"), "net {net}: {page}");
            assert!(page.contains(&format!("{},{}", to.x, to.y)), "{page}");
            // `?` lists them once unlocked
            s.update(&mut st, &[key(KeyCode::Char('?'))]);
            let page = text(&st, &s, 120, 40);
            assert!(
                page.contains("dev console") && page.contains("inspect"),
                "{page}"
            );
            s.update(&mut st, &[key(KeyCode::Char('?'))]);
            // dev mode off: none of it
            s.update(&mut st, &[key(KeyCode::Char('`'))]);
            assert!(!st.dev);
            assert!(!text(&st, &s, 140, 50).contains("INSPECT"));
            s.update(&mut st, &[key(KeyCode::Char(':'))]);
            assert!(s.console.is_none());
        }
    }

    #[test]
    fn a_drop_waits_for_what_holds_you() {
        // what holds you in the city holds your Net self out of a drop elsewhere, with
        // walking's own words: a call, a scene, a run in flight
        type Hold = fn(&mut AppState);
        let holds: [(&str, Hold); 3] = [
            ("not with a call on the line", |st| {
                st.missions.call = Some(crate::missions::Call {
                    story: None,
                    from: "mezz".into(),
                    lines: vec!["you there?".into()],
                    rang: st.clock_tick(),
                    answered: None,
                    line: 0,
                    shown: 0,
                    full: false,
                })
            }),
            ("not now", |st| {
                st.missions.scenes.push_back((None, "waking".to_string()))
            }),
            ("finish the run first", |st| {
                st.net.run = Some(game::Run {
                    program: game::ItemId::new("crack"),
                    target: game::HackTarget::Deck,
                    done: 0,
                    work: 3,
                })
            }),
        ];
        for (line, hold) in holds {
            let (mut st, _m, mut up) = up_here();
            let from = over(&up);
            let was = st.player.as_ref().map(|p| p.pos);
            hold(&mut st);
            put_over(&mut up, from.offset(3, 0));
            assert!(matches!(
                up.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Stay
            ));
            assert_eq!(
                st.player.as_ref().map(|p| p.pos),
                was,
                "{line}: not dropped"
            );
            assert_eq!(
                st.log.latest().map(|e| e.text.as_str()),
                Some(line),
                "the words walking says"
            );
            // over your own tile, ↵ is where you are: nothing to refuse
            put_over(&mut up, from);
            assert!(matches!(
                up.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Pop { .. }
            ));
        }
    }
}
