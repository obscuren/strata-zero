//! Packets: the traffic on the open Net's lanes, a look only. A packet leaves a node,
//! runs the lane to another node that lane joins it to, and there picks the next one
//! at random (not straight back while it has a choice), so packets wander the grid
//! from node to node and now and then cross. A white head with a fading tail.
//!
//! The traffic is kept by the map screen and moved on with the clock: packets are seeded
//! at the nodes round the view as it comes near, and dropped once they are far off it.

use citygen::rng::hash64;
use citygen::{Pos, Rect};
use std::collections::{HashMap, HashSet, VecDeque};

/// Ticks a packet takes per tile.
pub const PACKET_TICKS: u64 = 6;
/// How far along the lanes a node looks for the nodes it is joined to.
const REACH: usize = 64;
/// One node in this many sends a packet.
const SENDERS: u64 = 2;
/// Packets are seeded this far round the view, and kept until this much further.
const SEED_MARGIN: i32 = 12;
const KEEP_MARGIN: i32 = 40;
/// The most hops a packet takes in one step of the clock, so a long pause can't stall.
const MAX_HOPS: usize = 8;

/// A packet between two nodes: the lane tiles from beside one to beside the other, and
/// the tick it left.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Packet {
    id: u64,
    /// The node that sent it.
    sender: Pos,
    from: Pos,
    to: Pos,
    path: Vec<Pos>,
    started: u64,
    hops: u64,
}

impl Packet {
    fn ends(&self) -> u64 {
        self.started + self.path.len() as u64 * PACKET_TICKS
    }

    /// Its head at `tick`: an index into its path.
    fn head(&self, tick: u64) -> usize {
        let i = (tick.saturating_sub(self.started) / PACKET_TICKS) as usize;
        i.min(self.path.len().saturating_sub(1))
    }
}

/// The open Net's traffic, kept between frames.
#[derive(Clone, Debug, Default)]
pub struct Traffic {
    packets: Vec<Packet>,
    /// For each node tile, the nodes a lane joins it to without passing another node,
    /// each with the lane between.
    links: HashMap<Pos, Vec<(Pos, Vec<Pos>)>>,
    /// Nodes whose packet is out.
    sent: HashSet<Pos>,
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

impl Traffic {
    pub fn new(seed: u64) -> Traffic {
        Traffic {
            seed,
            ..Traffic::default()
        }
    }

    /// The city it was made for.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    fn links_of(
        &mut self,
        lane: &impl Fn(Pos) -> bool,
        is_node: &impl Fn(Pos) -> bool,
        node: Pos,
    ) -> &Vec<(Pos, Vec<Pos>)> {
        self.links
            .entry(node)
            .or_insert_with(|| joined(lane, is_node, node))
    }

    /// The next hop from `at`, having come from `from`: a node joined to it at random,
    /// not straight back while there is another; `None` where nothing joins it.
    fn next_hop(
        &mut self,
        lane: &impl Fn(Pos) -> bool,
        is_node: &impl Fn(Pos) -> bool,
        id: u64,
        hop: u64,
        at: Pos,
        from: Option<Pos>,
    ) -> Option<(Pos, Vec<Pos>)> {
        let seed = self.seed;
        let links = self.links_of(lane, is_node, at);
        let choices: Vec<&(Pos, Vec<Pos>)> = links
            .iter()
            .filter(|(to, _)| Some(*to) != from || links.len() == 1)
            .collect();
        if choices.is_empty() {
            return None;
        }
        let pick = hash64(seed ^ id, hop) as usize % choices.len();
        Some(choices[pick].clone())
    }

    /// The clock moves to `tick` with `view` on screen: packets due at their node hop on,
    /// the nodes round the view that send and have no packet out send one, and packets
    /// far off the view are dropped.
    pub fn advance(
        &mut self,
        lane: impl Fn(Pos) -> bool,
        is_node: impl Fn(Pos) -> bool,
        nodes: impl IntoIterator<Item = Pos>,
        view: Rect,
        tick: u64,
    ) {
        let seeds = view.grow(SEED_MARGIN);
        for node in nodes {
            if !seeds.contains(node) || self.sent.contains(&node) {
                continue;
            }
            let id = (node.x as u32 as u64) << 32 | node.y as u32 as u64;
            if !hash64(self.seed, id).is_multiple_of(SENDERS) {
                continue;
            }
            self.sent.insert(node);
            let Some((to, path)) = self.next_hop(&lane, &is_node, id, 0, node, None) else {
                continue;
            };
            // Somewhere along its first lane already, so the grid is busy at once.
            let along = hash64(self.seed ^ 0x9e37, id) % path.len().max(1) as u64;
            self.packets.push(Packet {
                id,
                sender: node,
                from: node,
                to,
                path,
                started: tick.saturating_sub(along * PACKET_TICKS),
                hops: 0,
            });
        }
        let mut packets = std::mem::take(&mut self.packets);
        for p in &mut packets {
            for _ in 0..MAX_HOPS {
                if tick < p.ends() {
                    break;
                }
                let ended = p.ends();
                let hop = p.hops + 1;
                match self.next_hop(&lane, &is_node, p.id, hop, p.to, Some(p.from)) {
                    Some((to, path)) => {
                        p.from = p.to;
                        p.to = to;
                        p.path = path;
                        p.started = ended;
                        p.hops = hop;
                    }
                    None => break,
                }
            }
            // Behind by more than its hops allow: it starts its lane over now.
            if tick >= p.ends() {
                p.started = tick;
            }
        }
        let keep = view.grow(KEEP_MARGIN);
        let sent = &mut self.sent;
        packets.retain(|p| {
            let at = p.path[p.head(tick)];
            let stays = keep.contains(at);
            if !stays {
                sent.remove(&p.sender);
            }
            stays
        });
        self.packets = packets;
    }

    /// Each lane tile a packet lights at `tick`, with how much: the head 0.6, its tail 0.3
    /// and 0.13 behind it; `true` for the head. Where two packets meet the brighter shows.
    pub fn glows(&self, tick: u64) -> HashMap<Pos, (f32, bool)> {
        let mut out: HashMap<Pos, (f32, bool)> = HashMap::new();
        for p in &self.packets {
            let at = p.head(tick);
            for (k, glow) in [(0usize, 0.6f32), (1, 0.3), (2, 0.13)] {
                let Some(i) = at.checked_sub(k) else { break };
                let e = out.entry(p.path[i]).or_insert((0.0, false));
                if glow > e.0 {
                    *e = (glow, k == 0);
                }
            }
        }
        out
    }

    /// How many packets are out.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.packets.len()
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
    fn a_packet_runs_node_to_node_and_moves_on() {
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
        // At its lane's end it hops on, from the node it reached.
        let end = p.ends();
        t.advance(&lane, &node, all(), view, end);
        let q = t.packets.iter().find(|q| q.id == p.id).unwrap();
        assert_eq!(q.from, p.to, "on from where it got to");
        assert_eq!(q.started, end);
        // Lit on lane tiles only, one head per packet.
        let g = t.glows(end + PACKET_TICKS);
        assert!(g.keys().all(|p| lane(*p)));
        assert!(g.values().filter(|(_, h)| *h).count() >= 1);
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
