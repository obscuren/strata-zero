//! Inside a construct: its body in its tier's colour, the rooms carved out of it and
//! lit by what they hold (the vault amber), the spine a beam, the ICE in its own
//! colours and the stretch of spine each piece watches tinted.

use super::{AMBER, NetLookView, ROOM, TERMINAL, Tier, VOID, WHITE, ice_colour, node_colour, put};
use crate::render::Rgb;
use crate::render::map::ConstructMap;
use crate::render::overlay::SIGNAL;
use crate::render::style::StylePack;
use crate::render::tileset::{DOUBLE, LIGHT, node_glyph};
use crate::render::viewport::screen_to_tile;
use citygen::{Construct, Feature, IceGrade, NodeKind, Pos, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// The lattice left in the void round a construct.
const LATTICE: Rgb = Rgb(13, 42, 36);

/// A piece's colour for its watch, without the pulse, and how strongly it tints.
pub fn watch_tint(grade: IceGrade) -> (Rgb, f32) {
    match grade {
        IceGrade::White => (Rgb(235, 240, 250), 0.25),
        IceGrade::Grey => (Rgb(255, 0, 200), 0.30),
        IceGrade::Black => (Rgb(255, 40, 40), 0.45),
    }
}

fn open(k: &Construct, p: Pos) -> bool {
    k.tiles
        .get(p)
        .is_some_and(|t| t.terrain != Terrain::NetWall)
}

fn border(k: &Construct, p: Pos) -> bool {
    let s = k.tiles.size();
    p.x == 0 || p.y == 0 || p.x == s.w as i32 - 1 || p.y == s.h as i32 - 1
}

/// A wall that faces a room, the spine or a door, diagonals included.
pub fn edge(k: &Construct, p: Pos) -> bool {
    k.tiles
        .get(p)
        .is_some_and(|t| t.terrain == Terrain::NetWall)
        && p.neighbors8().any(|q| open(k, q))
}

fn mask(p: Pos, joins: impl Fn(Pos) -> bool) -> usize {
    [(0, -1, 1), (1, 0, 2), (0, 1, 4), (-1, 0, 8)]
        .into_iter()
        .filter(|(dx, dy, _)| joins(p.offset(*dx, *dy)))
        .map(|(_, _, bit)| bit)
        .sum()
}

pub(super) fn draw(
    m: &ConstructMap,
    view: &NetLookView,
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
    style: &StylePack,
) {
    let k = m.construct;
    let tier = Tier::of(k.security);
    let body = tier.colour();
    let vault = k.vault_room();
    for sy in area.y..area.bottom() {
        for sx in area.x..area.right() {
            let p = screen_to_tile(area, origin, (sx, sy));
            let Some(t) = k.tiles.get(p) else {
                let dot = p.x.rem_euclid(4) == 0 && p.y.rem_euclid(2) == 0;
                put(
                    buf,
                    sx,
                    sy,
                    if dot { '·' } else { ' ' },
                    LATTICE,
                    VOID,
                    false,
                );
                continue;
            };
            let in_vault = vault.is_some_and(|r| r.contains(p));
            let (mut ch, mut fg, mut bg, mut bold) = match t.terrain {
                Terrain::NetWall if border(k, p) => {
                    let m = mask(p, |q| {
                        border(k, q)
                            && k.tiles.get(q).is_some_and(|t| {
                                matches!(t.terrain, Terrain::NetWall | Terrain::NetGate)
                            })
                    });
                    (tier.walls()[m], body, VOID.mix(body, 0.10), false)
                }
                Terrain::NetWall if edge(k, p) => {
                    let m = mask(p, |q| edge(k, q) && !border(k, q));
                    (LIGHT[m], body.scale(0.75), VOID.mix(body, 0.10), false)
                }
                Terrain::NetWall => ('░', body.scale(0.5), VOID.mix(body, 0.07), false),
                Terrain::NetLane => {
                    let m = mask(p, |q| {
                        k.tiles.get(q).is_some_and(|t| {
                            matches!(t.terrain, Terrain::NetLane | Terrain::NetGate)
                        })
                    });
                    (
                        DOUBLE[m],
                        SIGNAL.mix(WHITE, 0.25),
                        VOID.mix(SIGNAL, 0.2),
                        false,
                    )
                }
                Terrain::NetGate => ('▯', WHITE, VOID.mix(SIGNAL, 0.25), true),
                _ if in_vault => (' ', WHITE, VOID.mix(AMBER, 0.16), false),
                _ => (' ', WHITE, VOID.mix(ROOM, 0.10), false),
            };
            match t.feature {
                Some(Feature::Node(kind)) => {
                    ch = node_glyph(kind, style);
                    fg = match kind {
                        NodeKind::DataVault => AMBER,
                        NodeKind::Terminal => TERMINAL,
                        _ => node_colour(kind, ROOM, Some(k.security), view.tick),
                    };
                    bold = matches!(kind, NodeKind::DataVault | NodeKind::Server);
                }
                Some(Feature::Ice(grade)) => {
                    ch = '◈';
                    fg = ice_colour(grade, view.tick);
                    bg = VOID.mix(watch_tint(grade).0, 0.35);
                    bold = true;
                }
                _ => {}
            }
            if let Some(grade) = view.watch.get(&p)
                && !matches!(t.feature, Some(Feature::Ice(_)))
            {
                let (tint, f) = watch_tint(*grade);
                bg = bg.mix(tint, f);
            }
            put(buf, sx, sy, ch, fg, bg, bold);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::map::MapRef;
    use crate::render::netlook::NetLook;
    use crate::render::viewport::Overlay;
    use citygen::{City, GenParams, Generator, SizePreset};
    use ratatui::style::Color;

    /// Seed 8, made once for these tests.
    fn city() -> &'static City {
        static CITY: std::sync::OnceLock<City> = std::sync::OnceLock::new();
        CITY.get_or_init(|| {
            Generator::run(GenParams {
                seed: 8,
                size: SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap()
        })
    }

    /// The first construct with black ICE at a vault.
    fn black_vault(c: &City) -> Construct {
        c.buildings
            .iter()
            .filter_map(|b| citygen::construct::generate_construct(c, b.id))
            .find(|k| k.vault_room().is_some() && k.security >= 230 && k.guards.len() == 2)
            .expect("a black construct with a vault")
    }

    fn paint(c: &City, k: &Construct, view: &NetLookView) -> Buffer {
        let style = StylePack::load().unwrap();
        let constructs = {
            let mut m = crate::constructs::Constructs::default();
            m.insert(k.clone());
            m
        };
        let interiors = crate::interiors::Interiors::default();
        let map = MapRef::for_place(
            c,
            &interiors,
            &constructs,
            citygen::Place::Construct(k.building),
            0,
        )
        .unwrap();
        let area = Rect::new(0, 0, k.size.w as u16 + 4, k.size.h as u16 + 2);
        let mut buf = Buffer::empty(area);
        NetLook { view }.draw(&map, Pos::new(-2, -1), area, 0, &mut buf, &style);
        buf
    }

    fn cell(buf: &Buffer, p: Pos) -> &ratatui::buffer::Cell {
        &buf[((p.x + 2) as u16, (p.y + 1) as u16)]
    }

    fn rgb(c: Color) -> (u8, u8, u8) {
        match c {
            Color::Rgb(r, g, b) => (r, g, b),
            other => panic!("not rgb: {other:?}"),
        }
    }

    #[test]
    fn the_body_edges_and_rooms() {
        let c = city();
        let k = black_vault(c);
        let buf = paint(c, &k, &NetLookView::default());
        let size = k.tiles.size();
        let all = citygen::Rect::new(0, 0, size.w as i32, size.h as i32);
        let body = all
            .iter()
            .find(|p| !border(&k, *p) && k.tiles[*p].terrain == Terrain::NetWall && !edge(&k, *p))
            .expect("a wall deep in the body");
        assert_eq!(cell(&buf, body).symbol(), "░");
        let facing = all
            .iter()
            .find(|p| !border(&k, *p) && edge(&k, *p))
            .expect("a wall facing a room");
        assert!(LIGHT.contains(&cell(&buf, facing).symbol().chars().next().unwrap()));
        assert_eq!(
            cell(&buf, Pos::new(1, 0)).symbol(),
            "═",
            "a black construct's border"
        );
        let vault = k.vault_room().unwrap();
        let floor = vault
            .iter()
            .find(|p| k.tiles[*p].feature.is_none())
            .expect("a bare vault tile");
        assert_eq!(cell(&buf, floor).bg, VOID.mix(AMBER, 0.16).to_color());
        let piece = k.guards[1].pos;
        assert_eq!(cell(&buf, piece).symbol(), "◈");
    }

    #[test]
    fn the_watch_tints_the_spine() {
        let c = city();
        let k = black_vault(c);
        let watch = crate::net::watched(&k);
        assert!(!watch.is_empty());
        let buf = paint(
            c,
            &k,
            &NetLookView {
                watch: watch.clone(),
                ..Default::default()
            },
        );
        let black = watch
            .iter()
            .find(|(_, g)| **g == IceGrade::Black)
            .map(|(p, _)| *p)
            .expect("spine the black piece watches");
        let (r, g, _) = rgb(cell(&buf, black).bg);
        assert!(r > g, "red over the black piece's spine");
    }

    #[test]
    fn the_void_round_a_construct() {
        let c = city();
        let k = black_vault(c);
        let buf = paint(c, &k, &NetLookView::default());
        assert_eq!(cell(&buf, Pos::new(-1, 3)).bg, VOID.to_color());
    }
}
