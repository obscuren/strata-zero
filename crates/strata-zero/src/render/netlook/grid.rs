//! The open Net in relief: a dark void lit only near data, lanes as beams with
//! packets on them, each construct stepped by its tier and named on its plate, hubs
//! ringed.

use super::plate::{self, Ring};
use super::{
    AMBER, NetArea, NetLookView, Tier, VOID, WHITE, label_for, node_colour, put, security_of,
};
use crate::render::Rgb;
use crate::render::map::TileMap;
use crate::render::overlay::SIGNAL;
use crate::render::style::StylePack;
use crate::render::tileset::{DOUBLE, LIGHT, node_glyph};
use crate::render::viewport::screen_to_tile;
use citygen::{BuildingId, Feature, NodeKind, Pos, Rect as TileRect, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::HashMap;

/// A hub's rings of dots in the void: radius (a cell counted twice as tall as wide)
/// and brightness.
const HUB_RINGS: [(f32, f32); 3] = [(5.0, 0.8), (9.0, 0.45), (13.0, 0.22)];

/// A construct in view.
struct Built {
    tier: Tier,
    security: Option<u8>,
    steps: i32,
    footprint: TileRect,
    colour: Rgb,
}

fn is_data(t: Terrain) -> bool {
    matches!(
        t,
        Terrain::NetLane | Terrain::NetWall | Terrain::NetGate | Terrain::NetFloor
    )
}

/// How far the nearest lane or construct is: 1 or 2, else 3.
pub fn near_data(map: &dyn TileMap, p: Pos) -> i32 {
    for r in 1..=2i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) == r
                    && map
                        .tile(p.offset(dx, dy))
                        .is_some_and(|t| is_data(t.terrain))
                {
                    return r;
                }
            }
        }
    }
    3
}

/// Bits N=1, E=2, S=4, W=8 for the neighbours `joins` accepts.
fn mask(p: Pos, joins: impl Fn(Pos) -> bool) -> usize {
    [(0, -1, 1), (1, 0, 2), (0, 1, 4), (-1, 0, 8)]
        .into_iter()
        .filter(|(dx, dy, _)| joins(p.offset(*dx, *dy)))
        .map(|(_, _, bit)| bit)
        .sum()
}

/// The Net of `net` drawn over `area` from tile `origin` of its map's frame: its own
/// city's constructs and hubs (none on the open Net), and only the cells it keeps.
pub(super) fn draw(
    net: NetArea,
    view: &NetLookView,
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
    style: &StylePack,
) {
    let NetArea { map, city, keep } = net;
    let tiles = TileRect::new(origin.x, origin.y, area.width as i32, area.height as i32);
    let mut built: HashMap<BuildingId, Built> = HashMap::new();
    let mut labels: HashMap<Pos, char> = HashMap::new();
    let buildings = city
        .into_iter()
        .flat_map(|c| c.buildings.iter().map(move |b| (c, b)));
    for (city, b) in buildings.filter(|(_, b)| b.footprint.intersects(tiles)) {
        let security = security_of(city, b.id);
        let tier = Tier::of(security.unwrap_or(0));
        let steps = plate::steps_in(b.footprint, tier.steps());
        let colour = if view.tracked == Some(b.id) {
            AMBER
        } else {
            tier.colour()
        };
        let has_node = |p: Pos| {
            map.tile(p)
                .is_some_and(|t| matches!(t.feature, Some(Feature::Node(_))))
        };
        for (p, ch) in plate::layout(
            plate::plate(b.footprint, steps),
            &label_for(city, b.id, view.home),
            has_node,
        ) {
            labels.insert(p, ch);
        }
        built.insert(
            b.id,
            Built {
                tier,
                security,
                steps,
                footprint: b.footprint,
                colour,
            },
        );
    }
    let hubs: Vec<Pos> = city
        .iter()
        .flat_map(|c| c.net.nodes.iter())
        .filter(|n| n.kind == NodeKind::Backbone)
        .map(|n| n.cyber)
        .filter(|h| tiles.grow(14).contains(*h))
        .collect();
    let theme =
        |d: Option<citygen::DistrictId>| d.and_then(|d| city?.district_opt(d)).map(|d| d.theme);
    let glows = &view.packets;
    for sy in area.y..area.bottom() {
        for sx in area.x..area.right() {
            let p = screen_to_tile(area, origin, (sx, sy));
            if keep.is_some_and(|keep| !keep(p)) {
                continue;
            }
            let Some(tile) = map.tile(p) else {
                put(buf, sx, sy, ' ', VOID, VOID, false);
                continue;
            };
            let owner = tile.building.and_then(|b| built.get(&b));
            let (mut ch, mut fg, mut bg, mut bold) = match (tile.terrain, owner) {
                (Terrain::NetLane, _) => {
                    let m = mask(p, |q| {
                        map.tile(q).is_some_and(|t| {
                            matches!(t.terrain, Terrain::NetLane | Terrain::NetGate)
                        })
                    });
                    let base = VOID.mix(SIGNAL, 0.22);
                    match glows.get(&p) {
                        Some((g, true)) => {
                            (DOUBLE[m], Rgb(255, 255, 255), base.mix(SIGNAL, *g), true)
                        }
                        Some((g, false)) => (
                            DOUBLE[m],
                            SIGNAL.mix(WHITE, 0.25),
                            base.mix(SIGNAL, *g),
                            false,
                        ),
                        None => (DOUBLE[m], SIGNAL.mix(WHITE, 0.25), base, false),
                    }
                }
                (Terrain::NetWall, Some(b)) => {
                    let m = mask(p, |q| {
                        map.tile(q).is_some_and(|t| {
                            matches!(t.terrain, Terrain::NetWall | Terrain::NetGate)
                                && t.building == tile.building
                        })
                    });
                    (
                        b.tier.walls()[m],
                        b.colour.scale(0.85),
                        VOID.mix(b.colour, 0.06),
                        false,
                    )
                }
                (Terrain::NetGate, Some(b)) => ('▯', WHITE, VOID.mix(b.colour, 0.3), true),
                (Terrain::NetFloor, Some(b)) => floor_cell(b, p, labels.get(&p).copied()),
                // A hub, or anything of the Net no building owns.
                (Terrain::NetWall, None) => {
                    let m = mask(p, |q| {
                        map.tile(q).is_some_and(|t| {
                            matches!(t.terrain, Terrain::NetWall | Terrain::NetGate)
                                && t.building.is_none()
                        })
                    });
                    (DOUBLE[m], WHITE, VOID.mix(WHITE, 0.12), false)
                }
                (Terrain::NetGate, None) => ('▯', WHITE, VOID.mix(WHITE, 0.12), true),
                // the void (the open Net past a city's outline): no lattice, never walked
                (Terrain::Void, _) => void_cell(
                    map,
                    p,
                    tile.variant & !1,
                    theme(tile.district),
                    &hubs,
                    style,
                ),
                _ => void_cell(map, p, tile.variant, theme(tile.district), &hubs, style),
            };
            if let Some(Feature::Node(kind)) = tile.feature {
                ch = node_glyph(kind, style);
                fg = node_colour(
                    kind,
                    owner.map_or(WHITE, |b| b.colour),
                    owner.and_then(|b| b.security),
                    view.tick,
                );
                bold = matches!(kind, NodeKind::Ice | NodeKind::Server | NodeKind::DataVault);
                // Soaking up a packet: it flashes in the traffic's signal.
                if let Some(g) = view.soaks.get(&p) {
                    bg = bg.mix(SIGNAL, *g);
                    fg = fg.mix(WHITE, *g);
                    bold = true;
                }
            }
            put(buf, sx, sy, ch, fg, bg, bold);
        }
    }
}

/// A floor tile of a construct seen from outside: on a ring, a light line brighter the
/// higher it steps; on the plate, the brightest fill and the label.
fn floor_cell(b: &Built, p: Pos, label: Option<char>) -> (char, Rgb, Rgb, bool) {
    let c = b.colour;
    let fill = |k: i32| VOID.mix(c, 0.05 + 0.13 * k as f32);
    match plate::ring_of(b.footprint, b.steps, p) {
        Ring::Line(k) => {
            let on_ring = |q: Pos| b.footprint.contains(q) && plate::inset(b.footprint, q) == k;
            let m = mask(p, on_ring);
            let lift = k as f32 / (b.steps + 1) as f32;
            (LIGHT[m], c.scale(0.45).mix(c, lift), fill(k), false)
        }
        Ring::Plate => {
            let bg = VOID.mix(c, 0.05 + 0.13 * (b.steps + 1) as f32 + 0.08);
            match label {
                Some(ch) => (ch, c.mix(WHITE, 0.8), bg, true),
                None => (' ', c, bg, false),
            }
        }
        Ring::Edge => (' ', c, fill(0), false),
    }
}

/// The open grid: dark, its lattice dots only near data, rings round the hubs.
fn void_cell(
    map: &dyn TileMap,
    p: Pos,
    variant: u8,
    theme: Option<citygen::Theme>,
    hubs: &[Pos],
    style: &StylePack,
) -> (char, Rgb, Rgb, bool) {
    let near = near_data(map, p);
    let accent = style.theme_or_neutral(theme).accent;
    let bg = match near {
        1 => VOID.mix(SIGNAL, 0.09),
        2 => VOID.mix(SIGNAL, 0.03),
        _ => VOID,
    };
    let mut ch = if variant & 1 == 1 && near <= 2 {
        '·'
    } else {
        ' '
    };
    let mut fg = accent
        .mix(SIGNAL, 0.4)
        .scale(if near == 1 { 0.5 } else { 0.3 });
    for h in hubs {
        let r = ((p.x - h.x) as f32).hypot(((p.y - h.y) * 2) as f32);
        if let Some((_, f)) = HUB_RINGS
            .iter()
            .find(|(radius, _)| (r - radius).abs() < 0.5)
        {
            ch = '·';
            fg = WHITE.scale(*f);
        }
    }
    (ch, fg, bg, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::map::MapRef;
    use crate::render::netlook::{NetLook, TERMINAL};
    use crate::render::viewport::Overlay;
    use citygen::{City, GenParams, Generator, SizePreset, StratumKind};
    use ratatui::buffer::Buffer;

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap()
    }

    /// The whole Net drawn with the look, one cell a tile.
    fn paint(c: &City, view: &NetLookView) -> Buffer {
        let style = StylePack::load().unwrap();
        let map = MapRef::stratum_of(c, StratumKind::Net);
        let area = Rect::new(0, 0, c.size.w as u16, c.size.h as u16);
        let mut buf = Buffer::empty(area);
        NetLook { view, keep: None }.draw(&map, Pos::new(0, 0), area, 0, &mut buf, &style);
        buf
    }

    fn net_tiles(c: &City) -> impl Iterator<Item = (Pos, &citygen::Tile)> {
        let s = c.stratum(StratumKind::Net).unwrap();
        TileRect::new(0, 0, c.size.w as i32, c.size.h as i32)
            .iter()
            .map(move |p| (p, &s.tiles[p]))
    }

    fn fg(buf: &Buffer, p: Pos) -> ratatui::style::Color {
        buf[(p.x as u16, p.y as u16)].fg
    }

    #[test]
    fn the_void_shows_dots_only_near_data() {
        let c = city();
        let map = MapRef::stratum_of(&c, StratumKind::Net);
        let buf = paint(&c, &NetLookView::default());
        let dot = |p: Pos| buf[(p.x as u16, p.y as u16)].symbol() == "·";
        let lattice = |t: &citygen::Tile| t.terrain == Terrain::NetGrid && t.variant & 1 == 1;
        let near = net_tiles(&c)
            .find(|(p, t)| lattice(t) && near_data(&map, *p) == 1)
            .expect("a lattice dot beside data");
        assert!(dot(near.0));
        let far = net_tiles(&c)
            .find(|(p, t)| {
                lattice(t)
                    && near_data(&map, *p) == 3
                    && !c.net.nodes.iter().any(|n| {
                        n.kind == NodeKind::Backbone
                            && ((p.x - n.cyber.x) as f32).hypot(((p.y - n.cyber.y) * 2) as f32)
                                < 14.0
                    })
            })
            .expect("a lattice tile far from data and hubs");
        assert!(!dot(far.0));
        assert_eq!(
            buf[(far.0.x as u16, far.0.y as u16)].bg,
            VOID.to_color(),
            "the void is dark"
        );
    }

    #[test]
    fn a_void_cell_is_never_a_dot() {
        // the open Net past a city's outline holds void: walking there is barred, so it
        // draws as nothing even beside data, whatever its variant
        let mut c = city();
        let map0 = MapRef::stratum_of(&c, StratumKind::Net);
        let at = net_tiles(&c)
            .find(|(p, t)| t.terrain == Terrain::NetGrid && near_data(&map0, *p) == 1)
            .map(|(p, _)| p)
            .expect("a grid tile beside data");
        let t = c
            .stratum_mut(StratumKind::Net)
            .unwrap()
            .tiles
            .get_mut(at)
            .unwrap();
        t.terrain = Terrain::Void;
        t.variant |= 1;
        let buf = paint(&c, &NetLookView::default());
        assert_eq!(buf[(at.x as u16, at.y as u16)].symbol(), " ");
    }

    #[test]
    fn terminals_draw_in_their_own_colour() {
        let c = city();
        let buf = paint(&c, &NetLookView::default());
        let of = |k: NodeKind| {
            net_tiles(&c)
                .find(|(_, t)| t.feature == Some(Feature::Node(k)))
                .map(|(p, _)| p)
        };
        let term = of(NodeKind::Terminal).expect("a terminal");
        assert_eq!(fg(&buf, term), TERMINAL.to_color());
        if let Some(lock) = of(NodeKind::DoorLock) {
            assert_ne!(fg(&buf, lock), TERMINAL.to_color());
        }
    }

    #[test]
    fn walls_say_the_tier_and_the_tracked_construct_is_amber() {
        let c = city();
        let (p, b) = net_tiles(&c)
            .find(|(_, t)| t.terrain == Terrain::NetWall && t.building.is_some())
            .map(|(p, t)| (p, t.building.unwrap()))
            .expect("a construct wall");
        let tier = Tier::of(security_of(&c, b).unwrap_or(0));
        let plain = paint(&c, &NetLookView::default());
        assert_eq!(fg(&plain, p), tier.colour().scale(0.85).to_color());
        assert!(
            tier.walls().contains(
                &plain[(p.x as u16, p.y as u16)]
                    .symbol()
                    .chars()
                    .next()
                    .unwrap()
            )
        );
        let tracked = paint(
            &c,
            &NetLookView {
                tracked: Some(b),
                ..Default::default()
            },
        );
        assert_eq!(fg(&tracked, p), AMBER.scale(0.85).to_color());
    }

    #[test]
    fn a_construct_half_off_screen_keeps_its_plate() {
        let c = city();
        let style = StylePack::load().unwrap();
        let map = MapRef::stratum_of(&c, StratumKind::Net);
        let b = c
            .buildings
            .iter()
            .filter(|b| security_of(&c, b.id).is_some())
            .max_by_key(|b| b.footprint.w * b.footprint.h)
            .unwrap();
        let fp = b.footprint;
        let view = NetLookView::default();
        // Whole, then with the camera so its left half is off the area.
        let whole_area = Rect::new(0, 0, c.size.w as u16, c.size.h as u16);
        let mut whole = Buffer::empty(whole_area);
        NetLook {
            view: &view,
            keep: None,
        }
        .draw(&map, Pos::new(0, 0), whole_area, 0, &mut whole, &style);
        let origin = Pos::new(fp.x + fp.w / 2, fp.y - 2);
        let area = Rect::new(0, 0, 40, (fp.h + 4) as u16);
        let mut half = Buffer::empty(area);
        NetLook {
            view: &view,
            keep: None,
        }
        .draw(&map, origin, area, 0, &mut half, &style);
        for p in fp.iter().filter(|p| p.x >= origin.x && p.x < origin.x + 40) {
            let (sx, sy) = ((p.x - origin.x) as u16, (p.y - origin.y) as u16);
            assert_eq!(
                half[(sx, sy)].symbol(),
                whole[(p.x as u16, p.y as u16)].symbol(),
                "{p:?}"
            );
        }
    }

    #[test]
    fn the_look_draws_only_the_cells_it_keeps() {
        // where other areas share the view, the city's look leaves their cells alone,
        // off its map too
        let c = city();
        let style = StylePack::load().unwrap();
        let map = MapRef::stratum_of(&c, StratumKind::Net);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        for cell in buf.content.iter_mut() {
            cell.set_char('x');
        }
        let view = NetLookView::default();
        let keep = |p: Pos| p.x >= 0 && p.x < 10;
        NetLook {
            view: &view,
            keep: Some(&keep),
        }
        .draw(&map, Pos::new(-20, 0), area, 0, &mut buf, &style);
        for x in 0..40u16 {
            let kept = (20..30).contains(&x);
            assert_eq!(buf[(x, 3)].symbol() == "x", !kept, "{x}");
        }
        // with no city (the open Net) no building's id is read: a construct's wall
        // draws as nobody's, not in its tier's colour
        let (wall, b) = net_tiles(&c)
            .find(|(_, t)| t.terrain == Terrain::NetWall && t.building.is_some())
            .map(|(p, t)| (p, t.building.unwrap()))
            .expect("a construct wall");
        let fg_at = |city: Option<&City>| {
            let mut out = Buffer::empty(Rect::new(0, 0, c.size.w as u16, c.size.h as u16));
            let net = NetArea {
                map: &map,
                city,
                keep: None,
            };
            super::super::draw_area(net, &view, Pos::new(0, 0), out.area, &mut out, &style);
            fg(&out, wall)
        };
        let tier = Tier::of(security_of(&c, b).unwrap_or(0));
        assert_eq!(fg_at(Some(&c)), tier.colour().scale(0.85).to_color());
        assert_eq!(fg_at(None), WHITE.to_color());
    }

    #[test]
    fn off_the_net_is_void() {
        let c = city();
        let style = StylePack::load().unwrap();
        let map = MapRef::stratum_of(&c, StratumKind::Net);
        let area = Rect::new(0, 0, 10, 4);
        let mut buf = Buffer::empty(area);
        let view = NetLookView::default();
        NetLook {
            view: &view,
            keep: None,
        }
        .draw(&map, Pos::new(-20, -20), area, 0, &mut buf, &style);
        assert_eq!(buf[(3, 2)].bg, VOID.to_color());
    }
}
