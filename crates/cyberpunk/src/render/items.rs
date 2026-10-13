//! Items in the world: piles on the ground and the ping that lights terminals.

use crate::anim::effects::pulse;
use crate::app::Ping;
use crate::render::glyphs::kind_icon;
use crate::render::overlay::SIGNAL;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::render::{Rgb, Visibility};
use citygen::{City, Grid, Pos, StratumKind};
use game::{Content, Ground, ItemKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

/// The colour of things lying on the ground.
pub const AMBER: Rgb = Rgb(255, 200, 80);

/// Draws one glyph per pile on the drawn stratum: the kind's icon, or `≡` for a
/// mixed pile. Hidden on unseen tiles, dimmed on remembered ones; with no
/// visibility grid (the free camera) every pile shows.
pub struct GroundOverlay<'a> {
    pub ground: &'a Ground,
    pub content: &'a Content,
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for GroundOverlay<'_> {
    fn draw(
        &self,
        _city: &City,
        stratum: StratumKind,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for (pos, stacks) in self.ground.on(stratum) {
            let Some((sx, sy)) = tile_to_screen(area, origin, pos) else {
                continue;
            };
            let seen = match self.vis {
                None => Visibility::Visible,
                Some(v) => v.get(pos).copied().unwrap_or(Visibility::Unseen),
            };
            let strength = match seen {
                Visibility::Visible => 1.0,
                Visibility::Remembered => 0.45,
                Visibility::Unseen => continue,
            };
            let kinds: Vec<ItemKind> = stacks
                .iter()
                .filter_map(|s| self.content.item(&s.item).map(|i| i.kind))
                .collect();
            let glyph = match kinds.first() {
                Some(k) if kinds.iter().all(|x| x == k) => kind_icon(*k),
                _ => '≡',
            };
            buf[(sx, sy)]
                .set_char(glyph)
                .set_fg(AMBER.scale(strength).to_color());
        }
    }
}

/// A ping in flight: every connector serving the drawn stratum within its radius
/// pulses in the signal colour until it expires.
pub struct PingOverlay<'a> {
    pub ping: Option<&'a Ping>,
    /// Sight on the drawn stratum: connectors out of sight get a marker glyph.
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for PingOverlay<'_> {
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
        let Some(ping) = self.ping else { return };
        if tick >= ping.until || stratum != ping.stratum {
            return;
        }
        let glow = SIGNAL.scale(0.5 + 0.5 * pulse(tick, 20)).to_color();
        for k in city
            .connectors
            .iter()
            .filter(|k| k.serves.contains(&stratum) && k.pos.chebyshev(ping.pos) <= ping.radius)
        {
            let Some((sx, sy)) = tile_to_screen(area, origin, k.pos) else {
                continue;
            };
            let in_sight = match self.vis {
                None => true,
                Some(v) => v.get(k.pos).copied() == Some(Visibility::Visible),
            };
            let cell = &mut buf[(sx, sy)];
            // Out of sight the tile is blank or dimmed: the ping draws its own marker.
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
    use crate::render::viewport::MapView;
    use crate::walker::Player;
    use citygen::{ConnectorKind, GenParams, Generator, SizePreset};
    use game::ItemId;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
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
        overlay: &dyn Overlay,
    ) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(120, 60)).unwrap();
        t.draw(|f| {
            MapView {
                city: c,
                stratum,
                origin: Pos::new(0, 0),
                tick,
                style: s,
                vis,
                overlays: &[overlay],
                cursor: None,
            }
            .render(f.area(), f.buffer_mut())
        })
        .unwrap();
        t.backend().buffer().clone()
    }

    #[test]
    fn ground_overlay_draws_piles_by_sight() {
        let c = city();
        let content = Content::load().unwrap();
        let s = StylePack::load().unwrap();
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&c, 0);
        let here = p.pos;
        let near = here
            .neighbors8()
            .find(|q| c.walkable(p.stratum, *q))
            .unwrap();
        let far = (0..60)
            .flat_map(|y| (0..120).map(move |x| Pos::new(x, y)))
            .find(|q| p.vis()[*q] == Visibility::Unseen && c.walkable(p.stratum, *q))
            .unwrap();
        let mut g = Ground::default();
        g.add(p.stratum, here, ItemId::new("copper_coil"), 2);
        g.add(p.stratum, near, ItemId::new("copper_coil"), 1);
        g.add(p.stratum, near, ItemId::new("dead_phone"), 1);
        g.add(p.stratum, far, ItemId::new("dead_phone"), 1);
        let at = |buf: &Buffer, q: Pos| buf[(q.x as u16, q.y as u16)].clone();
        let overlay = |vis| GroundOverlay {
            ground: &g,
            content: &content,
            vis,
        };
        let buf = render(&c, &s, p.stratum, 0, Some(p.vis()), &overlay(Some(p.vis())));
        assert_eq!(at(&buf, here).symbol(), "▪");
        assert_eq!(at(&buf, here).fg, AMBER.to_color());
        assert_eq!(at(&buf, near).symbol(), "≡");
        assert_ne!(
            at(&buf, far).fg,
            AMBER.to_color(),
            "unseen piles stay hidden"
        );
        let mut away = p.clone();
        away.pos = far;
        away.update_fov(&c, 0);
        let buf = render(
            &c,
            &s,
            p.stratum,
            0,
            Some(away.vis()),
            &overlay(Some(away.vis())),
        );
        assert_eq!(
            at(&buf, here).fg,
            AMBER.scale(0.45).to_color(),
            "remembered: dimmed"
        );
        let buf = render(&c, &s, p.stratum, 0, None, &overlay(None));
        assert_eq!(
            at(&buf, far).fg,
            AMBER.to_color(),
            "free camera sees everything"
        );
        // A stratum without piles never panics.
        render(&c, &s, StratumKind::Net, 0, None, &overlay(None));
    }

    #[test]
    fn ping_overlay_lights_connectors_in_range() {
        let c = city();
        let s = StylePack::load().unwrap();
        let street = StratumKind::Street;
        let jack = c
            .connectors
            .iter()
            .find(|k| k.kind == ConnectorKind::Jack && k.serves.contains(&street))
            .unwrap()
            .pos;
        let other = c
            .connectors
            .iter()
            .find(|k| k.serves.contains(&street) && k.pos.chebyshev(jack) > 10)
            .unwrap()
            .pos;
        let ping = Ping {
            stratum: street,
            pos: jack,
            radius: 3,
            until: 100,
        };
        let glow = |tick| SIGNAL.scale(0.5 + 0.5 * pulse(tick, 20)).to_color();
        let buf = render(
            &c,
            &s,
            street,
            10,
            None,
            &PingOverlay {
                ping: Some(&ping),
                vis: None,
            },
        );
        assert_eq!(buf[(jack.x as u16, jack.y as u16)].fg, glow(10));
        assert_ne!(buf[(other.x as u16, other.y as u16)].fg, glow(10));
        let buf = render(
            &c,
            &s,
            street,
            100,
            None,
            &PingOverlay {
                ping: Some(&ping),
                vis: None,
            },
        );
        assert_ne!(buf[(jack.x as u16, jack.y as u16)].fg, glow(100), "expired");
        let buf = render(
            &c,
            &s,
            StratumKind::Undercity,
            10,
            None,
            &PingOverlay {
                ping: Some(&ping),
                vis: None,
            },
        );
        assert_ne!(
            buf[(jack.x as u16, jack.y as u16)].fg,
            glow(10),
            "other stratum"
        );
    }

    #[test]
    fn ping_overlay_marks_connectors_out_of_sight() {
        let c = city();
        let s = StylePack::load().unwrap();
        let street = StratumKind::Street;
        let mut p = Player::spawn(&c, 1);
        p.update_fov(&c, 0);
        let hidden = c
            .connectors
            .iter()
            .find(|k| {
                k.serves.contains(&street)
                    && k.pos.chebyshev(p.pos) <= 24
                    && p.vis()[k.pos] == Visibility::Unseen
            })
            .expect("a connector in ping range but out of sight")
            .pos;
        let ping = Ping {
            stratum: street,
            pos: p.pos,
            radius: 24,
            until: 100,
        };
        let overlay = PingOverlay {
            ping: Some(&ping),
            vis: Some(p.vis()),
        };
        let buf = render(&c, &s, street, 10, Some(p.vis()), &overlay);
        let cell = &buf[(hidden.x as u16, hidden.y as u16)];
        let glow = SIGNAL.scale(0.5 + 0.5 * pulse(10, 20)).to_color();
        assert_eq!(cell.fg, glow);
        assert_ne!(
            cell.symbol(),
            " ",
            "a marker shows where sight does not reach"
        );
        assert!(!cell.modifier.contains(ratatui::style::Modifier::DIM));
    }
}
