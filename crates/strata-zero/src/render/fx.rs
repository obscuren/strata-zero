//! Ability effects over the map: glitch kicks, expanding wavefronts and glowing markers.

use crate::anim::effects::{glitch_cells, pulse};
use crate::app::FxInstance;
use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::render::{Rgb, Visibility};
use citygen::rng::hash64;
use citygen::{Grid, Pos};
use game::Fx;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use std::collections::HashSet;

/// Glyphs that ride a wavefront.
const SPARKS: &[char] = &['ｦ', 'ｱ', 'ﾂ', 'ﾈ', '╳', '≋'];
/// Cells of ripple behind the front, front included.
pub const BAND: f32 = 9.0;
/// Cells from one ripple crest to the next.
const WAVELENGTH: f32 = 3.0;
/// The deeper blue alternate crests lean toward.
const DEEP: Rgb = Rgb(60, 110, 255);
const KICK_CYAN: Color = Color::Rgb(0, 255, 230);
const KICK_MAGENTA: Color = Color::Rgb(255, 0, 200);
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
        for inst in self
            .fx
            .iter()
            .filter(|i| i.place == map.place() && i.started <= tick && tick < i.until())
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
        // Connectors live on the city's strata; a floor's lifts get no marker yet.
        let Some(stratum) = map.stratum() else {
            return;
        };
        let city = map.city();
        let glow = colour.scale(0.5 + 0.5 * pulse(tick, 20)).to_color();
        for k in city
            .connectors
            .iter()
            .filter(|k| k.serves.contains(&stratum) && k.pos.chebyshev(inst.origin) <= inst.radius)
        {
            let Some((sx, sy)) = tile_to_screen(area, origin, k.pos) else {
                continue;
            };
            let in_sight = match self.vis {
                None => true,
                Some(v) => v.get(k.pos).copied() == Some(Visibility::Visible),
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
    use super::*;
    use crate::render::style::StylePack;
    use crate::render::viewport::{MapView, Overlay};
    use crate::walker::Player;
    use citygen::{City, GenParams, Generator, Place, Pos, SizePreset, StratumKind};
    use game::Fx;
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
        }
    }

    fn aimed(fx: Fx, origin: Pos, targets: Vec<Pos>) -> FxInstance {
        let mut i = instance(fx, origin, 0, Vec::new());
        i.targets = targets;
        i
    }

    const BOLT: (u8, u8, u8) = (255, 240, 150);

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
}
