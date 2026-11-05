//! Packets: the traffic on the open Net's lanes, a look only. A packet leaves a node and
//! runs the lane to another node that lane joins it to, picked at random; there the node
//! soaks it up: the head goes in, the tail drains in after it and the node flashes. A
//! while later the node that sent it sends another. Packets on the same lanes cross now
//! and then. A white head with a fading tail.
//!
//! The traffic is kept by the map screen and moved on with the clock: packets are seeded
//! at the nodes round the view as it comes near, and dropped once they are far off it.

use citygen::rng::hash64;
use citygen::{City, Feature, Pos, Rect, StratumKind, Terrain};
use std::collections::{HashMap, HashSet, VecDeque};

/// Ticks a packet takes per tile.
pub const PACKET_TICKS: u64 = 6;
/// Steps a node takes to soak a packet up: its head in, then the two of its tail.
pub const SOAK_STEPS: usize = 3;
/// How far along the lanes a node looks for the nodes it is joined to.
const REACH: usize = 64;
/// One node in this many sends packets.
const SENDERS: u64 = 2;
/// The longest a sender waits, in steps, after its packet is soaked up.
const PAUSE_STEPS: u64 = 40;
/// Packets are seeded this far round the view, and kept until this much further.
const SEED_MARGIN: i32 = 12;
const KEEP_MARGIN: i32 = 40;
/// On a Net with few nodes (the open Net's trunks), the columns between relays along a
/// lane running east and west; half as many rows along one running north and south, a
/// row being twice as tall.
pub const RELAY: i32 = 24;

/// What the traffic lights in a frame: each lane tile with its glow and whether it's a
/// head (`Traffic::glows`), and each node soaking a packet up (`Traffic::soaks`).
pub type Lit = (HashMap<Pos, (f32, bool)>, HashMap<Pos, f32>);

/// Whether lane tile `p` is a relay, where a packet on a Net with few nodes stops and
/// another goes on as at a node: a lane's end (where it stops, or runs on past where
/// it's drawn), a junction, and every `RELAY` columns along an east-west run (every half
/// as many rows along a north-south one); never beside a node, which ends the lane
/// itself.
pub fn relay(lane: &impl Fn(Pos) -> bool, node: &impl Fn(Pos) -> bool, p: Pos) -> bool {
    if !lane(p) || node(p) || p.neighbors4().any(node) {
        return false;
    }
    let [n, e, s, w] = [(0, -1), (1, 0), (0, 1), (-1, 0)].map(|(dx, dy)| lane(p.offset(dx, dy)));
    match [n, e, s, w].iter().filter(|l| **l).count() {
        0 => false,
        2 => (e && w && p.x.rem_euclid(RELAY) == 0) || (n && s && p.y.rem_euclid(RELAY / 2) == 0),
        _ => true,
    }
}

/// A packet between two nodes: the lane tiles from beside one to beside the other, and
/// the tick it left.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Packet {
    /// The node that sent it.
    from: Pos,
    to: Pos,
    path: Vec<Pos>,
    started: u64,
}

impl Packet {
    /// Steps since it left at `tick`: an index into its path while it runs, past the
    /// path's end while its node soaks it up.
    fn step(&self, tick: u64) -> usize {
        (tick.saturating_sub(self.started) / PACKET_TICKS) as usize
    }

    /// Soaked up whole: head and tail in the node.
    fn gone(&self, tick: u64) -> bool {
        self.step(tick) >= self.path.len() + SOAK_STEPS
    }

    /// Where it is on the lane at `tick`: its head's tile, or the lane's last while it
    /// goes into the node.
    fn at(&self, tick: u64) -> Pos {
        self.path[self.step(tick).min(self.path.len() - 1)]
    }
}

/// The open Net's traffic, kept between frames.
#[derive(Clone, Debug, Default)]
pub struct Traffic {
    packets: Vec<Packet>,
    /// For each node tile, the nodes a lane joins it to without passing another node,
    /// each with the lane between.
    links: HashMap<Pos, Vec<(Pos, Vec<Pos>)>>,
    /// Senders with a packet out.
    out: HashSet<Pos>,
    /// Senders whose last packet was soaked up: the tick they send the next, and how many
    /// they have sent.
    next: HashMap<Pos, (u64, u64)>,
    seed: u64,
}

/// The nodes a lane joins `node` to without passing another node, nearest first, each
/// with the lane tiles from beside `node` to beside it.
pub fn joined(
    lane: &impl Fn(Pos) -> bool,
    is_node: &impl Fn(Pos) -> bool,
    node: Pos,
) -> Vec<(Pos, Vec<Pos>)> {
    let mut came: HashMap<Pos, Option<Pos>> = HashMap::new();
    let mut queue = VecDeque::new();
    for q in node.neighbors4() {
        if lane(q) && !is_node(q) {
            came.insert(q, None);
            queue.push_back((q, 1usize));
        }
    }
    let mut found: Vec<(Pos, Vec<Pos>)> = Vec::new();
    let mut met = HashSet::new();
    while let Some((at, d)) = queue.pop_front() {
        for q in at.neighbors4() {
            if q == node {
                continue;
            }
            if is_node(q) {
                if met.insert(q) {
                    let mut path = vec![at];
                    let mut back = at;
                    while let Some(Some(p)) = came.get(&back) {
                        path.push(*p);
                        back = *p;
                    }
                    path.reverse();
                    found.push((q, path));
                }
                continue;
            }
            if d < REACH && lane(q) && !came.contains_key(&q) {
                came.insert(q, Some(at));
                queue.push_back((q, d + 1));
            }
        }
    }
    found
}

fn key(p: Pos) -> u64 {
    (p.x as u32 as u64) << 32 | p.y as u32 as u64
}

impl Traffic {
    pub fn new(seed: u64) -> Traffic {
        Traffic {
            seed,
            ..Traffic::default()
        }
    }

    /// Where `node`'s packet number `n` goes: a node joined to it, at random.
    fn destination(
        &mut self,
        lane: &impl Fn(Pos) -> bool,
        is_node: &impl Fn(Pos) -> bool,
        node: Pos,
        n: u64,
    ) -> Option<(Pos, Vec<Pos>)> {
        let seed = self.seed;
        let links = self
            .links
            .entry(node)
            .or_insert_with(|| joined(lane, is_node, node));
        if links.is_empty() {
            return None;
        }
        let pick = hash64(seed ^ key(node), n) as usize % links.len();
        Some(links[pick].clone())
    }

    /// The clock moves to `tick` with `view` on screen: packets soaked up whole are gone
    /// and their senders wait a while; the senders round the view with nothing out and no
    /// wait left send one; packets far off the view are dropped.
    pub fn advance(
        &mut self,
        lane: impl Fn(Pos) -> bool,
        is_node: impl Fn(Pos) -> bool,
        nodes: impl IntoIterator<Item = Pos>,
        view: Rect,
        tick: u64,
    ) {
        // Soaked up: the sender waits, then sends its next.
        let seed = self.seed;
        let mut packets = std::mem::take(&mut self.packets);
        packets.retain(|p| {
            if !p.gone(tick) {
                return true;
            }
            self.out.remove(&p.from);
            let n = self.next.get(&p.from).map_or(0, |(_, n)| *n) + 1;
            let pause = hash64(seed ^ 0x50a4, key(p.from) ^ n) % (PAUSE_STEPS + 1);
            self.next.insert(p.from, (tick + pause * PACKET_TICKS, n));
            false
        });
        self.packets = packets;
        let seeds = view.grow(SEED_MARGIN);
        for node in nodes {
            if !seeds.contains(node) || self.out.contains(&node) {
                continue;
            }
            if !hash64(self.seed, key(node)).is_multiple_of(SENDERS) {
                continue;
            }
            let (due, n) = self.next.get(&node).copied().unwrap_or((0, 0));
            if tick < due {
                continue;
            }
            let fresh = !self.next.contains_key(&node);
            let Some((to, path)) = self.destination(&lane, &is_node, node, n) else {
                continue;
            };
            // The first a node sends near the view is somewhere along its lane already,
            // so the grid is busy at once; later ones leave from the node.
            let along = if fresh {
                hash64(self.seed ^ 0x9e37, key(node)) % path.len().max(1) as u64
            } else {
                0
            };
            self.out.insert(node);
            self.next.insert(node, (due, n));
            self.packets.push(Packet {
                from: node,
                to,
                path,
                started: tick.saturating_sub(along * PACKET_TICKS),
            });
        }
        // Far off the view: dropped, and seeded afresh on the way back.
        let keep = view.grow(KEEP_MARGIN);
        let (out, next) = (&mut self.out, &mut self.next);
        self.packets.retain(|p| {
            let stays = keep.contains(p.at(tick));
            if !stays {
                out.remove(&p.from);
                next.remove(&p.from);
            }
            stays
        });
    }

    /// Each lane tile a packet lights at `tick`, with how much: the head 0.6, its tail 0.3
    /// and 0.13 behind it; `true` for the head. Going into its node the head is gone and
    /// the tail drains after it. Where two packets meet the brighter shows.
    pub fn glows(&self, tick: u64) -> HashMap<Pos, (f32, bool)> {
        let mut out: HashMap<Pos, (f32, bool)> = HashMap::new();
        for p in &self.packets {
            let at = p.step(tick);
            for (k, glow) in [(0usize, 0.6f32), (1, 0.3), (2, 0.13)] {
                let Some(i) = at.checked_sub(k) else { break };
                let Some(&tile) = p.path.get(i) else { continue };
                let e = out.entry(tile).or_insert((0.0, false));
                if glow > e.0 {
                    *e = (glow, k == 0);
                }
            }
        }
        out
    }

    /// The nodes soaking a packet up at `tick`, with how bright: brightest as the head
    /// goes in, fading as the tail follows.
    pub fn soaks(&self, tick: u64) -> HashMap<Pos, f32> {
        let mut out: HashMap<Pos, f32> = HashMap::new();
        for p in &self.packets {
            let Some(into) = p.step(tick).checked_sub(p.path.len()) else {
                continue;
            };
            let glow = [0.7f32, 0.45, 0.2][into.min(SOAK_STEPS - 1)];
            let e = out.entry(p.to).or_insert(0.0);
            *e = e.max(glow);
        }
        out
    }

    /// The traffic on `city`'s lanes moved on to `tick` with `view` (cells of its own
    /// frame) on screen: what it lights.
    pub fn run_city(&mut self, city: &City, view: Rect, tick: u64) -> Lit {
        let net = city.stratum(StratumKind::Net);
        let tile = |p: Pos| net.and_then(|s| s.tiles.get(p));
        self.advance(
            |p| tile(p).is_some_and(|t| t.terrain == Terrain::NetLane),
            |p| tile(p).is_some_and(|t| matches!(t.feature, Some(Feature::Node(_)))),
            city.net.nodes.iter().map(|n| n.cyber),
            view,
            tick,
        );
        (self.glows(tick), self.soaks(tick))
    }

    /// The area round `view` a Net's packets are sent from: the cells `run_sparse` needs
    /// of its nodes and lanes.
    pub fn seeds(view: Rect) -> Rect {
        view.grow(SEED_MARGIN)
    }

    /// The traffic on a Net with few nodes (the open Net's trunks) moved on to `tick` with
    /// `view` on screen: its packets stop at its lanes' `relay`s as at its nodes, `near`
    /// the cells of its lanes and nodes round the view (`seeds`). What it lights: a
    /// packet going into a relay lights it as the lane, there being no node to flash.
    pub fn run_sparse(
        &mut self,
        lane: impl Fn(Pos) -> bool,
        node: impl Fn(Pos) -> bool,
        near: impl IntoIterator<Item = Pos>,
        view: Rect,
        tick: u64,
    ) -> Lit {
        let end = |p: Pos| node(p) || relay(&lane, &node, p);
        let ends: Vec<Pos> = near.into_iter().filter(|p| end(*p)).collect();
        self.advance(&lane, end, ends, view, tick);
        let mut glows = self.glows(tick);
        let soaks = self.soaks(tick);
        for (p, g) in &soaks {
            if node(*p) {
                continue;
            }
            // the head going in as bright as a head on the lane, its tail after it
            let (glow, head) = (g * 0.6 / 0.7, *g >= 0.7);
            let e = glows.entry(*p).or_insert((0.0, false));
            if glow > e.0 {
                *e = (glow, head);
            }
        }
        (glows, soaks)
    }

    /// The joins between nodes forgotten, to be looked for again as each next sends:
    /// more of the Net has come in round them.
    pub fn forget_joins(&mut self) {
        self.links.clear();
    }

    /// How many packets are out.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.packets.len()
    }
}

/// The traffic of every Net the view takes in, kept by the map screen between frames:
/// each city's and town's on its own lanes, in its own frame, found by its seed (so a
/// city's packets carry on as it's handed over, live or not); and the open Net's, on its
/// trunks, in world cells.
#[derive(Clone, Debug, Default)]
pub struct Nets {
    cities: HashMap<u64, Traffic>,
    open: Option<Traffic>,
}

impl Nets {
    /// The traffic on the lanes of the city or town seeded `seed`.
    pub fn city(&mut self, seed: u64) -> &mut Traffic {
        self.cities
            .entry(seed)
            .or_insert_with(|| Traffic::new(seed))
    }

    /// The traffic on the open Net's trunks of the world seeded `seed`.
    pub fn open(&mut self, seed: u64) -> &mut Traffic {
        if self.open.as_ref().is_none_or(|t| t.seed != seed) {
            self.open = Some(Traffic::new(seed));
        }
        self.open.get_or_insert_with(|| Traffic::new(seed))
    }

    /// The open Net round the view changed (more of it streamed in, a city came in
    /// over it): its joins are looked for again.
    pub fn open_changed(&mut self) {
        if let Some(t) = self.open.as_mut() {
            t.forget_joins();
        }
    }

    /// The traffic of the city or town seeded `seed`, if it has run.
    #[cfg(test)]
    pub fn of_city(&self, seed: u64) -> Option<&Traffic> {
        self.cities.get(&seed)
    }

    /// The open Net's traffic, if it has run.
    #[cfg(test)]
    pub fn open_net(&self) -> Option<&Traffic> {
        self.open.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lane tiles and node tiles.
    fn grid<'a>(
        lanes: &'a [(i32, i32)],
        nodes: &'a [(i32, i32)],
    ) -> (impl Fn(Pos) -> bool + 'a, impl Fn(Pos) -> bool + 'a) {
        (
            move |p: Pos| lanes.contains(&(p.x, p.y)),
            move |p: Pos| nodes.contains(&(p.x, p.y)),
        )
    }

    /// A at (0,0), B at (10,0) on a lane between them; C at (5,-6) up a lane that leaves
    /// it at (5,0).
    /// Tiles as (x, y).
    type Tiles = Vec<(i32, i32)>;

    fn layout() -> (Tiles, Tiles) {
        let mut lanes: Vec<(i32, i32)> = (1..10).map(|x| (x, 0)).collect();
        lanes.extend((-5..0).map(|y| (5, y)));
        (lanes, vec![(0, 0), (10, 0), (5, -6)])
    }

    #[test]
    fn a_node_is_joined_to_the_nodes_its_lanes_reach() {
        let (lanes, nodes) = layout();
        let (lane, node) = grid(&lanes, &nodes);
        let from_a = joined(&lane, &node, Pos::new(0, 0));
        let to: Vec<Pos> = from_a.iter().map(|(p, _)| *p).collect();
        assert!(
            to.contains(&Pos::new(10, 0)) && to.contains(&Pos::new(5, -6)),
            "{to:?}"
        );
        let (_, path) = from_a.iter().find(|(p, _)| *p == Pos::new(10, 0)).unwrap();
        assert_eq!(path.first(), Some(&Pos::new(1, 0)), "from beside A");
        assert_eq!(path.last(), Some(&Pos::new(9, 0)), "to beside B");
        assert_eq!(path.len(), 9);
        // A node in the way ends the lane there: D at (5,0) parts A from B.
        let nodes = vec![(0, 0), (10, 0), (5, 0)];
        let lanes: Vec<(i32, i32)> = (1..10).filter(|x| *x != 5).map(|x| (x, 0)).collect();
        let (lane, node) = grid(&lanes, &nodes);
        let to: Vec<Pos> = joined(&lane, &node, Pos::new(0, 0))
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        assert_eq!(to, vec![Pos::new(5, 0)]);
    }

    #[test]
    fn a_packet_is_soaked_up_by_its_node_and_its_sender_sends_again() {
        let (lanes, nodes) = layout();
        let (lane, node) = grid(&lanes, &nodes);
        let view = Rect::new(-2, -8, 16, 10);
        let all = || nodes.iter().map(|(x, y)| Pos::new(*x, *y));
        // Enough seeds that some node sends.
        let mut t = (0..16)
            .map(|s| {
                let mut t = Traffic::new(s);
                t.advance(&lane, &node, all(), view, 0);
                t
            })
            .find(|t| t.len() > 0)
            .expect("a node sends");
        let p = t.packets[0].clone();
        assert!(all().any(|n| n == p.from) && all().any(|n| n == p.to));
        assert_ne!(p.from, p.to);
        let end = p.started + p.path.len() as u64 * PACKET_TICKS;
        // At the lane's end the head goes into the node: no head on the lane, the tail
        // draining after it, the node flashing.
        t.advance(&lane, &node, all(), view, end);
        let mine = t
            .packets
            .iter()
            .find(|q| q.from == p.from && q.started == p.started)
            .expect("still draining");
        assert_eq!(mine.step(end), p.path.len(), "its head is in the node");
        let alone = Traffic {
            packets: vec![mine.clone()],
            ..Traffic::default()
        };
        let g = alone.glows(end);
        assert!(g.values().all(|(_, h)| !h), "no head on the lane");
        assert_eq!(g.get(p.path.last().unwrap()).map(|(v, _)| *v), Some(0.3));
        assert!(
            alone.soaks(end).get(&p.to).is_some_and(|v| *v > 0.5),
            "the node flashes"
        );
        // Soaked up whole: gone, and never back the way it came.
        let gone = end + SOAK_STEPS as u64 * PACKET_TICKS;
        t.advance(&lane, &node, all(), view, gone);
        assert!(
            !t.packets
                .iter()
                .any(|q| q.from == p.from && q.started <= end),
            "the packet is gone"
        );
        // A while later the sender sends again, from itself.
        let later = gone + (PAUSE_STEPS + 1) * PACKET_TICKS;
        t.advance(&lane, &node, all(), view, later);
        let q = t
            .packets
            .iter()
            .find(|q| q.from == p.from)
            .expect("the sender sends again");
        assert_eq!(q.started, later, "fresh from the node");
        assert!(t.glows(later).keys().all(|p| lane(*p)), "lit on lanes only");
    }

    #[test]
    fn a_lane_with_few_nodes_is_broken_up_by_relays() {
        // a trunk from x 1 to 99 along y 0, a branch north at x 50, a node at the west
        // end on its own light line
        let mut lanes: Vec<(i32, i32)> = (1..100).map(|x| (x, 0)).collect();
        lanes.extend((-20..0).map(|y| (50, y)));
        let nodes = vec![(0, 0)];
        let (lane, node) = grid(&lanes, &nodes);
        let relays: Vec<(i32, i32)> = Rect::new(-5, -25, 110, 30)
            .iter()
            .filter(|p| relay(&lane, &node, *p))
            .map(|p| (p.x, p.y))
            .collect();
        // the branch's end and every 12 rows up it; along the trunk every 24 columns,
        // the junction and the east end; not the west end, beside the node
        assert_eq!(
            relays,
            vec![
                (50, -20),
                (50, -12),
                (24, 0),
                (48, 0),
                (50, 0),
                (72, 0),
                (96, 0),
                (99, 0)
            ]
        );
    }

    #[test]
    fn packets_run_a_lane_with_no_node_on_it() {
        let lanes: Vec<(i32, i32)> = (0..100).map(|x| (x, 0)).collect();
        let (lane, node) = grid(&lanes, &[]);
        let view = Rect::new(0, -10, 100, 20);
        let near = || {
            Traffic::seeds(view)
                .iter()
                .filter(|p| lane(*p))
                .collect::<Vec<_>>()
        };
        let mut lit = 0;
        let mut t = Traffic::new(5);
        for tick in 0..400 {
            let (glows, _) = t.run_sparse(&lane, &node, near(), view, tick);
            assert!(glows.keys().all(|p| lane(*p)), "lit on the lane only");
            lit += glows.values().filter(|(_, head)| *head).count();
        }
        assert!(lit > 0, "heads run the lane");
    }

    #[test]
    fn packets_far_off_the_view_go_and_come_back() {
        let (lanes, nodes) = layout();
        let (lane, node) = grid(&lanes, &nodes);
        let all = || nodes.iter().map(|(x, y)| Pos::new(*x, *y));
        let near = Rect::new(-2, -8, 16, 10);
        let mut t = (0..16)
            .map(|s| {
                let mut t = Traffic::new(s);
                t.advance(&lane, &node, all(), near, 0);
                t
            })
            .find(|t| t.len() > 0)
            .unwrap();
        let far = Rect::new(500, 500, 10, 10);
        t.advance(&lane, &node, all(), far, 1);
        assert_eq!(t.len(), 0, "dropped far off the view");
        t.advance(&lane, &node, all(), near, 2);
        assert!(t.len() > 0, "sent again on the way back");
    }
}
