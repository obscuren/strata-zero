//! A venue's frame: the building it stands in. Its footprint, its floors, its doors to
//! each stratum and where crawls reach it come from a city's building, or from a type's
//! own frame when no building gives one.

use super::VenueType;
use crate::geom::{Dir, Pos, Rect, Size};
use crate::ids::BuildingId;
use crate::interior::layouts::{exit_tile_on, floor_tile, glazed_edges, is_corner, outward};
use crate::interior::plan::{bridge_level, floor_size, storeys};
use crate::rng::hash64;
use crate::tile::Terrain;
use crate::world::{BuildingKind, City, ConnectorKind, StratumKind, Theme};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueFrame {
    pub building: Option<BuildingId>,
    pub kind: BuildingKind,
    pub theme: Theme,
    pub footprint: Rect,
    /// The footprint x 3, at least 12 x 9: every level at or above -1.
    pub size: Size,
    /// The building's lowest and top level: lowest is -1 when it reaches the Undercity.
    pub storeys: (i32, i32),
    pub doors: Vec<FrameDoor>,
    pub crawls: Vec<FrameCrawl>,
    /// The sides with windows: those facing open ground on the street.
    pub faces: Vec<Dir>,
}

/// A door from the city into the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameDoor {
    pub stratum: StratumKind,
    pub level: i32,
    /// The city-map door, the city tile you step out onto, and the floor tile (None: the
    /// roof door, which the layout stands beside a stair).
    pub door: Pos,
    pub landing: Pos,
    pub at: Option<Pos>,
    /// The ground outside it has no other way off (`enclosed`): nobody comes to it but
    /// through the building, so it is no way in from the city and takes no lock. A type's
    /// own frame stands in no city: none of its doors is.
    #[serde(default)]
    pub enclosed: bool,
}

/// Where a crawl reaches the frame from a stratum: the city tile you go in from, and
/// the floor tile on `level` it comes out on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameCrawl {
    pub stratum: StratumKind,
    pub level: i32,
    pub outside: Pos,
    pub at: Pos,
}

/// A building's layout seed: the city seed and the building, as a floor's is.
pub fn building_seed(city_seed: u64, b: BuildingId) -> u64 {
    hash64(city_seed, b.index() as u64)
}

/// The level a door onto stratum `k` opens from: the tunnel's -1, the street's 0, the
/// `bridge` floor's skyways and the roof's top.
fn door_level(k: StratumKind, top: i32, bridge: i32) -> i32 {
    match k {
        StratumKind::Undercity => -1,
        StratumKind::Skyways => bridge,
        StratumKind::Heights => top,
        StratumKind::Street | StratumKind::Net => 0,
    }
}

/// A door on stratum `k` at footprint border tile `door`; the roof door has no floor
/// tile and lands on itself.
fn frame_door(fp: Rect, size: Size, k: StratumKind, level: i32, door: Pos) -> FrameDoor {
    let (landing, at) = if k == StratumKind::Heights {
        (door, None)
    } else {
        (outward(fp, door), Some(exit_tile_on(fp, size, door)))
    };
    FrameDoor {
        stratum: k,
        level,
        door,
        landing,
        at,
        enclosed: false,
    }
}

/// Whether the ground outside `door` on stratum `k` has no other way off: the walkable
/// tiles its `landing` joins on `k`, never walking through a door, reach no other door (of
/// any building, this one's too), no connector and no edge of the map. A landing nobody
/// can stand on is not taken for enclosed.
fn enclosed(city: &City, k: StratumKind, door: Pos, landing: Pos) -> bool {
    let Some(s) = city.stratum(k) else {
        return false;
    };
    let (w, h) = (s.tiles.size().w as i32, s.tiles.size().h as i32);
    let edge = |p: Pos| p.x == 0 || p.y == 0 || p.x == w - 1 || p.y == h - 1;
    let ways_off: HashSet<Pos> = (city.connectors.iter())
        .filter(|c| c.kind != ConnectorKind::Jack && c.serves.len() > 1 && c.serves.contains(&k))
        .map(|c| c.pos)
        .collect();
    let mut seen = HashSet::from([door, landing]);
    let mut queue = VecDeque::from([landing]);
    while let Some(p) = queue.pop_front() {
        let Some(t) = s.tiles.get(p).filter(|t| t.is_walkable()) else {
            if p == landing {
                return false;
            }
            continue;
        };
        let another_door = p != door && t.terrain == Terrain::Door;
        if another_door || edge(p) || ways_off.contains(&p) {
            return false;
        }
        for n in p.neighbors4() {
            if seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    true
}

/// The roof crawl: the roof tile nearest the footprint's centre that isn't a roof door,
/// coming out on the floor tile below it on the top level.
fn roof_crawl(
    fp: Rect,
    size: Size,
    top: i32,
    doors: &[Pos],
    roof: impl Fn(Pos) -> bool,
) -> Option<FrameCrawl> {
    let centre = fp.center();
    fp.iter()
        .filter(|p| roof(*p) && !doors.contains(p))
        .min_by_key(|p| (p.dist2(centre), p.y, p.x))
        .map(|p| FrameCrawl {
            stratum: StratumKind::Heights,
            level: top,
            outside: p,
            at: floor_tile(fp, size, p),
        })
}

/// The tunnel crawl: of the tunnel tiles beside the footprint's sides, the one farthest
/// from the Undercity doors (then nearest the footprint's centre), coming out on -1 one
/// tile in from the frame wall it is beside.
fn tunnel_crawl(
    fp: Rect,
    size: Size,
    doors: &[Pos],
    tunnel: impl Fn(Pos) -> bool,
) -> Option<FrameCrawl> {
    let centre = fp.center();
    let landings: Vec<Pos> = doors.iter().map(|d| outward(fp, *d)).collect();
    fp.border()
        .filter(|p| !is_corner(fp, *p))
        .map(|p| (p, outward(fp, p)))
        .filter(|(_, o)| tunnel(*o) && !landings.contains(o))
        .min_by_key(|(_, o)| {
            let far = doors.iter().map(|d| d.manhattan(*o)).min().unwrap_or(0);
            (-far, o.dist2(centre), o.y, o.x)
        })
        .map(|(p, o)| {
            let wall = exit_tile_on(fp, size, p);
            FrameCrawl {
                stratum: StratumKind::Undercity,
                level: -1,
                outside: o,
                at: wall.offset(p.x - o.x, p.y - o.y),
            }
        })
}

/// A footprint edge, as `layouts::edge` numbers it, as the side it faces.
fn side(edge: u8) -> Dir {
    match edge {
        0 => Dir::W,
        1 => Dir::E,
        2 => Dir::N,
        _ => Dir::S,
    }
}

/// The footprint border tile `at` tiles along `side` from its start (its west or north
/// end).
fn along(fp: Rect, side: Dir, at: i32) -> Pos {
    match side {
        Dir::N => Pos::new(fp.x + at, fp.y),
        Dir::S => Pos::new(fp.x + at, fp.bottom() - 1),
        Dir::W => Pos::new(fp.x, fp.y + at),
        _ => Pos::new(fp.right() - 1, fp.y + at),
    }
}

impl VenueFrame {
    /// A closed building's frame, as the city made it; `None` for an open building.
    pub fn of_building(city: &City, id: BuildingId) -> Option<VenueFrame> {
        let b = city.building_opt(id)?;
        if !b.kind.is_closed() {
            return None;
        }
        let fp = b.footprint;
        let size = floor_size(fp);
        let (lowest, top) = storeys(city, id);
        let portals = city.portals(id);
        let doors = portals
            .iter()
            .map(|&(k, door)| {
                let d = frame_door(fp, size, k, door_level(k, top, bridge_level(top)), door);
                FrameDoor {
                    enclosed: enclosed(city, k, d.door, d.landing),
                    ..d
                }
            })
            .collect();
        let doors_on = |k: StratumKind| -> Vec<Pos> {
            portals
                .iter()
                .filter(|(s, _)| *s == k)
                .map(|(_, p)| *p)
                .collect()
        };
        // A crawl starts on open roof, or in a tunnel as an Undercity door opens onto.
        let walkable = |k: StratumKind, p: Pos, terrains: &[Terrain]| {
            city.tile(k, p)
                .is_some_and(|t| terrains.contains(&t.terrain) && t.is_walkable())
        };
        let mut crawls = Vec::new();
        if b.span.contains(StratumKind::Heights) {
            let doors = doors_on(StratumKind::Heights);
            crawls.extend(roof_crawl(fp, size, top, &doors, |p| {
                walkable(StratumKind::Heights, p, &[Terrain::Rooftop])
            }));
        }
        if b.span.contains(StratumKind::Undercity) {
            let doors = doors_on(StratumKind::Undercity);
            let tunnel = [Terrain::Tunnel, Terrain::Rail, Terrain::Grate];
            crawls.extend(tunnel_crawl(fp, size, &doors, |p| {
                walkable(StratumKind::Undercity, p, &tunnel)
            }));
        }
        let glazed = glazed_edges(city, fp);
        let faces = Dir::CARDINAL
            .into_iter()
            .filter(|d| glazed.iter().any(|e| side(*e) == *d))
            .collect();
        Some(VenueFrame {
            building: Some(id),
            kind: b.kind,
            theme: city.district(b.district).theme,
            footprint: fp,
            size,
            storeys: (lowest, top),
            doors,
            crawls,
            faces,
        })
    }

    /// A type's own frame, for a venue no building gives one: its footprint at the
    /// origin, its doors along their sides, a roof door above the first street door on
    /// an open roof, a tunnel along the west side, and windows on every side. It stands
    /// in no district, so it takes the Corp Core's look.
    pub fn own(t: &VenueType) -> VenueFrame {
        let f = &t.frame;
        let fp = Rect::new(0, 0, f.footprint.0, f.footprint.1);
        let size = floor_size(fp);
        let top = f.top;
        let bridge = f.bridge.unwrap_or_else(|| bridge_level(top));
        let mut doors: Vec<FrameDoor> = f
            .doors
            .iter()
            .map(|d| {
                let door = along(fp, d.side, d.at);
                frame_door(
                    fp,
                    size,
                    d.stratum,
                    door_level(d.stratum, top, bridge),
                    door,
                )
            })
            .collect();
        let mut crawls = Vec::new();
        if f.roof {
            // As the city places a roof door on an open roof: above the main street
            // door, else on the tile nearest the centre.
            let centre = fp.center();
            let roof = doors
                .iter()
                .find(|d| d.stratum == StratumKind::Street)
                .map(|d| d.door)
                .unwrap_or_else(|| {
                    fp.iter()
                        .min_by_key(|p| (p.dist2(centre), p.y, p.x))
                        .unwrap_or(centre)
                });
            doors.push(frame_door(fp, size, StratumKind::Heights, top, roof));
            crawls.extend(roof_crawl(fp, size, top, &[roof], |_| true));
        }
        if f.undercity {
            let tunnels: Vec<Pos> = doors
                .iter()
                .filter(|d| d.stratum == StratumKind::Undercity)
                .map(|d| d.door)
                .collect();
            crawls.extend(tunnel_crawl(fp, size, &tunnels, |p| {
                p.x == fp.x - 1 && (fp.y..fp.bottom()).contains(&p.y)
            }));
        }
        VenueFrame {
            building: None,
            kind: f.kind,
            theme: Theme::CorpCore,
            footprint: fp,
            size,
            storeys: (if f.undercity { -1 } else { 0 }, top),
            doors,
            crawls,
            faces: Dir::CARDINAL.to_vec(),
        }
    }

    /// The frame with its top raised to `top` (the roof door moves up with it, and so
    /// does the roof crawl); a `top` at or below the frame's leaves it as it is.
    pub fn raised(mut self, top: i32) -> VenueFrame {
        if top <= self.storeys.1 {
            return self;
        }
        self.storeys.1 = top;
        for d in self
            .doors
            .iter_mut()
            .filter(|d| d.stratum == StratumKind::Heights)
        {
            d.level = top;
        }
        for c in self
            .crawls
            .iter_mut()
            .filter(|c| c.stratum == StratumKind::Heights)
        {
            c.level = top;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interior::FloorPlan;
    use crate::interior::venue::VenuePack;
    use crate::interior::venue::testutil::cities;
    use crate::world::{BuildingKind, StratumKind};

    #[test]
    fn a_buildings_frame_is_its_footprint_floors_and_doors() {
        for c in cities() {
            for b in c
                .buildings
                .iter()
                .filter(|b| matches!(b.kind, BuildingKind::Hotel | BuildingKind::Office))
            {
                let f = VenueFrame::of_building(&c, b.id).unwrap();
                let classic = FloorPlan::classic(&c, b.id).unwrap();
                assert_eq!(f.size, classic.size);
                assert_eq!(f.storeys, (classic.lowest(), classic.top()));
                for (k, door) in c.portals(b.id) {
                    let d = f
                        .doors
                        .iter()
                        .find(|d| d.door == door && d.stratum == k)
                        .unwrap();
                    assert_eq!(Some(d.level), classic.door_level(k));
                    if k != StratumKind::Heights {
                        assert!(d.at.is_some());
                    }
                }
                assert_eq!(f.doors.len(), c.portals(b.id).len());
            }
        }
    }

    #[test]
    fn the_own_frame_is_the_types_frame() {
        for t in &VenuePack::builtin().types {
            let f = VenueFrame::own(t);
            assert_eq!(
                (f.size.w as i32, f.size.h as i32),
                (
                    (t.frame.footprint.0 * 3).max(12),
                    (t.frame.footprint.1 * 3).max(9)
                )
            );
            assert_eq!(
                f.storeys,
                (if t.frame.undercity { -1 } else { 0 }, t.frame.top)
            );
            assert_eq!(
                f.doors.len(),
                t.frame.doors.len() + usize::from(t.frame.roof)
            );
            assert!(f.building.is_none());
        }
    }
}
