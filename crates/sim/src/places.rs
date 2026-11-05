//! What the city offers people: street doors, kerbs, lots, beats, corners, turf and
//! posts, derived once from the map.

use crate::paths::walk_cost;
use citygen::pathing::bfs_nearest;
use citygen::world::{BuildingKind, City, StratumKind, Theme};
use citygen::{BuildingId, DistrictId, Pos, Terrain};
use std::collections::{HashMap, HashSet};

/// Terrain a door may open onto.
pub fn is_street(t: Terrain) -> bool {
    matches!(
        t,
        Terrain::Sidewalk
            | Terrain::Plaza
            | Terrain::Alley
            | Terrain::Road
            | Terrain::Walkway
            | Terrain::Bridge
    )
}

fn terrain(city: &City, p: Pos) -> Option<Terrain> {
    city.tile(StratumKind::Street, p).map(|t| t.terrain)
}

#[derive(Default)]
pub struct Places {
    /// Street doors per building: Door tiles on the footprint's edge with the street outside.
    pub doors: HashMap<BuildingId, Vec<Pos>>,
    /// Road tiles beside a sidewalk, where a car can stop.
    pub kerbs: Vec<Pos>,
    /// Road tiles beside a Parking or Garage building.
    pub lots: Vec<(BuildingId, Vec<Pos>)>,
    /// Where sex workers stand: Strip sidewalks under club and bar fronts, else Night
    /// Market alleys.
    pub beats: HashMap<DistrictId, Vec<Pos>>,
    /// Sidewalk tiles at road crossings, per district.
    pub corners: HashMap<DistrictId, Vec<Pos>>,
    /// The sidewalk ring around each Megablock.
    pub turf: HashMap<BuildingId, Vec<Pos>>,
    /// The street tile beside a building's first door, where muscle stands.
    pub posts: HashMap<BuildingId, Pos>,
    /// Cells of the map's edge a road crosses, where people come in from off the map
    /// (`commute::ways_in`).
    pub ways_in: Vec<Pos>,
    /// Somewhere to get something to eat at midday, each by its first door: noodle bars,
    /// markets and bars, in building order.
    pub food: Vec<(BuildingId, Pos)>,
    /// Where a drink after work is had in this city: of a bar, a noodle bar and the
    /// Strip, those it has, in that order.
    pub drinks: Vec<crate::content::Leisure>,
    kerb_set: HashSet<Pos>,
}

impl Places {
    pub fn derive(city: &City) -> Places {
        let mut doors: HashMap<BuildingId, Vec<Pos>> = HashMap::new();
        let mut posts = HashMap::new();
        for b in &city.buildings {
            let mut found = Vec::new();
            for p in b.footprint.border() {
                if terrain(city, p) != Some(Terrain::Door) {
                    continue;
                }
                let outside = p.neighbors4().find(|n| {
                    !b.footprint.contains(*n) && terrain(city, *n).is_some_and(is_street)
                });
                if let Some(o) = outside {
                    if found.is_empty() {
                        posts.insert(b.id, o);
                    }
                    found.push(p);
                }
            }
            if !found.is_empty() {
                doors.insert(b.id, found);
            }
        }

        let mut kerbs = Vec::new();
        let mut corners: HashMap<DistrictId, Vec<Pos>> = HashMap::new();
        if let Some(street) = city.stratum(StratumKind::Street) {
            for (p, t) in street.tiles.iter() {
                match t.terrain {
                    Terrain::Road => {
                        if p.neighbors4()
                            .any(|n| terrain(city, n) == Some(Terrain::Sidewalk))
                        {
                            kerbs.push(p);
                        }
                    }
                    Terrain::Sidewalk => {
                        let diagonal_roads = p
                            .neighbors8()
                            .filter(|n| n.manhattan(p) == 2)
                            .filter(|n| terrain(city, *n) == Some(Terrain::Road))
                            .count();
                        if diagonal_roads >= 2
                            && let Some(d) = t.district
                        {
                            corners.entry(d).or_default().push(p);
                        }
                    }
                    _ => {}
                }
            }
        }
        let kerb_set: HashSet<Pos> = kerbs.iter().copied().collect();

        let lots = city
            .buildings
            .iter()
            .filter(|b| matches!(b.kind, BuildingKind::Parking | BuildingKind::Garage))
            .map(|b| {
                let tiles: Vec<Pos> = b
                    .footprint
                    .grow(1)
                    .border()
                    .filter(|p| kerb_set.contains(p))
                    .collect();
                (b.id, tiles)
            })
            .filter(|(_, t)| !t.is_empty())
            .collect();

        let mut beats: HashMap<DistrictId, Vec<Pos>> = HashMap::new();
        for d in city
            .districts
            .iter()
            .filter(|d| d.theme == Theme::NeonStrip)
        {
            let fronts: Vec<Pos> = city
                .buildings
                .iter()
                .filter(|b| {
                    b.district == d.id && matches!(b.kind, BuildingKind::Club | BuildingKind::Bar)
                })
                .flat_map(|b| {
                    let fp = b.footprint;
                    fp.grow(1)
                        .border()
                        .filter(move |p| p.neighbors4().any(|n| fp.contains(n)))
                        .collect::<Vec<_>>()
                })
                .filter(|p| d.bounds.contains(*p))
                .filter(|p| matches!(terrain(city, *p), Some(Terrain::Sidewalk | Terrain::Plaza)))
                .collect();
            if !fronts.is_empty() {
                beats.insert(d.id, fronts);
            }
        }
        if beats.is_empty() {
            for d in city
                .districts
                .iter()
                .filter(|d| d.theme == Theme::NightMarket)
            {
                let alleys: Vec<Pos> = d
                    .bounds
                    .iter()
                    .filter(|p| terrain(city, *p) == Some(Terrain::Alley))
                    .collect();
                if !alleys.is_empty() {
                    beats.insert(d.id, alleys);
                }
            }
        }

        let turf = city
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Megablock)
            .map(|b| {
                let ring: Vec<Pos> = b
                    .footprint
                    .grow(2)
                    .iter()
                    .filter(|p| !b.footprint.contains(*p))
                    .filter(|p| terrain(city, *p) == Some(Terrain::Sidewalk))
                    .collect();
                (b.id, ring)
            })
            .filter(|(_, t)| !t.is_empty())
            .collect();

        let door_set: HashSet<Pos> = doors.values().flatten().copied().collect();
        let ways_in = crate::commute::ways_in(city, &door_set);
        let has = |kind: BuildingKind| {
            city.buildings
                .iter()
                .any(|b| b.kind == kind && doors.contains_key(&b.id))
        };
        let food = city
            .buildings
            .iter()
            .filter(|b| {
                matches!(
                    b.kind,
                    BuildingKind::NoodleBar | BuildingKind::Market | BuildingKind::Bar
                )
            })
            .filter_map(|b| Some((b.id, *doors.get(&b.id)?.first()?)))
            .collect();
        use crate::content::Leisure;
        let drinks = [
            (Leisure::Bar, has(BuildingKind::Bar)),
            (Leisure::NoodleBar, has(BuildingKind::NoodleBar)),
            (Leisure::Strip, !beats.is_empty()),
        ]
        .into_iter()
        .filter_map(|(l, here)| here.then_some(l))
        .collect();

        Places {
            doors,
            kerbs,
            lots,
            beats,
            corners,
            turf,
            posts,
            ways_in,
            food,
            drinks,
            kerb_set,
        }
    }

    pub fn is_kerb(&self, p: Pos) -> bool {
        self.kerb_set.contains(&p)
    }

    /// The nearest kerb on foot from `from`, within a few hundred tiles.
    pub fn nearest_kerb(&self, city: &City, from: Pos) -> Option<Pos> {
        let path = bfs_nearest(
            city.size,
            from,
            |p| walk_cost(city, p, p).is_some(),
            |p| self.kerb_set.contains(&p),
            400,
        )?;
        path.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::{drive_path, walk_path};
    use citygen::{GenParams, Generator, SizePreset, Theme};

    fn city() -> City {
        Generator::run(GenParams {
            // Seed 6 at eight districts carries both a Night Market and a Neon Strip.
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap()
    }

    fn terrain(c: &City, p: Pos) -> Option<Terrain> {
        c.tile(StratumKind::Street, p).map(|t| t.terrain)
    }

    #[test]
    fn doors_sit_on_footprint_edges_with_the_street_outside() {
        let c = city();
        let pl = Places::derive(&c);
        let with_doors = c
            .buildings
            .iter()
            .filter(|b| pl.doors.get(&b.id).is_some_and(|d| !d.is_empty()))
            .count();
        assert!(
            with_doors * 10 >= c.buildings.len() * 8,
            "{with_doors} of {} buildings have a street door",
            c.buildings.len()
        );
        for b in &c.buildings {
            for &d in pl.doors.get(&b.id).map(|v| v.as_slice()).unwrap_or(&[]) {
                assert_eq!(terrain(&c, d), Some(Terrain::Door), "{d:?} of {:?}", b.id);
                assert!(
                    b.footprint.border().any(|p| p == d),
                    "{d:?} is on the edge of {:?}",
                    b.id
                );
                assert!(
                    d.neighbors4()
                        .any(|n| !b.footprint.contains(n) && terrain(&c, n).is_some_and(is_street)),
                    "{d:?} opens onto the street"
                );
            }
        }
    }

    #[test]
    fn kerbs_lots_corners_turf_and_posts() {
        let c = city();
        let pl = Places::derive(&c);
        assert!(!pl.kerbs.is_empty());
        for &k in &pl.kerbs {
            assert_eq!(terrain(&c, k), Some(Terrain::Road));
            assert!(
                k.neighbors4()
                    .any(|n| terrain(&c, n) == Some(Terrain::Sidewalk))
            );
        }
        for (_, tiles) in &pl.lots {
            assert!(tiles.iter().all(|&t| terrain(&c, t) == Some(Terrain::Road)));
        }
        assert!(
            pl.corners.values().any(|v| !v.is_empty()),
            "some district has corners"
        );
        for v in pl.corners.values() {
            assert!(v.iter().all(|&p| terrain(&c, p) == Some(Terrain::Sidewalk)));
        }
        for (b, tiles) in &pl.turf {
            let fp = c.buildings[b.index()].footprint;
            assert!(
                tiles
                    .iter()
                    .all(|&p| terrain(&c, p) == Some(Terrain::Sidewalk)
                        && !fp.contains(p)
                        && fp.grow(2).contains(p))
            );
        }
        for (b, &post) in &pl.posts {
            let doors = &pl.doors[b];
            assert!(terrain(&c, post).is_some_and(is_street));
            assert!(
                doors.iter().any(|d| d.manhattan(post) == 1),
                "the post is beside a door"
            );
        }
    }

    #[test]
    fn beats_are_strip_fronts_or_night_market_alleys() {
        let c = city();
        let pl = Places::derive(&c);
        let strip: Vec<_> = c
            .districts
            .iter()
            .filter(|d| d.theme == Theme::NeonStrip)
            .collect();
        let market: Vec<_> = c
            .districts
            .iter()
            .filter(|d| d.theme == Theme::NightMarket)
            .collect();
        assert!(
            !strip.is_empty() || !market.is_empty(),
            "the fixture needs a Strip or a Market"
        );
        if !strip.is_empty() {
            for d in &strip {
                let beats = &pl.beats[&d.id];
                assert!(!beats.is_empty(), "{} has beats", d.name);
                for &p in beats {
                    assert!(matches!(
                        terrain(&c, p),
                        Some(Terrain::Sidewalk | Terrain::Plaza)
                    ));
                    assert!(d.bounds.contains(p));
                    assert!(
                        c.buildings.iter().any(|b| matches!(
                            b.kind,
                            BuildingKind::Club | BuildingKind::Bar
                        ) && p
                            .neighbors4()
                            .any(|n| b.footprint.contains(n))),
                        "{p:?} is under a club or bar front"
                    );
                }
            }
        }
        // With the Strip gone, the beats fall back to the Night Market's alleys.
        let mut no_strip = c.clone();
        for d in &mut no_strip.districts {
            if d.theme == Theme::NeonStrip {
                d.theme = Theme::OldTown;
            }
        }
        let pl2 = Places::derive(&no_strip);
        if !market.is_empty() {
            let any = market
                .iter()
                .any(|d| pl2.beats.get(&d.id).is_some_and(|v| !v.is_empty()));
            assert!(any, "night market alleys carry the beat");
            for d in &market {
                for &p in pl2.beats.get(&d.id).map(|v| v.as_slice()).unwrap_or(&[]) {
                    assert_eq!(terrain(&c, p), Some(Terrain::Alley));
                }
            }
        }
    }

    #[test]
    fn paths_respect_their_terrain_and_kerbs_are_reachable() {
        let c = city();
        let pl = Places::derive(&c);
        let doors: Vec<Pos> = pl
            .doors
            .values()
            .filter_map(|v| v.first().copied())
            .collect();
        assert!(doors.len() >= 2);
        let mut walked = 0;
        for pair in doors.windows(2).take(12) {
            if let Some(path) = walk_path(&c, pair[0], pair[1]) {
                walked += 1;
                assert_eq!(path.first(), Some(&pair[0]));
                assert_eq!(path.last(), Some(&pair[1]));
                for &p in &path {
                    assert!(walk_cost(&c, p, p).is_some(), "{p:?} is pedestrian terrain");
                }
                assert!(path.windows(2).all(|w| w[0].manhattan(w[1]) == 1));
            }
        }
        assert!(walked >= 6, "most door pairs connect on foot: {walked}");
        let mut driven = 0;
        for pair in pl.kerbs.windows(7).take(12) {
            if let Some(path) = drive_path(&c, pair[0], pair[6]) {
                driven += 1;
                for &p in &path {
                    assert!(
                        matches!(
                            terrain(&c, p),
                            Some(Terrain::Road | Terrain::Bridge | Terrain::Alley)
                        ) || crate::paths::is_crossing(&c, p),
                        "{p:?} is road, alley or a crossing"
                    );
                }
            }
        }
        assert!(driven >= 6, "kerbs connect by road: {driven}");
        let door = doors[0];
        let kerb = pl.nearest_kerb(&c, door).expect("a kerb near a door");
        assert!(pl.kerbs.contains(&kerb));
        assert!(walk_path(&c, door, kerb).is_some());
    }
}
