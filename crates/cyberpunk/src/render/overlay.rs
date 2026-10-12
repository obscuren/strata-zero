//! The digital overlay: Net nodes, subnet outlines and links drawn over the physical map.

use crate::anim::effects::pulse;
use crate::render::Rgb;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use citygen::pathing::bresenham;
use citygen::{City, NodeId, NodeKind, Pos, StratumKind, SubnetOwner};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Signal cyan used for everything digital.
pub const SIGNAL: Rgb = Rgb(0, 255, 200);

/// Net nodes as wireframe glyphs; the focused node's subnet lights up with its links.
pub struct Digital {
    pub enabled: bool,
    pub focus: Option<NodeId>,
}

impl Digital {
    /// Index into the style's `net` glyph set for a node kind.
    fn glyph_index(kind: NodeKind) -> usize {
        match kind {
            NodeKind::Terminal => 0,
            NodeKind::Server => 1,
            NodeKind::Backbone => 2,
            NodeKind::DataVault => 3,
            NodeKind::Ice => 5,
            _ => 4,
        }
    }

    pub fn node_under(city: &City, stratum: StratumKind, p: Pos) -> Option<NodeId> {
        city.net.node_at(stratum, p).map(|n| n.id)
    }
}

impl Overlay for Digital {
    fn draw(
        &self,
        city: &City,
        stratum: StratumKind,
        origin: Pos,
        area: Rect,
        tick: u64,
        buf: &mut Buffer,
        style: &StylePack,
    ) {
        if !self.enabled {
            return;
        }
        let glyphs = &style.glyphs.net;
        let focus_subnet = self.focus.map(|id| city.net.node(id).subnet);

        // Faint outline on every building that owns a subnet.
        for sn in &city.net.subnets {
            let SubnetOwner::Building(b) = sn.owner else {
                continue;
            };
            let building = city.building(b);
            if !building.span.contains(stratum) {
                continue;
            }
            let hot = focus_subnet == Some(sn.id);
            for p in building.footprint.border() {
                if let Some((sx, sy)) = tile_to_screen(area, origin, p) {
                    let c = &mut buf[(sx, sy)];
                    let bg = match c.bg {
                        Color::Rgb(r, g, b) => Rgb(r, g, b),
                        _ => Rgb(0, 0, 0),
                    };
                    let strength = if hot { 0.5 } else { 0.18 };
                    c.set_bg(bg.mix(SIGNAL, strength).to_color());
                }
            }
        }

        // Links of the focused subnet, as dotted lines between anchors on this stratum.
        if let Some(sn) = focus_subnet {
            for link in &city.net.links {
                let a = city.net.node(link.0);
                let b = city.net.node(link.1);
                if a.subnet != sn && b.subnet != sn {
                    continue;
                }
                if a.anchor.0 != stratum || b.anchor.0 != stratum {
                    continue;
                }
                for p in bresenham(a.anchor.1, b.anchor.1) {
                    if city.net.node_at(stratum, p).is_some() {
                        continue;
                    }
                    if let Some((sx, sy)) = tile_to_screen(area, origin, p) {
                        buf[(sx, sy)]
                            .set_char('·')
                            .set_fg(SIGNAL.scale(0.8).to_color());
                    }
                }
            }
        }

        let glow = 0.6 + 0.4 * pulse(tick, 40);
        for n in city.net.nodes_on(stratum) {
            let Some((sx, sy)) = tile_to_screen(area, origin, n.anchor.1) else {
                continue;
            };
            let g = glyphs[Self::glyph_index(n.kind) % glyphs.len()];
            let hot = focus_subnet == Some(n.subnet);
            let fg = if hot {
                Rgb(255, 255, 255)
            } else {
                SIGNAL.scale(glow)
            };
            buf[(sx, sy)].set_char(g).set_fg(fg.to_color());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::style::StylePack;
    use crate::render::viewport::MapView;
    use citygen::{GenParams, Generator, NodeKind, Pos, SizePreset, StratumKind};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Widget;

    #[test]
    fn overlay_draws_nodes_on_current_stratum_only() {
        let c = Generator::run(GenParams {
            seed: 8,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let s = StylePack::load().unwrap();
        // A server that is alone on its tile (backbones may share a server's anchor).
        let node = c
            .net
            .nodes
            .iter()
            .find(|n| {
                n.kind == NodeKind::Server
                    && c.net.nodes.iter().filter(|m| m.anchor == n.anchor).count() == 1
            })
            .unwrap();
        let (k, p) = node.anchor;
        let mut t = Terminal::new(TestBackend::new(80, 40)).unwrap();
        let draw = |t: &mut Terminal<TestBackend>, stratum, focus| {
            t.draw(|f| {
                MapView {
                    city: &c,
                    stratum,
                    origin: Pos::new(0, 0),
                    tick: 0,
                    style: &s,
                    vis: None,
                    overlays: &[&Digital {
                        enabled: true,
                        focus,
                    }],
                    cursor: None,
                }
                .render(f.area(), f.buffer_mut())
            })
            .unwrap();
        };
        draw(&mut t, k, None);
        assert_eq!(t.backend().buffer()[(p.x as u16, p.y as u16)].symbol(), "◆");
        let other = StratumKind::PHYSICAL
            .iter()
            .copied()
            .find(|s| *s != k)
            .unwrap();
        draw(&mut t, other, None);
        assert_ne!(t.backend().buffer()[(p.x as u16, p.y as u16)].symbol(), "◆");
        draw(&mut t, k, Some(node.id));
        assert_eq!(Digital::node_under(&c, k, p), Some(node.id));
        assert_eq!(Digital::node_under(&c, k, Pos::new(-1, -1)), None);
        let off = Digital {
            enabled: false,
            focus: None,
        };
        t.draw(|f| {
            MapView {
                city: &c,
                stratum: k,
                origin: Pos::new(0, 0),
                tick: 0,
                style: &s,
                vis: None,
                overlays: &[&off],
                cursor: None,
            }
            .render(f.area(), f.buffer_mut())
        })
        .unwrap();
        assert_ne!(t.backend().buffer()[(p.x as u16, p.y as u16)].symbol(), "◆");
    }
}
