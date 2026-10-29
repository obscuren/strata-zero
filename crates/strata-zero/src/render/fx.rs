//! Ability effects over the map: glitch kicks, expanding wavefronts and glowing markers.

mod shapes;

use crate::anim::effects::{glitch_cells, pulse};
use crate::app::FxInstance;
use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::render::{Rgb, Visibility};
use citygen::pathing::bresenham;
use citygen::rng::hash64;
use citygen::{Grid, Pos};
use game::Fx;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use std::collections::HashSet;

/// Glyphs that ride a wavefront, and that a tear scatters.
const SPARKS: &[char] = &['ｦ', 'ｱ', 'ﾂ', 'ﾈ', '╳', '≋'];
/// Ticks a muzzle flash lights the muzzle.
pub const FLASH_TICKS: u64 = 2;
/// Ticks a streak stays: white, the colour, ember.
pub const STREAK_TICKS: u64 = 3;
/// Ticks chips, a spark or a tear last at the impact.
pub const IMPACT_TICKS: u64 = 3;
/// Ticks a damage number stays up.
pub const NUMBER_TICKS: u64 = 14;
/// Ticks a number takes to rise one row.
pub const NUMBER_RISE: u64 = 4;
/// Ticks a spent casing lies on the ground before it is gone.
pub const CASING_TICKS: u64 = 60;
/// Ticks a bloom plays before its scorch.
pub const BLOOM_TICKS: u64 = 13;
/// Ticks a whiteout plays before its scorch.
pub const WHITEOUT_TICKS: u64 = 8;
/// Ticks a shock plays.
pub const SHOCK_TICKS: u64 = 4;
/// Ticks a push's trail stays.
pub const TRAIL_TICKS: u64 = 4;
/// Ticks a pulse rings out.
pub const PULSE_TICKS: u64 = 7;

/// Ticks a tracer takes from the muzzle to its impact: `speed` tiles a tick, at
/// least one tick.
pub fn tracer_ticks(origin: Pos, target: Pos, speed: u8) -> u64 {
    let dist = origin.chebyshev(target).max(1) as u64;
    dist.div_ceil(speed.max(1) as u64).max(1)
}
/// Cells of ripple behind the front, front included.
pub const BAND: f32 = 9.0;
/// Cells from one ripple crest to the next.
const WAVELENGTH: f32 = 3.0;
/// The deeper blue alternate crests lean toward.
const DEEP: Rgb = Rgb(60, 110, 255);
const KICK_CYAN: Color = Color::Rgb(0, 255, 230);
const KICK_MAGENTA: Color = Color::Rgb(255, 0, 200);
/// The chrome's cyan, for a tear's glyphs.
const TEAR_CYAN: Color = Color::Rgb(0, 229, 255);
const BLACK: Color = Color::Rgb(0, 0, 0);

/// Distance from `origin` in cell widths, a row counting as two: round on screen.
pub fn screen_dist(p: Pos, origin: Pos) -> f32 {
    let dx = (p.x - origin.x) as f32;
    let dy = (p.y - origin.y) as f32 * 2.0;
    (dx * dx + dy * dy).sqrt()
}

/// How far a wave travels in cell widths: to the corner of the ability's square radius.
pub fn reach(radius: i32) -> f32 {
    radius.max(0) as f32 * 5f32.sqrt()
}

/// Ticks a wave keeps running past its reach so the ripple band clears.
pub fn wave_tail_ticks(ticks: u64, radius: i32) -> u64 {
    let reach = reach(radius);
    if reach <= 0.0 {
        return 0;
    }
    (BAND * ticks as f32 / reach).ceil() as u64
}

pub struct FxOverlay<'a> {
    pub fx: &'a [FxInstance],
    /// Sight on the drawn stratum: glow markers show on connectors out of sight.
    pub vis: Option<&'a Grid<Visibility>>,
}

/// The stains blasts leave where they hurt, on the ground: drawn before the people,
/// so whoever stands on a scorch, and the aim over it, are drawn over it. The live
/// shapes of the same blasts are `FxOverlay`'s.
pub struct ScorchOverlay<'a> {
    pub fx: &'a [FxInstance],
    /// Sight on the drawn stratum: no stain where you never looked.
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for ScorchOverlay<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for inst in self.fx.iter().filter(|i| {
            i.place == map.place()
                && matches!(i.fx, Fx::Bloom | Fx::Whiteout)
                && i.started <= tick
                && tick < i.until()
        }) {
            shapes::scorch(inst, tick - inst.started, self.vis, origin, area, buf);
        }
    }
}

impl Overlay for FxOverlay<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        // Flashes draw last, over the first cells of the round they light.
        let is_flash = |i: &&FxInstance| matches!(i.fx, Fx::Flash { .. });
        let ordered = self
            .fx
            .iter()
            .filter(|i| !is_flash(i))
            .chain(self.fx.iter().filter(is_flash));
        for inst in
            ordered.filter(|i| i.place == map.place() && i.started <= tick && tick < i.until())
        {
            let elapsed = tick - inst.started;
            match inst.fx {
                Fx::Kick(ticks) => {
                    let p = elapsed as f32 / ticks.max(1) as f32;
                    for (x, y, ch) in glitch_cells(area, tick, p) {
                        let tint = if hash64(x as u64, y as u64).is_multiple_of(2) {
                            KICK_CYAN
                        } else {
                            KICK_MAGENTA
                        };
                        buf[(x, y)].set_char(ch).set_fg(tint);
                    }
                }
                Fx::Wave { colour, ticks } => {
                    wave(
                        inst,
                        Rgb(colour.0, colour.1, colour.2),
                        ticks,
                        elapsed,
                        origin,
                        area,
                        tick,
                        buf,
                    );
                }
                Fx::Glow { colour, .. } => {
                    self.glow(
                        inst,
                        Rgb(colour.0, colour.1, colour.2),
                        map,
                        origin,
                        area,
                        tick,
                        buf,
                    );
                }
                Fx::Bolt { colour, ticks } => bolt(
                    inst,
                    Rgb(colour.0, colour.1, colour.2),
                    ticks,
                    elapsed,
                    origin,
                    area,
                    tick,
                    buf,
                ),
                Fx::Tracer { colour, speed } => tracer(
                    inst,
                    Rgb(colour.0, colour.1, colour.2),
                    speed,
                    elapsed,
                    origin,
                    area,
                    buf,
                ),
                Fx::Streak { colour } => streak(
                    inst,
                    Rgb(colour.0, colour.1, colour.2),
                    elapsed,
                    origin,
                    area,
                    buf,
                ),
                Fx::Flash { big } => flash(inst, big, elapsed, origin, area, buf),
                Fx::Chips => chips(inst, elapsed, origin, area, buf),
                Fx::Spark => spark(inst, elapsed, origin, area, buf),
                Fx::Tear => tear(inst, tick, origin, area, buf),
                Fx::Number { tag } => number(inst, tag, elapsed, origin, area, buf),
                Fx::Casing => casing(inst, elapsed, origin, area, buf),
                Fx::Throw => shapes::throw(inst, elapsed, map, origin, area, buf),
                Fx::Bloom => shapes::bloom(inst, elapsed, origin, area, buf),
                Fx::Whiteout => shapes::whiteout(inst, elapsed, map, origin, area, buf),
                Fx::Shock => shapes::shock(inst, elapsed, origin, area, buf),
                // Drawn by a later task.
                Fx::Haze => {}
                Fx::Trail => shapes::trail(inst, elapsed, origin, area, buf),
                Fx::Pulse { colour } => shapes::pulse_rings(
                    inst,
                    Rgb(colour.0, colour.1, colour.2),
                    elapsed,
                    origin,
                    area,
                    buf,
                ),
                Fx::Mark { colour, .. } => mark(
                    inst,
                    Rgb(colour.0, colour.1, colour.2),
                    origin,
                    area,
                    tick,
                    buf,
                ),
            }
        }
    }
}

/// The ring: a bright front, a ripple of fading crests behind it that shimmer the
/// glyphs outward, and the scan's revealed tiles kept blank until the front reaches them.
#[allow(clippy::too_many_arguments)]
fn wave(
    inst: &FxInstance,
    colour: Rgb,
    ticks: u64,
    elapsed: u64,
    origin: Pos,
    area: Rect,
    tick: u64,
    buf: &mut Buffer,
) {
    let reach = reach(inst.radius);
    let r = reach * elapsed as f32 / ticks.max(1) as f32;
    let hidden: HashSet<Pos> = inst
        .revealed
        .iter()
        .copied()
        .filter(|p| screen_dist(*p, inst.origin) > r)
        .collect();
    let alt = colour.mix(DEEP, 0.45);
    // The lens samples glyphs before this wave touched them.
    let before = buf.clone();
    for sy in area.y..area.bottom() {
        for sx in area.x..area.right() {
            let p = Pos::new(
                origin.x + (sx - area.x) as i32,
                origin.y + (sy - area.y) as i32,
            );
            let d = screen_dist(p, inst.origin);
            if d > reach {
                continue;
            }
            let cell = &mut buf[(sx, sy)];
            if hidden.contains(&p) {
                cell.set_char(' ').set_fg(BLACK).set_bg(BLACK);
                cell.modifier.remove(Modifier::DIM);
                continue;
            }
            if d > r {
                continue;
            }
            let u = r - d;
            if u >= BAND {
                continue;
            }
            cell.modifier.remove(Modifier::DIM);
            if u < 1.0 {
                cell.set_fg(colour.to_color())
                    .set_bg(colour.scale(0.35).to_color());
                if hash64(hash64(sx as u64, sy as u64), tick).is_multiple_of(10) {
                    let i = (hash64(tick, sx as u64 + sy as u64) % SPARKS.len() as u64) as usize;
                    cell.set_char(SPARKS[i]);
                }
                continue;
            }
            let amp = 1.0 - u / BAND;
            let phase = u / WAVELENGTH;
            let ripple = 0.5 + 0.5 * (phase * std::f32::consts::TAU).cos();
            let b = amp * ripple;
            let tint = if (phase.round() as i64) % 2 == 0 {
                colour
            } else {
                alt
            };
            if b > 0.55 {
                // Near a crest the cell shows the glyph one step further out: a lens.
                let nx = sx as i32 + (p.x - inst.origin.x).signum();
                let ny = sy as i32 + (p.y - inst.origin.y).signum();
                if nx >= area.x as i32
                    && nx < area.right() as i32
                    && ny >= area.y as i32
                    && ny < area.bottom() as i32
                {
                    let symbol = before[(nx as u16, ny as u16)].symbol().to_string();
                    cell.set_symbol(&symbol);
                }
            }
            cell.set_fg(tint.scale(0.25 + 0.75 * b).to_color())
                .set_bg(tint.scale(0.3 * b).to_color());
        }
    }
}

/// A path from `a` to `b` that only ever steps horizontally or vertically and never
/// away from `b`: runs of one to four cells along one axis, then a turn to the other,
/// until it lands. `seed` picks the run lengths, so one seed gives one route.
fn snake_points(a: Pos, b: Pos, seed: u64) -> Vec<Pos> {
    let (sx, sy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
    let (mut x, mut y) = (a.x, a.y);
    let mut out = vec![a];
    let mut horizontal = hash64(seed, 0).is_multiple_of(2);
    let mut k = 1u64;
    while x != b.x || y != b.y {
        let (rx, ry) = ((b.x - x).abs(), (b.y - y).abs());
        if horizontal && rx == 0 {
            horizontal = false;
        } else if !horizontal && ry == 0 {
            horizontal = true;
        }
        let remaining = if horizontal { rx } else { ry };
        let run = (1 + (hash64(seed, k) % 4) as i32).min(remaining);
        k += 1;
        for _ in 0..run {
            if horizontal {
                x += sx;
            } else {
                y += sy;
            }
            out.push(Pos::new(x, y));
        }
        horizontal = !horizontal;
    }
    out
}

/// The glyph for a bolt cell: a run along its axis, or the corner joining the way in
/// to the way out.
fn bolt_glyph(came: (i32, i32), goes: (i32, i32)) -> char {
    if came == goes {
        return if came.0 == 0 { '│' } else { '─' };
    }
    let left = came.0 == 1 || goes.0 == -1;
    let up = came.1 == 1 || goes.1 == -1;
    match (left, up) {
        (true, true) => '┘',
        (true, false) => '┐',
        (false, true) => '└',
        (false, false) => '┌',
    }
}

/// Lines from the origin to every target that only run horizontally or vertically,
/// turning like a circuit trace, grown over `ticks`; they ignore walls and sight. The
/// route is re-rolled every four ticks, so the arc crawls while it burns.
#[allow(clippy::too_many_arguments)]
fn bolt(
    inst: &FxInstance,
    colour: Rgb,
    ticks: u64,
    elapsed: u64,
    origin: Pos,
    area: Rect,
    tick: u64,
    buf: &mut Buffer,
) {
    let frac = (elapsed as f32 / ticks.max(1) as f32).min(1.0);
    let phase = tick / 4;
    for (ti, target) in inst.targets.iter().enumerate() {
        let seed = hash64(hash64(ti as u64, inst.started), phase);
        let pts = snake_points(inst.origin, *target, seed);
        let n = ((pts.len() as f32) * frac).ceil() as usize;
        for i in 1..n.min(pts.len()) {
            let p = pts[i];
            let Some((sx, sy)) = tile_to_screen(area, origin, p) else {
                continue;
            };
            let came = (p.x - pts[i - 1].x, p.y - pts[i - 1].y);
            let goes = pts.get(i + 1).map_or(came, |q| (q.x - p.x, q.y - p.y));
            let spark = came == goes && hash64(i as u64 + 7, ti as u64 ^ phase).is_multiple_of(5);
            let glyph = if spark { '*' } else { bolt_glyph(came, goes) };
            let bright = hash64(hash64(sx as u64, sy as u64), phase).is_multiple_of(3);
            let fg = if bright { colour } else { colour.scale(0.6) };
            let cell = &mut buf[(sx, sy)];
            cell.set_char(glyph)
                .set_fg(fg.to_color())
                .set_bg(colour.scale(0.2).to_color());
            cell.modifier.remove(Modifier::DIM);
        }
    }
}

/// The glyph of a round's path cell by the step into it: a run along an axis, or
/// a diagonal; `heavy` for a streak.
pub fn line_glyph(came: (i32, i32), heavy: bool) -> char {
    match (came.0 != 0, came.1 != 0) {
        (true, false) => {
            if heavy {
                '━'
            } else {
                '─'
            }
        }
        (false, _) => {
            if heavy {
                '┃'
            } else {
                '│'
            }
        }
        (true, true) => {
            if came.0 * came.1 > 0 {
                '╲'
            } else {
                '╱'
            }
        }
    }
}

/// The shot's line from the firer to its impact, both included; empty without a
/// target.
fn shot_line(inst: &FxInstance) -> Vec<Pos> {
    match inst.targets.first() {
        Some(t) => bresenham(inst.origin, *t),
        None => Vec::new(),
    }
}

/// Paints one tile's cell on screen, if it is on screen.
fn paint(
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
    p: Pos,
    f: impl FnOnce(&mut ratatui::buffer::Cell),
) {
    if let Some((sx, sy)) = tile_to_screen(area, origin, p) {
        let cell = &mut buf[(sx, sy)];
        cell.modifier.remove(Modifier::DIM);
        f(cell);
    }
}

const WHITE: Rgb = Rgb(255, 255, 255);
/// The warm light a muzzle throws.
const MUZZLE_LIGHT: Rgb = Rgb(255, 170, 40);
/// Cell widths a muzzle flash lights on its first tick; half that on its second.
const FLASH_GLOW: f32 = 3.5;
/// Cell widths a tracer's head lights as it flies.
const TRACER_GLOW: f32 = 2.2;

/// Warms the background of every cell within `reach` cell widths of `centre` (a
/// row counting as two, so the pool is round), `strength` at the centre falling to
/// nothing at the edge; glyphs are kept.
fn glow(origin: Pos, area: Rect, buf: &mut Buffer, centre: Pos, reach: f32, strength: f32) {
    let r = reach.ceil() as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            let p = centre.offset(dx, dy);
            let d = screen_dist(p, centre);
            if d >= reach {
                continue;
            }
            let t = strength * (1.0 - d / reach);
            paint(origin, area, buf, p, |c| {
                let bg = match c.bg {
                    Color::Rgb(r, g, b) => Rgb(r, g, b),
                    _ => Rgb(0, 0, 0),
                };
                c.set_bg(bg.mix(MUZZLE_LIGHT, t).to_color());
            });
        }
    }
}

/// The muzzle: the firer's cell lit, a spark the next cell on, a dot after it; a
/// big flash spills above and below the spark on its first tick.
fn flash(inst: &FxInstance, big: bool, elapsed: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let line = shot_line(inst);
    if line.len() < 2 || elapsed >= FLASH_TICKS {
        return;
    }
    let first = elapsed == 0;
    let muzzle = if first {
        Rgb(255, 240, 180)
    } else {
        Rgb(255, 170, 40)
    };
    // The light the shot throws on the street around it, gone with the flash.
    if first {
        glow(origin, area, buf, line[0], FLASH_GLOW, 0.4);
    } else {
        glow(origin, area, buf, line[0], FLASH_GLOW / 2.0, 0.3);
    }
    paint(origin, area, buf, line[0], |c| {
        c.set_fg(BLACK)
            .set_bg(muzzle.to_color())
            .modifier
            .insert(Modifier::BOLD);
    });
    let spark = line[1];
    paint(origin, area, buf, spark, |c| {
        c.set_char(if first { '✦' } else { '*' })
            .set_fg(Rgb(255, 230, 120).to_color())
            .modifier
            .insert(Modifier::BOLD);
    });
    if !first {
        return;
    }
    if let Some(dot) = line.get(2) {
        paint(origin, area, buf, *dot, |c| {
            c.set_char('·')
                .set_fg(Rgb(255, 200, 90).to_color())
                .modifier
                .insert(Modifier::BOLD);
        });
    }
    if big {
        let amber = Rgb(255, 200, 90).to_color();
        paint(origin, area, buf, spark.offset(0, -1), |c| {
            c.set_char('`').set_fg(amber);
        });
        paint(origin, area, buf, spark.offset(0, 1), |c| {
            c.set_char(',').set_fg(amber);
        });
    }
}

/// A round in flight: a bright head `speed` cells further every tick, three
/// fading cells of trail behind it, never behind the firer.
#[allow(clippy::too_many_arguments)]
fn tracer(
    inst: &FxInstance,
    colour: Rgb,
    speed: u8,
    elapsed: u64,
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
) {
    let line = shot_line(inst);
    let head = 1 + speed.max(1) as usize * elapsed as usize;
    if head >= line.len() {
        return;
    }
    // The content colour leads the trail; the two cells behind it are the fixed embers.
    // The head carries a little light with it.
    glow(origin, area, buf, line[head], TRACER_GLOW, 0.3);
    let trail = [colour, Rgb(200, 110, 20), Rgb(110, 55, 10)];
    for (i, tint) in trail.iter().enumerate() {
        let Some(k) = head.checked_sub(i + 1) else {
            break;
        };
        if k == 0 {
            break;
        }
        let came = (line[k].x - line[k - 1].x, line[k].y - line[k - 1].y);
        paint(origin, area, buf, line[k], |c| {
            c.set_char(line_glyph(came, false)).set_fg(tint.to_color());
            if i == 0 {
                c.modifier.insert(Modifier::BOLD);
            }
        });
    }
    paint(origin, area, buf, line[head], |c| {
        c.set_char('•')
            .set_fg(Rgb(255, 240, 200).to_color())
            .modifier
            .insert(Modifier::BOLD);
    });
}

/// A round drawn whole: white, then its colour, then ember, from the cell past the
/// firer to the one before the impact.
fn streak(inst: &FxInstance, colour: Rgb, elapsed: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let line = shot_line(inst);
    if elapsed >= STREAK_TICKS || line.len() < 3 {
        return;
    }
    let tint = match elapsed {
        0 => Rgb(240, 240, 240),
        1 => colour,
        _ => Rgb(120, 60, 10),
    };
    for k in 1..line.len() - 1 {
        let came = (line[k].x - line[k - 1].x, line[k].y - line[k - 1].y);
        paint(origin, area, buf, line[k], |c| {
            c.set_char(line_glyph(came, true)).set_fg(tint.to_color());
            if elapsed == 0 {
                c.modifier.insert(Modifier::BOLD);
            }
        });
    }
}

/// Which way the chips and sparks fly: away from the firer along x (right on a tie).
fn away(inst: &FxInstance, target: Pos) -> i32 {
    if inst.origin.x > target.x { -1 } else { 1 }
}

/// A hit: the target lit red for two ticks, chips flying off it for three.
fn chips(inst: &FxInstance, elapsed: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let Some(target) = inst.targets.first().copied() else {
        return;
    };
    if elapsed >= IMPACT_TICKS {
        return;
    }
    if elapsed < 2 {
        let bg = if elapsed == 0 {
            Rgb(255, 40, 40)
        } else {
            Rgb(160, 20, 20)
        };
        paint(origin, area, buf, target, |c| {
            c.set_fg(WHITE.to_color())
                .set_bg(bg.to_color())
                .modifier
                .insert(Modifier::BOLD);
        });
    }
    const CHIPS: [(i32, i32, char); 6] = [
        (1, -1, '\''),
        (1, 0, '·'),
        (1, 1, ','),
        (2, -1, '·'),
        (2, 1, '`'),
        (0, -1, '·'),
    ];
    let dir = away(inst, target);
    let drift = (elapsed / 2) as i32;
    let tint = if elapsed == 0 {
        Rgb(255, 120, 120)
    } else {
        Rgb(180, 60, 60)
    };
    for (i, (dx, dy, glyph)) in CHIPS.iter().enumerate() {
        if i as u64 > 2 * elapsed + 1 {
            break;
        }
        let p = target.offset((dx + drift) * dir, *dy);
        paint(origin, area, buf, p, |c| {
            c.set_char(*glyph).set_fg(tint.to_color());
        });
    }
}

/// A miss: a spark where the round stopped, flecks back toward the firer.
fn spark(inst: &FxInstance, elapsed: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let Some(target) = inst.targets.first().copied() else {
        return;
    };
    if elapsed >= IMPACT_TICKS {
        return;
    }
    let back = -away(inst, target);
    // A tinted shot sparks in its own colour; a gun's in hot amber.
    let (glyph, tint, fleck, ember) = match (elapsed, inst.tint) {
        (0, None) => (
            '✶',
            Rgb(255, 240, 160),
            Rgb(255, 220, 120),
            Rgb(200, 110, 20),
        ),
        (1, None) => (
            '×',
            Rgb(255, 170, 40),
            Rgb(255, 220, 120),
            Rgb(200, 110, 20),
        ),
        (_, None) => ('·', Rgb(120, 60, 10), Rgb(255, 220, 120), Rgb(200, 110, 20)),
        (e, Some(t)) => {
            let t = Rgb(t.0, t.1, t.2);
            let (glyph, main) = match e {
                0 => ('✶', t.mix(WHITE, 0.4)),
                1 => ('×', t),
                _ => ('·', t.scale(0.45)),
            };
            (glyph, main, t.scale(0.85), t.scale(0.6))
        }
    };
    paint(origin, area, buf, target, |c| {
        c.set_char(glyph).set_fg(tint.to_color());
        if elapsed == 0 {
            c.modifier.insert(Modifier::BOLD);
        }
    });
    match elapsed {
        0 => {
            let fleck = fleck.to_color();
            paint(origin, area, buf, target.offset(back, -1), |c| {
                c.set_char('`').set_fg(fleck);
            });
            paint(origin, area, buf, target.offset(back, 1), |c| {
                c.set_char(',').set_fg(fleck);
            });
        }
        1 => paint(origin, area, buf, target.offset(2 * back, 1), |c| {
            c.set_char('·').set_fg(ember.to_color());
        }),
        _ => {}
    }
}

/// A hit's tear: the cells around the target scrambled into the net's glitch set.
fn tear(inst: &FxInstance, tick: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let Some(target) = inst.targets.first().copied() else {
        return;
    };
    for dy in -1..=1 {
        for dx in -3..=3 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let h = hash64(
                hash64((dx + 7) as u64, (dy + 3) as u64),
                hash64(tick, inst.started),
            );
            if !h.is_multiple_of(3) {
                continue;
            }
            let glyph = SPARKS[(h % SPARKS.len() as u64) as usize];
            // The net's cyan and magenta; a tinted shot tears in its colour and the
            // deep blue behind it.
            let tint = match (inst.tint, h.is_multiple_of(2)) {
                (None, true) => KICK_MAGENTA,
                (None, false) => TEAR_CYAN,
                (Some(t), true) => Rgb(t.0, t.1, t.2).mix(DEEP, 0.6).to_color(),
                (Some(t), false) => Rgb(t.0, t.1, t.2).to_color(),
            };
            paint(origin, area, buf, target.offset(dx, dy), |c| {
                c.set_char(glyph).set_fg(tint);
            });
        }
    }
}

/// The damage lifting off the target: a row higher every `NUMBER_RISE` ticks,
/// fading after ten; a tag sits on a red field.
fn number(inst: &FxInstance, tag: bool, elapsed: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let Some(target) = inst.targets.first().copied() else {
        return;
    };
    if elapsed >= NUMBER_TICKS {
        return;
    }
    let y = target.y - 1 - (elapsed / NUMBER_RISE) as i32;
    let fade = elapsed >= 10;
    let (fg, bg, text, x0) = if tag {
        // A red field for a gun's tag; any tinted one (the net's, an EMP's,
        // Redline's) on the cyan field, whatever its tint.
        let (fg, bg) = match (inst.tint, fade) {
            (None, false) => (WHITE, Rgb(170, 20, 20)),
            (None, true) => (Rgb(200, 120, 120), Rgb(80, 10, 10)),
            (Some(_), false) => (WHITE, CYAN_FIELD),
            (Some(_), true) => (Rgb(120, 170, 180), Rgb(0, 40, 50)),
        };
        (fg, Some(bg), format!(" {} ", inst.label), target.x - 1)
    } else {
        let fg = match (inst.tint, fade) {
            (Some(t), false) => Rgb(t.0, t.1, t.2),
            (Some(t), true) => Rgb(t.0, t.1, t.2).scale(0.6),
            (None, false) => Rgb(255, 235, 235),
            (None, true) => Rgb(170, 60, 60),
        };
        (fg, None, inst.label.clone(), target.x)
    };
    for (i, ch) in text.chars().enumerate() {
        paint(origin, area, buf, Pos::new(x0 + i as i32, y), |c| {
            c.set_char(ch).set_fg(fg.to_color());
            if let Some(bg) = bg {
                c.set_bg(bg.to_color());
            }
            c.modifier.insert(Modifier::BOLD);
        });
    }
}

/// A spent casing: out of the muzzle to the shooter's side on the first tick, a
/// hop of a cell or two, then lying there as brass that dims to the ground's tone.
fn casing(inst: &FxInstance, elapsed: u64, origin: Pos, area: Rect, buf: &mut Buffer) {
    let Some(target) = inst.targets.first().copied() else {
        return;
    };
    if elapsed >= CASING_TICKS {
        return;
    }
    let from = inst.origin;
    let dir = ((target.x - from.x).signum(), (target.y - from.y).signum());
    let (dir, side) = if dir == (0, 0) {
        ((1, 0), (0, -1))
    } else {
        (dir, (dir.1, -dir.0))
    };
    let h = hash64(inst.started, hash64(from.x as u64, from.y as u64));
    let hop = 1 + (h % 2) as i32;
    let back = ((h >> 1) % 2) as i32;
    let out = from.offset(side.0, side.1);
    let landing = from.offset(side.0 * hop - dir.0 * back, side.1 * hop - dir.1 * back);
    let brass = Rgb(240, 200, 90);
    match elapsed {
        0 => paint(origin, area, buf, out, |c| {
            c.set_char('\'').set_fg(brass.to_color());
        }),
        1 => paint(origin, area, buf, landing, |c| {
            c.set_char('\'').set_fg(brass.to_color());
        }),
        _ => {
            // Lying there: brass at first, the ground's tone by the end.
            let t = (elapsed - 2) as f32 / (CASING_TICKS - 2) as f32;
            let tint = Rgb(200, 160, 60).mix(Rgb(70, 60, 40), t);
            paint(origin, area, buf, landing, |c| {
                c.set_char(',').set_fg(tint.to_color());
            });
        }
    }
}

/// The field every tinted tag sits on: the net's, an EMP's, Redline's `+ turn`.
const CYAN_FIELD: Rgb = Rgb(0, 90, 110);
/// A pulsing mark on every target tile, whatever its visibility.
fn mark(inst: &FxInstance, colour: Rgb, origin: Pos, area: Rect, tick: u64, buf: &mut Buffer) {
    let glow = colour.scale(0.55 + 0.45 * pulse(tick, 20)).to_color();
    for t in &inst.targets {
        let Some((sx, sy)) = tile_to_screen(area, origin, *t) else {
            continue;
        };
        let cell = &mut buf[(sx, sy)];
        cell.set_char('◉')
            .set_fg(glow)
            .set_bg(colour.scale(0.2).to_color());
        cell.modifier.remove(Modifier::DIM);
    }
}

impl FxOverlay<'_> {
    /// Pulsing markers on the connectors in range; out of sight they get their own glyph.
    #[allow(clippy::too_many_arguments)]
    fn glow(
        &self,
        inst: &FxInstance,
        colour: Rgb,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
    ) {
        let glow = colour.scale(0.5 + 0.5 * pulse(tick, 20)).to_color();
        // With targets (a construct's nodes) the glow sits on them; otherwise on the
        // stratum's connectors. A floor's lifts get no marker yet.
        let tiles: Vec<Pos> = if !inst.targets.is_empty() {
            inst.targets.clone()
        } else {
            let Some(stratum) = map.stratum() else {
                return;
            };
            map.city()
                .connectors
                .iter()
                .filter(|k| {
                    k.serves.contains(&stratum) && k.pos.chebyshev(inst.origin) <= inst.radius
                })
                .map(|k| k.pos)
                .collect()
        };
        for p in tiles {
            let Some((sx, sy)) = tile_to_screen(area, origin, p) else {
                continue;
            };
            let in_sight = match self.vis {
                None => true,
                Some(v) => v.get(p).copied() == Some(Visibility::Visible),
            };
            let cell = &mut buf[(sx, sy)];
            if !in_sight {
                cell.set_char('◆');
                cell.modifier.remove(Modifier::DIM);
            }
            cell.set_fg(glow);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::shapes::blast_hash;
    use super::*;
    use crate::render::style::StylePack;
    use crate::render::viewport::{MapView, Overlay};
    use crate::walker::Player;
    use citygen::{City, GenParams, Generator, Place, Pos, SizePreset, StratumKind};
    use game::{Arc, Fx};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::{Color, Modifier};
    use ratatui::widgets::Widget;

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap()
    }

    fn render(
        c: &City,
        s: &StylePack,
        stratum: StratumKind,
        tick: u64,
        vis: Option<&Grid<Visibility>>,
        overlays: &[&dyn Overlay],
        size: (u16, u16),
    ) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(size.0, size.1)).unwrap();
        t.draw(|f| {
            MapView {
                map: &MapRef::stratum_of(c, stratum),
                origin: Pos::new(0, 0),
                tick,
                style: s,
                vis,
                overlays,
                cursor: None,
            }
            .render(f.area(), f.buffer_mut())
        })
        .unwrap();
        t.backend().buffer().clone()
    }

    fn instance(fx: Fx, origin: Pos, radius: i32, revealed: Vec<Pos>) -> FxInstance {
        FxInstance {
            fx,
            place: Place::Stratum(StratumKind::Street),
            origin,
            radius,
            started: 100,
            revealed,
            targets: Vec::new(),
            label: String::new(),
            hit: false,
            tint: None,
            arc: None,
        }
    }

    fn aimed(fx: Fx, origin: Pos, targets: Vec<Pos>) -> FxInstance {
        let mut i = instance(fx, origin, 0, Vec::new());
        i.targets = targets;
        i
    }

    const BOLT: (u8, u8, u8) = (255, 240, 150);

    /// A shot's shape from `origin` to `target`, started at tick 100.
    fn shot(fx: Fx, origin: Pos, target: Pos, label: &str) -> FxInstance {
        let mut i = aimed(fx, origin, vec![target]);
        i.label = label.to_string();
        i.hit = true;
        i
    }

    /// The street with one instance drawn at `tick`, and the plain street beside it.
    fn drawn(c: &City, s: &StylePack, inst: &FxInstance, tick: u64) -> (Buffer, Buffer) {
        let overlay = FxOverlay {
            fx: std::slice::from_ref(inst),
            vis: None,
        };
        let plain = render(c, s, StratumKind::Street, tick, None, &[], (120, 60));
        let with = render(
            c,
            s,
            StratumKind::Street,
            tick,
            None,
            &[&overlay],
            (120, 60),
        );
        (with, plain)
    }

    fn at(buf: &Buffer, x: i32, y: i32) -> (String, Color, Color, bool) {
        let c = &buf[(x as u16, y as u16)];
        (
            c.symbol().to_string(),
            c.fg,
            c.bg,
            c.modifier.contains(Modifier::BOLD),
        )
    }

    fn rgb(c: (u8, u8, u8)) -> Color {
        Color::Rgb(c.0, c.1, c.2)
    }

    const FIRER: Pos = Pos::new(40, 30);
    const TARGET: Pos = Pos::new(52, 30);

    #[test]
    fn flash_lights_the_muzzle_two_ticks() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Flash { big: false }, FIRER, TARGET, "");
        let (b, plain) = drawn(&c, &s, &inst, 100);
        let (glyph, fg, bg, bold) = at(&b, 40, 30);
        assert_eq!(glyph, at(&plain, 40, 30).0, "the firer keeps its glyph");
        assert_eq!((fg, bg, bold), (rgb((0, 0, 0)), rgb((255, 240, 180)), true));
        assert_eq!(at(&b, 41, 30).0, "✦");
        assert_eq!(at(&b, 41, 30).1, rgb((255, 230, 120)));
        assert_eq!(at(&b, 42, 30).0, "·");
        assert_eq!(at(&b, 42, 30).1, rgb((255, 200, 90)));
        // Small: no glyph above, only the glow on the ground.
        assert_eq!(
            at(&b, 41, 29).0,
            at(&plain, 41, 29).0,
            "small: nothing above"
        );
        let (b, plain) = drawn(&c, &s, &inst, 101);
        assert_eq!(at(&b, 40, 30).2, rgb((255, 170, 40)));
        assert_eq!(at(&b, 41, 30).0, "*");
        assert_eq!(
            at(&b, 42, 30).0,
            at(&plain, 42, 30).0,
            "the dot is one tick"
        );
        let (b, plain) = drawn(&c, &s, &inst, 102);
        assert_eq!(b, plain, "gone after two ticks");
    }

    #[test]
    fn a_flash_lights_the_tiles_around_the_muzzle_and_a_tracer_its_neighbours() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Flash { big: false }, FIRER, TARGET, "");
        let (b, plain) = drawn(&c, &s, &inst, 100);
        let warmer = |x: i32, y: i32| {
            let (Color::Rgb(r, g, _), Color::Rgb(pr, pg, _)) = (at(&b, x, y).2, at(&plain, x, y).2)
            else {
                panic!("rgb cells")
            };
            (r, g) > (pr, pg)
        };
        assert!(warmer(40, 29), "above the muzzle");
        assert!(warmer(38, 30), "two cells behind");
        assert!(warmer(42, 31), "ahead and below");
        assert_eq!(
            at(&b, 44, 31).2,
            at(&plain, 44, 31).2,
            "the pool stays tight"
        );
        assert_eq!(
            at(&b, 46, 30).2,
            at(&plain, 46, 30).2,
            "six cells out is dark"
        );
        let (b1, _) = drawn(&c, &s, &inst, 101);
        let Color::Rgb(r0, ..) = at(&b, 40, 29).2 else {
            panic!()
        };
        let Color::Rgb(r1, ..) = at(&b1, 40, 29).2 else {
            panic!()
        };
        assert!(r1 < r0, "the glow fades on its second tick");
        let (b, plain) = drawn(&c, &s, &inst, 102);
        assert_eq!(b, plain);
        // The tracer head throws a faint light on the cells beside it.
        let fx = Fx::Tracer {
            colour: (255, 170, 40),
            speed: 3,
        };
        let inst = shot(fx, FIRER, TARGET, "");
        let (b, plain) = drawn(&c, &s, &inst, 101);
        let (Color::Rgb(r, ..), Color::Rgb(pr, ..)) = (at(&b, 44, 29).2, at(&plain, 44, 29).2)
        else {
            panic!("rgb cells")
        };
        assert!(r > pr, "above the head at (44,30)");
        assert_eq!(
            at(&b, 44, 28).2,
            at(&plain, 44, 28).2,
            "two rows up is dark"
        );
    }

    #[test]
    fn a_casing_pops_out_beside_the_muzzle_and_lies_there_dimming() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Casing, FIRER, TARGET, "");
        // Shot along +x: the casing flies out above the shooter.
        let (b, _) = drawn(&c, &s, &inst, 100);
        assert_eq!(at(&b, 40, 29).0, "'");
        assert_eq!(at(&b, 40, 29).1, rgb((240, 200, 90)));
        // Landed: a `,` within two cells above and one behind, where it stays.
        let (b, plain) = drawn(&c, &s, &inst, 102);
        let landed: Vec<(i32, i32)> = (38..=40)
            .flat_map(|x| (28..=29).map(move |y| (x, y)))
            .filter(|(x, y)| at(&b, *x, *y).0 == "," && at(&plain, *x, *y).0 != ",")
            .collect();
        assert_eq!(landed.len(), 1, "{landed:?}");
        let (lx, ly) = landed[0];
        assert_eq!(at(&b, lx, ly).1, rgb((200, 160, 60)));
        let (b, _) = drawn(&c, &s, &inst, 100 + CASING_TICKS - 1);
        assert_eq!(at(&b, lx, ly).0, ",");
        let Color::Rgb(r, ..) = at(&b, lx, ly).1 else {
            panic!()
        };
        assert!(r < 120, "faded to the ground's tone: {r}");
        let (b, plain) = drawn(&c, &s, &inst, 100 + CASING_TICKS);
        assert_eq!(b, plain, "gone");
        // Shot straight down: it flies out to the right.
        let inst = shot(Fx::Casing, FIRER, Pos::new(40, 40), "");
        let (b, _) = drawn(&c, &s, &inst, 100);
        assert_eq!(at(&b, 41, 30).0, "'");
    }

    #[test]
    fn big_flash_adds_the_cells_above_and_below() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Flash { big: true }, FIRER, TARGET, "");
        let (b, _) = drawn(&c, &s, &inst, 100);
        assert_eq!(at(&b, 41, 29).0, "`");
        assert_eq!(at(&b, 41, 29).1, rgb((255, 200, 90)));
        assert_eq!(at(&b, 41, 31).0, ",");
        let (b, plain) = drawn(&c, &s, &inst, 101);
        assert_eq!(at(&b, 41, 29), at(&plain, 41, 29));
        assert_eq!(at(&b, 41, 31), at(&plain, 41, 31));
    }

    #[test]
    fn tracer_head_moves_three_tiles_a_tick_with_a_fading_trail() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let fx = Fx::Tracer {
            colour: (255, 170, 40),
            speed: 3,
        };
        let inst = shot(fx, FIRER, TARGET, "");
        let (b, plain) = drawn(&c, &s, &inst, 100);
        assert_eq!(at(&b, 41, 30).0, "•");
        assert_eq!(at(&b, 41, 30).1, rgb((255, 240, 200)));
        // Never behind the firer: the glyph and its colour stand; only the head's
        // light reaches the ground there.
        assert_eq!(
            (at(&b, 40, 30).0, at(&b, 40, 30).1),
            (at(&plain, 40, 30).0, at(&plain, 40, 30).1),
            "never behind the firer"
        );
        let (b, _) = drawn(&c, &s, &inst, 101);
        assert_eq!(at(&b, 44, 30).0, "•");
        assert_eq!(
            at(&b, 43, 30),
            ("─".into(), rgb((255, 170, 40)), at(&b, 43, 30).2, true)
        );
        assert_eq!(at(&b, 42, 30).1, rgb((200, 110, 20)));
        assert_eq!(at(&b, 41, 30).1, rgb((110, 55, 10)));
        let (b, _) = drawn(&c, &s, &inst, 103);
        assert_eq!(at(&b, 50, 30).0, "•");
        let (b, plain) = drawn(&c, &s, &inst, 104);
        assert_eq!(b, plain, "arrived: the instance has ended");
    }

    #[test]
    fn tracer_on_a_diagonal_uses_the_slash_glyphs() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let fx = Fx::Tracer {
            colour: (255, 170, 40),
            speed: 3,
        };
        let down = shot(fx.clone(), FIRER, Pos::new(46, 36), "");
        let (b, _) = drawn(&c, &s, &down, 101);
        assert_eq!(at(&b, 44, 34).0, "•");
        assert_eq!(at(&b, 43, 33).0, "╲");
        let up = shot(fx, FIRER, Pos::new(46, 24), "");
        let (b, _) = drawn(&c, &s, &up, 101);
        assert_eq!(at(&b, 43, 27).0, "╱");
    }

    #[test]
    fn streak_whitens_then_ambers_then_embers() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(
            Fx::Streak {
                colour: (0, 229, 255),
            },
            FIRER,
            TARGET,
            "",
        );
        let (b, plain) = drawn(&c, &s, &inst, 100);
        for x in 41..=51 {
            assert_eq!(at(&b, x, 30).0, "━", "x {x}");
            assert_eq!(at(&b, x, 30).1, rgb((240, 240, 240)));
            assert!(at(&b, x, 30).3, "bold on the first tick");
        }
        assert_eq!(at(&b, 40, 30), at(&plain, 40, 30));
        assert_eq!(at(&b, 52, 30), at(&plain, 52, 30));
        let (b, _) = drawn(&c, &s, &inst, 101);
        assert_eq!(at(&b, 45, 30).1, rgb((0, 229, 255)));
        let (b, _) = drawn(&c, &s, &inst, 102);
        assert_eq!(at(&b, 45, 30).1, rgb((120, 60, 10)));
        let (b, plain) = drawn(&c, &s, &inst, 103);
        assert_eq!(b, plain);
    }

    #[test]
    fn chips_invert_the_target_and_fly_away_from_the_shooter() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Chips, FIRER, TARGET, "");
        let (b, plain) = drawn(&c, &s, &inst, 100);
        let (glyph, fg, bg, bold) = at(&b, 52, 30);
        assert_eq!(glyph, at(&plain, 52, 30).0);
        assert_eq!(
            (fg, bg, bold),
            (rgb((255, 255, 255)), rgb((255, 40, 40)), true)
        );
        assert_eq!(at(&b, 53, 29).0, "'");
        assert_eq!(at(&b, 53, 29).1, rgb((255, 120, 120)));
        assert_eq!(at(&b, 53, 30).0, "·");
        assert_eq!(
            at(&b, 53, 31),
            at(&plain, 53, 31),
            "the third chip is not out yet"
        );
        let (b, _) = drawn(&c, &s, &inst, 101);
        assert_eq!(at(&b, 52, 30).2, rgb((160, 20, 20)));
        assert_eq!(at(&b, 53, 31).0, ",");
        assert_eq!(at(&b, 54, 29).0, "·");
        assert_eq!(at(&b, 53, 31).1, rgb((180, 60, 60)));
        let (b, plain) = drawn(&c, &s, &inst, 102);
        assert_eq!(at(&b, 52, 30), at(&plain, 52, 30), "the flash is two ticks");
        assert_eq!(at(&b, 54, 29).0, "'", "the chips drift a cell outward");
        let (b, plain) = drawn(&c, &s, &inst, 103);
        assert_eq!(b, plain);
        // From the right the chips fly left.
        let inst = shot(Fx::Chips, Pos::new(60, 30), TARGET, "");
        let (b, _) = drawn(&c, &s, &inst, 100);
        assert_eq!(at(&b, 51, 29).0, "'");
    }

    #[test]
    fn spark_at_the_impact_three_ticks() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Spark, FIRER, TARGET, "");
        let (b, _) = drawn(&c, &s, &inst, 100);
        assert_eq!(
            at(&b, 52, 30),
            ("✶".into(), rgb((255, 240, 160)), at(&b, 52, 30).2, true)
        );
        assert_eq!(at(&b, 51, 29).0, "`");
        assert_eq!(at(&b, 51, 31).0, ",");
        assert_eq!(at(&b, 51, 31).1, rgb((255, 220, 120)));
        let (b, plain) = drawn(&c, &s, &inst, 101);
        assert_eq!(at(&b, 52, 30).0, "×");
        assert_eq!(at(&b, 52, 30).1, rgb((255, 170, 40)));
        assert_eq!(at(&b, 51, 29), at(&plain, 51, 29));
        assert_eq!(at(&b, 50, 31).0, "·");
        assert_eq!(at(&b, 50, 31).1, rgb((200, 110, 20)));
        let (b, _) = drawn(&c, &s, &inst, 102);
        assert_eq!(at(&b, 52, 30).0, "·");
        assert_eq!(at(&b, 52, 30).1, rgb((120, 60, 10)));
        let (b, plain) = drawn(&c, &s, &inst, 103);
        assert_eq!(b, plain);
    }

    #[test]
    fn tear_scrambles_the_box_around_the_target() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Tear, FIRER, TARGET, "");
        let (b, plain) = drawn(&c, &s, &inst, 100);
        let mut torn = 0;
        for y in 0..60 {
            for x in 0..120 {
                if at(&b, x, y) == at(&plain, x, y) {
                    continue;
                }
                torn += 1;
                assert!((49..=55).contains(&x) && (29..=31).contains(&y), "{x},{y}");
                assert!((x, y) != (52, 30), "the target itself is left alone");
                let (glyph, fg, _, _) = at(&b, x, y);
                assert!(SPARKS.contains(&glyph.chars().next().unwrap()), "{glyph}");
                assert!(fg == rgb((0, 229, 255)) || fg == rgb((255, 0, 200)));
            }
        }
        assert!((3..=14).contains(&torn), "{torn} cells torn");
        let (b2, _) = drawn(&c, &s, &inst, 101);
        assert_ne!(b, b2, "the tear shifts every tick");
        let (b, plain) = drawn(&c, &s, &inst, 103);
        assert_eq!(b, plain);
    }

    #[test]
    fn number_rises_a_row_every_four_ticks_and_fades() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Number { tag: false }, FIRER, TARGET, "-4");
        let (b, _) = drawn(&c, &s, &inst, 100);
        assert_eq!(
            at(&b, 52, 29),
            ("-".into(), rgb((255, 235, 235)), at(&b, 52, 29).2, true)
        );
        assert_eq!(at(&b, 53, 29).0, "4");
        let (b, _) = drawn(&c, &s, &inst, 104);
        assert_eq!(at(&b, 52, 28).0, "-");
        let (b, _) = drawn(&c, &s, &inst, 110);
        assert_eq!(at(&b, 52, 27).0, "-");
        assert_eq!(at(&b, 52, 27).1, rgb((170, 60, 60)));
        let (b, _) = drawn(&c, &s, &inst, 113);
        assert_eq!(at(&b, 53, 26).0, "4");
        let (b, plain) = drawn(&c, &s, &inst, 114);
        assert_eq!(b, plain);
        // The net lends its bolt's colour.
        let mut tinted = inst.clone();
        tinted.tint = Some((255, 0, 200));
        let (b, _) = drawn(&c, &s, &tinted, 100);
        assert_eq!(at(&b, 52, 29).1, rgb((255, 0, 200)));
        let (b, _) = drawn(&c, &s, &tinted, 110);
        assert_eq!(at(&b, 52, 27).1, rgb((153, 0, 120)));
    }

    #[test]
    fn tag_draws_on_a_red_field() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Number { tag: true }, FIRER, TARGET, "-4");
        let (b, _) = drawn(&c, &s, &inst, 100);
        assert_eq!(
            at(&b, 51, 29),
            (" ".into(), rgb((255, 255, 255)), rgb((170, 20, 20)), true)
        );
        assert_eq!(at(&b, 52, 29).0, "-");
        assert_eq!(at(&b, 53, 29).0, "4");
        assert_eq!(
            at(&b, 54, 29),
            (" ".into(), rgb((255, 255, 255)), rgb((170, 20, 20)), true)
        );
        let (b, _) = drawn(&c, &s, &inst, 110);
        assert_eq!(at(&b, 52, 27).1, rgb((200, 120, 120)));
        assert_eq!(at(&b, 52, 27).2, rgb((80, 10, 10)));
    }

    #[test]
    fn a_tinted_shot_tears_sparks_and_tags_in_its_colour() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let cyan = (0, 229, 255);
        let mut tear = shot(Fx::Tear, FIRER, TARGET, "");
        tear.tint = Some(cyan);
        let (b, plain) = drawn(&c, &s, &tear, 100);
        let deep = Rgb(0, 229, 255).mix(DEEP, 0.6).to_color();
        let mut torn = 0;
        for y in 29..=31 {
            for x in 49..=55 {
                if at(&b, x, y) != at(&plain, x, y) {
                    torn += 1;
                    let fg = at(&b, x, y).1;
                    assert!(
                        fg == rgb(cyan) || fg == deep,
                        "{fg:?}: no magenta in a tinted tear"
                    );
                }
            }
        }
        assert!(torn > 0);
        let mut spark = shot(Fx::Spark, FIRER, TARGET, "");
        spark.tint = Some(cyan);
        let (b, _) = drawn(&c, &s, &spark, 101);
        assert_eq!(
            at(&b, 52, 30).1,
            rgb(cyan),
            "the spark in the shot's colour"
        );
        let mut tag = shot(Fx::Number { tag: true }, FIRER, TARGET, "-2");
        tag.tint = Some(cyan);
        let (b, _) = drawn(&c, &s, &tag, 100);
        assert_eq!(at(&b, 52, 29).2, rgb((0, 90, 110)), "the cyan field");
        assert_eq!(at(&b, 52, 29).1, rgb((255, 255, 255)));
    }

    #[test]
    fn a_future_instance_draws_nothing() {
        let (c, s) = (city(), StylePack::load().unwrap());
        let inst = shot(Fx::Streak { colour: (1, 1, 1) }, FIRER, TARGET, "");
        let (b, plain) = drawn(&c, &s, &inst, 99);
        assert_eq!(b, plain);
        let no_target = aimed(Fx::Chips, FIRER, Vec::new());
        let (b, plain) = drawn(&c, &s, &no_target, 100);
        assert_eq!(b, plain);
    }

    #[test]
    fn bolt_grows_toward_its_targets() {
        let c = city();
        let s = StylePack::load().unwrap();
        let origin = Pos::new(40, 30);
        let target = Pos::new(52, 30);
        let inst = aimed(
            Fx::Bolt {
                colour: BOLT,
                ticks: 12,
            },
            origin,
            vec![target],
        );
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&inst),
            vis: None,
        };
        let plain = render(&c, &s, StratumKind::Street, 106, None, &[], (120, 60));
        let half = render(
            &c,
            &s,
            StratumKind::Street,
            106,
            None,
            &[&overlay],
            (120, 60),
        );
        let touched = |buf: &Buffer, x: i32| {
            (29..=31).any(|y| buf[(x as u16, y as u16)] != plain[(x as u16, y as u16)])
        };
        assert!(
            touched(&half, 44),
            "the line has reached a third of the way"
        );
        assert!(!touched(&half, 51), "and not the far end yet");
        let full = render(
            &c,
            &s,
            StratumKind::Street,
            111,
            None,
            &[&overlay],
            (120, 60),
        );
        assert!(touched(&full, 51), "by its last tick it reaches the target");
        assert_eq!(
            half[(40, 30)],
            plain[(40, 30)],
            "the firer's own tile is untouched"
        );
    }

    #[test]
    fn mark_draws_through_walls_and_off_screen_targets_are_skipped() {
        let c = city();
        let s = StylePack::load().unwrap();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&MapRef::stratum_of(&c, StratumKind::Street), 0);
        let unseen = (0..60)
            .flat_map(|y| (0..120).map(move |x| Pos::new(x, y)))
            .find(|q| p.vis()[*q] == Visibility::Unseen)
            .unwrap();
        let inst = aimed(
            Fx::Mark {
                colour: BOLT,
                ticks: 600,
            },
            p.pos,
            vec![unseen, Pos::new(500, 500), Pos::new(-9, 3)],
        );
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&inst),
            vis: Some(p.vis()),
        };
        let buf = render(
            &c,
            &s,
            StratumKind::Street,
            150,
            Some(p.vis()),
            &[&overlay],
            (120, 60),
        );
        assert_eq!(buf[(unseen.x as u16, unseen.y as u16)].symbol(), "◉");
        let bolt = aimed(
            Fx::Bolt {
                colour: BOLT,
                ticks: 12,
            },
            p.pos,
            vec![Pos::new(500, 500), Pos::new(-9, 3)],
        );
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&bolt),
            vis: None,
        };
        for tick in [100u64, 106, 111] {
            render(
                &c,
                &s,
                StratumKind::Street,
                tick,
                None,
                &[&overlay],
                (120, 60),
            );
            render(
                &c,
                &s,
                StratumKind::Street,
                tick,
                None,
                &[&overlay],
                (20, 10),
            );
        }
        // No targets: nothing drawn.
        let none = aimed(
            Fx::Bolt {
                colour: BOLT,
                ticks: 12,
            },
            p.pos,
            Vec::new(),
        );
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&none),
            vis: None,
        };
        let plain = render(&c, &s, StratumKind::Street, 106, None, &[], (120, 60));
        assert_eq!(
            render(
                &c,
                &s,
                StratumKind::Street,
                106,
                None,
                &[&overlay],
                (120, 60)
            ),
            plain
        );
    }

    const CYAN: (u8, u8, u8) = (0, 229, 255);

    #[test]
    fn wave_is_round_on_screen_and_hides_unreached_tiles() {
        let c = city();
        let s = StylePack::load().unwrap();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&MapRef::stratum_of(&c, StratumKind::Street), 0);
        let revealed = p.reveal(&MapRef::stratum_of(&c, StratumKind::Street), 12);
        let origin = p.pos;
        // Tick 106: elapsed 6 of 12, the front sits at half the reach, 13.4 cells.
        let r = reach(12) * 6.0 / 12.0;
        let pick = |lo: f32, hi: f32| {
            revealed
                .iter()
                .copied()
                .find(|q| {
                    let d = screen_dist(*q, origin);
                    d > lo && d <= hi
                })
                .expect("a revealed tile in that band")
        };
        let front = pick(r - 1.0, r);
        let beyond = pick(r + 2.0, reach(12));
        let behind = pick(r - 5.0, r - 1.5);
        let inst = instance(
            Fx::Wave {
                colour: CYAN,
                ticks: 12,
            },
            origin,
            12,
            revealed.clone(),
        );
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&inst),
            vis: Some(p.vis()),
        };
        let buf = render(
            &c,
            &s,
            StratumKind::Street,
            106,
            Some(p.vis()),
            &[&overlay],
            (120, 60),
        );
        let cell = |q: Pos| buf[(q.x as u16, q.y as u16)].clone();
        assert_eq!(
            cell(front).fg,
            Color::Rgb(0, 229, 255),
            "front tile carries the colour"
        );
        assert!(
            !cell(front).modifier.contains(Modifier::DIM),
            "the front is bright even over remembered tiles"
        );
        assert_eq!(
            cell(beyond).symbol(),
            " ",
            "unreached revealed tile is hidden"
        );
        assert_eq!(cell(beyond).bg, Color::Rgb(0, 0, 0));
        assert_ne!(
            cell(behind).fg,
            Color::Rgb(0, 0, 0),
            "the ripple tints what it passed"
        );
        // A square corner of the scan (chebyshev 12, screen distance beyond the front) is
        // still hidden now and painted by the time the wave ends.
        let plain = render(
            &c,
            &s,
            StratumKind::Street,
            200,
            Some(p.vis()),
            &[],
            (120, 60),
        );
        let after = render(
            &c,
            &s,
            StratumKind::Street,
            200,
            Some(p.vis()),
            &[&overlay],
            (120, 60),
        );
        assert_eq!(after, plain);
        // Round: the row directly above the origin reaches only half as far as the row.
        assert!((screen_dist(Pos::new(origin.x + 4, origin.y), origin) - 4.0).abs() < 1e-5);
        assert!((screen_dist(Pos::new(origin.x, origin.y + 2), origin) - 4.0).abs() < 1e-5);
        assert_eq!(wave_tail_ticks(15, 12), 6);
        assert_eq!(wave_tail_ticks(20, 24), 4);
        assert_eq!(wave_tail_ticks(10, 0), 0);
    }

    #[test]
    fn kick_scatters_then_stops() {
        let c = city();
        let s = StylePack::load().unwrap();
        let inst = instance(Fx::Kick(4), Pos::new(10, 10), 0, Vec::new());
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&inst),
            vis: None,
        };
        let plain = render(&c, &s, StratumKind::Street, 100, None, &[], (120, 60));
        let kicked = render(
            &c,
            &s,
            StratumKind::Street,
            100,
            None,
            &[&overlay],
            (120, 60),
        );
        assert_ne!(kicked, plain, "the first tick scatters glyphs");
        let plain = render(&c, &s, StratumKind::Street, 104, None, &[], (120, 60));
        let done = render(
            &c,
            &s,
            StratumKind::Street,
            104,
            None,
            &[&overlay],
            (120, 60),
        );
        assert_eq!(done, plain, "nothing after its ticks");
    }

    #[test]
    fn glow_marks_connector_out_of_sight() {
        let c = city();
        let s = StylePack::load().unwrap();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&MapRef::stratum_of(&c, StratumKind::Street), 0);
        let k = c
            .connectors
            .iter()
            .find(|k| {
                k.serves.contains(&StratumKind::Street)
                    && k.pos.chebyshev(p.pos) <= 24
                    && p.vis()[k.pos] != Visibility::Visible
            })
            .expect("a connector in range but out of sight");
        let inst = instance(
            Fx::Glow {
                colour: (255, 0, 200),
                ticks: 180,
            },
            p.pos,
            24,
            Vec::new(),
        );
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&inst),
            vis: Some(p.vis()),
        };
        let buf = render(
            &c,
            &s,
            StratumKind::Street,
            110,
            Some(p.vis()),
            &[&overlay],
            (120, 60),
        );
        let cell = &buf[(k.pos.x as u16, k.pos.y as u16)];
        assert_eq!(cell.symbol(), "◆");
        assert!(
            matches!(cell.fg, Color::Rgb(r, 0, _) if r > 100),
            "{:?}",
            cell.fg
        );
    }

    #[test]
    fn fx_draws_only_on_its_stratum() {
        let c = city();
        let s = StylePack::load().unwrap();
        let mut kick = instance(Fx::Kick(12), Pos::new(10, 10), 0, Vec::new());
        kick.place = Place::Stratum(StratumKind::Net);
        let mut bolt = aimed(
            Fx::Bolt {
                colour: BOLT,
                ticks: 12,
            },
            Pos::new(40, 30),
            vec![Pos::new(52, 21)],
        );
        bolt.place = Place::Stratum(StratumKind::Net);
        let mut mark = aimed(
            Fx::Mark {
                colour: BOLT,
                ticks: 600,
            },
            Pos::new(40, 30),
            vec![Pos::new(52, 21)],
        );
        mark.place = Place::Stratum(StratumKind::Net);
        let all = [kick, bolt, mark];
        // Each shape really draws at this tick on its own stratum, so the Street
        // check below cannot pass because a shape had already ended.
        let plain_net = render(&c, &s, StratumKind::Net, 106, None, &[], (120, 60));
        for (i, inst) in all.iter().enumerate() {
            let alone = FxOverlay {
                fx: std::slice::from_ref(inst),
                vis: None,
            };
            let on_net = render(&c, &s, StratumKind::Net, 106, None, &[&alone], (120, 60));
            assert_ne!(
                on_net, plain_net,
                "shape {i} is live on the Net at this tick"
            );
        }
        let overlay = FxOverlay {
            fx: &all,
            vis: None,
        };
        let plain = render(&c, &s, StratumKind::Street, 106, None, &[], (120, 60));
        let drawn = render(
            &c,
            &s,
            StratumKind::Street,
            106,
            None,
            &[&overlay],
            (120, 60),
        );
        assert_eq!(drawn, plain, "kick, bolt and mark all stay on the Net");
    }

    #[test]
    fn wave_at_the_map_edge_is_safe() {
        let c = city();
        let s = StylePack::load().unwrap();
        let revealed: Vec<Pos> = (-6..6)
            .flat_map(|y| (-6..6).map(move |x| Pos::new(x, y)))
            .collect();
        let inst = instance(
            Fx::Wave {
                colour: CYAN,
                ticks: 18,
            },
            Pos::new(-5, -5),
            12,
            revealed,
        );
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&inst),
            vis: None,
        };
        for tick in [100u64, 105, 112, 120] {
            render(
                &c,
                &s,
                StratumKind::Street,
                tick,
                None,
                &[&overlay],
                (20, 10),
            );
            render(
                &c,
                &s,
                StratumKind::Street,
                tick,
                None,
                &[&overlay],
                (120, 60),
            );
        }
    }

    fn step_is_orthogonal(a: Pos, b: Pos) -> bool {
        (a.x - b.x).abs() + (a.y - b.y).abs() == 1
    }

    #[test]
    fn snake_path_only_moves_horizontally_or_vertically() {
        let a = Pos::new(40, 30);
        let b = Pos::new(52, 21);
        let pts = snake_points(a, b, 7);
        assert_eq!((pts[0], *pts.last().unwrap()), (a, b));
        assert_eq!(pts.len(), 12 + 9 + 1, "manhattan length: never a step away");
        assert!(
            pts.windows(2).all(|w| step_is_orthogonal(w[0], w[1])),
            "{pts:?}"
        );
        assert_eq!(snake_points(a, b, 7), pts, "stable for a seed");
        assert_ne!(
            snake_points(a, b, 8),
            pts,
            "another seed snakes differently"
        );
        let turns = pts
            .windows(3)
            .filter(|w| (w[1].x - w[0].x) != (w[2].x - w[1].x))
            .count();
        assert!(
            turns >= 3,
            "runs of one to four cells: {turns} turns in {pts:?}"
        );
        assert_eq!(snake_points(a, a, 1), vec![a]);
        let flat = snake_points(a, Pos::new(52, 30), 3);
        assert!(
            flat.iter().all(|p| p.y == 30),
            "no room to snake on a flat line"
        );
    }

    #[test]
    fn bolt_snakes_with_corners_and_no_diagonals() {
        let c = city();
        let s = StylePack::load().unwrap();
        let origin = Pos::new(40, 30);
        let target = Pos::new(52, 21);
        let inst = aimed(
            Fx::Bolt {
                colour: BOLT,
                ticks: 12,
            },
            origin,
            vec![target],
        );
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&inst),
            vis: None,
        };
        let plain = render(&c, &s, StratumKind::Street, 111, None, &[], (120, 60));
        let full = render(
            &c,
            &s,
            StratumKind::Street,
            111,
            None,
            &[&overlay],
            (120, 60),
        );
        let mut glyphs = String::new();
        let mut outside = 0;
        for y in 0..60u16 {
            for x in 0..120u16 {
                if full[(x, y)] != plain[(x, y)] {
                    glyphs.push_str(full[(x, y)].symbol());
                    if !(40..=52).contains(&x) || !(21..=30).contains(&y) {
                        outside += 1;
                    }
                }
            }
        }
        assert!(
            !glyphs.contains('╱') && !glyphs.contains('╲'),
            "no diagonal glyphs: {glyphs}"
        );
        assert!(
            glyphs.chars().any(|g| "┌┐└┘".contains(g)),
            "corners where it turns: {glyphs}"
        );
        assert_eq!(
            outside, 0,
            "the bolt stays inside the box between firer and node"
        );
    }

    // ---- the grenade and ability shapes

    const L: Pos = Pos::new(40, 30);
    const GREN: Rgb = Rgb(190, 190, 140);
    const WHITE_RGB: Rgb = Rgb(255, 255, 255);
    const CYAN_RGB: Rgb = Rgb(0, 229, 255);

    /// Every tile within `r` of `c`, the centre first: the footprint on open ground.
    fn square(c: Pos, r: i32) -> Vec<Pos> {
        let mut out = vec![c];
        for dy in -r..=r {
            for dx in -r..=r {
                if (dx, dy) != (0, 0) {
                    out.push(c.offset(dx, dy));
                }
            }
        }
        out
    }

    fn blast_at(fx: Fx, r: i32, hit: bool) -> FxInstance {
        let mut i = instance(fx, L, r, Vec::new());
        i.targets = square(L, r);
        i.hit = hit;
        i
    }

    /// The instance drawn at `started + e`, its scorch on the ground layer and its
    /// shapes over it, and the plain street beside it.
    fn at_e(inst: &FxInstance, e: u64) -> (Buffer, Buffer) {
        at_e_seen(inst, e, None)
    }

    /// `at_e` with sight on the street.
    fn at_e_seen(inst: &FxInstance, e: u64, vis: Option<&Grid<Visibility>>) -> (Buffer, Buffer) {
        let (c, s) = (city(), StylePack::load().unwrap());
        let tick = inst.started + e;
        let fx = std::slice::from_ref(inst);
        let scorch = ScorchOverlay { fx, vis };
        let overlay = FxOverlay { fx, vis };
        let plain = render(&c, &s, StratumKind::Street, tick, vis, &[], (120, 60));
        let with = render(
            &c,
            &s,
            StratumKind::Street,
            tick,
            vis,
            &[&scorch, &overlay],
            (120, 60),
        );
        (with, plain)
    }

    fn cell(b: &Buffer, p: Pos) -> (String, Color, Color, bool) {
        at(b, p.x, p.y)
    }

    fn col(c: Rgb) -> Color {
        c.to_color()
    }

    /// A glyph in a colour, whatever is behind it.
    fn glyph_fg(b: &Buffer, p: Pos) -> (String, Color) {
        let (g, fg, _, _) = cell(b, p);
        (g, fg)
    }

    fn thrown(arc: Arc, to: Pos) -> FxInstance {
        let mut i = aimed(Fx::Throw, Pos::new(30, 30), vec![to]);
        i.arc = Some(arc);
        i
    }

    #[test]
    fn throw_lob_rises_and_lands() {
        // 11 tiles east: five cells in flight, then the landing, bounce and settle.
        let inst = thrown(Arc::Lob, Pos::new(41, 30));
        let (b, _) = at_e(&inst, 2);
        // Head at line cell 5, round(2.2 * 4 * s(1 - s)) = 2 rows up.
        let (g, fg, _, bold) = cell(&b, Pos::new(35, 28));
        assert_eq!((g.as_str(), fg, bold), ("o", col(GREN), true));
        // The three behind it, each at its own height, fading.
        let fade = |t: f32| col(Rgb(120, 120, 120).mix(Rgb(50, 50, 50), t));
        assert_eq!(
            glyph_fg(&b, Pos::new(33, 28)),
            ("·".to_string(), fade(1.0 / 3.0))
        );
        assert_eq!(
            glyph_fg(&b, Pos::new(31, 29)),
            ("·".to_string(), fade(2.0 / 3.0))
        );
        assert_ne!(cell(&b, Pos::new(35, 30)).0, "o", "not on the line itself");
        let (b, _) = at_e(&inst, 5);
        let (g, fg, _, bold) = cell(&b, Pos::new(41, 30));
        assert_eq!((g.as_str(), fg, bold), ("●", col(Rgb(255, 240, 180)), true));
        let (b, _) = at_e(&inst, 6);
        let (g, fg, _, bold) = cell(&b, Pos::new(42, 29));
        assert_eq!((g.as_str(), fg, bold), ("o", col(GREN), true));
        let (b, _) = at_e(&inst, 7);
        let (g, fg, _, bold) = cell(&b, Pos::new(41, 30));
        assert_eq!((g.as_str(), fg, bold), ("o", col(GREN), true));
        let (b, plain) = at_e(&inst, 8);
        assert_eq!(b, plain, "nothing after the settle");
    }

    #[test]
    fn throw_shadow_grows_on_the_line() {
        // 21 tiles: the first cell is low, the middle is high, all on the row.
        let inst = thrown(Arc::Shadow, Pos::new(51, 30));
        let (b, _) = at_e(&inst, 0);
        let h = 4.0 * (1.0 / 21.0) * (20.0 / 21.0);
        let (g, fg, _, bold) = cell(&b, Pos::new(31, 30));
        assert_eq!(
            (g.as_str(), fg, bold),
            ("·", col(GREN.mix(WHITE_RGB, h)), false)
        );
        let (b, _) = at_e(&inst, 1);
        let (g, _, _, bold) = cell(&b, Pos::new(33, 30));
        assert_eq!((g.as_str(), bold), ("o", false));
        let (b, _) = at_e(&inst, 5);
        let h = 4.0 * (11.0 / 21.0) * (10.0 / 21.0);
        let (g, fg, _, bold) = cell(&b, Pos::new(41, 30));
        assert_eq!(
            (g.as_str(), fg, bold),
            ("O", col(GREN.mix(WHITE_RGB, h)), true)
        );
        for y in [28, 29, 31, 32] {
            assert_ne!(cell(&b, Pos::new(41, y)).0, "O", "never off the row");
        }
    }

    #[test]
    fn bloom_by_tick() {
        let inst = blast_at(Fx::Bloom, 2, true);
        let ring1 = Pos::new(41, 30);
        let ring2 = Pos::new(42, 30);
        let (b, plain) = at_e(&inst, 0);
        assert_eq!(
            cell(&b, L),
            ("✺".into(), col(Rgb(0, 0, 0)), col(Rgb(255, 240, 180)), true)
        );
        assert_eq!(cell(&b, ring1), cell(&plain, ring1));
        let (b, plain) = at_e(&inst, 1);
        assert_eq!(
            cell(&b, L),
            ("✺".into(), col(WHITE_RGB), col(Rgb(255, 120, 40)), true)
        );
        assert_eq!(
            cell(&b, ring1),
            (
                "*".into(),
                col(Rgb(255, 230, 120)),
                col(Rgb(200, 60, 20)),
                true
            )
        );
        assert_eq!(cell(&b, ring2), cell(&plain, ring2), "ring 2 waits for e2");
        let (b, _) = at_e(&inst, 2);
        assert_eq!(
            cell(&b, L),
            (
                "▒".into(),
                col(Rgb(255, 200, 90)),
                col(Rgb(120, 40, 10)),
                false
            )
        );
        assert_eq!(
            cell(&b, ring1),
            (
                "░".into(),
                col(Rgb(255, 170, 40)),
                col(Rgb(120, 40, 10)),
                false
            )
        );
        assert_eq!(
            cell(&b, ring2),
            (
                "*".into(),
                col(Rgb(255, 200, 90)),
                col(Rgb(90, 30, 10)),
                true
            )
        );
        let (b, plain) = at_e(&inst, 3);
        assert_eq!(
            cell(&b, L),
            (
                "▓".into(),
                col(Rgb(90, 90, 90)),
                col(Rgb(50, 40, 36)),
                false
            )
        );
        assert_eq!(
            cell(&b, ring1),
            (
                "▒".into(),
                col(Rgb(120, 120, 120)),
                col(Rgb(60, 40, 30)),
                false
            )
        );
        let (g, fg, bg, _) = cell(&b, ring2);
        assert_eq!(
            (g.as_str(), fg, bg),
            ("·", col(Rgb(255, 170, 40)), cell(&plain, ring2).2)
        );
    }

    #[test]
    fn bloom_stays_inside_its_footprint_until_the_smoke_rises() {
        let inst = blast_at(Fx::Bloom, 2, true);
        for e in 0..4 {
            let (b, plain) = at_e(&inst, e);
            for y in 0..60 {
                for x in 0..120 {
                    let p = Pos::new(x, y);
                    if p.chebyshev(L) > 2 {
                        assert_eq!(cell(&b, p), cell(&plain, p), "e{e} touched {p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn bloom_smoke_and_embers_follow_the_hash() {
        let inst = blast_at(Fx::Bloom, 2, true);
        let (b, _) = at_e(&inst, 6);
        let smoke = Rgb(110, 110, 110).mix(Rgb(45, 45, 45), 2.0 / 9.0);
        let (mut smoked, mut embers) = (0, 0);
        for p in square(L, 2) {
            let h = blast_hash(p, 3);
            if h.is_multiple_of(3) {
                // Risen (6 - 4) / 4 = 0 rows.
                assert_eq!(
                    glyph_fg(&b, p),
                    ("░".to_string(), col(smoke)),
                    "smoke at {p:?}"
                );
                smoked += 1;
            } else if h.is_multiple_of(7) {
                assert_eq!(
                    glyph_fg(&b, p),
                    ("·".to_string(), col(Rgb(200, 110, 20))),
                    "ember at {p:?}"
                );
                embers += 1;
            }
        }
        assert!(smoked > 0, "some smoke");
        assert!(embers > 0, "some embers");
        // At e9 the embers are gone and the smoke has risen a row.
        let (b, _) = at_e(&inst, 9);
        let smoke = Rgb(110, 110, 110).mix(Rgb(45, 45, 45), 5.0 / 9.0);
        let risen = square(L, 2)
            .into_iter()
            .find(|p| blast_hash(*p, 4).is_multiple_of(3))
            .expect("some smoke at e9");
        assert_eq!(
            glyph_fg(&b, risen.offset(0, -1)),
            ("░".to_string(), col(smoke))
        );
    }

    #[test]
    fn bloom_scorch_stays_after_the_blast_when_it_hurt() {
        let inst = blast_at(Fx::Bloom, 2, true);
        let ring1 = Pos::new(41, 30);
        let (b, plain) = at_e(&inst, BLOOM_TICKS);
        assert_eq!(
            cell(&b, L),
            (
                "▒".into(),
                col(Rgb(45, 40, 36)),
                col(Rgb(18, 16, 15)),
                false
            )
        );
        assert_eq!(
            cell(&b, ring1),
            (
                "░".into(),
                col(Rgb(38, 34, 30)),
                col(Rgb(20, 17, 16)),
                false
            )
        );
        assert_eq!(cell(&b, Pos::new(42, 30)), cell(&plain, Pos::new(42, 30)));
        // Under the live shape the stain is already down at e5.
        let (b, _) = at_e(&inst, 5);
        assert_eq!(cell(&b, ring1).2, col(Rgb(20, 17, 16)));
        // Remembered, it is dimmed as the map's memory is; never seen, not drawn.
        let mut vis = Grid::filled(city().size, Visibility::Visible);
        vis.set(L, Visibility::Remembered);
        vis.set(ring1, Visibility::Unseen);
        let (b, plain) = at_e_seen(&inst, BLOOM_TICKS, Some(&vis));
        assert_eq!(
            cell(&b, L),
            (
                "▒".into(),
                col(Rgb(45, 40, 36).scale(0.35)),
                col(Rgb(18, 16, 15).scale(0.35)),
                false
            )
        );
        assert!(
            b[(L.x as u16, L.y as u16)].modifier.contains(Modifier::DIM),
            "dim as the memory is"
        );
        assert_eq!(cell(&b, ring1), cell(&plain, ring1));
        // A blast that did no harm leaves nothing.
        let calm = blast_at(Fx::Bloom, 2, false);
        let (b, plain) = at_e(&calm, BLOOM_TICKS);
        assert_eq!(b, plain);
    }

    #[test]
    fn whiteout_by_tick() {
        let inst = blast_at(Fx::Whiteout, 3, true);
        let (ring1, ring2) = (Pos::new(41, 30), Pos::new(42, 30));
        let (b, plain) = at_e(&inst, 0);
        for p in [L, ring1, ring2, Pos::new(43, 33)] {
            let (g, fg, bg, bold) = cell(&b, p);
            assert_eq!(g, cell(&plain, p).0, "glyphs kept");
            assert_eq!((fg, bg, bold), (col(Rgb(0, 0, 0)), col(WHITE_RGB), true));
        }
        let (b, _) = at_e(&inst, 1);
        let (_, fg, bg, bold) = cell(&b, ring1);
        assert_eq!(
            (fg, bg, bold),
            (col(Rgb(60, 0, 0)), col(Rgb(255, 160, 60)), true)
        );
        let (_, fg, bg, _) = cell(&b, ring2);
        assert_eq!((fg, bg), (col(Rgb(60, 0, 0)), col(Rgb(255, 110, 50))));
        let (b, _) = at_e(&inst, 2);
        let stain = (col(Rgb(30, 27, 25)), col(Rgb(16, 14, 13)));
        assert_eq!(cell(&b, L), ("▓".into(), stain.0, stain.1, false));
        assert_eq!(cell(&b, ring1), ("▒".into(), stain.0, stain.1, false));
        assert_eq!(cell(&b, ring2), ("░".into(), stain.0, stain.1, false));
        // Debris at distance 3 on the axis, and halved up on the diagonal.
        assert_eq!(
            glyph_fg(&b, Pos::new(43, 30)),
            ("·".to_string(), col(Rgb(230, 230, 230)))
        );
        assert_eq!(cell(&b, Pos::new(43, 28)).0, "`");
        // Debris at distance 4 at e4, two fifths of the way to the grey.
        let (b, _) = at_e(&inst, 4);
        assert_eq!(
            glyph_fg(&b, Pos::new(44, 30)),
            (
                "·".to_string(),
                col(Rgb(230, 230, 230).mix(Rgb(90, 90, 90), 0.4))
            )
        );
        // The stain stays after the blast when it hurt, and not when it did not.
        let (b, _) = at_e(&inst, WHITEOUT_TICKS);
        assert_eq!(cell(&b, L), ("▓".into(), stain.0, stain.1, false));
        let calm = blast_at(Fx::Whiteout, 3, false);
        let (b, plain) = at_e(&calm, WHITEOUT_TICKS);
        assert_eq!(b, plain);
        let (b, _) = at_e(&calm, 2);
        assert_ne!(cell(&b, L).0, "▓", "a flashbang stains nothing");
    }

    #[test]
    fn shock_by_tick() {
        let inst = blast_at(Fx::Shock, 2, true);
        let (ring1, ring2) = (Pos::new(41, 30), Pos::new(42, 30));
        let (b, _) = at_e(&inst, 0);
        assert_eq!(
            cell(&b, L),
            ("◉".into(), col(CYAN_RGB), col(Rgb(0, 60, 75)), true)
        );
        let (b, _) = at_e(&inst, 1);
        assert_eq!(cell(&b, L).0, "○");
        assert_eq!(
            cell(&b, ring1),
            ("≋".into(), col(CYAN_RGB), col(Rgb(0, 40, 50)), true)
        );
        let (b, _) = at_e(&inst, 2);
        assert_eq!(
            cell(&b, ring2),
            (
                "≋".into(),
                col(Rgb(0, 180, 200)),
                col(Rgb(0, 30, 38)),
                false
            )
        );
        assert_eq!(
            glyph_fg(&b, ring1),
            ("·".to_string(), col(Rgb(0, 120, 140)))
        );
        let (b, _) = at_e(&inst, 3);
        assert_eq!(
            glyph_fg(&b, ring2),
            ("·".to_string(), col(Rgb(0, 120, 140)))
        );
        let (b, plain) = at_e(&inst, 4);
        assert_eq!(b, plain, "gone after four ticks");
    }

    #[test]
    fn trail_arrows_fade() {
        let pushed = |to| aimed(Fx::Trail, L, vec![to]);
        let inst = pushed(Pos::new(41, 30));
        for (e, c) in [
            (0, Rgb(255, 120, 120)),
            (1, Rgb(255, 120, 120)),
            (2, Rgb(180, 60, 60)),
            (3, Rgb(180, 60, 60)),
        ] {
            let (b, _) = at_e(&inst, e);
            assert_eq!(glyph_fg(&b, L), ("»".to_string(), col(c)), "e{e}");
        }
        let (b, plain) = at_e(&inst, 4);
        assert_eq!(b, plain);
        for (to, arrow) in [
            (Pos::new(39, 30), "«"),
            (Pos::new(40, 31), "∨"),
            (Pos::new(40, 29), "∧"),
            (Pos::new(41, 29), "»"),
            (Pos::new(43, 29), "»"),
            (Pos::new(41, 27), "∧"),
        ] {
            let (b, _) = at_e(&pushed(to), 0);
            assert_eq!(cell(&b, L).0, arrow, "{to:?}");
        }
    }

    #[test]
    fn pulse_rings_out() {
        let inst = aimed(
            Fx::Pulse {
                colour: (0, 229, 255),
            },
            L,
            Vec::new(),
        );
        let c = CYAN_RGB;
        let (b, plain) = at_e(&inst, 0);
        let (g, fg, bg, bold) = cell(&b, L);
        assert_eq!(g, cell(&plain, L).0);
        assert_eq!((fg, bg, bold), (col(Rgb(0, 0, 0)), col(c), true));
        let (b, _) = at_e(&inst, 1);
        for (dx, dy) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
        ] {
            assert_eq!(glyph_fg(&b, L.offset(dx, dy)), ("·".to_string(), col(c)));
        }
        assert_ne!(cell(&b, L.offset(2, 0)).0, "·");
        let (b, _) = at_e(&inst, 3);
        assert_eq!(
            glyph_fg(&b, L.offset(2, 0)),
            ("·".to_string(), col(c.scale(2.0 / 3.0)))
        );
        let (b, _) = at_e(&inst, 5);
        assert_eq!(
            glyph_fg(&b, L.offset(0, -3)),
            ("·".to_string(), col(c.scale(1.0 / 3.0)))
        );
        let (b, plain) = at_e(&inst, 7);
        assert_eq!(b, plain);
    }

    #[test]
    fn a_tinted_tag_sits_on_the_cyan_field() {
        let mut tag = shot(Fx::Number { tag: true }, FIRER, TARGET, "+ turn");
        tag.tint = Some((0, 229, 255));
        let (b, _) = at_e(&tag, 1);
        let (_, fg, bg, bold) = cell(&b, Pos::new(TARGET.x, TARGET.y - 1));
        assert_eq!((fg, bg, bold), (col(WHITE_RGB), col(Rgb(0, 90, 110)), true));
        let (b, _) = at_e(&tag, 11);
        let (_, fg, bg, _) = cell(&b, Pos::new(TARGET.x, TARGET.y - 3));
        assert_eq!((fg, bg), (col(Rgb(120, 170, 180)), col(Rgb(0, 40, 50))));
    }
}
