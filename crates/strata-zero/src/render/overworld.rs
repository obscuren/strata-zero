//! The overworld's atlas look: one cell a tile, the ground as glyphs, highways as double
//! lines, rivers as thin blue ones and dry washes as faint sandy ones, cities shaded by
//! density in their districts' colours.
//! `atlas_cell` is what `strata world`'s sheet draws; the overworld screen draws it in
//! motion under the clock's light (`live`).

mod beacons;
pub mod live;
mod neon;
mod weather;

use crate::render::style::StylePack;
use crate::render::{Cell, Rgb};
use citygen::Pos;
use citygen::overworld::{Ground, Overworld, PoiKind, Water};
use citygen::rng::hash64;

/// The atlas's background.
pub const ATLAS_BG: Rgb = Rgb(4, 6, 10);

/// Box glyphs by link mask (N 1, E 2, S 4, W 8).
const DOUBLE: [char; 16] = [
    '═', '║', '═', '╚', '║', '║', '╔', '╠', '═', '╝', '═', '╩', '╗', '╣', '╦', '╬',
];
const ROUND: [char; 16] = [
    '─', '│', '─', '╰', '│', '│', '╭', '├', '─', '╯', '─', '┴', '╮', '┤', '┬', '┼',
];

const DASHED: [char; 16] = [
    '┄', '┆', '┄', '╰', '┆', '┆', '╭', '├', '┄', '╯', '┄', '┴', '╮', '┤', '┬', '┼',
];

/// A POI's glyph and colour.
pub fn poi_glyph(kind: PoiKind) -> (char, Rgb) {
    match kind {
        PoiKind::TruckStop => ('⌂', Rgb(255, 176, 0)),
        PoiKind::Checkpoint => ('╪', Rgb(255, 40, 72)),
        PoiKind::Relay => ('⬡', Rgb(0, 255, 200)),
        PoiKind::Camp => ('△', Rgb(255, 128, 64)),
        PoiKind::Wreck => ('×', Rgb(255, 40, 72)),
        PoiKind::Arcology => ('▣', Rgb(168, 144, 216)),
        PoiKind::Solar => ('▦', Rgb(122, 176, 224)),
        PoiKind::WaterPlant => ('◎', Rgb(0, 229, 255)),
        PoiKind::TestRange => ('◈', Rgb(255, 0, 200)),
        PoiKind::Rig => ('◆', Rgb(200, 160, 80)),
    }
}

fn cell(ch: char, fg: Rgb, bg: Rgb) -> Cell {
    Cell {
        ch,
        fg,
        bg,
        bold: false,
        dim: false,
    }
}

/// The tile's stable roll in 0..1.
fn roll(world: &Overworld, p: Pos) -> f32 {
    let packed = ((p.x as u32 as u64) << 32) | p.y as u32 as u64;
    (hash64(hash64(world.seed, 0xA71A5), packed) >> 40) as f32 / (1u64 << 24) as f32
}

/// What an atlas cell shows, for the clock's light: each kind dims to its own depth at
/// night (`live::lit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Ground,
    Water,
    Urban,
    /// A highway on the open ground.
    Road,
    /// A highway through a city or town.
    CityRoad,
    Place,
}

/// Tile `p` of `world` in the atlas look.
pub fn atlas_cell(world: &Overworld, p: Pos, style: &StylePack) -> Cell {
    base_cell(world, p, style).map_or(cell(' ', ATLAS_BG, ATLAS_BG), |(c, _)| c)
}

/// Tile `p` in the atlas look and what it shows; none past the world's edge.
fn base_cell(world: &Overworld, p: Pos, style: &StylePack) -> Option<(Cell, Kind)> {
    let t = world.tile(p)?;
    let (mut c, kind) = ground_cell(world, p, style);
    if let Some(poi) = world.poi_at(p) {
        let (ch, fg) = poi_glyph(poi.kind);
        c.ch = ch;
        c.fg = fg;
        c.bold = true;
        return Some((c, Kind::Place));
    }
    if t.road != 0 {
        let in_city = t.urban.is_some();
        let bg = if in_city {
            Rgb(26, 20, 8)
        } else if t.water != Water::None {
            Rgb(10, 26, 46)
        } else {
            ATLAS_BG
        };
        let fg = if in_city {
            Rgb(240, 200, 112)
        } else {
            Rgb(200, 160, 80)
        };
        let mut c = cell(DOUBLE[(t.road & 15) as usize], fg, bg);
        c.bold = in_city;
        let kind = if in_city { Kind::CityRoad } else { Kind::Road };
        return Some((c, kind));
    }
    Some((c, kind))
}

/// Everything under the roads and POIs: river, city, water, ground.
fn ground_cell(world: &Overworld, p: Pos, style: &StylePack) -> (Cell, Kind) {
    let t = world.tile(p).expect("in bounds");
    let r = roll(world, p);
    if t.water == Water::River {
        let c = cell(
            ROUND[(t.river & 15) as usize],
            Rgb(58, 138, 216),
            Rgb(6, 18, 31),
        );
        return (c, Kind::Water);
    }
    if t.wash != 0 && t.urban.is_none() {
        // A dry wash: a faint dashed sandy line, so it never reads as a road.
        let c = cell(DASHED[(t.wash & 15) as usize], Rgb(88, 76, 56), ATLAS_BG);
        return (c, Kind::Ground);
    }
    if let Some(u) = t.urban {
        let v = u.density as f32 / 255.0;
        let a = style.theme_or_neutral(Some(u.theme)).accent;
        let ch = if v > 0.75 {
            if r < 0.45 { '█' } else { '▓' }
        } else if v > 0.45 {
            if r < 0.4 { '▓' } else { '▒' }
        } else if v > 0.22 {
            if r < 0.35 { '▒' } else { '░' }
        } else if r < 0.45 {
            '░'
        } else if r < 0.75 {
            '▫'
        } else {
            '·'
        };
        let fg = Rgb(42, 46, 56)
            .mix(Rgb(154, 160, 174), v)
            .mix(a, 0.22 + 0.38 * v);
        let bg = ATLAS_BG.mix(a, 0.03 + 0.06 * v);
        if v > 0.45 && r > 0.92 {
            return (cell('▪', a, bg), Kind::Urban);
        }
        return (cell(ch, fg, bg), Kind::Urban);
    }
    match t.water {
        Water::Lake => return (cell('≈', Rgb(42, 90, 138), Rgb(10, 26, 46)), Kind::Water),
        Water::Sea => {
            let shore = p
                .neighbors8()
                .any(|q| world.tile(q).is_some_and(|n| n.water != Water::Sea));
            let c = if shore {
                cell('~', Rgb(42, 90, 138), Rgb(10, 26, 46))
            } else {
                cell(
                    if r < 0.12 { '~' } else { ' ' },
                    Rgb(26, 58, 92),
                    Rgb(5, 12, 24),
                )
            };
            return (c, Kind::Water);
        }
        _ => {}
    }
    let bg = ATLAS_BG;
    let c = match t.ground {
        Ground::Peak => cell('▲', Rgb(200, 204, 216), bg),
        Ground::Mountains => cell('▲', Rgb(120, 124, 138), bg),
        Ground::Hills => cell(if r < 0.7 { '∩' } else { '·' }, Rgb(122, 102, 72), bg),
        Ground::Spill => {
            let ch = if r < 0.18 {
                '≈'
            } else if r < 0.6 {
                '·'
            } else {
                ' '
            };
            let fg = if r > 0.9 {
                Rgb(154, 255, 90)
            } else {
                Rgb(74, 138, 34)
            };
            cell(ch, fg, Rgb(6, 12, 4))
        }
        Ground::Salt => cell(if r < 0.5 { '░' } else { '·' }, Rgb(90, 88, 82), bg),
        Ground::Fields => cell('▬', Rgb(58, 106, 48), bg),
        Ground::Scrub => {
            let ch = if r < 0.35 {
                '"'
            } else if r < 0.6 {
                ','
            } else {
                ' '
            };
            cell(ch, Rgb(74, 90, 52), bg)
        }
        Ground::Flats => {
            let ch = if r < 0.3 {
                '·'
            } else if r < 0.45 {
                '.'
            } else {
                ' '
            };
            cell(ch, Rgb(74, 66, 52), bg)
        }
    };
    (c, Kind::Ground)
}

/// The survey chart's paper.
pub const CHART_BG: Rgb = Rgb(4, 6, 10);

/// How high a tile's ground stands, for the chart's contours.
fn band(world: &Overworld, p: Pos) -> i32 {
    match world.tile(p).map(|t| (t.water, t.ground)) {
        None | Some((Water::Sea | Water::Lake, _)) => -1,
        Some((_, Ground::Hills)) => 3,
        Some((_, Ground::Mountains)) => 5,
        Some((_, Ground::Peak)) => 7,
        Some((_, Ground::Scrub)) => 1,
        Some(_) => 0,
    }
}

/// Tile `p` of `world` as round 1's survey chart (the deck MAP's world zoom): contour
/// dots up the hills, the coast hatched, dead ground marked, the cities as tinted
/// ground, rivers in thin blue, highways heavy and amber.
pub fn chart_cell(world: &Overworld, p: Pos, style: &StylePack) -> Cell {
    let Some(t) = world.tile(p) else {
        return cell(' ', CHART_BG, CHART_BG);
    };
    let mut c = if matches!(t.water, Water::Sea | Water::Lake) {
        let shore = p.neighbors8().any(|q| {
            world
                .tile(q)
                .is_some_and(|n| !matches!(n.water, Water::Sea | Water::Lake))
        });
        if shore {
            cell('░', Rgb(28, 74, 108), Rgb(5, 11, 20))
        } else {
            let dot = (p.x + p.y * 3).rem_euclid(6) == 0;
            cell(if dot { '·' } else { ' ' }, Rgb(18, 48, 72), Rgb(5, 11, 20))
        }
    } else {
        let b0 = band(world, p);
        let edge = [(1, 0), (0, 1), (-1, 0), (0, -1)].iter().any(|(dx, dy)| {
            let b = band(world, p.offset(*dx, *dy));
            b >= 0 && b < b0
        });
        let mut c = cell(' ', CHART_BG, CHART_BG);
        if edge && b0 >= 3 {
            let k = ((b0 - 3) as f32 / 4.0).min(1.0);
            c = cell('·', Rgb(42, 36, 24).mix(Rgb(138, 112, 72), k), CHART_BG);
        }
        match t.ground {
            Ground::Spill => {
                let x = (p.x + p.y).rem_euclid(3) == 0;
                c = cell(if x { '╳' } else { ' ' }, Rgb(62, 122, 24), Rgb(7, 16, 10));
            }
            Ground::Salt => {
                let x = p.y % 2 == 0 && p.x % 3 != 0;
                c = cell(if x { '─' } else { ' ' }, Rgb(58, 57, 52), CHART_BG);
            }
            _ => {}
        }
        c
    };
    if let Some(u) = t.urban {
        let v = u.density as f32 / 255.0;
        let acc = style.theme_or_neutral(Some(u.theme)).accent;
        let bg = CHART_BG.mix(acc, 0.10 + 0.22 * v);
        let top = world
            .tile(p.offset(0, -1))
            .is_none_or(|n| n.urban.is_none());
        c = if top {
            cell('▔', bg.mix(acc, 0.6), bg)
        } else {
            cell(' ', CHART_BG, bg)
        };
    }
    if t.water == Water::River {
        c.ch = ROUND[(t.river & 15) as usize];
        c.fg = Rgb(58, 138, 216);
    }
    if t.road != 0 {
        c.ch = HEAVY[(t.road & 15) as usize];
        c.fg = Rgb(224, 176, 96);
    }
    if let Some(poi) = world.poi_at(p) {
        let (ch, fg) = poi_glyph(poi.kind);
        c.ch = ch;
        c.fg = fg;
        c.bold = true;
    }
    c
}

/// Heavy box glyphs by link mask (N 1, E 2, S 4, W 8): the Net's trunks.
pub const HEAVY: [char; 16] = [
    '━', '┃', '━', '┗', '┃', '┃', '┏', '┣', '━', '┛', '━', '┻', '┓', '┫', '┳', '╋',
];

/// The Net's void and its signal colour (the city Net's).
const NET_VOID: Rgb = Rgb(1, 2, 4);
const NET_SIGNAL: Rgb = Rgb(0, 255, 200);

/// The world jacked in, round 1's D: trunks down the highways with packets on them,
/// the cities as node fields in their districts' colours, every POI a node with a spur
/// to its trunk, the dead grid between. Built once for a world (its spurs and the
/// trunks' packet stops).
pub struct NetAtlas {
    /// Spur cells from each POI to the nearest trunk, with their glyph.
    pub spurs: std::collections::HashMap<Pos, char>,
    /// Where along its highway each trunk tile is, for the packets.
    stops: std::collections::HashMap<Pos, Vec<usize>>,
}

impl NetAtlas {
    pub fn new(world: &Overworld) -> NetAtlas {
        let mut stops: std::collections::HashMap<Pos, Vec<usize>> = Default::default();
        for r in &world.routes {
            for (k, p) in r.path.iter().enumerate() {
                stops.entry(*p).or_default().push(k);
            }
        }
        let mut spurs = std::collections::HashMap::new();
        for poi in &world.pois {
            if poi_node(poi.kind).is_none() {
                continue;
            }
            let road = |p: Pos| world.tile(p).is_some_and(|t| t.road != 0);
            let near = (1..=6i32).find_map(|d| {
                (-d..=d)
                    .flat_map(|dx| {
                        let dy = d - dx.abs();
                        [poi.at.offset(dx, dy), poi.at.offset(dx, -dy)]
                    })
                    .find(|q| road(*q))
            });
            let Some(to) = near else { continue };
            // across to the trunk's column, then along it
            let mut q = poi.at;
            while q != to {
                let across = q.x != to.x;
                q = if across {
                    q.offset((to.x - q.x).signum(), 0)
                } else {
                    q.offset(0, (to.y - q.y).signum())
                };
                if q != to && !road(q) && world.poi_at(q).is_none() {
                    spurs.insert(q, if across { '┄' } else { '┆' });
                }
            }
        }
        NetAtlas { spurs, stops }
    }

    /// Tile `p` jacked in, at game tick `tick` (the packets move along the trunks).
    pub fn cell(&self, world: &Overworld, p: Pos, style: &StylePack, tick: u64) -> Cell {
        let Some(t) = world.tile(p) else {
            return cell(' ', NET_VOID, NET_VOID);
        };
        let lattice = p.x % 4 == 0 && p.y % 2 == 0;
        let wet = matches!(t.water, Water::Sea | Water::Lake);
        let mut c = if wet {
            cell(
                if lattice { '·' } else { ' ' },
                Rgb(10, 26, 40),
                Rgb(1, 3, 9),
            )
        } else {
            cell(if lattice { '·' } else { ' ' }, Rgb(15, 51, 48), NET_VOID)
        };
        if let Some(u) = t.urban {
            let v = u.density as f32 / 255.0;
            let acc = style.theme_or_neutral(Some(u.theme)).accent;
            let bg = NET_VOID.mix(acc, 0.05 + 0.10 * v);
            let r = roll(world, p);
            c = if r < 0.18 + 0.4 * v {
                let ch = if r < 0.06 * (1.0 + v) {
                    '◆'
                } else if r < 0.2 {
                    '◇'
                } else {
                    '▫'
                };
                cell(ch, Rgb(26, 74, 72).mix(acc, 0.4 + 0.5 * v), bg)
            } else {
                cell('·', NET_VOID.mix(acc, 0.25), bg)
            };
        }
        if let Some(ch) = self.spurs.get(&p) {
            c.ch = *ch;
            c.fg = Rgb(42, 106, 96);
        }
        if t.road != 0 {
            let in_city = t.urban.is_some();
            c.ch = HEAVY[(t.road & 15) as usize];
            c.fg = if in_city {
                NET_SIGNAL
            } else {
                Rgb(31, 138, 118)
            };
            c.bold = in_city;
            // a packet every 23 tiles, a tile on every few ticks
            let step = (tick / crate::render::netlook::packets::PACKET_TICKS) as usize;
            let packet = self
                .stops
                .get(&p)
                .is_some_and(|ks| ks.iter().any(|k| (k + step) % 23 == 7));
            if packet && !in_city {
                c.fg = Rgb(200, 255, 240);
                c.bold = true;
            }
        }
        if let Some(poi) = world.poi_at(p)
            && let Some((ch, tier)) = poi_node(poi.kind)
        {
            c = cell(ch, tier, NET_VOID.mix(tier, 0.18));
            c.bold = true;
        }
        c
    }
}

/// A POI's node on the Net and its tier's colour; a camp has none.
fn poi_node(kind: PoiKind) -> Option<(char, Rgb)> {
    let open = NET_SIGNAL;
    let black = Rgb(255, 40, 72);
    let grey = Rgb(159, 230, 255);
    Some(match kind {
        PoiKind::TruckStop | PoiKind::Wreck | PoiKind::Solar => ('◇', grey),
        PoiKind::Checkpoint | PoiKind::TestRange => ('◈', black),
        PoiKind::Relay => ('⬡', open),
        PoiKind::WaterPlant => ('◆', open),
        PoiKind::Arcology | PoiKind::Rig => ('⌗', grey),
        PoiKind::Camp => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::style::StylePack;
    use citygen::overworld::{Overworld, WorldParams};

    #[test]
    fn every_world_tile_charts_one_cell() {
        let w = Overworld::generate(WorldParams {
            seed: 3,
            ..Default::default()
        })
        .unwrap();
        let style = StylePack::load().unwrap();
        let mut glyphs = std::collections::BTreeSet::new();
        for (p, _) in w.tiles.iter() {
            let c = chart_cell(&w, p, &style);
            assert_eq!(
                unicode_width::UnicodeWidthChar::width(c.ch),
                Some(1),
                "{p:?}"
            );
            glyphs.insert(c.ch);
        }
        for ch in ['░', '▔', '·'] {
            assert!(
                glyphs.contains(&ch),
                "{ch}: the coast, a city's edge, the contours"
            );
        }
        assert!(HEAVY.iter().any(|h| glyphs.contains(h)), "the highways");
    }

    #[test]
    fn the_net_over_the_world_is_trunks_nodes_and_dead_grid() {
        // round 1's D: the highways as trunks, cities as node fields, every POI a node
        // with its spur to a trunk, the dead grid between
        let w = Overworld::generate(WorldParams {
            seed: 3,
            ..Default::default()
        })
        .unwrap();
        let style = StylePack::load().unwrap();
        let net = NetAtlas::new(&w);
        let mut trunks = 0;
        let mut nodes = 0;
        for (p, t) in w.tiles.iter() {
            let c = net.cell(&w, p, &style, 0);
            assert_eq!(
                unicode_width::UnicodeWidthChar::width(c.ch),
                Some(1),
                "{:?} {p:?}",
                c.ch
            );
            if t.road != 0 && w.poi_at(p).is_none() {
                assert!(HEAVY.contains(&c.ch), "a trunk at {p:?}: {:?}", c.ch);
                trunks += 1;
            }
            if t.urban.is_some() && t.road == 0 && matches!(c.ch, '◆' | '◇' | '▫') {
                nodes += 1;
            }
        }
        assert!(
            trunks > 100 && nodes > 20,
            "{trunks} trunk tiles, {nodes} city nodes"
        );
        for poi in &w.pois {
            let c = net.cell(&w, poi.at, &style, 0);
            if poi.kind != PoiKind::Camp {
                assert!(c.bold && c.ch != '·', "{:?} at {:?}", poi.kind, poi.at);
            }
        }
        assert!(!net.spurs.is_empty(), "spurs from the POIs to the trunks");
        // the dead grid: a lattice dot on bare ground
        let bare = w
            .tiles
            .iter()
            .find(|(p, t)| {
                t.road == 0
                    && t.urban.is_none()
                    && t.water == Water::None
                    && p.x % 4 == 0
                    && p.y % 2 == 0
                    && w.poi_at(*p).is_none()
                    && !net.spurs.contains_key(p)
            })
            .map(|(p, _)| p)
            .unwrap();
        assert_eq!(net.cell(&w, bare, &style, 0).ch, '·');
    }

    #[test]
    fn every_kind_of_tile_draws_one_cell() {
        let w = Overworld::generate(WorldParams {
            seed: 3,
            ..Default::default()
        })
        .unwrap();
        let style = StylePack::load().unwrap();
        let mut kinds = std::collections::BTreeSet::new();
        for (p, t) in w.tiles.iter() {
            let c = atlas_cell(&w, p, &style);
            assert_eq!(
                unicode_width::UnicodeWidthChar::width(c.ch),
                Some(1),
                "{:?} {p:?}",
                c.ch
            );
            kinds.insert(format!(
                "{:?}{:?}{}{}",
                t.water,
                t.ground,
                t.road != 0,
                t.urban.is_some()
            ));
        }
        assert!(
            kinds.len() > 12,
            "a world has many kinds of tile: {kinds:?}"
        );
        for poi in &w.pois {
            assert_eq!(atlas_cell(&w, poi.at, &style).ch, poi_glyph(poi.kind).0);
        }
    }
}
