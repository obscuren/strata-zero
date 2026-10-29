//! The grenade and ability shapes drawn over the map: a thrown thing's flight, the
//! four blasts and the scorch two of them leave, a shove's trail and a boost's
//! rings. What sets a blast off and whom it hurts is `crate::blast`'s.

use super::{BLOOM_TICKS, HAZE_RISE, HAZE_TICKS, WHITE, WHITEOUT_TICKS, paint};
use crate::app::FxInstance;
use crate::render::map::MapRef;
use crate::render::viewport::tile_to_screen;
use crate::render::{Rgb, Visibility};
use citygen::pathing::bresenham;
use citygen::rng::hash64;
use citygen::{Grid, Pos};
use game::Fx;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

const GREN: Rgb = Rgb(190, 190, 140);
const AMBER: Rgb = Rgb(255, 170, 40);
const CYAN: Rgb = Rgb(0, 229, 255);
/// A gas cloud's sickly green, and the dark it thins to.
const HAZE: Rgb = Rgb(150, 220, 60);
const HAZE_THIN: Rgb = Rgb(40, 70, 20);

/// Sets a cell's glyph, colours and weight; `None` keeps what is there.
#[allow(clippy::too_many_arguments)]
fn stamp(
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
    p: Pos,
    ch: Option<char>,
    fg: Rgb,
    bg: Option<Rgb>,
    bold: bool,
) {
    paint(origin, area, buf, p, |c| {
        if let Some(ch) = ch {
            c.set_char(ch);
        }
        c.set_fg(fg.to_color());
        if let Some(bg) = bg {
            c.set_bg(bg.to_color());
        }
        if bold {
            c.modifier.insert(Modifier::BOLD);
        } else {
            c.modifier.remove(Modifier::BOLD);
        }
    });
}

/// The footprint's ring of a tile: its chebyshev distance from the landing.
fn ring_of(inst: &FxInstance, p: Pos) -> i32 {
    inst.origin.chebyshev(p)
}

/// The blast shapes' scatter: a stable hash of a tile and a half-tick count.
pub(super) fn blast_hash(p: Pos, k: u64) -> u64 {
    hash64(hash64(p.x as u64, p.y as u64), k)
}

/// Whether a thing can lie on `p`: open ground of a stratum.
fn open_ground(map: &MapRef, p: Pos) -> bool {
    map.stratum().is_some_and(|k| {
        map.city()
            .tile(k, p)
            .is_some_and(|t| t.is_walkable() && !crate::foes::blocks_sight(t))
    })
}

/// Whether `from` sees `p` across open ground.
fn sees(map: &MapRef, from: Pos, p: Pos) -> bool {
    map.stratum().is_some_and(|k| {
        map.city()
            .tile(k, p)
            .is_some_and(|t| !crate::foes::blocks_sight(t))
    }) && crate::foes::line_clear(map.city(), map.place(), from, p)
}

/// A thrown thing crossing the line two cells a tick, three cells of trail behind
/// it, then the landing, the bounce and the settle.
pub(super) fn throw(
    inst: &FxInstance,
    elapsed: u64,
    map: &MapRef,
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
) {
    let Some(to) = inst.targets.first().copied() else {
        return;
    };
    let line = bresenham(inst.origin, to);
    let dist = line.len().saturating_sub(1);
    // Cells in flight: the ones strictly between the hand and the landing, two apart.
    let flying = dist.saturating_sub(1).div_ceil(2) as u64;
    let shadow = inst.arc == Some(game::Arc::Shadow);
    for i in elapsed.saturating_sub(3)..=elapsed {
        if i >= flying {
            continue;
        }
        let idx = 1 + 2 * i as usize;
        let s = idx as f32 / dist as f32;
        let h = 4.0 * s * (1.0 - s);
        let on = line[idx];
        // A lob rises up the screen whatever the direction; a shadow stays on the line.
        let lifted = on.offset(0, -((2.2 * h).round() as i32));
        if i != elapsed {
            let fade = Rgb(120, 120, 120).mix(Rgb(50, 50, 50), (elapsed - i) as f32 / 3.0);
            let at = if shadow { on } else { lifted };
            stamp(origin, area, buf, at, Some('·'), fade, None, false);
        } else if shadow {
            let glyph = if h < 0.3 {
                '·'
            } else if h < 0.7 {
                'o'
            } else {
                'O'
            };
            stamp(
                origin,
                area,
                buf,
                on,
                Some(glyph),
                GREN.mix(WHITE, h),
                None,
                h > 0.6,
            );
        } else {
            stamp(origin, area, buf, lifted, Some('o'), GREN, None, true);
        }
    }
    match elapsed.checked_sub(flying) {
        Some(0) => stamp(
            origin,
            area,
            buf,
            to,
            Some('●'),
            Rgb(255, 240, 180),
            None,
            true,
        ),
        Some(1) => {
            // The bounce: one cell past the landing and a row up, on it when that is blocked.
            let before = line.len().checked_sub(2).map_or(inst.origin, |k| line[k]);
            let step = if before == to {
                (1, 0)
            } else {
                (to.x - before.x, to.y - before.y)
            };
            let past = to.offset(step.0, step.1 - 1);
            let at = if open_ground(map, past) { past } else { to };
            stamp(origin, area, buf, at, Some('o'), GREN, None, true);
        }
        Some(2) => stamp(origin, area, buf, to, Some('o'), GREN, None, true),
        _ => {}
    }
}

/// The scorch's stain on one tile as the ground lies in sight: whole in sight,
/// dimmed as the map's memory is, nothing where you never looked.
#[allow(clippy::too_many_arguments)]
fn stain(
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
    vis: Option<&Grid<Visibility>>,
    p: Pos,
    ch: char,
    fg: Rgb,
    bg: Rgb,
) {
    let seen = vis.map_or(Visibility::Visible, |v| {
        v.get(p).copied().unwrap_or(Visibility::Unseen)
    });
    match seen {
        Visibility::Visible => stamp(origin, area, buf, p, Some(ch), fg, Some(bg), false),
        Visibility::Remembered => {
            if let Some((sx, sy)) = tile_to_screen(area, origin, p) {
                let c = &mut buf[(sx, sy)];
                c.set_char(ch)
                    .set_fg(fg.scale(0.35).to_color())
                    .set_bg(bg.scale(0.35).to_color());
                c.modifier.remove(Modifier::BOLD);
            }
        }
        Visibility::Unseen => {}
    }
}

/// Where a blast hurt, the stain it leaves: from a bloom's e3 the centre and its
/// first ring, from a whiteout's e2 the whole footprint by ring. Drawn on the
/// ground, under everything that stands on it.
pub(super) fn scorch(
    inst: &FxInstance,
    e: u64,
    vis: Option<&Grid<Visibility>>,
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
) {
    if !inst.hit {
        return;
    }
    match inst.fx {
        Fx::Bloom if e >= 3 => {
            let (fg, bg) = (Rgb(45, 40, 36), Rgb(18, 16, 15));
            stain(origin, area, buf, vis, inst.origin, '▒', fg, bg);
            let (fg, bg) = (Rgb(38, 34, 30), Rgb(20, 17, 16));
            for p in inst.targets.iter().filter(|p| ring_of(inst, **p) == 1) {
                stain(origin, area, buf, vis, *p, '░', fg, bg);
            }
        }
        Fx::Whiteout if e >= 2 => {
            let (fg, bg) = (Rgb(30, 27, 25), Rgb(16, 14, 13));
            for &p in &inst.targets {
                let glyph = match ring_of(inst, p) {
                    0 => '▓',
                    1 => '▒',
                    _ => '░',
                };
                stain(origin, area, buf, vis, p, glyph, fg, bg);
            }
        }
        _ => {}
    }
}

/// A fireball: a white flash, rings of flame, smoke rising off the footprint; the
/// stain it leaves is `scorch`'s.
pub(super) fn bloom(inst: &FxInstance, e: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    if e >= BLOOM_TICKS {
        return;
    }
    let flame = Rgb(255, 200, 90);
    let hot = Rgb(255, 230, 120);
    let ash = Rgb(90, 90, 90);
    let ember = Rgb(120, 40, 10);
    for &p in &inst.targets {
        let (ch, fg, bg, bold) = match (e, ring_of(inst, p)) {
            (0, 0) => ('✺', Rgb(0, 0, 0), Some(Rgb(255, 240, 180)), true),
            (1, 0) => ('✺', WHITE, Some(Rgb(255, 120, 40)), true),
            (1, 1) => ('*', hot, Some(Rgb(200, 60, 20)), true),
            (2, 0) => ('▒', flame, Some(ember), false),
            (2, 1) => ('░', AMBER, Some(ember), false),
            (2, _) => ('*', flame, Some(Rgb(90, 30, 10)), true),
            (3, 0) => ('▓', ash, Some(Rgb(50, 40, 36)), false),
            (3, 1) => ('▒', Rgb(120, 120, 120), Some(Rgb(60, 40, 30)), false),
            (3, _) => ('·', AMBER, None, false),
            (4.., _) => {
                let h = blast_hash(p, e / 2);
                if h.is_multiple_of(3) {
                    let fade = Rgb(110, 110, 110).mix(Rgb(45, 45, 45), (e - 4) as f32 / 9.0);
                    let risen = p.offset(0, -(((e - 4) / 4) as i32));
                    stamp(origin, area, buf, risen, Some('░'), fade, None, false);
                } else if h.is_multiple_of(7) && e < 8 {
                    stamp(
                        origin,
                        area,
                        buf,
                        p,
                        Some('·'),
                        Rgb(200, 110, 20),
                        None,
                        false,
                    );
                }
                continue;
            }
            _ => continue,
        };
        stamp(origin, area, buf, p, Some(ch), fg, bg, bold);
    }
}

/// A flash: the footprint inverted white, then hot orange, then debris flying off
/// in the eight directions; the stain it leaves is `scorch`'s.
pub(super) fn whiteout(
    inst: &FxInstance,
    e: u64,
    map: &MapRef,
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
) {
    match e {
        0 => {
            for &p in &inst.targets {
                stamp(origin, area, buf, p, None, Rgb(0, 0, 0), Some(WHITE), true);
            }
        }
        1 => {
            for &p in &inst.targets {
                let bg = if ring_of(inst, p) < 2 {
                    Rgb(255, 160, 60)
                } else {
                    Rgb(255, 110, 50)
                };
                stamp(origin, area, buf, p, None, Rgb(60, 0, 0), Some(bg), true);
            }
        }
        _ => {
            if e < WHITEOUT_TICKS {
                const DEBRIS: [(i32, i32, char); 8] = [
                    (1, 0, '·'),
                    (-1, 0, '·'),
                    (0, 1, ','),
                    (0, -1, '\''),
                    (1, -1, '`'),
                    (-1, -1, '\''),
                    (1, 1, ','),
                    (-1, 1, ','),
                ];
                let d = 3 + ((e - 2) / 2) as i32;
                let fade = Rgb(230, 230, 230).mix(Rgb(90, 90, 90), (e - 2) as f32 / 5.0);
                for (dx, dy, ch) in DEBRIS {
                    // Halved on the vertical axis of a diagonal: a row is twice a column.
                    let dv = if dx != 0 && dy != 0 { (d + 1) / 2 } else { d };
                    let p = inst.origin.offset(dx * d, dy * dv);
                    if sees(map, inst.origin, p) {
                        stamp(origin, area, buf, p, Some(ch), fade, None, false);
                    }
                }
            }
        }
    }
}

/// An EMP: a cyan core, a ring of static spreading out, then settling to dots.
pub(super) fn shock(inst: &FxInstance, e: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let dim = Rgb(0, 120, 140);
    for &p in &inst.targets {
        let (ch, fg, bg, bold) = match (e, ring_of(inst, p)) {
            (0, 0) => ('◉', CYAN, Some(Rgb(0, 60, 75)), true),
            (1, 0) => ('○', CYAN, None, true),
            (1, 1) => ('≋', CYAN, Some(Rgb(0, 40, 50)), true),
            (2, 1) => ('·', dim, None, false),
            (2, 2..) => ('≋', Rgb(0, 180, 200), Some(Rgb(0, 30, 38)), false),
            (3, 2..) => ('·', dim, None, false),
            _ => continue,
        };
        stamp(origin, area, buf, p, Some(ch), fg, bg, bold);
    }
}

/// How thick a haze hangs at `e`, up to 1: rising over `HAZE_RISE`, then thinning
/// to nothing at `HAZE_TICKS`.
fn haze_thickness(e: u64) -> f32 {
    if e < HAZE_RISE {
        (e + 1) as f32 / HAZE_RISE as f32
    } else {
        HAZE_TICKS.saturating_sub(e) as f32 / (HAZE_TICKS - HAZE_RISE) as f32
    }
}

/// A gas: the footprint fills with green shade, thickest at the centre and drifting
/// every other tick, then thins away. It leaves no stain.
pub(super) fn haze(inst: &FxInstance, e: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let thick = haze_thickness(e);
    let radius = inst.radius.max(1) as f32;
    for &p in &inst.targets {
        let local = thick * (1.0 - 0.4 * ring_of(inst, p) as f32 / radius);
        let h = blast_hash(p, e / 2);
        if (h % 100) as f32 >= local * 100.0 {
            continue;
        }
        let ch = if local > 0.6 && (h / 100).is_multiple_of(2) {
            '▒'
        } else {
            '░'
        };
        let fg = HAZE.mix(HAZE_THIN, 1.0 - local);
        stamp(origin, area, buf, p, Some(ch), fg, None, false);
    }
}

/// The arrow a shove leaves on the tile the foe left: along the greater axis of the
/// push, horizontal on a tie.
pub(super) fn trail(inst: &FxInstance, e: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let Some(to) = inst.targets.first().copied() else {
        return;
    };
    let (dx, dy) = (to.x - inst.origin.x, to.y - inst.origin.y);
    let arrow = match (dx.abs() >= dy.abs(), dx > 0, dy > 0) {
        _ if (dx, dy) == (0, 0) => return,
        (true, true, _) => '»',
        (true, false, _) => '«',
        (false, _, true) => '∨',
        (false, _, false) => '∧',
    };
    let tint = if e < 2 {
        Rgb(255, 120, 120)
    } else {
        Rgb(180, 60, 60)
    };
    stamp(
        origin,
        area,
        buf,
        inst.origin,
        Some(arrow),
        tint,
        None,
        false,
    );
}

/// Boost: the firer's cell lit, then rings of dots spreading out and thinning.
pub(super) fn pulse_rings(
    inst: &FxInstance,
    colour: Rgb,
    e: u64,
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
) {
    if e == 0 {
        stamp(
            origin,
            area,
            buf,
            inst.origin,
            None,
            Rgb(0, 0, 0),
            Some(colour),
            true,
        );
    }
    let rings: [(i32, u64, u64, f32); 3] =
        [(1, 1, 2, 1.0), (2, 2, 4, 2.0 / 3.0), (3, 4, 6, 1.0 / 3.0)];
    for (r, from, to, level) in rings {
        if !(from..=to).contains(&e) {
            continue;
        }
        let tint = colour.scale(level);
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) == r {
                    let p = inst.origin.offset(dx, dy);
                    stamp(origin, area, buf, p, Some('·'), tint, None, false);
                }
            }
        }
    }
}
