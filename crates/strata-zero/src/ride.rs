//! Lanes carry you: on the open Net a step along a lane runs on to the next stop.
//! Spec `docs/superpowers/specs/2026-11-03-net-look-design.md`, "Lanes carry you".
//! The lanes are the Net's wherever it goes: the city's, and past its edge the open
//! Net's trunks and the other cities' and towns' lanes.

use crate::render::map::TileMap;
use citygen::{Dir, Feature, Pos, Terrain};

/// Ticks a ride takes per tile.
pub const RIDE_TICKS: u64 = 1;

/// A ride in flight: the tiles still to ride (its stop last), the ones left behind, and
/// when the next tile is taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ride {
    pub path: Vec<Pos>,
    pub done: Vec<Pos>,
    pub next_at: u64,
    /// Its stop is only where the Net wasn't built yet when the path was found
    /// (`runs_on`): from there it rides on over what has come in since.
    pub runs_on: bool,
}
/// Whether you ride the lanes: the Data surfer talent.
pub fn surfs(st: &crate::app::AppState) -> bool {
    st.character.flags(&st.content).contains("data_surfer")
}

/// Slipstream's odds a tile ridden, in percent, and the share of each trace it sheds.
pub const SLIP_CHANCE: u64 = 5;
pub const SLIP_SHARE: u32 = 10;

/// A tile ridden with the Slipstream talent: a `SLIP_CHANCE` in a hundred that every
/// trace on you (a building's, a district's) sheds `SLIP_SHARE` percent of its heat,
/// at least one. Whether it did.
pub fn slipstream(st: &mut crate::app::AppState, at: Pos) -> bool {
    if !st.character.flags(&st.content).contains("slipstream") {
        return false;
    }
    let seed = st.city.as_ref().map_or(0, |c| c.seed);
    let roll = citygen::rng::hash64(
        seed ^ st.clock_tick(),
        (at.x as u64) << 32 | (at.y as u32 as u64),
    ) % 100;
    #[cfg(test)]
    let roll = match st.net.force_slip {
        Some(true) => 0,
        Some(false) => 99,
        None => roll,
    };
    if roll >= SLIP_CHANCE {
        return false;
    }
    let mut shed = false;
    let net = &mut st.net;
    for t in net
        .traces
        .values_mut()
        .chain(net.district_traces.values_mut())
    {
        if t.heat > 0 {
            let cut = (t.heat as u32 * SLIP_SHARE).div_ceil(100).max(1) as u8;
            t.heat = t.heat.saturating_sub(cut);
            shed = true;
        }
    }
    if shed {
        st.say("slipstream · the trace loses a tenth of you");
    }
    shed
}

/// The longest ride, so a loop of lane can never run away.
const MAX_RIDE: usize = 512;

fn lane(net: &dyn TileMap, p: Pos) -> bool {
    net.tile(p).is_some_and(|t| t.terrain == Terrain::NetLane)
}

/// A hub's gate: a gate no construct owns, walked like lane.
fn hub_gate(net: &dyn TileMap, p: Pos) -> bool {
    net.tile(p)
        .is_some_and(|t| t.terrain == Terrain::NetGate && t.building.is_none())
}

fn construct_gate(net: &dyn TileMap, p: Pos) -> bool {
    net.tile(p)
        .is_some_and(|t| t.terrain == Terrain::NetGate && t.building.is_some())
}

fn rideable(net: &dyn TileMap, p: Pos) -> bool {
    lane(net, p) || hub_gate(net, p)
}

fn has_node(net: &dyn TileMap, p: Pos) -> bool {
    net.tile(p)
        .is_some_and(|t| matches!(t.feature, Some(Feature::Node(_))))
}

/// The tiles a step from `from` toward `dir` rides over the Net `net`, up to and
/// including its stop; empty when the step does not run along a lane. A ride starts on
/// a lane or a hub's gate, onto lane; it follows the lane round a bend and stops on a
/// junction, a hub's gate, a node, the lane's end, or beside a construct's gate (it
/// never goes in). Where the Net isn't built yet the lane ends for now (`runs_on`).
pub fn ride_path(net: &dyn TileMap, from: Pos, dir: Dir) -> Vec<Pos> {
    let first = from.step(dir);
    if !rideable(net, from) || !lane(net, first) {
        return Vec::new();
    }
    let mut path = vec![first];
    let (mut came, mut at) = (from, first);
    while path.len() < MAX_RIDE {
        if has_node(net, at) || hub_gate(net, at) {
            break;
        }
        let around: Vec<Pos> = at.neighbors4().filter(|q| *q != came).collect();
        if around.iter().any(|q| construct_gate(net, *q)) {
            break;
        }
        let ways: Vec<Pos> = around.into_iter().filter(|q| rideable(net, *q)).collect();
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

/// Whether a ride that came from `came` onto `at` may have stopped there only because
/// the Net past it isn't built yet: a plain lane cell with no way on and a cell beside
/// it not built.
fn cut_short(net: &dyn TileMap, came: Pos, at: Pos) -> bool {
    let around = || at.neighbors4().filter(move |q| *q != came);
    lane(net, at)
        && !has_node(net, at)
        && !around().any(|q| rideable(net, q) || construct_gate(net, q))
        && around().any(|q| net.tile(q).is_none())
}

/// Whether `path`, a ride from `from` over `net` as `ride_path` found it, stopped only
/// short of the Net not built yet (`cut_short` at its stop): once the Net past it has
/// come in, it rides on (`ride_path` from its last tile but one). Asked of a map that
/// carries on past the city's edge; on one with nothing past it, a cell off it is
/// nothing to wait for.
pub fn runs_on(net: &dyn TileMap, from: Pos, path: &[Pos]) -> bool {
    let came = path.len().checked_sub(2).map_or(from, |i| path[i]);
    path.last().is_some_and(|at| cut_short(net, came, *at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::map::MapRef;
    use citygen::{BuildingId, City, NodeKind, StratumKind, Tile};

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

    /// The ride a step `dir` from `from` takes over `c`'s Net.
    fn ride(c: &City, from: (i32, i32), dir: Dir) -> Vec<Pos> {
        let net = MapRef::stratum_of(c, StratumKind::Net);
        ride_path(&net, Pos::new(from.0, from.1), dir)
    }

    fn end(c: &City, from: (i32, i32), dir: Dir) -> Option<Pos> {
        ride(c, from, dir).last().copied()
    }

    /// `c`'s Net built only west of column `past`, as the stream holds the open Net
    /// round the view.
    struct Built<'a> {
        net: MapRef<'a>,
        past: i32,
    }

    impl TileMap for Built<'_> {
        fn size(&self) -> citygen::Size {
            self.net.size()
        }
        fn tile(&self, p: Pos) -> Option<&citygen::Tile> {
            self.net.tile(p).filter(|_| p.x < self.past)
        }
        fn light(&self, _p: Pos) -> u8 {
            0
        }
        fn theme(&self, _p: Pos) -> Option<citygen::Theme> {
            None
        }
        fn kind(&self, _p: Pos) -> Option<citygen::BuildingKind> {
            None
        }
        fn connector(&self, _p: Pos) -> Option<crate::render::ConnDir> {
            None
        }
    }

    #[test]
    fn a_ride_stops_where_the_net_isnt_built_and_runs_on_once_it_is() {
        let c = net(&[
            "........", //
            "========", //
            "...=....", //
        ]);
        let cut = Built {
            net: MapRef::stratum_of(&c, StratumKind::Net),
            past: 3,
        };
        let path = ride_path(&cut, Pos::new(0, 1), Dir::E);
        assert_eq!(
            path,
            vec![Pos::new(1, 1), Pos::new(2, 1)],
            "up to what's built"
        );
        assert!(runs_on(&cut, Pos::new(0, 1), &path));
        // built on: from its last tile but one, on to the junction, a stop
        let on = ride(&c, (1, 1), Dir::E);
        assert_eq!(on, vec![Pos::new(2, 1), Pos::new(3, 1)]);
        let full = MapRef::stratum_of(&c, StratumKind::Net);
        assert!(!runs_on(&full, Pos::new(1, 1), &on));
        // a lane's end or a loop closing is a stop
        let c = net(&[
            "........", //
            "=====...", //
            "........", //
        ]);
        let full = MapRef::stratum_of(&c, StratumKind::Net);
        assert!(!runs_on(&full, Pos::new(0, 1), &ride(&c, (0, 1), Dir::E)));
        let c = net(&[
            "......", //
            ".====.", //
            ".=..=.", //
            ".====.", //
            "......", //
        ]);
        let full = MapRef::stratum_of(&c, StratumKind::Net);
        assert!(!runs_on(&full, Pos::new(1, 1), &ride(&c, (1, 1), Dir::E)));
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
        assert!(ride(&c, (2, 0), Dir::S).is_empty(), "onto the lane");
        assert!(ride(&c, (2, 1), Dir::N).is_empty(), "off it");
        assert_eq!(end(&c, (2, 1), Dir::W), Some(Pos::new(0, 1)));
    }

    #[test]
    fn a_loop_of_lane_stops() {
        let c = net(&[
            "====", //
            "=..=", //
            "====", //
        ]);
        let path = ride(&c, (0, 0), Dir::E);
        assert!(path.len() < 12, "{path:?}");
    }
}
