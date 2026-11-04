//! Lanes carry you: on the open Net a step along a lane runs on to the next stop.
//! Spec `docs/superpowers/specs/2026-11-03-net-look-design.md`, "Lanes carry you".

use citygen::{City, Dir, Feature, Pos, StratumKind, Terrain, Tile};

/// Ticks a ride takes per tile.
pub const RIDE_TICKS: u64 = 1;

/// A ride in flight: the tiles still to ride (its stop last), the ones left behind, and
/// when the next tile is taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ride {
    pub path: Vec<Pos>,
    pub done: Vec<Pos>,
    pub next_at: u64,
}
/// The longest ride, so a loop of lane can never run away.
const MAX_RIDE: usize = 512;

fn tile(city: &City, p: Pos) -> Option<&Tile> {
    city.tile(StratumKind::Net, p)
}

fn lane(city: &City, p: Pos) -> bool {
    tile(city, p).is_some_and(|t| t.terrain == Terrain::NetLane)
}

/// A hub's gate: a gate no construct owns, walked like lane.
fn hub_gate(city: &City, p: Pos) -> bool {
    tile(city, p).is_some_and(|t| t.terrain == Terrain::NetGate && t.building.is_none())
}

fn construct_gate(city: &City, p: Pos) -> bool {
    tile(city, p).is_some_and(|t| t.terrain == Terrain::NetGate && t.building.is_some())
}

fn rideable(city: &City, p: Pos) -> bool {
    lane(city, p) || hub_gate(city, p)
}

fn has_node(city: &City, p: Pos) -> bool {
    tile(city, p).is_some_and(|t| matches!(t.feature, Some(Feature::Node(_))))
}

/// The tiles a step from `from` toward `dir` rides, up to and including its stop; empty
/// when the step does not run along a lane. A ride starts on a lane or a hub's gate,
/// onto lane; it follows the lane round a bend and stops on a junction, a hub's gate, a
/// node, the lane's end, or beside a construct's gate (it never goes in).
pub fn ride_path(city: &City, from: Pos, dir: Dir) -> Vec<Pos> {
    let first = from.step(dir);
    if !rideable(city, from) || !lane(city, first) {
        return Vec::new();
    }
    let mut path = vec![first];
    let (mut came, mut at) = (from, first);
    while path.len() < MAX_RIDE {
        if has_node(city, at) || hub_gate(city, at) {
            break;
        }
        let around: Vec<Pos> = at.neighbors4().filter(|q| *q != came).collect();
        if around.iter().any(|q| construct_gate(city, *q)) {
            break;
        }
        let ways: Vec<Pos> = around.into_iter().filter(|q| rideable(city, *q)).collect();
        let [next] = ways[..] else {
            break;
        };
        if path.contains(&next) || next == from {
            break;
        }
        path.push(next);
        (came, at) = (at, next);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{BuildingId, NodeKind, Tile};

    /// A Net of open grid with lanes, gates and nodes painted from rows of text:
    /// `=` lane, `h` hub gate, `g` a construct's gate, `n` a node on lane, `.` grid.
    fn net(rows: &[&str]) -> City {
        let h = rows.len() as u32;
        let w = rows[0].chars().count() as u32;
        let mut city = City::empty(citygen::GenParams {
            seed: 42,
            size: citygen::SizePreset::Custom { w, h },
            ..Default::default()
        });
        city.strata
            .push(citygen::Stratum::new(StratumKind::Net, city.size));
        let s = city.strata.last_mut().unwrap();
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                let p = Pos::new(x as i32, y as i32);
                let mut t = Tile::new(Terrain::NetGrid);
                match c {
                    '=' => t.terrain = Terrain::NetLane,
                    'h' => t.terrain = Terrain::NetGate,
                    'g' => {
                        t.terrain = Terrain::NetGate;
                        t.building = Some(BuildingId(0));
                    }
                    'n' => {
                        t.terrain = Terrain::NetLane;
                        t.feature = Some(Feature::Node(NodeKind::Camera));
                    }
                    _ => {}
                }
                s.tiles[p] = t;
            }
        }
        city
    }

    fn end(c: &City, from: (i32, i32), dir: Dir) -> Option<Pos> {
        ride_path(c, Pos::new(from.0, from.1), dir).last().copied()
    }

    #[test]
    fn a_ride_stops_at_the_junction() {
        let c = net(&[
            "...=....", //
            "=======.", //
            "...=....", //
        ]);
        assert_eq!(end(&c, (0, 1), Dir::E), Some(Pos::new(3, 1)));
        // From the junction on, to the lane's end.
        assert_eq!(end(&c, (3, 1), Dir::E), Some(Pos::new(6, 1)));
    }

    #[test]
    fn a_ride_follows_a_bend() {
        let c = net(&[
            "====....", //
            "...=....", //
            "...====.", //
        ]);
        assert_eq!(end(&c, (0, 0), Dir::E), Some(Pos::new(6, 2)));
    }

    #[test]
    fn a_ride_stops_beside_a_construct_gate_and_on_a_hub_gate() {
        let c = net(&[
            "=====g..", //
            "........", //
            "===h===.", //
        ]);
        assert_eq!(end(&c, (0, 0), Dir::E), Some(Pos::new(4, 0)), "never in");
        // Onto lane, then the hub's gate is a stop; from it the ride goes on.
        assert_eq!(
            end(&c, (0, 2), Dir::E),
            Some(Pos::new(3, 2)),
            "a hub's gate is a stop"
        );
        assert_eq!(end(&c, (3, 2), Dir::E), Some(Pos::new(6, 2)));
    }

    #[test]
    fn a_ride_stops_on_a_node() {
        let c = net(&["===n===."]);
        assert_eq!(end(&c, (0, 0), Dir::E), Some(Pos::new(3, 0)));
    }

    #[test]
    fn a_step_off_or_across_a_lane_does_not_ride() {
        let c = net(&[
            "........", //
            "========", //
            "........", //
        ]);
        assert!(
            ride_path(&c, Pos::new(2, 0), Dir::S).is_empty(),
            "onto the lane"
        );
        assert!(ride_path(&c, Pos::new(2, 1), Dir::N).is_empty(), "off it");
        assert_eq!(end(&c, (2, 1), Dir::W), Some(Pos::new(0, 1)));
    }

    #[test]
    fn a_loop_of_lane_stops() {
        let c = net(&[
            "====", //
            "=..=", //
            "====", //
        ]);
        let path = ride_path(&c, Pos::new(0, 0), Dir::E);
        assert!(path.len() < 12, "{path:?}");
    }
}
