//! A venue as a graph of its rooms, to find routes through it: each room of its levels
//! and each stratum its ways in open onto is a node, and each way between them (a door,
//! a doorless join, a link from one stop to another, a way in) is an edge, under the lock
//! that holds it shut. A route is walked by a way of playing: a set of means on top of open
//! doors and the cards and codes it finds on the way.

use super::mission::room_holding;
use super::{MissionLayer, VenueLayout, WayInKind, Ways};
use crate::geom::Pos;
use crate::interior::{DoorLock, LinkKind, LinkStop};
use crate::world::StratumKind;
use std::collections::{BTreeSet, VecDeque};

/// What a node of the graph is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeAt {
    Room {
        level: i32,
        room: usize,
    },
    /// The city on a stratum, beyond every way in from it.
    Outside(StratumKind),
}

/// What holds an edge shut, and the ways past it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphLock {
    /// The zone whose card (or person, by talk) opens it.
    pub zone: u8,
    pub ways: Ways,
    /// The code that opens it: (level, number).
    pub code: Option<(i32, u16)>,
    /// This direction passes without a key: a one-way lock's free side.
    pub free: bool,
}

/// A way from one node to another; a door is two, one each way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphEdge {
    pub from: usize,
    pub to: usize,
    pub lock: Option<GraphLock>,
    /// The planned door it goes through: (level, index in the level's doors).
    pub door: Option<(i32, usize)>,
}

/// What a route holds: the deepest zone's card, the deepest zone of anyone talked to, and
/// the codes it has found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub card: u8,
    pub talked: u8,
    pub codes: BTreeSet<(i32, u16)>,
}

/// The nodes a way reaches, and the keys it ends with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reach {
    pub nodes: Vec<bool>,
    pub keys: Keys,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VenueGraph {
    pub nodes: Vec<NodeAt>,
    pub edges: Vec<GraphEdge>,
    /// The open ways in; `Outside` nodes are where routes in start and routes out end.
    pub starts: Vec<usize>,
    /// What lies in a room: a card of a zone, a code, a person of a zone.
    pub cards: Vec<(usize, u8)>,
    pub codes: Vec<(usize, (i32, u16))>,
    pub people: Vec<(usize, u8)>,
    pub goal: Option<usize>,
}

/// The four ways of playing, each a set of means on top of open doors and the cards
/// and codes found on the way.
pub const PLAYS: [(&str, Ways); 4] = [
    ("force", Ways::FORCE),
    ("the Net", Ways::NET),
    ("the Keyjack", Ways(Ways::KEYJACK.0 | Ways::CRAWL.0)),
    ("talk", Ways::TALK),
];

/// The ways past a private room's keypad.
const KEYPAD: Ways = Ways(Ways::CODE.0 | Ways::KEYJACK.0 | Ways::NET.0 | Ways::FORCE.0);
/// The ways past a lift's card reader.
const LIFT: Ways = Ways(Ways::CARD.0 | Ways::NET.0 | Ways::TALK.0);
/// A crawl's grille: only a crawler passes.
const GRILLE: GraphLock = GraphLock {
    zone: 0,
    ways: Ways::CRAWL,
    code: None,
    free: false,
};

/// Whether an edge under `lock` passes for means `m` holding `k`.
pub(super) fn passes(lock: Option<&GraphLock>, m: Ways, k: &Keys) -> bool {
    let Some(l) = lock else { return true };
    l.free
        || (l.ways.has(Ways::CARD) && k.card >= l.zone)
        || (l.ways.has(Ways::CODE) && l.code.is_some_and(|c| k.codes.contains(&c)))
        || (l.ways.has(Ways::TALK) && m.has(Ways::TALK) && k.talked >= l.zone)
        || Ways(l.ways.0 & m.0 & !(Ways::CARD.0 | Ways::CODE.0 | Ways::TALK.0)).0 != 0
}

/// A door's lock on `level` as the graph holds it, free when `free`.
fn of_door(level: i32, l: &DoorLock, free: bool) -> GraphLock {
    GraphLock {
        zone: l.zone,
        ways: l.ways,
        code: l.ways.has(Ways::CODE).then_some((level, l.number)),
        free,
    }
}

/// Whether a link carries you from its stop `a` to its stop `b`: on at `a`, off at `b`,
/// and the way it runs. A link you can neither leave at its first stop nor board at its
/// last runs only up, and the other way round only down.
pub(super) fn rides(stops: &[LinkStop], a: usize, b: usize) -> bool {
    let (first, last) = (&stops[0], &stops[stops.len() - 1]);
    let up = !first.off && !last.on;
    let down = !first.on && !last.off;
    a != b && stops[a].on && stops[b].off && (!up || b > a) && (!down || b < a)
}

impl VenueGraph {
    /// Every level's rooms, doors (a private room's door is a Keypad lock coded by its
    /// number, ways Code | Keyjack | Net | Force), links (a lift edge carries a Card | Net | Talk
    /// lock at the card of either stop; a crawl edge a Grille lock, ways CRAWL), exits and crawl
    /// ways in (a Grille lock). `fitted` adds the `fit` lock of those doors; `kept` keeps only those
    /// levels, their links' stops and their ways in.
    ///
    /// A door joins its rooms both ways, a one-way lock free on the edge out of the room
    /// holding its free side; a doorless join is open. An exit joins its stratum's
    /// `Outside` and the room of its doorstep both ways, under its lock, free going out.
    /// The graph holds no cards, codes, people or goal.
    pub fn of(layout: &VenueLayout, fitted: &[(i32, usize)], kept: Option<&[i32]>) -> VenueGraph {
        let keeps = |level: i32| kept.is_none_or(|k| k.contains(&level));
        let mut g = VenueGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            starts: Vec::new(),
            cards: Vec::new(),
            codes: Vec::new(),
            people: Vec::new(),
            goal: None,
        };
        let levels: Vec<_> = layout.levels.iter().filter(|x| keeps(x.level)).collect();
        for x in &levels {
            g.nodes.extend((0..x.rooms.len()).map(|room| NodeAt::Room {
                level: x.level,
                room,
            }));
        }
        for x in &levels {
            for (k, d) in x.doors.iter().enumerate() {
                let (Some(a), Some(b)) = (g.room(x.level, d.rooms.0), g.room(x.level, d.rooms.1))
                else {
                    continue;
                };
                let door = Some((x.level, k));
                if d.open {
                    g.both(a, b, None, None, door);
                    continue;
                }
                let fit = d.fit.filter(|_| fitted.contains(&(x.level, k)));
                if let Some(l) = d.lock.or(fit) {
                    let inside =
                        |r: usize| l.free_from.is_some_and(|p| x.rooms[r].rect.contains(p));
                    let ab = of_door(x.level, &l, inside(d.rooms.0));
                    let ba = of_door(x.level, &l, inside(d.rooms.1));
                    g.both(a, b, Some(ab), Some(ba), door);
                    continue;
                }
                let private = [d.rooms.0, d.rooms.1]
                    .into_iter()
                    .find(|&r| x.rooms[r].private);
                let lock = private.map(|r| GraphLock {
                    zone: x.rooms[r].zone,
                    ways: KEYPAD,
                    code: Some((x.level, x.rooms[r].number)),
                    free: false,
                });
                g.both(a, b, lock, lock, door);
            }
        }
        for link in &layout.links {
            let at: Vec<Option<usize>> = (link.stops.iter())
                .map(|s| {
                    let x = levels.iter().find(|x| x.level == s.level)?;
                    let room = x.rooms.iter().position(|r| r.rect.contains(s.at))?;
                    g.room(s.level, room)
                })
                .collect();
            for (i, si) in link.stops.iter().enumerate() {
                for (j, sj) in link.stops.iter().enumerate() {
                    let (Some(from), Some(to)) = (at[i], at[j]) else {
                        continue;
                    };
                    if !rides(&link.stops, i, j) {
                        continue;
                    }
                    let card = si.card.max(sj.card);
                    let lock = match link.kind {
                        LinkKind::Lift if card > 0 => Some(GraphLock {
                            zone: card,
                            ways: LIFT,
                            code: None,
                            free: false,
                        }),
                        LinkKind::Crawl => Some(GRILLE),
                        _ => None,
                    };
                    g.edges.push(GraphEdge {
                        from,
                        to,
                        lock,
                        door: None,
                    });
                }
            }
        }
        for w in layout.ways_in.iter().filter(|w| keeps(w.level)) {
            let Some(room) = g.room(w.level, w.room) else {
                continue;
            };
            let stratum = match w.kind {
                WayInKind::Door { stratum, .. } | WayInKind::Crawl { stratum, .. } => stratum,
            };
            let outside = g.outside(stratum);
            match w.kind {
                WayInKind::Door { .. } => {
                    let lock = |free| w.lock.map(|l| of_door(w.level, &l, free));
                    g.both(outside, room, lock(false), lock(true), None);
                }
                WayInKind::Crawl { .. } => {
                    g.both(outside, room, Some(GRILLE), Some(GRILLE), None);
                }
            }
        }
        g
    }

    /// A mission's copy of `layout`: the graph of its kept levels with its fitted doors
    /// locked, the objective's room as the goal, the cards and codes the layer leaves, and
    /// its guards as people of their posts' zones.
    pub fn copy(layout: &VenueLayout, layer: &MissionLayer) -> VenueGraph {
        let mut g = VenueGraph::of(layout, &layer.fitted, Some(&layer.kept));
        let node = |g: &VenueGraph, level: i32, at: Pos| {
            room_holding(layout, level, at).and_then(|r| g.room(level, r))
        };
        g.goal = node(&g, layer.objective.level, layer.objective.at);
        g.cards = (layer.cards.iter())
            .filter_map(|c| Some((node(&g, c.level, c.at)?, c.zone)))
            .collect();
        g.codes = (layer.codes.iter())
            .filter_map(|c| Some((node(&g, c.level, c.at)?, c.lock)))
            .collect();
        g.people = (layer.guards.iter())
            .filter_map(|p| Some((node(&g, p.level, p.at)?, p.zone)))
            .collect();
        g
    }

    /// The node of room `room` on `level`.
    pub fn room(&self, level: i32, room: usize) -> Option<usize> {
        let at = NodeAt::Room { level, room };
        self.nodes.iter().position(|n| *n == at)
    }

    /// The node of the city on `stratum`, a start, added when it is new.
    fn outside(&mut self, stratum: StratumKind) -> usize {
        let at = NodeAt::Outside(stratum);
        if let Some(n) = self.nodes.iter().position(|n| *n == at) {
            return n;
        }
        self.nodes.push(at);
        self.starts.push(self.nodes.len() - 1);
        self.nodes.len() - 1
    }

    /// An edge each way between `a` and `b` through `door`: `ab` the lock from `a`, `ba`
    /// from `b`.
    fn both(
        &mut self,
        a: usize,
        b: usize,
        ab: Option<GraphLock>,
        ba: Option<GraphLock>,
        door: Option<(i32, usize)>,
    ) {
        self.edges.push(GraphEdge {
            from: a,
            to: b,
            lock: ab,
            door,
        });
        self.edges.push(GraphEdge {
            from: b,
            to: a,
            lock: ba,
            door,
        });
    }

    /// Each node's edges out, by index.
    fn out(&self) -> Vec<Vec<usize>> {
        let mut out = vec![Vec::new(); self.nodes.len()];
        for (i, e) in self.edges.iter().enumerate() {
            out[e.from].push(i);
        }
        out
    }

    /// Each node's edges in, by index.
    fn into(&self) -> Vec<Vec<usize>> {
        let mut into = vec![Vec::new(); self.nodes.len()];
        for (i, e) in self.edges.iter().enumerate() {
            into[e.to].push(i);
        }
        into
    }

    /// The nodes that get back to one of `to` over the edges means `means` holding `keys`
    /// can pass.
    fn back(&self, into: &[Vec<usize>], to: &[usize], means: Ways, keys: &Keys) -> Vec<bool> {
        let mut back = vec![false; self.nodes.len()];
        let mut queue: VecDeque<usize> = to.iter().copied().collect();
        for &n in to {
            back[n] = true;
        }
        while let Some(n) = queue.pop_front() {
            for e in into[n].iter().map(|&i| &self.edges[i]) {
                if !back[e.from] && passes(e.lock.as_ref(), means, keys) {
                    back[e.from] = true;
                    queue.push_back(e.from);
                }
            }
        }
        back
    }

    /// The nodes that get back to one of `to` over the edges means `means` holding `keys`
    /// can pass: where a key `reach` picks up counts.
    pub fn returns(&self, to: &[usize], means: Ways, keys: &Keys) -> Vec<bool> {
        self.back(&self.into(), to, means, keys)
    }

    /// Everything a way of means `means` holding `keys` reaches from `from`: a pass over
    /// the edges it can pass, then the cards and codes lying in the rooms it reached (and,
    /// with talk, the zone of each person it reached, who hands over their card), again
    /// until a pass finds nothing new. A key counts only in a room it can get back to
    /// `from` from, holding the keys so far: one picked up past a one-way door it can't
    /// pass back through leaves it there. Keys only grow, so once it can get back, it
    /// always can.
    pub fn reach(&self, from: &[usize], means: Ways, keys: Keys) -> Reach {
        let (out, into) = (self.out(), self.into());
        let mut keys = keys;
        let mut nodes = vec![false; self.nodes.len()];
        for &n in from {
            nodes[n] = true;
        }
        loop {
            let mut queue: VecDeque<usize> = (0..nodes.len()).filter(|&n| nodes[n]).collect();
            while let Some(n) = queue.pop_front() {
                for e in out[n].iter().map(|&i| &self.edges[i]) {
                    if !nodes[e.to] && passes(e.lock.as_ref(), means, &keys) {
                        nodes[e.to] = true;
                        queue.push_back(e.to);
                    }
                }
            }
            let back = self.back(&into, from, means, &keys);
            let taken = |n: usize| nodes[n] && back[n];
            let had = keys.clone();
            for &(n, zone) in &self.cards {
                if taken(n) {
                    keys.card = keys.card.max(zone);
                }
            }
            for &(n, code) in &self.codes {
                if taken(n) {
                    keys.codes.insert(code);
                }
            }
            if means.has(Ways::TALK) {
                for &(n, zone) in &self.people {
                    if taken(n) {
                        keys.talked = keys.talked.max(zone);
                        keys.card = keys.card.max(zone);
                    }
                }
            }
            if keys == had {
                return Reach { nodes, keys };
            }
        }
    }

    /// A route from a way in to the goal: the keys it ends with.
    pub fn route_in(&self, means: Ways) -> Option<Keys> {
        let goal = self.goal?;
        let r = self.reach(&self.starts, means, Keys::default());
        r.nodes[goal].then_some(r.keys)
    }

    /// A route from the goal out into the city, starting with `keys`.
    pub fn route_out(&self, means: Ways, keys: Keys) -> bool {
        let Some(goal) = self.goal else {
            return false;
        };
        let r = self.reach(&[goal], means, keys);
        (self.nodes.iter().zip(&r.nodes))
            .any(|(at, &reached)| reached && matches!(at, NodeAt::Outside(_)))
    }

    /// The fewest locks on any route from a way in to `goal`, any lock passable: a lock
    /// passed from its free side costs none, and a crawl's grille is a lock.
    pub fn depth(&self, goal: usize) -> Option<u32> {
        self.cheapest().0[goal]
    }

    /// One route from a way in to `goal` with the fewest locks, as `depth` counts them:
    /// its edges in order, by index. None when nothing reaches it.
    pub fn cheapest_route(&self, goal: usize) -> Option<Vec<usize>> {
        let (dist, parent) = self.cheapest();
        dist[goal]?;
        let mut route = Vec::new();
        let mut n = goal;
        while let Some(e) = parent[n] {
            route.push(e);
            n = self.edges[e].from;
        }
        route.reverse();
        Some(route)
    }

    /// The fewest locks from the ways in to each node, and the edge each is reached by on
    /// such a route: a search that takes a free edge before a locked one.
    fn cheapest(&self) -> (Vec<Option<u32>>, Vec<Option<usize>>) {
        let out = self.out();
        let mut dist: Vec<Option<u32>> = vec![None; self.nodes.len()];
        let mut parent: Vec<Option<usize>> = vec![None; self.nodes.len()];
        let mut queue = VecDeque::new();
        for &s in &self.starts {
            dist[s] = Some(0);
            queue.push_back(s);
        }
        while let Some(n) = queue.pop_front() {
            let d = dist[n].expect("a queued node has a distance");
            for &i in &out[n] {
                let e = &self.edges[i];
                let cost = u32::from(e.lock.is_some_and(|l| !l.free));
                if dist[e.to].is_none_or(|t| d + cost < t) {
                    dist[e.to] = Some(d + cost);
                    parent[e.to] = Some(i);
                    if cost == 0 {
                        queue.push_front(e.to);
                    } else {
                        queue.push_back(e.to);
                    }
                }
            }
        }
        (dist, parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interior::venue::testutil::test_type;
    use crate::interior::venue::{VenueFrame, lay_out};

    /// A graph of `n` nodes: node 0 is the street, the rest rooms on level 0; `edges` are
    /// directed; the street is the start.
    fn graph(n: usize, edges: &[(usize, usize, Option<GraphLock>)]) -> VenueGraph {
        let nodes = (0..n)
            .map(|i| match i {
                0 => NodeAt::Outside(StratumKind::Street),
                _ => NodeAt::Room { level: 0, room: i },
            })
            .collect();
        VenueGraph {
            nodes,
            edges: (edges.iter())
                .map(|&(from, to, lock)| GraphEdge {
                    from,
                    to,
                    lock,
                    door: None,
                })
                .collect(),
            starts: vec![0],
            cards: Vec::new(),
            codes: Vec::new(),
            people: Vec::new(),
            goal: None,
        }
    }

    /// A door both ways between `a` and `b`, under the same lock.
    fn both(a: usize, b: usize, lock: Option<GraphLock>) -> [(usize, usize, Option<GraphLock>); 2] {
        [(a, b, lock), (b, a, lock)]
    }

    fn lock(zone: u8, ways: Ways) -> Option<GraphLock> {
        Some(GraphLock {
            zone,
            ways,
            code: None,
            free: false,
        })
    }

    /// The same lock, passing without a key this way.
    fn free(l: Option<GraphLock>) -> Option<GraphLock> {
        l.map(|l| GraphLock { free: true, ..l })
    }

    /// 0 and 1 open to each other; 1 to 2 through a card lock of zone 2, free from 2.
    fn one_way() -> VenueGraph {
        let card = lock(2, Ways::CARD);
        let mut edges = both(0, 1, None).to_vec();
        edges.extend([(1, 2, card), (2, 1, free(card))]);
        let mut g = graph(3, &edges);
        g.goal = Some(2);
        g
    }

    #[test]
    fn a_one_way_lock_is_free_only_from_its_side() {
        let g = one_way();
        for (name, m) in PLAYS {
            assert_eq!(g.route_in(m), None, "{name}");
        }
        assert!(g.route_out(Ways::default(), Keys::default()));
    }

    #[test]
    fn a_card_found_on_the_way_opens_its_lock() {
        let mut g = one_way();
        g.cards.push((1, 2));
        let keys = g.route_in(Ways::FORCE).expect("force reaches the card");
        assert_eq!(keys.card, 2);
        for (name, m) in PLAYS {
            let keys = g.route_in(m).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(keys.card, 2, "{name}");
        }
    }

    #[test]
    fn each_way_passes_only_its_locks() {
        for w in [
            Ways::NET,
            Ways::KEYJACK,
            Ways::FORCE,
            Ways::TALK,
            Ways::CRAWL,
        ] {
            let mut edges = both(0, 1, None).to_vec();
            edges.extend(both(1, 2, lock(0, w)));
            let mut g = graph(3, &edges);
            g.goal = Some(2);
            for (name, m) in PLAYS {
                assert_eq!(g.route_in(m).is_some(), m.has(w), "{w:?} by {name}");
            }
        }
        let (_, keyjack) = PLAYS[2];
        assert!(keyjack.has(Ways::KEYJACK) && keyjack.has(Ways::CRAWL));
    }

    #[test]
    fn talk_hands_over_the_card_of_who_you_reach() {
        for ways in [Ways::CARD, Ways::TALK] {
            let mut edges = both(0, 1, None).to_vec();
            edges.extend(both(1, 2, lock(3, ways)));
            let mut g = graph(3, &edges);
            g.goal = Some(2);
            g.people.push((1, 3));
            let keys = g.route_in(Ways::TALK).expect("talk");
            assert_eq!((keys.card, keys.talked), (3, 3), "{ways:?}");
            assert_eq!(g.route_in(Ways::FORCE), None, "{ways:?}");
        }
    }

    #[test]
    fn depth_counts_locks_and_grilles_on_the_cheapest_route() {
        let door = lock(1, Ways::CARD | Ways::NET);
        let grille = lock(0, Ways::CRAWL);
        // 0 - 1 open; A: 1 - 2 - 5 through two locked doors; B: 1 - 3 a grille, 3 - 4
        // open, 4 - 5 a locked door.
        let edges = |b_open: Option<GraphLock>, a_more: bool| {
            let mut e = both(0, 1, None).to_vec();
            e.extend(both(1, 2, door));
            e.extend(both(2, 5, door));
            e.extend(both(1, 3, grille));
            e.extend(both(3, 4, b_open));
            e.extend(both(4, 5, door));
            if a_more {
                // A third lock on A, between 2 and a new 6 before the goal.
                e.retain(|&(f, t, _)| (f, t) != (2, 5) && (f, t) != (5, 2));
                e.extend(both(2, 6, door));
                e.extend(both(6, 5, door));
            }
            e
        };
        assert_eq!(graph(7, &edges(None, false)).depth(5), Some(2));
        assert_eq!(graph(7, &edges(door, false)).depth(5), Some(2), "A still");
        assert_eq!(graph(7, &edges(door, true)).depth(5), Some(3));
        let mut cut = graph(7, &edges(None, false));
        cut.edges.retain(|e| e.to != 5);
        assert_eq!(cut.depth(5), None, "nothing reaches it");
        assert_eq!(cut.cheapest_route(5), None);
        // A cheapest route runs from a start to the goal through as many locks as the
        // depth counts.
        for (b_open, a_more) in [(None, false), (door, false), (door, true)] {
            let g = graph(7, &edges(b_open, a_more));
            let route = g.cheapest_route(5).unwrap();
            assert!(g.starts.contains(&g.edges[route[0]].from));
            assert_eq!(g.edges[*route.last().unwrap()].to, 5);
            assert!(
                route
                    .windows(2)
                    .all(|w| g.edges[w[0]].to == g.edges[w[1]].from)
            );
            let locks = route.iter().filter(|&&i| g.edges[i].lock.is_some()).count();
            assert_eq!(Some(locks as u32), g.depth(5));
        }
    }

    #[test]
    fn a_free_side_costs_nothing() {
        let card = lock(1, Ways::CARD);
        let mut edges = both(0, 1, None).to_vec();
        edges.extend([(1, 2, free(card)), (2, 1, card)]);
        assert_eq!(graph(3, &edges).depth(2), Some(0));
    }

    #[test]
    fn the_way_out_is_found_apart_from_the_way_in() {
        // In from the street to 1, then through a door free from 1 that is locked coming
        // back; out of 2 only by an exit that opens from inside.
        let card = lock(1, Ways::CARD);
        let mut edges = both(0, 1, None).to_vec();
        edges.extend([(1, 2, free(card)), (2, 1, card)]);
        let exit = [(2, 0, free(card)), (0, 2, card)];
        let mut with = edges.clone();
        with.extend(exit);
        let mut g = graph(3, &with);
        g.goal = Some(2);
        for (name, m) in PLAYS {
            let keys = g.route_in(m).unwrap_or_else(|| panic!("{name}"));
            assert!(g.route_out(m, keys), "{name}: out by the exit");
        }
        let mut g = graph(3, &edges);
        g.goal = Some(2);
        for (name, m) in PLAYS {
            let keys = g.route_in(m).unwrap_or_else(|| panic!("{name}"));
            assert!(!g.route_out(m, keys), "{name}: no way out");
        }
    }

    #[test]
    fn a_key_counts_only_where_you_can_get_back_from_it() {
        // The street (0) is open to D (1) and A (3); D to R (2) is free, R to D a card
        // lock of zone 2; R holds a card of zone 1; A to the goal (4) is a zone 1 lock.
        let card2 = lock(2, Ways::CARD);
        let build = |back: Option<GraphLock>| {
            let mut edges = both(0, 1, None).to_vec();
            edges.extend(both(0, 3, None));
            edges.extend([(1, 2, free(card2)), (2, 1, back)]);
            edges.extend(both(3, 4, lock(1, Ways::CARD)));
            let mut g = graph(5, &edges);
            g.cards.push((2, 1));
            g.goal = Some(4);
            g
        };
        let stuck = build(card2);
        assert_eq!(stuck.route_in(Ways::CARD), None, "R's card leaves you in R");
        for (name, m) in PLAYS {
            assert_eq!(stuck.route_in(m), None, "{name}");
        }
        let keys = build(free(card2)).route_in(Ways::CARD);
        assert_eq!(keys.map(|k| k.card), Some(1), "back out of R with its card");
    }

    #[test]
    fn a_key_past_the_goal_counts_only_where_you_can_get_back() {
        // The goal (1) opens onto the street (0) through a zone 1 lock; the goal to R (2)
        // is free, R to the goal a card lock of zone 2; R holds a card of zone 1.
        let card2 = lock(2, Ways::CARD);
        let build = |back: Option<GraphLock>| {
            let mut edges = both(0, 1, lock(1, Ways::CARD)).to_vec();
            edges.extend([(1, 2, free(card2)), (2, 1, back)]);
            let mut g = graph(3, &edges);
            g.cards.push((2, 1));
            g.goal = Some(1);
            g
        };
        assert!(!build(card2).route_out(Ways::CARD, Keys::default()));
        assert!(build(free(card2)).route_out(Ways::CARD, Keys::default()));
    }

    #[test]
    fn a_one_way_link_runs_only_its_way() {
        let stop = |level, on, off| LinkStop {
            level,
            at: crate::geom::Pos::new(1, 1),
            card: 0,
            on,
            off,
        };
        let up = [
            stop(0, true, false),
            stop(1, true, true),
            stop(2, true, true),
            stop(3, false, true),
        ];
        assert!(rides(&up, 0, 3) && rides(&up, 1, 2));
        assert!(!rides(&up, 2, 1) && !rides(&up, 3, 0) && !rides(&up, 1, 0));
        let down = [
            stop(0, false, true),
            stop(1, true, true),
            stop(2, true, false),
        ];
        assert!(rides(&down, 2, 0) && rides(&down, 1, 0));
        assert!(!rides(&down, 1, 2) && !rides(&down, 0, 1));
        // A stair you leave only at its ends: on anywhere, off at either end.
        let ends = [
            stop(0, true, true),
            stop(1, true, false),
            stop(2, true, true),
        ];
        assert!(rides(&ends, 1, 0) && rides(&ends, 1, 2) && rides(&ends, 0, 2));
        assert!(!rides(&ends, 0, 1) && !rides(&ends, 2, 1));
    }

    #[test]
    fn a_layout_is_its_rooms_doors_links_and_ways_in() {
        let t = test_type();
        let l = lay_out(&t, VenueFrame::own(&t), 0).unwrap();
        let g = VenueGraph::of(&l, &[], None);
        let rooms: usize = l.levels.iter().map(|x| x.rooms.len()).sum();
        let strata: BTreeSet<StratumKind> = (l.ways_in.iter())
            .map(|w| match w.kind {
                WayInKind::Door { stratum, .. } | WayInKind::Crawl { stratum, .. } => stratum,
            })
            .collect();
        assert_eq!(g.nodes.len(), rooms + strata.len());
        assert_eq!(g.starts.len(), strata.len());
        assert!(
            g.starts
                .iter()
                .all(|&s| matches!(g.nodes[s], NodeAt::Outside(_)))
        );
        assert!(g.cards.is_empty() && g.codes.is_empty() && g.people.is_empty());
        for x in &l.levels {
            for d in &x.doors {
                let (a, b) = (g.room(x.level, d.rooms.0), g.room(x.level, d.rooms.1));
                let (a, b) = (a.unwrap(), b.unwrap());
                let ab = g.edges.iter().find(|e| (e.from, e.to) == (a, b)).unwrap();
                let ba = g.edges.iter().find(|e| (e.from, e.to) == (b, a)).unwrap();
                let private = x.rooms[d.rooms.0].private || x.rooms[d.rooms.1].private;
                match d.lock {
                    _ if d.open => assert_eq!((ab.lock, ba.lock), (None, None)),
                    Some(k) => {
                        let free =
                            |r: usize| k.free_from.is_some_and(|p| x.rooms[r].rect.contains(p));
                        assert_eq!(ab.lock.unwrap().free, free(d.rooms.0));
                        assert_eq!(ba.lock.unwrap().free, free(d.rooms.1));
                        if k.free_from.is_some() {
                            assert!(ab.lock.unwrap().free != ba.lock.unwrap().free, "one way");
                        }
                    }
                    None if private => {
                        let k = ab.lock.unwrap();
                        assert_eq!(k.ways, KEYPAD);
                        assert!(k.code.is_some_and(|(level, _)| level == x.level));
                    }
                    None => assert_eq!(ab.lock, None, "{:?}", d.fit),
                }
            }
        }
        // Each stratum's exits and crawls in, and nothing else, join its outside.
        for &s in &g.starts {
            let NodeAt::Outside(k) = g.nodes[s] else {
                unreachable!()
            };
            let ways = (l.ways_in.iter()).filter(|w| match w.kind {
                WayInKind::Door { stratum, .. } | WayInKind::Crawl { stratum, .. } => stratum == k,
            });
            assert_eq!(
                g.edges.iter().filter(|e| e.from == s).count(),
                ways.count(),
                "{k:?}"
            );
        }
        // Fitting every candidate adds their locks.
        let fitted = l.candidates();
        assert!(!fitted.is_empty());
        let all = VenueGraph::of(&l, &fitted, None);
        let locks = |g: &VenueGraph| g.edges.iter().filter(|e| e.lock.is_some()).count();
        assert_eq!(locks(&all), locks(&g) + 2 * fitted.len());
        // Keeping one level keeps its rooms and its ways in.
        let kept = VenueGraph::of(&l, &[], Some(&[0]));
        let ground = l.level(0).unwrap();
        let on: BTreeSet<StratumKind> = (l.ways_in.iter().filter(|w| w.level == 0))
            .map(|w| match w.kind {
                WayInKind::Door { stratum, .. } | WayInKind::Crawl { stratum, .. } => stratum,
            })
            .collect();
        assert_eq!(kept.nodes.len(), ground.rooms.len() + on.len());
        assert!(kept.nodes.iter().all(|n| match n {
            NodeAt::Room { level, .. } => *level == 0,
            NodeAt::Outside(_) => true,
        }));
    }

    #[test]
    fn reach_picks_up_codes_and_opens_their_keypads() {
        let pad = Some(GraphLock {
            zone: 1,
            ways: Ways::CODE | Ways::KEYJACK,
            code: Some((0, 60_000)),
            free: false,
        });
        let mut edges = both(0, 1, None).to_vec();
        edges.extend(both(1, 2, pad));
        let mut g = graph(3, &edges);
        assert!(!g.reach(&[0], Ways::FORCE, Keys::default()).nodes[2]);
        g.codes.push((1, (0, 60_000)));
        let r = g.reach(&[0], Ways::FORCE, Keys::default());
        assert!(r.nodes[2]);
        assert!(r.keys.codes.contains(&(0, 60_000)));
    }
}
