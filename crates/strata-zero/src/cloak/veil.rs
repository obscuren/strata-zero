//! The veil (#36): while the cloak holds, the map drifts in pink and cyan. Every phase is
//! the same drift turned up or down: `heat` drives the split, its shiver and the wave (0
//! calm, 1 an empty cell, past 1 out of control), and a plain frame is one where the
//! cloak cuts out and the plain map shows through. A port of the agreed mock's `look()`
//! and frame (docs/superpowers/specs/2026-11-15-stealth-mock.html) to frame ticks.

use super::Phase;
use crate::app::AppState;
use crate::render::map::{MapRef, TileMap};
use crate::render::viewport::{screen_to_tile, tile_to_screen};
use crate::render::{Rgb, Visibility};
use crate::ui::chrome::{CYAN_RGB, MAGENTA_RGB, PANEL_BG, PANEL_BG_RGB};
use citygen::Pos;
use citygen::rng::hash64;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Frame ticks a roll holds: the mock steps every 90 ms, 3 frames is 100 ms.
pub const VEIL_STEP: u64 = 3;
/// Where a pink and a cyan copy land on one cell.
pub const NEAR_WHITE: (u8, u8, u8) = (244, 248, 255);
/// The floor's tint while the veil is over half on: its glyph mixed from the ground toward
/// cyan, the mock's 28% cyan over the background.
const FLOOR_TINT: f32 = 0.28;
/// The wash toward cyan, even over the map: the mock's 5% tint and its 1px lines one row
/// in three (8% a third), which a cell can't hold.
const WASH: f32 = 0.077;

/// How the veil looks this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// 0 calm, 1 an empty cell, past 1 out of control.
    pub heat: f32,
    /// How far the copies land, in cells.
    pub split: i32,
    /// How strong the wash is, 0..1.
    pub veil: f32,
    /// Rows ripple under a moving band.
    pub wave: bool,
    /// A frame where the cloak cuts out: the plain map.
    pub plain: bool,
}

/// What a map cell is to the veil.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    /// Bare floor: takes copies.
    Empty,
    /// A wall, furniture, a foe: throws copies.
    Solid,
    /// Neither: people, items, you, what you can't see.
    Off,
}

/// The split at rest: 1 cell when the pool is full, 3 near empty.
pub fn rest(heat: f32) -> i32 {
    1 + (heat * 2.4).round() as i32
}

/// The look of `phase` at progress `p` (0..1; holding ignores it), from the heat it
/// plays from. `roll(k)` is this frame's k-th roll in 0..1: 0, 1 and 2 the shiver, 3 the
/// plain frame.
pub fn look(phase: Phase, p: f32, hot: f32, roll: &dyn Fn(u64) -> f32) -> Look {
    let p = p.clamp(0.0, 1.0);
    let shiver = |h: f32| {
        if h > 0.35 && roll(0) < h.min(1.0) {
            let sign = if roll(1) < 0.5 { -1 } else { 1 };
            let size = if h > 1.2 && roll(2) >= 0.5 { 2 } else { 1 };
            sign * size
        } else {
            0
        }
    };
    let drift = |heat: f32, veil: f32| Look {
        heat,
        split: (rest(heat) + shiver(heat)).max(1),
        veil,
        wave: true,
        plain: false,
    };
    let plain = Look {
        heat: 0.0,
        split: 0,
        veil: 0.0,
        wave: false,
        plain: true,
    };
    match phase {
        Phase::Holding => drift(hot, 1.0),
        // The cloak catches: it sputters in, wide and wild, then settles to its rest.
        Phase::GoingIn if p < 0.45 => {
            let q = p / 0.45;
            if roll(3) < lerp(0.85, 0.1, q) {
                plain
            } else {
                drift(lerp(0.9, 1.5, q), q)
            }
        }
        Phase::GoingIn => drift(lerp(1.5, hot, ease((p - 0.45) / 0.55)), 1.0),
        // The drift you know, pushed past empty, then sputtering out.
        Phase::BurningOut if p < 0.55 => drift(lerp(hot, 1.7, ease(p / 0.55)), 1.0),
        Phase::BurningOut => {
            let q = (p - 0.55) / 0.45;
            if roll(3) < lerp(0.2, 1.0, q * q) {
                plain
            } else {
                drift(lerp(1.7, 0.6, q), 1.0 - q * 0.6)
            }
        }
        // The same drift, eased out.
        Phase::SwitchingOff => {
            let q = ease(p);
            Look {
                heat: lerp(hot, 0.0, q),
                split: lerp(rest(hot) as f32, 0.0, q).round() as i32,
                veil: 1.0 - q,
                wave: q < 0.6,
                plain: false,
            }
        }
    }
}

/// The veil's phase now, its progress 0..1 and the heat it plays from: a fade while it
/// runs, else holding at the pool's heat while a cloak is on, else none. Going in eases
/// toward the pool's heat as it is now, so holding takes over where it ends.
pub fn now(st: &AppState) -> Option<(Phase, f32, f32)> {
    let on = super::heat(st);
    if let Some(f) = st.veil.fade {
        let len = super::fade_ticks(f.phase).max(1);
        let p = st.tick.saturating_sub(f.since) as f32 / len as f32;
        if p < 1.0 {
            return Some(match on {
                Some(h) => (Phase::GoingIn, p, h),
                None => (f.phase, p, f.hot),
            });
        }
    }
    on.map(|h| (Phase::Holding, 1.0, h))
}

/// The veil over the explore map `map`, its top-left tile `origin`, while a cloak or its
/// fade is on: walls, furniture and the foes you see throw copies onto the bare floor you
/// see, and the foes and your `@` stay where they stand. Off with `st.veil.off`.
pub fn draw_map(buf: &mut Buffer, map: Rect, origin: Pos, st: &AppState, mref: &MapRef) {
    if st.veil.off {
        return;
    }
    let Some(((phase, p, hot), you)) = now(st).zip(st.player.as_ref()) else {
        return;
    };
    // On foot only: a fade left as you jack in plays out off the Net's map.
    if super::on_the_net(you.place) {
        return;
    }
    let seed = hash64(you.pos.x as u64, you.pos.y as u64);
    let look = look(phase, p, hot, &|k| roll(st.tick, seed, k));
    let screen = |at: Pos| tile_to_screen(map, origin, at);
    let foes: Vec<Pos> = (crate::foes::in_sight(st).into_iter())
        .map(|i| st.foes[i].pos)
        .collect();
    let keep: Vec<(u16, u16)> = foes.iter().filter_map(|p| screen(*p)).collect();
    let vis = you.vis();
    let ground = |x: u16, y: u16| {
        let at = screen_to_tile(map, origin, (x, y));
        let Some(seen) = vis.get(at).copied() else {
            return Ground::Off;
        };
        let walkable = mref.walkable(at);
        let bare = || matches!(buf[(x, y)].symbol(), " " | "·" | "▫" | ".");
        match seen {
            Visibility::Visible if foes.contains(&at) => Ground::Solid,
            Visibility::Visible | Visibility::Remembered if !walkable => Ground::Solid,
            Visibility::Visible if bare() => Ground::Empty,
            _ => Ground::Off,
        }
    };
    let area = map.intersection(buf.area);
    let grounds: Vec<Ground> = (area.y..area.bottom())
        .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
        .map(|(x, y)| ground(x, y))
        .collect();
    let ground = |x: u16, y: u16| {
        grounds[usize::from(y - area.y) * usize::from(area.width) + usize::from(x - area.x)]
    };
    draw(
        buf,
        map,
        &look,
        st.tick,
        seed,
        &ground,
        screen(you.pos),
        &keep,
    );
}

/// The k-th roll of the frame at `tick`, in 0..1: the same for a veil step.
pub fn roll(tick: u64, seed: u64, k: u64) -> f32 {
    hash64(hash64(tick / VEIL_STEP, k), seed) as f32 / u64::MAX as f32
}

/// `a` to `b` by `t`, clamped.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// A copy's colour.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Channel {
    Pink,
    Cyan,
    Both,
}

impl Channel {
    fn rgb(self) -> Rgb {
        match self {
            Channel::Pink => MAGENTA_RGB,
            Channel::Cyan => CYAN_RGB,
            Channel::Both => Rgb(NEAR_WHITE.0, NEAR_WHITE.1, NEAR_WHITE.2),
        }
    }
}

/// `c` mixed toward `to` by `by`; a colour that isn't Rgb counts as the panel.
fn toward(c: Color, to: Rgb, by: f32) -> Color {
    let from = match c {
        Color::Rgb(r, g, b) => Rgb(r, g, b),
        _ => PANEL_BG_RGB,
    };
    from.mix(to, by).to_color()
}

/// The veil over the map rect `map`, in the mock's order: each `Solid` cell throws a
/// pink copy `split` cells left and a cyan one right onto `Empty` floor, the floor takes
/// a cyan tint, and the wave shifts rows sideways. Your row never shifts, and a foe
/// really there (`keep`) shifted along with its row leaves only a channel copy of
/// itself; the cells in `keep` and `you` are put back as they were. Last, the cyan wash
/// lies over the whole map, as the mock's sits behind it. Nothing outside `map` and the
/// buffer is touched.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    buf: &mut Buffer,
    map: Rect,
    look: &Look,
    tick: u64,
    seed: u64,
    ground: &dyn Fn(u16, u16) -> Ground,
    you: Option<(u16, u16)>,
    keep: &[(u16, u16)],
) {
    let area = map.intersection(buf.area);
    if look.plain || (look.split == 0 && look.veil == 0.0 && !look.wave) || area.is_empty() {
        return;
    }
    let before = buf.clone();
    let at = |x: u16, y: u16| {
        usize::from(y - area.y) * usize::from(area.width) + usize::from(x - area.x)
    };
    let cols = |x: i32| x >= i32::from(area.x) && x < i32::from(area.right());

    // The copies: the last to land gives the symbol; pink and cyan together read white.
    let mut landed = vec![false; usize::from(area.width) * usize::from(area.height)];
    // A copy's strength over its ground, a shifted foe's too.
    let by = (0.5 + 0.45 * look.heat).min(1.0);
    if look.split > 0 {
        let mut hits: Vec<Option<((u16, u16), Channel)>> = vec![None; landed.len()];
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if Some((x, y)) == you || ground(x, y) != Ground::Solid {
                    continue;
                }
                for (dx, channel) in [(-look.split, Channel::Pink), (look.split, Channel::Cyan)] {
                    let to = i32::from(x) + dx;
                    if !cols(to) {
                        continue;
                    }
                    let hit = &mut hits[at(to as u16, y)];
                    let channel = match *hit {
                        Some((_, prev)) if prev != channel => Channel::Both,
                        _ => channel,
                    };
                    *hit = Some(((x, y), channel));
                }
            }
        }
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let Some((from, channel)) = hits[at(x, y)] else {
                    continue;
                };
                if Some((x, y)) == you || ground(x, y) != Ground::Empty {
                    continue;
                }
                let cell = &mut buf[(x, y)];
                let fg = toward(cell.bg, channel.rgb(), by);
                cell.set_symbol(before[from].symbol()).set_fg(fg);
                landed[at(x, y)] = true;
            }
        }
    }

    // The floor's tint, where no copy landed.
    if look.veil > 0.5 {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if !landed[at(x, y)] && ground(x, y) == Ground::Empty {
                    let cell = &mut buf[(x, y)];
                    cell.set_fg(toward(cell.bg, CYAN_RGB, FLOOR_TINT));
                }
            }
        }
    }

    // The wave: a band of rows running down the map, shifting them sideways.
    if look.wave {
        let t = tick as f64 * 1000.0 / 30.0;
        let heat = f64::from(look.heat);
        let rows = i64::from(map.height);
        let band = (rows - 2).min(1 + (4.0 * heat).round() as i64);
        let period = (150.0 - 100.0 * heat).max(35.0);
        let head = ((t / period).floor() as i64).rem_euclid(rows + band + 4);
        let back = (t / 600.0).floor() as i64 % 2 == 1;
        let slip = f64::from(roll(tick, seed, 4)) < (0.12 * heat * heat).min(0.5);
        let shifted = buf.clone();
        let your_row = you.map(|(_, row)| row);
        for y in area.y..area.bottom() {
            if Some(y) == your_row {
                continue;
            }
            let into = head - i64::from(y - map.y);
            let mut shift = 0;
            if into >= 0 && into < band {
                let mid = 1.0
                    - (into as f64 - (band - 1) as f64 / 2.0).abs() / (band as f64 / 2.0).max(1.0);
                shift = (((1.0 + 2.0 * heat) * mid).round() as i32).max(1);
                if back {
                    shift = -shift;
                }
            }
            if slip {
                shift += 1;
            }
            if shift == 0 {
                continue;
            }
            // A foe shifted along is only a copy: pink going left, cyan going right.
            let channel = if shift < 0 {
                Channel::Pink
            } else {
                Channel::Cyan
            };
            for x in area.x..area.right() {
                let from = i32::from(x) - shift;
                buf[(x, y)] = if cols(from) {
                    let mut cell = shifted[(from as u16, y)].clone();
                    if keep.contains(&(from as u16, y)) {
                        let fg = toward(cell.bg, channel.rgb(), by);
                        cell.set_fg(fg);
                    }
                    cell
                } else {
                    let mut blank = shifted[(x, y)].clone();
                    blank.reset();
                    blank.set_bg(PANEL_BG).set_fg(PANEL_BG);
                    blank
                };
            }
        }
    }

    // Foes stay on their true cells; your `@` never moves.
    for &(x, y) in keep.iter().chain(you.iter()) {
        if area.contains((x, y).into()) {
            buf[(x, y)] = before[(x, y)].clone();
        }
    }

    // The wash, last: under you and the foes too.
    if look.veil > 0.0 {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let cell = &mut buf[(x, y)];
                cell.set_bg(toward(cell.bg, CYAN_RGB, WASH * look.veil));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloak::testutil::{fire, frames, scrambler, street};
    use crate::cloak::{BREAK_TICKS, START_TICKS};
    use citygen::{Place, StratumKind};

    /// The test map: 40 × 12 at (2, 3) in a 60 × 20 buffer.
    const MAP: Rect = Rect::new(2, 3, 40, 12);

    /// The buffer, every cell a bare dark floor on the panel.
    fn grid() -> (Buffer, Rect) {
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                buf[(x, y)]
                    .set_char(' ')
                    .set_fg(Color::Rgb(28, 32, 40))
                    .set_bg(PANEL_BG);
            }
        }
        (buf, MAP)
    }

    fn put_wall(buf: &mut Buffer, at: (u16, u16)) {
        buf[at].set_char('#').set_fg(Color::Rgb(150, 160, 170));
    }

    /// Solid at the walls, Empty elsewhere in the map, Off outside it.
    fn grounds(walls: &[(u16, u16)]) -> impl Fn(u16, u16) -> Ground + use<> {
        let walls = walls.to_vec();
        move |x, y| {
            if !MAP.contains((x, y).into()) {
                Ground::Off
            } else if walls.contains(&(x, y)) {
                Ground::Solid
            } else {
                Ground::Empty
            }
        }
    }

    fn pinkish(c: Color) -> bool {
        matches!(c, Color::Rgb(r, g, b) if r > 100 && g < 60 && b > 80)
    }

    fn cyanish(c: Color) -> bool {
        matches!(c, Color::Rgb(r, g, b) if r < 60 && g > 100 && b > 100)
    }

    #[test]
    fn the_split_rests_at_one_full_and_three_empty() {
        assert_eq!((rest(0.0), rest(0.5), rest(1.0)), (1, 2, 3));
        let held = |h| look(Phase::Holding, 1.0, h, &|_| 1.0);
        assert_eq!(
            held(0.0),
            Look {
                heat: 0.0,
                split: 1,
                veil: 1.0,
                wave: true,
                plain: false
            }
        );
        assert_eq!(held(1.0).split, 3);
    }

    #[test]
    fn going_in_sputters_then_settles() {
        let early = look(Phase::GoingIn, 0.05, 0.3, &|_| 0.0);
        assert!(early.plain && early.split == 0 && !early.wave);
        let settled = look(Phase::GoingIn, 1.0, 0.3, &|_| 1.0);
        assert!((settled.heat - 0.3).abs() < 1e-4 && settled.veil == 1.0);
        let wild = look(Phase::GoingIn, 0.45, 0.3, &|_| 1.0);
        assert!(wild.heat > 1.4, "wide at first: {wild:?}");
    }

    #[test]
    fn burning_out_runs_past_empty_then_cuts_out() {
        let mid = look(Phase::BurningOut, 0.5, 1.0, &|_| 1.0);
        assert!(mid.heat > 1.5 && mid.split >= 4);
        assert!(
            look(Phase::BurningOut, 0.99, 1.0, &|_| 0.5).plain,
            "plain frames near the end"
        );
    }

    #[test]
    fn switching_off_eases_out_to_nothing() {
        let end = look(Phase::SwitchingOff, 1.0, 0.6, &|_| 1.0);
        assert_eq!((end.split, end.veil, end.wave), (0, 0.0, false));
        let half = look(Phase::SwitchingOff, 0.5, 0.6, &|_| 1.0);
        assert!(half.split >= 1 && half.veil > 0.0);
    }

    #[test]
    fn walls_throw_pink_left_and_cyan_right_onto_empty_floor() {
        let (mut buf, map) = grid();
        let wall = (20, 8);
        put_wall(&mut buf, wall);
        let still = Look {
            heat: 0.0,
            split: 1,
            veil: 0.0,
            wave: false,
            plain: false,
        };
        draw(&mut buf, map, &still, 0, 0, &grounds(&[wall]), None, &[]);
        assert_eq!(buf[(19, 8)].symbol(), "#");
        assert!(
            pinkish(buf[(19, 8)].fg) && cyanish(buf[(21, 8)].fg),
            "half way from the ground at heat 0"
        );
        assert_eq!(buf[(20, 8)].symbol(), "#", "the wall stays");
    }

    #[test]
    fn where_both_copies_land_it_reads_near_white() {
        let (mut buf, map) = grid();
        for w in [(18, 8), (20, 8)] {
            put_wall(&mut buf, w);
        }
        let still = Look {
            heat: 1.0,
            split: 1,
            veil: 0.0,
            wave: false,
            plain: false,
        };
        draw(
            &mut buf,
            map,
            &still,
            0,
            0,
            &grounds(&[(18, 8), (20, 8)]),
            None,
            &[],
        );
        let Color::Rgb(r, g, b) = buf[(19, 8)].fg else {
            panic!("rgb")
        };
        assert!(r > 220 && g > 220 && b > 220, "near-white: {r},{g},{b}");
    }

    #[test]
    fn copies_never_land_off_the_floor() {
        let (mut buf, map) = grid();
        let wall = (20, 8);
        let off = (19, 8); // a person, say: not bare floor
        put_wall(&mut buf, wall);
        buf[off].set_char('p').set_fg(Color::Rgb(200, 200, 40));
        let before = buf[off].clone();
        let walls = grounds(&[wall]);
        let ground = |x: u16, y: u16| {
            if (x, y) == off {
                Ground::Off
            } else {
                walls(x, y)
            }
        };
        let still = Look {
            heat: 0.0,
            split: 1,
            veil: 0.0,
            wave: false,
            plain: false,
        };
        draw(&mut buf, map, &still, 0, 0, &ground, None, &[]);
        assert_eq!(buf[off], before, "an Off cell keeps its symbol and colour");
        assert!(
            cyanish(buf[(21, 8)].fg),
            "the empty side still takes its copy"
        );
    }

    #[test]
    fn your_at_and_row_never_move() {
        for tick in 0..120 {
            let (mut buf, map) = grid();
            let you = (22, 9);
            buf[you].set_char('@');
            let before = buf.clone();
            let hot = Look {
                heat: 1.6,
                split: 3,
                veil: 1.0,
                wave: true,
                plain: false,
            };
            draw(&mut buf, map, &hot, tick, 7, &grounds(&[]), Some(you), &[]);
            for x in map.x..map.right() {
                assert_eq!(
                    buf[(x, 9)].symbol(),
                    before[(x, 9)].symbol(),
                    "tick {tick} x {x}"
                );
            }
            assert_eq!(
                (buf[you].symbol(), buf[you].fg),
                (before[you].symbol(), before[you].fg)
            );
        }
    }

    #[test]
    fn foes_stay_on_their_true_cells() {
        for tick in 0..120 {
            let (mut buf, map) = grid();
            let foe = (30, 5);
            put_wall(&mut buf, foe); // a foe is Solid as a wall is
            buf[foe].set_char('@').set_fg(crate::render::people::FOE);
            let before = buf.clone();
            let hot = Look {
                heat: 1.6,
                split: 3,
                veil: 1.0,
                wave: true,
                plain: false,
            };
            draw(&mut buf, map, &hot, tick, 7, &grounds(&[foe]), None, &[foe]);
            assert_eq!(
                (buf[foe].symbol(), buf[foe].fg),
                (before[foe].symbol(), before[foe].fg),
                "tick {tick}"
            );
        }
    }

    #[test]
    fn a_shifted_row_shows_its_foe_once() {
        let hot = Look {
            heat: 1.0,
            split: 0,
            veil: 1.0,
            wave: true,
            plain: false,
        };
        let foe = (30, 5);
        let mut shifted = 0;
        for tick in 0..120 {
            let (mut buf, map) = grid();
            for x in map.x..map.right() {
                buf[(x, foe.1)].set_char(char::from(b'a' + (x % 26) as u8));
            }
            buf[foe].set_char('@').set_fg(crate::render::people::FOE);
            let before = buf.clone();
            draw(&mut buf, map, &hot, tick, 7, &grounds(&[foe]), None, &[foe]);
            let row = |b: &Buffer| {
                (map.x..map.right())
                    .map(|x| b[(x, foe.1)].symbol().to_string())
                    .collect::<Vec<_>>()
            };
            if row(&buf) == row(&before) {
                continue;
            }
            shifted += 1;
            let at = |x: u16| &buf[(x, foe.1)];
            let foes = (map.x..map.right())
                .filter(|x| at(*x).symbol() == "@" && at(*x).fg == crate::render::people::FOE)
                .count();
            assert_eq!(foes, 1, "one real foe, tick {tick}");
            let copy = (map.x..map.right())
                .find(|x| *x != foe.0 && at(*x).symbol() == "@")
                .unwrap_or_else(|| panic!("the shifted copy, tick {tick}"));
            assert!(
                pinkish(at(copy).fg) || cyanish(at(copy).fg),
                "a channel copy, tick {tick}"
            );
        }
        assert!(shifted > 0, "the foe's row shifted at full heat");
    }

    #[test]
    fn the_wash_is_even_and_lies_under_you() {
        let (mut buf, map) = grid();
        let you = (22, 9);
        buf[you].set_char('@');
        let veiled = Look {
            heat: 0.0,
            split: 0,
            veil: 1.0,
            wave: false,
            plain: false,
        };
        draw(&mut buf, map, &veiled, 0, 0, &grounds(&[]), Some(you), &[]);
        let washed = buf[(map.x, map.y)].bg;
        assert_ne!(washed, PANEL_BG, "the wash is on");
        for y in map.y..map.bottom() {
            assert_eq!(buf[(map.x, y)].bg, washed, "row {y}: no stripes");
        }
        assert_eq!(buf[you].bg, washed, "no dark box under you");
        assert_eq!(buf[you].symbol(), "@");
    }

    #[test]
    fn the_wave_moves_some_row_at_full_heat() {
        let hot = Look {
            heat: 1.0,
            split: 0,
            veil: 0.0,
            wave: true,
            plain: false,
        };
        let you = (22, 9);
        let mut shifted = false;
        for tick in 0..120 {
            let (mut buf, map) = grid();
            for y in map.y..map.bottom() {
                for x in map.x..map.right() {
                    buf[(x, y)].set_char(char::from(b'a' + (x % 26) as u8)); // every column its own letter
                }
            }
            let before = buf.clone();
            draw(&mut buf, map, &hot, tick, 7, &grounds(&[]), Some(you), &[]);
            for x in map.x..map.right() {
                assert_eq!(
                    buf[(x, you.1)].symbol(),
                    before[(x, you.1)].symbol(),
                    "your row, tick {tick}"
                );
            }
            shifted |= (map.y..map.bottom()).filter(|y| *y != you.1).any(|y| {
                (map.x..map.right()).any(|x| buf[(x, y)].symbol() != before[(x, y)].symbol())
            });
        }
        assert!(shifted, "some other row moved in 4 s at full heat");
    }

    #[test]
    fn a_plain_frame_changes_nothing_and_frames_are_fixed() {
        let (mut buf, map) = grid();
        let before = buf.clone();
        let plain = look(Phase::GoingIn, 0.05, 0.3, &|_| 0.0);
        draw(&mut buf, map, &plain, 5, 0, &grounds(&[]), None, &[]);
        assert_eq!(buf, before);
        let hot = Look {
            heat: 1.2,
            split: 3,
            veil: 1.0,
            wave: true,
            plain: false,
        };
        let (mut a, _) = grid();
        let (mut b, _) = grid();
        draw(&mut a, map, &hot, 41, 9, &grounds(&[(20, 8)]), None, &[]);
        draw(&mut b, map, &hot, 41, 9, &grounds(&[(20, 8)]), None, &[]);
        assert_eq!(a, b);
        assert_eq!(roll(42, 9, 3), roll(44, 9, 3), "one roll a veil step");
        assert_ne!(roll(42, 9, 3), roll(45, 9, 3));
    }

    #[test]
    fn bare_floor_takes_its_tint_over_the_ground() {
        let (mut buf, map) = grid();
        let veiled = Look {
            heat: 0.0,
            split: 0,
            veil: 1.0,
            wave: false,
            plain: false,
        };
        draw(&mut buf, map, &veiled, 0, 0, &grounds(&[]), None, &[]);
        // The mock's 28% cyan over the background, whatever the floor's own colour.
        let tint = PANEL_BG_RGB.mix(CYAN_RGB, FLOOR_TINT).to_color();
        assert_eq!(buf[(10, 8)].fg, tint);
    }

    #[test]
    fn the_phase_follows_the_cloak() {
        let mut st = street();
        let k = scrambler(&mut st);
        assert_eq!(now(&st), None);
        fire(&mut st, "scramble").unwrap();
        assert_eq!(now(&st).map(|n| n.0), Some(Phase::GoingIn));
        frames(&mut st, START_TICKS);
        let (phase, p, hot) = now(&st).unwrap();
        assert_eq!((phase, p), (Phase::Holding, 1.0));
        assert!(hot > 0.0 && hot < 0.1);
        crate::cloak::testutil::set_charge(
            &mut st,
            k,
            game::Charge {
                left: 1,
                burned: false,
            },
        );
        frames(&mut st, 1);
        assert_eq!(now(&st).map(|n| n.0), Some(Phase::BurningOut));
        frames(&mut st, BREAK_TICKS);
        assert_eq!(now(&st), None);
    }

    #[test]
    fn going_in_settles_on_the_heat_holding_starts_from() {
        let mut st = street();
        let k = scrambler(&mut st);
        fire(&mut st, "scramble").unwrap();
        // Half the cell gone while it goes in: holding takes over at that heat, no step.
        let half = game::Charge {
            left: game::FULL_CELL / 2,
            burned: false,
        };
        crate::cloak::testutil::set_charge(&mut st, k, half);
        frames(&mut st, START_TICKS - 1);
        let (phase, p, hot) = now(&st).unwrap();
        assert_eq!(phase, Phase::GoingIn);
        let last = look(phase, p, hot, &|_| 1.0);
        frames(&mut st, 1);
        let (phase, p, hot) = now(&st).unwrap();
        assert_eq!(phase, Phase::Holding);
        let first = look(phase, p, hot, &|_| 1.0);
        assert_eq!(last.split, first.split, "{last:?} then {first:?}");
        assert!(
            (last.heat - first.heat).abs() < 0.05,
            "{last:?} then {first:?}"
        );
    }

    #[test]
    fn the_veil_is_on_foot_only() {
        let mut st = street();
        st.veil.off = false;
        scrambler(&mut st);
        fire(&mut st, "scramble").unwrap();
        frames(&mut st, START_TICKS + 1);
        let city = st.city.clone().unwrap();
        let on = |st: &AppState, k| {
            let (mut buf, map) = grid();
            let before = buf.clone();
            let mref = MapRef::stratum_of(&city, k);
            draw_map(&mut buf, map, Pos::new(0, 0), st, &mref);
            buf != before
        };
        assert!(on(&st, StratumKind::Street), "the street drifts");
        st.player.as_mut().unwrap().place = Place::Stratum(StratumKind::Net);
        assert!(!on(&st, StratumKind::Net), "the Net never does");
        st.player.as_mut().unwrap().place = Place::Construct(citygen::BuildingId(0));
        assert!(!on(&st, StratumKind::Street), "nor a construct");
    }

    #[test]
    fn a_cloaked_floor_draws_the_drift_and_keeps_every_guard() {
        use crate::sitenet::testutil::{drawn, office_copy};
        let (mut st, b) = office_copy(crate::venues::SITE_NET_SECURITY);
        // You on a lit floor with a guard in your sight, the daze off.
        let gat = crate::cloak::testutil::a_guard_in_sight(&mut st, b); // the guard's tile
        st.missions.daze = None;
        scrambler(&mut st);
        fire(&mut st, "scramble").unwrap();
        frames(&mut st, START_TICKS + 1);
        // Two draws at this one tick (`drawn` doesn't move it): the veil off, then on.
        st.veil.off = true;
        let plain = drawn(&mut st, (120, 40));
        st.veil.off = false;
        let veiled = drawn(&mut st, (120, 40));
        // `drawn` drops its screen: the guard's cell is worked out from yours.
        let (yx, yy) = st.drawn_you.get().unwrap();
        let yat = st.player.as_ref().unwrap().pos;
        let (gx, gy) = (
            (yx as i32 + gat.x - yat.x) as u16,
            (yy as i32 + gat.y - yat.y) as u16,
        );
        // The wash lies under them too (it runs last): a cell is where it stands by its
        // symbol and colour.
        let seen = |b: &Buffer, at: (u16, u16)| (b[at].symbol().to_string(), b[at].fg);
        assert_ne!(
            plain[(gx, gy)].symbol(),
            " ",
            "the guard is drawn there without the veil"
        );
        assert_eq!(
            seen(&veiled, (gx, gy)),
            seen(&plain, (gx, gy)),
            "the guard on its true cell"
        );
        assert_eq!(
            seen(&veiled, (yx, yy)),
            seen(&plain, (yx, yy)),
            "your @ unmoved"
        );
        assert_eq!(veiled[(yx, yy)].symbol(), "@");
        // On the map: the HUD around it has pink of its own.
        let map = crate::screens::explore::layout(plain.area, false).map;
        let pink = |b: &Buffer| {
            (map.y..map.bottom()).any(|y| (map.x..map.right()).any(|x| pinkish(b[(x, y)].fg)))
        };
        assert!(pink(&veiled), "pink copies");
        assert!(!pink(&plain), "none with the veil off");
        // And so on through a sweep of the wave down the map: nobody moves.
        for _ in 0..160 {
            frames(&mut st, 1);
            st.veil.off = true;
            let plain = drawn(&mut st, (120, 40));
            st.veil.off = false;
            let veiled = drawn(&mut st, (120, 40));
            let tick = st.tick;
            assert_eq!(
                seen(&veiled, (gx, gy)),
                seen(&plain, (gx, gy)),
                "tick {tick}"
            );
            assert_eq!(
                seen(&veiled, (yx, yy)),
                seen(&plain, (yx, yy)),
                "tick {tick}"
            );
        }
    }

    #[test]
    fn only_the_map() {
        let hot = Look {
            heat: 1.6,
            split: 3,
            veil: 1.0,
            wave: true,
            plain: false,
        };
        let all_empty = |_: u16, _: u16| Ground::Empty;
        let inside =
            |m: Rect, x: u16, y: u16| x >= m.x && x < m.right() && y >= m.y && y < m.bottom();
        let (_, small) = grid();
        // A map bigger than the buffer runs off its edge: clipped, no panic.
        let big = Rect::new(50, 10, 40, 12);
        for m in [small, big] {
            for tick in [0, 17, 41] {
                let (mut buf, _) = grid();
                put_wall(&mut buf, (52, 12));
                let before = buf.clone();
                draw(&mut buf, m, &hot, tick, 3, &all_empty, None, &[]);
                for y in 0..buf.area.height {
                    for x in 0..buf.area.width {
                        if !inside(m, x, y) {
                            assert_eq!(
                                buf[(x, y)],
                                before[(x, y)],
                                "outside the map at ({x},{y}), tick {tick}"
                            );
                        }
                    }
                }
            }
        }
    }
}
