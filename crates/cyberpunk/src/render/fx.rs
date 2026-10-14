//! Ability effects over the map: glitch kicks, expanding wavefronts and glowing markers.

use crate::anim::effects::{glitch_cells, pulse};
use crate::app::FxInstance;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::render::{Rgb, Visibility};
use citygen::rng::hash64;
use citygen::{City, Grid, Pos, StratumKind};
use game::Fx;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use std::collections::HashSet;

/// Glyphs that ride a wavefront.
const SPARKS: &[char] = &['ｦ', 'ｱ', 'ﾂ', 'ﾈ', '╳', '≋'];
/// Cells behind the front that still carry a fading tint.
const TAIL: f32 = 3.0;
const KICK_CYAN: Color = Color::Rgb(0, 255, 230);
const KICK_MAGENTA: Color = Color::Rgb(255, 0, 200);
const BLACK: Color = Color::Rgb(0, 0, 0);

pub struct FxOverlay<'a> {
    pub fx: &'a [FxInstance],
    /// Sight on the drawn stratum: glow markers show on connectors out of sight.
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for FxOverlay<'_> {
    fn draw(
        &self,
        city: &City,
        stratum: StratumKind,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for inst in self
            .fx
            .iter()
            .filter(|i| i.stratum == stratum && i.started <= tick && tick < i.until())
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
                        city,
                        stratum,
                        origin,
                        area,
                        tick,
                        buf,
                    );
                }
            }
        }
    }
}

/// The ring: a bright front, a fading tail behind it, and the scan's revealed tiles
/// kept blank until the front reaches them.
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
    let r = inst.radius as f32 * elapsed as f32 / ticks.max(1) as f32;
    let hidden: HashSet<Pos> = inst
        .revealed
        .iter()
        .copied()
        .filter(|p| p.chebyshev(inst.origin) as f32 > r)
        .collect();
    for sy in area.y..area.bottom() {
        for sx in area.x..area.right() {
            let p = Pos::new(
                origin.x + (sx - area.x) as i32,
                origin.y + (sy - area.y) as i32,
            );
            let d = p.chebyshev(inst.origin);
            if d > inst.radius {
                continue;
            }
            let cell = &mut buf[(sx, sy)];
            if hidden.contains(&p) {
                cell.set_char(' ').set_fg(BLACK).set_bg(BLACK);
                cell.modifier.remove(Modifier::DIM);
                continue;
            }
            let df = d as f32;
            if df <= r && df > r - 1.0 {
                cell.set_fg(colour.to_color())
                    .set_bg(colour.scale(0.25).to_color());
                cell.modifier.remove(Modifier::DIM);
                if hash64(hash64(sx as u64, sy as u64), tick).is_multiple_of(6) {
                    let i = (hash64(tick, sx as u64 + sy as u64) % SPARKS.len() as u64) as usize;
                    cell.set_char(SPARKS[i]);
                }
            } else if df <= r - 1.0 && df > r - 1.0 - TAIL {
                let t = (r - df - 1.0) / TAIL;
                cell.set_fg(colour.scale(0.8 * (1.0 - t)).to_color());
            }
        }
    }
}

impl FxOverlay<'_> {
    /// Pulsing markers on the connectors in range; out of sight they get their own glyph.
    #[allow(clippy::too_many_arguments)]
    fn glow(
        &self,
        inst: &FxInstance,
        colour: Rgb,
        city: &City,
        stratum: StratumKind,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
    ) {
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
    use citygen::{City, GenParams, Generator, Pos, SizePreset, StratumKind};
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
                city: c,
                stratum,
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
            stratum: StratumKind::Street,
            origin,
            radius,
            started: 100,
            revealed,
        }
    }

    const CYAN: (u8, u8, u8) = (0, 229, 255);

    #[test]
    fn wave_front_colours_and_hides_unreached_tiles() {
        let c = city();
        let s = StylePack::load().unwrap();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&c, 0);
        let revealed = p.reveal(&c, 12);
        let origin = p.pos;
        let at_distance = |d: i32| {
            revealed
                .iter()
                .copied()
                .find(|q| q.chebyshev(origin) == d)
                .expect("a revealed tile at that distance")
        };
        let (front, beyond, behind) = (at_distance(6), at_distance(10), at_distance(4));
        let inst = instance(
            Fx::Wave {
                colour: CYAN,
                ticks: 18,
            },
            origin,
            12,
            revealed.clone(),
        );
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&inst),
            vis: Some(p.vis()),
        };
        // Tick 109: elapsed 9 of 18, the front is at distance 6.
        let buf = render(
            &c,
            &s,
            p.stratum,
            109,
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
        // A revealed tile the front has passed keeps the glyph the map draws for it.
        let plain = render(&c, &s, p.stratum, 109, Some(p.vis()), &[], (120, 60));
        let shown = plain[(behind.x as u16, behind.y as u16)].clone();
        assert_eq!(
            cell(behind).symbol(),
            shown.symbol(),
            "behind the front: not hidden"
        );
        // Tick 200: the wave is over and the map is untouched.
        let plain = render(&c, &s, p.stratum, 200, Some(p.vis()), &[], (120, 60));
        let after = render(
            &c,
            &s,
            p.stratum,
            200,
            Some(p.vis()),
            &[&overlay],
            (120, 60),
        );
        assert_eq!(after, plain);
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
        p.update_fov(&c, 0);
        let k = c
            .connectors
            .iter()
            .find(|k| {
                k.serves.contains(&p.stratum)
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
            p.stratum,
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
        let mut inst = instance(Fx::Kick(4), Pos::new(10, 10), 0, Vec::new());
        inst.stratum = StratumKind::Net;
        let overlay = FxOverlay {
            fx: std::slice::from_ref(&inst),
            vis: None,
        };
        let plain = render(&c, &s, StratumKind::Street, 100, None, &[], (120, 60));
        let drawn = render(
            &c,
            &s,
            StratumKind::Street,
            100,
            None,
            &[&overlay],
            (120, 60),
        );
        assert_eq!(drawn, plain);
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
}
