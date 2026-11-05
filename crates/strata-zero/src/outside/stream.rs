//! The open ground as one stream round the view: its street and its Net, built tile by
//! tile from the world, lit by the street's rule, and who owns each overworld tile.
//! See `2026-11-05-streamed-world-design.md`, "Areas and the stream".
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the city screen walks the stream from #27's next tasks"
    )
)]

use super::ground::GroundView;
use super::{Outside, Settlement, TILE};
use crate::render::ConnDir;
use crate::render::map::TileMap;
use citygen::overworld::Overworld;
use citygen::{BuildingKind, City, Grid, Pos, Rect, Size, StratumKind, Terrain, Theme, Tile};

/// The open ground round the view, both its layers: the street's and the Net's. Not
/// saved: everything in it is made again from the world.
pub struct Stream {
    /// The street's layer, with its light (its windows' and its lamps').
    pub street: GroundView,
    /// The Net's layer.
    pub net: GroundView,
    /// The world's size in cells, from the last cover.
    cells: Size,
}

impl Default for Stream {
    fn default() -> Stream {
        Stream {
            street: GroundView::default(),
            net: GroundView::net(),
            cells: Size { w: 0, h: 0 },
        }
    }
}

/// A clone starts empty: the stream is made again from the world as it's needed.
impl Clone for Stream {
    fn clone(&self) -> Stream {
        Stream::default()
    }
}

impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("tiles", &(self.street.held() + self.net.held()))
            .finish()
    }
}

impl Stream {
    /// Both layers built over `area` (world cells) and a ring past it; far tiles
    /// forgotten.
    pub fn cover(&mut self, world: &Overworld, area: Rect) {
        self.cells = Size {
            w: TILE as u32 * world.size.w,
            h: TILE as u32 * world.size.h,
        };
        self.street.cover(world, area);
        self.net.cover(world, area);
    }
}

/// One layer of the stream as a map in world cells, seen at `minute` of the day: the
/// street's for any stratum but the Net.
#[derive(Clone, Copy)]
pub struct GroundMap<'a> {
    pub stream: &'a Stream,
    pub layer: StratumKind,
    pub minute: u16,
}

impl GroundMap<'_> {
    fn view(&self) -> &GroundView {
        match self.layer {
            StratumKind::Net => &self.stream.net,
            _ => &self.stream.street,
        }
    }
}

impl TileMap for GroundMap<'_> {
    fn size(&self) -> Size {
        self.stream.cells
    }

    fn tile(&self, p: Pos) -> Option<&Tile> {
        self.view().tile(p)
    }

    /// Ambient for the hour plus what the cell is lit by, as a city's stratum is lit.
    fn light(&self, p: Pos) -> u8 {
        citygen::lighting::ambient(self.layer, self.minute).saturating_add(self.emitted(p))
    }

    /// On the street, what burns all day and the lamps while they burn; on the Net, its
    /// grid's and its trunks' own.
    fn emitted(&self, p: Pos) -> u8 {
        match self.layer {
            StratumKind::Net => self.tile(p).map_or(0, |t| net_light(t.terrain)),
            _ => self.stream.street.emitted(p, self.lamps_on()),
        }
    }

    fn emits(&self, p: Pos) -> bool {
        self.tile(p).is_some_and(|t| {
            citygen::lighting::emits(t.terrain, t.feature, self.layer, self.lamps_on())
        })
    }

    fn daylight(&self) -> f32 {
        if citygen::lighting::sunlit(self.layer) {
            citygen::lighting::daylight(self.minute)
        } else {
            0.0
        }
    }

    fn lamps_on(&self) -> bool {
        citygen::lighting::lamps_on_at(self.layer, self.minute)
    }

    fn lit_sight(&self) -> bool {
        self.layer != StratumKind::Net
    }

    fn sky(&self) -> Option<((u8, u8, u8), f32)> {
        citygen::lighting::sky(self.layer, self.minute)
    }

    fn theme(&self, _p: Pos) -> Option<Theme> {
        None
    }

    fn kind(&self, _p: Pos) -> Option<BuildingKind> {
        None
    }

    fn connector(&self, _p: Pos) -> Option<ConnDir> {
        None
    }
}

/// The light the open Net's cells give, as a city's Net gives them past its outline
/// (citygen's `CyberspaceStage::on_its_site`): trunks and gates brightest, the grid's
/// dots, a construct's walls and floor dim, the void none.
fn net_light(terrain: Terrain) -> u8 {
    use citygen::lighting::tuning::{NET_GRID_LIGHT, NET_LANE_LIGHT};
    match terrain {
        Terrain::NetLane | Terrain::NetGate => NET_LANE_LIGHT,
        Terrain::NetGrid => NET_GRID_LIGHT,
        Terrain::NetWall | Terrain::NetFloor => 96,
        _ => 0,
    }
}

/// Who owns each overworld tile: the settlement whose map holds it, or none for the
/// open ground. Made from the world alone (and the home city's block), with the world.
#[derive(Clone)]
pub(super) struct Owners(Grid<Option<Settlement>>);

impl std::fmt::Debug for Owners {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let owned = self.0.count(|s| s.is_some());
        write!(f, "Owners({owned} tiles)")
    }
}

impl Owners {
    /// The owners of `o`'s world, `home` its home city. Every map is a settlement's
    /// block: the home city's, and every other's. Where several cover a tile it goes
    /// to the one whose own tiles lie nearest, the city before a town, then the lower
    /// city, then the lower town. A home city from before the world owns its whole
    /// block.
    pub(super) fn of(o: &Outside, home: &City) -> Owners {
        let w = o.world();
        let maps: Vec<(Settlement, Rect)> = std::iter::once((o.home(), o.block(home)))
            .chain(o.others())
            .collect();
        let mut covers = Grid::filled(w.size, 0u8);
        for (_, b) in &maps {
            for t in b.iter() {
                if let Some(n) = covers.get_mut(t) {
                    *n = n.saturating_add(1);
                }
            }
        }
        type Rank = (i64, bool, u8, Option<u8>);
        let mut best: Grid<Option<(Rank, Settlement)>> = Grid::filled(w.size, None);
        for &(s, b) in &maps {
            let own = match s.town {
                None => w.city_tiles(s.city),
                Some(t) => w.town_tiles(s.city, t),
            };
            for t in b.iter() {
                let Some(&n) = covers.get(t) else {
                    continue;
                };
                // alone on a tile it needs no rank
                let rank = if n > 1 {
                    let d = own.iter().map(|p| p.dist2(t)).min().unwrap_or(i64::MAX);
                    (d, s.town.is_some(), s.city, s.town)
                } else {
                    (0, false, 0, None)
                };
                if best[t].is_none_or(|(r, _)| rank < r) {
                    best[t] = Some((rank, s));
                }
            }
        }
        let mut owners = best.map(|b| b.as_ref().map(|(_, s)| *s));
        if home.params.site.is_none() {
            owners.fill_rect(o.block(home), Some(o.home()));
        }
        Owners(owners)
    }
}

impl Outside {
    /// Who owns overworld tile `tile`: the settlement whose map holds it, none on the
    /// open ground (or before the world is made).
    pub fn owner(&self, tile: Pos) -> Option<Settlement> {
        self.owners.as_ref()?.0.get(tile).copied().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outside::ground::CAP;
    use crate::outside::tests::sited;
    use crate::render::map::StratumMap;
    use citygen::overworld::detail;
    use citygen::{Feature, GenParams, Generator, SizePreset};
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    /// The night: the lamps burn.
    const NIGHT: u16 = 22 * 60;
    /// Noon: they don't.
    const NOON: u16 = 12 * 60;

    /// Seed 3's home city on its site, and its world.
    fn seed3() -> (&'static City, Outside) {
        let city = sited();
        (city, Outside::new(city).unwrap())
    }

    /// Every city and town of seed 3's world built on its site, the home city first:
    /// built once for every test.
    fn settlements() -> &'static [(Settlement, City)] {
        static ALL: OnceLock<Vec<(Settlement, City)>> = OnceLock::new();
        ALL.get_or_init(|| {
            let (home, o) = seed3();
            let w = o.world();
            let others = o.others().into_iter().map(|(s, _)| {
                let base = GenParams {
                    seed: 1 + ((s.city as u64) << 8) + s.town.map_or(0, |t| t as u64 + 1),
                    ..Default::default()
                };
                let params = match s.town {
                    None => Some(citygen::sited(w, s.city, base)),
                    Some(t) => citygen::sited_town(w, s.city, t, base),
                };
                (s, Generator::run(params.unwrap()).unwrap())
            });
            std::iter::once((o.home(), home.clone()))
                .chain(others)
                .collect()
        })
    }

    /// The edges of `city`'s map with world past them, each as the strip of its cells
    /// within 2 of it.
    fn edges_to_the_world(o: &Outside, city: &City) -> Vec<Rect> {
        let origin = o.origin(city);
        let (w, h) = (city.size.w as i32, city.size.h as i32);
        let world = o.world().size;
        let (ww, wh) = (TILE * world.w as i32, TILE * world.h as i32);
        [
            (origin.y > 0, Rect::new(0, 0, w, 3)),
            (origin.y + h < wh, Rect::new(0, h - 3, w, 3)),
            (origin.x > 0, Rect::new(0, 0, 3, h)),
            (origin.x + w < ww, Rect::new(w - 3, 0, 3, h)),
        ]
        .into_iter()
        .filter_map(|(past, strip)| past.then_some(strip))
        .collect()
    }

    /// Every map of the world: the home city's block and every other settlement's.
    fn maps(o: &Outside, city: &City) -> Vec<(Settlement, Rect)> {
        std::iter::once((o.home(), o.block(city)))
            .chain(o.others())
            .collect()
    }

    /// A settlement's own tiles.
    fn own(o: &Outside, s: Settlement) -> Vec<Pos> {
        match s.town {
            None => o.world().city_tiles(s.city),
            Some(t) => o.world().town_tiles(s.city, t),
        }
    }

    /// A city from before the world, its world seed 4's: a town of its own inside
    /// another city's block, and a map reaching into its own.
    fn before_the_world() -> (City, Outside) {
        let city = Generator::run(GenParams {
            seed: 4,
            size: SizePreset::Medium,
            ..Default::default()
        })
        .unwrap();
        let o = Outside::new(&city).unwrap();
        (city, o)
    }

    #[test]
    fn every_tile_has_one_owner() {
        let (city, o) = seed3();
        let maps = maps(&o, city);
        let own: Vec<Vec<Pos>> = maps.iter().map(|(s, _)| own(&o, *s)).collect();
        let w = o.world();
        let (mut open, mut shared) = (0, 0);
        for (t, _) in w.tiles.iter() {
            let over: Vec<usize> = (0..maps.len()).filter(|&i| maps[i].1.contains(t)).collect();
            let want = match over.len() {
                0 => {
                    open += 1;
                    None
                }
                1 => Some(maps[over[0]].0),
                _ => {
                    shared += 1;
                    // the nearest own tiles; the city before a town, then the lower index
                    over.iter()
                        .min_by_key(|&&i| {
                            let s = maps[i].0;
                            let d = own[i].iter().map(|p| p.dist2(t)).min();
                            (d.unwrap_or(i64::MAX), s.town.is_some(), s.city, s.town)
                        })
                        .map(|&i| maps[i].0)
                }
            };
            assert_eq!(o.owner(t), want, "{t:?} under {over:?}");
        }
        assert!(open > 0 && shared > 0, "{open} open, {shared} shared");
        for (i, (s, _)) in maps.iter().enumerate() {
            for t in &own[i] {
                assert_eq!(o.owner(*t), Some(*s), "{s:?}'s own tile {t:?}");
            }
        }
    }

    #[test]
    fn a_town_owns_its_own_tiles_inside_its_citys_block() {
        // seed 3's towns all lie off their cities' blocks; seed 4's world has one inside
        let (_, o) = before_the_world();
        let w = o.world();
        let (town, inside) = o
            .others()
            .into_iter()
            .filter(|(s, _)| s.city != o.home().city)
            .filter_map(|(s, _)| {
                let block = w.city_block(s.city);
                let inside: Vec<Pos> = w
                    .town_tiles(s.city, s.town?)
                    .into_iter()
                    .filter(|p| block.contains(*p))
                    .collect();
                (!inside.is_empty()).then_some((s, inside))
            })
            .next()
            .expect("a town inside its city's block in seed 4's world");
        for t in inside {
            assert_eq!(o.owner(t), Some(town), "{t:?}");
        }
    }

    #[test]
    fn a_city_from_before_the_world_owns_its_whole_block() {
        let (city, o) = before_the_world();
        let block = o.block(&city);
        let others = o.others();
        let contested = block
            .iter()
            .filter(|t| others.iter().any(|(_, b)| b.contains(*t)))
            .count();
        assert!(contested > 0, "another map reaches into the block");
        for t in block.iter() {
            assert_eq!(o.owner(t), Some(o.home()), "{t:?}");
        }
    }

    #[test]
    fn the_ground_past_a_city_is_the_citys_own_ground() {
        // every city and town of seed 3's world: along each edge of its map with world
        // past it, its cells with no district are the stream's, street and Net
        let o = seed3().1;
        let mut checked = BTreeMap::new();
        let mut off = Vec::new();
        for (s, city) in settlements() {
            let origin = o.origin(city);
            let mut stream = Stream::default();
            // each strip covered and checked in turn (the stream forgets what's far)
            for strip in edges_to_the_world(&o, city) {
                stream.cover(
                    o.world(),
                    Rect::new(origin.x + strip.x, origin.y + strip.y, strip.w, strip.h),
                );
                for p in strip.iter() {
                    for layer in [StratumKind::Street, StratumKind::Net] {
                        let mine = city.tile(layer, p).unwrap();
                        if mine.district.is_some() {
                            continue;
                        }
                        let ground = GroundMap {
                            stream: &stream,
                            layer,
                            minute: NIGHT,
                        };
                        let at = origin.offset(p.x, p.y);
                        let theirs = ground.tile(at).expect("the stream covers the edge");
                        *checked.entry(layer.label()).or_insert(0) += 1;
                        if (mine.terrain, mine.feature) != (theirs.terrain, theirs.feature) {
                            off.push((*s, p, layer, mine.terrain, mine.feature, theirs.terrain));
                        }
                        // and the open Net lights as the city's Net does past its outline
                        let lit = StratumMap {
                            city,
                            stratum: layer,
                            minute: NIGHT,
                        };
                        if layer == StratumKind::Net && mine.terrain == theirs.terrain {
                            assert_eq!(ground.light(at), lit.light(p), "{s:?} {p:?} {mine:?}");
                        }
                    }
                }
            }
        }
        assert!(
            checked.len() == 2 && checked.values().all(|n| *n > 10_000),
            "{checked:?}"
        );
        assert!(
            off.is_empty(),
            "{} of {checked:?} cells differ, e.g. {:?}",
            off.len(),
            &off[..off.len().min(12)]
        );
    }

    #[test]
    fn a_ground_lamp_lights_as_the_citys_lamps_do() {
        // every city and town of seed 3's world: off its map's edge and out of reach of
        // its own lamps (no district cell within 8), the open ground's lamps and windows
        // light as the city lights them, by night and by day
        assert!(citygen::lighting::lamps_on(NIGHT) && !citygen::lighting::lamps_on(NOON));
        let o = seed3().1;
        let (mut cells, mut lit, mut off) = (0, 0, Vec::new());
        for (s, city) in settlements() {
            let origin = o.origin(city);
            let (w, h) = (city.size.w as i32, city.size.h as i32);
            // the district cells round a cell, from a summed-area table
            let mut sum = vec![0i32; ((w + 1) * (h + 1)) as usize];
            let at = |x: i32, y: i32| (y * (w + 1) + x) as usize;
            for y in 0..h {
                for x in 0..w {
                    let d = city
                        .tile(StratumKind::Street, Pos::new(x, y))
                        .is_some_and(|t| t.district.is_some()) as i32;
                    sum[at(x + 1, y + 1)] =
                        d + sum[at(x, y + 1)] + sum[at(x + 1, y)] - sum[at(x, y)];
                }
            }
            let districts_near = |p: Pos, r: i32| {
                let (x0, y0) = ((p.x - r).max(0), (p.y - r).max(0));
                let (x1, y1) = ((p.x + r + 1).min(w), (p.y + r + 1).min(h));
                sum[at(x1, y1)] - sum[at(x0, y1)] - sum[at(x1, y0)] + sum[at(x0, y0)] > 0
            };
            // checked a chunk at a time (the stream forgets what's far)
            let chunk = 8 * TILE;
            let mut chunks: BTreeMap<(i32, i32), Vec<Pos>> = BTreeMap::new();
            for p in Rect::new(10, 10, w - 20, h - 20).iter() {
                if !districts_near(p, 8) {
                    chunks
                        .entry((p.x.div_euclid(chunk), p.y.div_euclid(chunk)))
                        .or_default()
                        .push(p);
                }
            }
            let mut stream = Stream::default();
            for ((cx, cy), ps) in chunks {
                stream.cover(
                    o.world(),
                    Rect::new(origin.x + cx * chunk, origin.y + cy * chunk, chunk, chunk),
                );
                for minute in [NIGHT, NOON] {
                    let mine = StratumMap {
                        city,
                        stratum: StratumKind::Street,
                        minute,
                    };
                    let ground = GroundMap {
                        stream: &stream,
                        layer: StratumKind::Street,
                        minute,
                    };
                    for &p in &ps {
                        let at = origin.offset(p.x, p.y);
                        let (a, b) = (mine.emitted(p), ground.emitted(at));
                        cells += usize::from(minute == NIGHT);
                        lit += usize::from(minute == NIGHT && a > 0);
                        if a.abs_diff(b) > 1 || mine.emits(p) != ground.emits(at) {
                            off.push((*s, p, minute, a, b));
                        }
                    }
                }
            }
        }
        assert!(
            off.is_empty(),
            "{} of {cells} cells differ (minute, city, ground), e.g. {:?}",
            off.len(),
            &off[..off.len().min(12)]
        );
        assert!(lit > 100, "{lit} of {cells} cells lit by the cities' lamps");
    }

    /// The lamp light at `p` a stream holds, at night.
    fn lamp_at(s: &Stream, p: Pos) -> u8 {
        GroundMap {
            stream: s,
            layer: StratumKind::Street,
            minute: NIGHT,
        }
        .emitted(p)
    }

    #[test]
    fn a_tile_lights_the_same_whatever_built_it() {
        let (_, o) = seed3();
        let w = o.world();
        // a tile of the open ground with a lamp on it
        let t = w
            .tiles
            .iter()
            .map(|(p, _)| p)
            .filter(|p| p.x > 8 && p.y > 8 && o.owner(*p).is_none())
            .find(|p| {
                detail::ground(w, p.x, p.y)
                    .iter()
                    .any(|(_, c)| c.feature == Some(Feature::Lamp))
            })
            .expect("a lamp on the open ground");
        let round = |r: i32| {
            Rect::new(
                TILE * (t.x - r),
                TILE * (t.y - r),
                TILE * (2 * r + 1),
                TILE * (2 * r + 1),
            )
        };
        let mut small = Stream::default();
        small.cover(w, round(1));
        let mut big = Stream::default();
        big.cover(w, round(3));
        // and one walked in from the west, each step building a column of its own
        let mut walked = Stream::default();
        for k in -4..=1 {
            walked.cover(w, Rect::new(TILE * (t.x + k), TILE * t.y, TILE, TILE));
        }
        let middle = Rect::new(TILE * t.x, TILE * t.y, TILE, TILE);
        assert!(
            middle.iter().any(|p| lamp_at(&small, p) > 0),
            "the lamp lights its tile"
        );
        for p in middle.iter() {
            assert_eq!(lamp_at(&small, p), lamp_at(&big, p), "{p:?}");
        }
        // every tile either built, to its edges, lights as the big one's
        for s in [&small, &walked] {
            for p in round(3).iter().filter(|p| s.street.tile(*p).is_some()) {
                assert_eq!(lamp_at(s, p), lamp_at(&big, p), "{p:?}");
            }
        }
    }

    #[test]
    fn a_copy_of_the_world_streams_afresh() {
        let (city, mut o) = seed3();
        let origin = o.origin(city);
        let area = Rect::new(origin.x, origin.y, TILE, TILE);
        o.stream.cover(o.world.as_ref().unwrap(), area);
        let held = o.stream.street.held() + o.stream.net.held();
        assert!(held > 0);
        assert!(format!("{o:?}").contains(&format!("Stream {{ tiles: {held} }}")));
        let copy = o.clone();
        assert_eq!(copy.stream.street.held() + copy.stream.net.held(), 0);
        let mine = o.world().city_tiles(o.home().city)[0];
        assert_eq!(
            copy.owner(mine),
            Some(o.home()),
            "who owns what comes along"
        );
    }

    #[test]
    fn the_stream_forgets_far_tiles() {
        let (_, o) = seed3();
        let w = o.world();
        let mut stream = Stream::default();
        // east along a row, then south: 400 tiles, four at a stride
        let path = (2..=312)
            .step_by(4)
            .map(|x| Pos::new(x, 10))
            .chain((10..=100).step_by(4).map(|y| Pos::new(312, y)));
        let start = Pos::new(TILE * 2, TILE * 10);
        for t in path {
            let at = Pos::new(TILE * t.x + TILE / 2, TILE * t.y + TILE / 2);
            stream.cover(w, crate::outside::ground::sight_area(at));
            assert!(
                stream.street.held() <= CAP && stream.net.held() <= CAP,
                "{stream:?} at {t:?}"
            );
            assert!(stream.street.tile(at).is_some() && stream.net.tile(at).is_some());
        }
        assert!(
            stream.street.tile(start).is_none(),
            "the start is forgotten"
        );
    }
}
