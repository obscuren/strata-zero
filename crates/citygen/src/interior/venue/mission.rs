//! A mission's layer on a venue: what a mission lays over the plain layout from its size,
//! its objective, its security and its own seed. It keeps some of the levels, fits locks
//! to border doors until the objective lies deep enough for the size, leaves each card
//! and code the way in and out needs where it can be picked up and carried back, posts
//! the guards and stands the cameras, turrets and containers. It never adds a wall or a
//! door: a copy's layout is the plain building's.

mod kept;
#[cfg(test)]
mod tests;

pub(super) use kept::joined;

use super::graph::passes;
use super::{
    Keys, LevelLayout, MissionSize, NodeAt, ObjectiveDef, PlannedRoom, Reach, SecurityDef,
    SizeRule, VenueGraph, VenueLayout, VenueType, Ways,
};
use crate::geom::Pos;
use crate::interior::RoomKind;
use crate::rng::hash64;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeSet, VecDeque};

/// What a mission asks of a site; the layer is laid again from these alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionInputs {
    pub size: MissionSize,
    /// Index into the type's objectives.
    pub objective: usize,
    /// 0 to 255, the scale a subnet's security uses.
    pub security: u8,
    pub seed: u64,
}

/// A tile on a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub level: i32,
    pub at: Pos,
}

/// A guard's post: where it stands, the zone whose card it carries, and the spots it
/// walks, starting at its post.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardPost {
    pub level: i32,
    pub at: Pos,
    pub zone: u8,
    pub patrol: Vec<Pos>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Camera,
    Turret,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Device {
    pub level: i32,
    pub at: Pos,
    pub kind: DeviceKind,
}

/// A zone's card, left lying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardSpot {
    pub zone: u8,
    pub level: i32,
    pub at: Pos,
}

/// A code note: where it lies, and the lock it opens as (level, number).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeSpot {
    pub level: i32,
    pub at: Pos,
    pub lock: (i32, u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionLayer {
    pub inputs: MissionInputs,
    /// Lowest first.
    pub kept: Vec<i32>,
    pub objective: Placed,
    /// Doors given their `fit` lock: (level, door index).
    pub fitted: Vec<(i32, usize)>,
    pub guards: Vec<GuardPost>,
    pub devices: Vec<Device>,
    pub containers: Vec<Placed>,
    /// In the order they were left: each lies before the lock it opens.
    pub cards: Vec<CardSpot>,
    pub codes: Vec<CodeSpot>,
}

impl MissionLayer {
    /// Lays a mission of `inputs` on `layout` of type `t` at the size `rule` gives, from
    /// one rng seeded by the layout's seed and the mission's. Err says why it can't.
    pub fn lay(
        layout: &VenueLayout,
        t: &VenueType,
        rule: &SizeRule,
        inputs: MissionInputs,
    ) -> Result<MissionLayer, String> {
        let mut rng = ChaCha8Rng::seed_from_u64(hash64(layout.seed, inputs.seed));
        let def = (t.objectives.get(inputs.objective))
            .ok_or_else(|| format!("{} has no objective {}", t.id, inputs.objective))?;
        let security = (t.security.iter())
            .find(|s| s.size == inputs.size)
            .ok_or_else(|| format!("{} has no {:?} security", t.id, inputs.size))?;
        let objective = objective(layout, def, &mut rng)?;
        let kept = kept::keep(layout, rule, objective.level, &mut rng)?;
        let mut layer = MissionLayer {
            inputs,
            kept,
            objective,
            fitted: Vec::new(),
            guards: Vec::new(),
            devices: Vec::new(),
            containers: Vec::new(),
            cards: Vec::new(),
            codes: Vec::new(),
        };
        let mut used = BTreeSet::from([(objective.level, objective.at)]);
        layer.fit(layout, rule, &mut rng)?;
        layer.leave_keys(layout, &mut used, &mut rng)?;
        layer.post_guards(layout, t, rule, security, &mut used, &mut rng)?;
        layer.stand_devices(layout, t, security, &mut used, &mut rng);
        Ok(layer)
    }

    /// Fits locks to candidate doors on the kept levels until the objective lies at least
    /// a number of locks deep the rng draws in the size's range: each time the candidate
    /// nearest the objective on one cheapest route in, passed from its locked side. With
    /// none left on that route, the size's least is enough.
    fn fit(
        &mut self,
        layout: &VenueLayout,
        rule: &SizeRule,
        rng: &mut ChaCha8Rng,
    ) -> Result<(), String> {
        let (least, most) = rule.locks;
        let target = u32::from(rng.random_range(least..=most.max(least)));
        let candidates: BTreeSet<(i32, usize)> = (layout.candidates().into_iter())
            .filter(|c| self.kept.contains(&c.0))
            .collect();
        loop {
            let g = VenueGraph::of(layout, &self.fitted, Some(&self.kept));
            let goal = goal_of(&g, layout, self.objective)?;
            let depth = (g.depth(goal)).ok_or("nothing reaches the objective")?;
            if depth >= target {
                return Ok(());
            }
            let route = g.cheapest_route(goal).unwrap_or_default();
            let next = route.iter().rev().find_map(|&i| {
                let e = &g.edges[i];
                let door = e.door.filter(|d| candidates.contains(d))?;
                if self.fitted.contains(&door) {
                    return None;
                }
                let NodeAt::Room { level, room } = g.nodes[e.from] else {
                    return None;
                };
                let x = layout.level(level)?;
                let fit = x.doors[door.1].fit?;
                let free = (fit.free_from).is_some_and(|p| x.rooms[room].rect.contains(p));
                (!free).then_some(door)
            });
            match next {
                Some(door) => self.fitted.push(door),
                None if depth >= u32::from(rule.locks.0) => return Ok(()),
                None => return Err(format!("too shallow for {:?}", self.inputs.size)),
            }
        }
    }

    /// Leaves the cards and codes the base way, with no means of its own, needs to get in
    /// to the objective and then out to the city: while it can't, a lock it is held at
    /// gets its key, in a room it has reached and can get back from.
    fn leave_keys(
        &mut self,
        layout: &VenueLayout,
        used: &mut BTreeSet<(i32, Pos)>,
        rng: &mut ChaCha8Rng,
    ) -> Result<(), String> {
        let base = Ways::default();
        let mut g = VenueGraph::of(layout, &self.fitted, Some(&self.kept));
        let goal = goal_of(&g, layout, self.objective)?;
        g.goal = Some(goal);
        let starts = g.starts.clone();
        // Each key opens one more lock, so a way gets past each lock once at most.
        let tries = g.edges.len() + 1;
        for _ in 0..tries {
            let r = g.reach(&starts, base, Keys::default());
            if r.nodes[goal] {
                break;
            }
            let back = g.returns(&starts, base, &r.keys);
            self.leave_key(layout, &mut g, &r, &back, &[goal], used, rng)?;
        }
        for _ in 0..tries {
            let keys = (g.route_in(base)).ok_or("no card or code gets in to the objective")?;
            let r = g.reach(&[goal], base, keys);
            if starts.iter().any(|&s| r.nodes[s]) {
                return Ok(());
            }
            let back = g.returns(&[goal], base, &r.keys);
            self.leave_key(layout, &mut g, &r, &back, &starts, used, rng)?;
        }
        Err("no card or code gets out from the objective".to_string())
    }

    /// The key to a lock that holds the way at `r` on a route on to `to` with the fewest
    /// locks it can't pass yet, one a card or a code opens: of those, the lowest zone's
    /// (then the one into the lowest node). It is left on a free spot of the room of the
    /// deepest zone the way has reached and can get `back` from, nearest the lock: a code
    /// for a lock that takes one, else its zone's card. A grille takes neither and is
    /// passed by.
    #[allow(clippy::too_many_arguments)]
    fn leave_key(
        &mut self,
        layout: &VenueLayout,
        g: &mut VenueGraph,
        r: &Reach,
        back: &[bool],
        to: &[usize],
        used: &mut BTreeSet<(i32, Pos)>,
        rng: &mut ChaCha8Rng,
    ) -> Result<(), String> {
        let left = locks_to(g, to, &r.keys);
        let held: Vec<_> = (g.edges.iter())
            .filter(|e| r.nodes[e.from] && !r.nodes[e.to])
            .filter_map(|e| {
                let l = e.lock.filter(|l| l.ways.has(Ways::CARD | Ways::CODE))?;
                Some((e.from, e.to, l, left[e.to].saturating_add(1)))
            })
            .collect();
        let fewest = held.iter().map(|h| h.3).min().unwrap_or(u32::MAX);
        let (from, lock) = (held.into_iter())
            .filter(|h| h.3 == fewest && fewest != u32::MAX)
            .min_by_key(|&(from, to, l, _)| (l.zone, to, from))
            .map(|(from, _, l, _)| (from, l))
            .ok_or("no card or code opens the way on")?;
        let hops = hops_from(g, from);
        let (node, level, room) = (0..g.nodes.len())
            .filter(|&n| r.nodes[n] && back[n])
            .filter_map(|n| match g.nodes[n] {
                NodeAt::Room { level, room } => Some((n, level, room)),
                NodeAt::Outside(_) => None,
            })
            .filter(|&(_, level, room)| !free_spots(layout, level, room, used).is_empty())
            .min_by_key(|&(n, level, room)| {
                let zone = layout.level(level).map_or(0, |x| x.rooms[room].zone);
                (Reverse(zone), hops[n], n)
            })
            .ok_or("no spot to leave a key on")?;
        let spots = free_spots(layout, level, room, used);
        let at = spots[rng.random_range(0..spots.len())];
        used.insert((level, at));
        match lock.code {
            Some(code) => {
                self.codes.push(CodeSpot {
                    level,
                    at,
                    lock: code,
                });
                g.codes.push((node, code));
            }
            None => {
                self.cards.push(CardSpot {
                    zone: lock.zone,
                    level,
                    at,
                });
                g.cards.push((node, lock.zone));
            }
        }
        Ok(())
    }

    /// The guards: from the size's fewest at security 0 to its most at 255, dealt to the
    /// zones the security weighs by their weights, one to a free spot of a kept room of
    /// its zone; each walks 2 to 4 spots of its zone's rooms on its level from its post.
    fn post_guards(
        &mut self,
        layout: &VenueLayout,
        t: &VenueType,
        rule: &SizeRule,
        security: &SecurityDef,
        used: &mut BTreeSet<(i32, Pos)>,
        rng: &mut ChaCha8Rng,
    ) -> Result<(), String> {
        let n = scale(rule.guards, self.inputs.security);
        let weights: Vec<(u8, u32)> = (security.guards.iter())
            .filter(|(_, w)| *w > 0)
            .filter_map(|(z, w)| Some((t.zone(z)?, u32::from(*w))))
            .collect();
        let mut pools: Vec<Vec<(i32, Pos)>> = (weights.iter())
            .map(|&(zone, _)| {
                let mut v = self.spots(layout, |_, _, r| r.zone == zone);
                v.retain(|s| !used.contains(s));
                v.shuffle(rng);
                v
            })
            .collect();
        let mut dealt = vec![0u32; weights.len()];
        for _ in 0..n {
            // The zone furthest under its share: the most weight per guard it would hold.
            let i = (0..weights.len())
                .filter(|&i| !pools[i].is_empty())
                .max_by(|&a, &b| {
                    let (wa, wb) = (weights[a].1, weights[b].1);
                    (wa * (dealt[b] + 1))
                        .cmp(&(wb * (dealt[a] + 1)))
                        .then(b.cmp(&a))
                })
                .ok_or_else(|| format!("too few spots for {n} guards"))?;
            dealt[i] += 1;
            let (level, at) = pools[i].pop().expect("a pool with spots");
            used.insert((level, at));
            let zone = weights[i].0;
            let mut others: Vec<Pos> = (self
                .spots(layout, |x, _, r| x.level == level && r.zone == zone))
            .into_iter()
            .map(|(_, p)| p)
            .filter(|&p| p != at)
            .collect();
            others.shuffle(rng);
            let walk = rng.random_range(2..=4);
            let patrol = std::iter::once(at)
                .chain(others.into_iter().take(walk - 1))
                .collect();
            self.guards.push(GuardPost {
                level,
                at,
                zone,
                patrol,
            });
        }
        Ok(())
    }

    /// Cameras on the spots of spine and hall rooms nearest the fitted doors, a door at a
    /// time; turrets in the security's turret zones; containers in the rooms off the spine
    /// above public; each count from its range by security, as many as find a spot. A
    /// turret or a container stands where every tile round it is its room's open floor
    /// and no other one or a guard stands, so it cuts no way.
    fn stand_devices(
        &mut self,
        layout: &VenueLayout,
        t: &VenueType,
        security: &SecurityDef,
        used: &mut BTreeSet<(i32, Pos)>,
        rng: &mut ChaCha8Rng,
    ) {
        let s = self.inputs.security;
        let walked: BTreeSet<(i32, Pos)> = (self.guards.iter())
            .flat_map(|g| g.patrol.iter().map(move |p| (g.level, *p)))
            .collect();
        // Cameras.
        let mut aims: Vec<(i32, Pos)> = (self.fitted.iter())
            .filter_map(|&(level, k)| Some((level, layout.level(level)?.doors.get(k)?.at)))
            .collect();
        if aims.is_empty() {
            aims = (self.kept.iter())
                .filter_map(|&level| layout.level(level))
                .flat_map(|x| {
                    x.doors
                        .iter()
                        .filter(|d| d.lock.is_some())
                        .map(|d| (x.level, d.at))
                })
                .collect();
        }
        let watch = self.spots(layout, |x, i, r| {
            i < x.bands.len() || r.kind == RoomKind::Hall
        });
        // With no spot by the doors left: another spine or hall spot, then any room's.
        let mut rest = watch.clone();
        rest.shuffle(rng);
        let mut anywhere = self.spots(layout, |_, _, _| true);
        anywhere.shuffle(rng);
        let mut next = 0;
        for _ in 0..scale(security.cameras, s) {
            let by_door = (0..aims.len()).find_map(|k| {
                let (level, door) = aims[(next + k) % aims.len()];
                let spot = (watch.iter())
                    .filter(|&&(l, p)| l == level && !used.contains(&(l, p)))
                    .min_by_key(|&&(_, p)| (p.manhattan(door), p.y, p.x))?;
                Some((k, *spot))
            });
            let at = match by_door {
                Some((k, at)) => {
                    next = (next + k + 1) % aims.len();
                    Some(at)
                }
                None => (rest.iter().chain(&anywhere))
                    .copied()
                    .find(|s| !used.contains(s)),
            };
            let Some((level, at)) = at else {
                break;
            };
            used.insert((level, at));
            self.devices.push(Device {
                level,
                at,
                kind: DeviceKind::Camera,
            });
        }
        // Turrets and containers stand in the way.
        let zones: Vec<u8> = (security.turret_zones.iter())
            .filter_map(|z| t.zone(z))
            .collect();
        let mut turrets = self.spots(layout, |_, _, r| zones.contains(&r.zone));
        turrets.shuffle(rng);
        let mut blocked: BTreeSet<(i32, Pos)> =
            self.guards.iter().map(|g| (g.level, g.at)).collect();
        for (level, at) in self.blockers(
            layout,
            &turrets,
            scale(security.turrets, s),
            used,
            &walked,
            &mut blocked,
        ) {
            self.devices.push(Device {
                level,
                at,
                kind: DeviceKind::Turret,
            });
        }
        let mut boxes = self.spots(layout, |x, i, r| i >= x.bands.len() && r.zone > 0);
        boxes.shuffle(rng);
        for (level, at) in self.blockers(
            layout,
            &boxes,
            scale(security.containers, s),
            used,
            &walked,
            &mut blocked,
        ) {
            self.containers.push(Placed { level, at });
        }
    }

    /// Up to `n` of `spots`, in order, that a piece nobody walks through can stand on:
    /// free, walked by no guard, with every tile round it its room's open floor off the
    /// room's edge (where the plain level's furniture stands, so a copy's furniture is
    /// the plain level's) and none of them `blocked`.
    fn blockers(
        &self,
        layout: &VenueLayout,
        spots: &[(i32, Pos)],
        n: u32,
        used: &mut BTreeSet<(i32, Pos)>,
        walked: &BTreeSet<(i32, Pos)>,
        blocked: &mut BTreeSet<(i32, Pos)>,
    ) -> Vec<(i32, Pos)> {
        let mut out = Vec::new();
        for &(level, at) in spots {
            if out.len() >= n as usize {
                break;
            }
            if used.contains(&(level, at)) || walked.contains(&(level, at)) {
                continue;
            }
            let Some(x) = layout.level(level) else {
                continue;
            };
            let Some(room) = room_holding(layout, level, at) else {
                continue;
            };
            if !x.rooms[room].rect.inset(2).contains(at) {
                continue;
            }
            let round = |q: Pos| open_floor(x, room, q) && !blocked.contains(&(level, q));
            if !at.neighbors8().all(round) {
                continue;
            }
            used.insert((level, at));
            blocked.insert((level, at));
            out.push((level, at));
        }
        out
    }

    /// Every spot of the kept levels' rooms that `pick` takes (a level, a room's index and
    /// the room), lowest level first, in room order.
    fn spots(
        &self,
        layout: &VenueLayout,
        pick: impl Fn(&LevelLayout, usize, &PlannedRoom) -> bool,
    ) -> Vec<(i32, Pos)> {
        let mut v = Vec::new();
        for x in self.kept.iter().filter_map(|&level| layout.level(level)) {
            for (i, r) in x.rooms.iter().enumerate() {
                if pick(x, i, r) {
                    v.extend(r.spots.iter().map(|&p| (x.level, p)));
                }
            }
        }
        v
    }
}

/// `lo` at security 0 to `hi` at 255, in proportion between.
fn scale<T: Into<u32>>((lo, hi): (T, T), security: u8) -> u32 {
    let (lo, hi) = (lo.into(), hi.into());
    lo + hi.saturating_sub(lo) * u32::from(security) / 255
}

/// The room of `level` that has `at` as a spot, else whose floor holds it.
pub(super) fn room_holding(layout: &VenueLayout, level: i32, at: Pos) -> Option<usize> {
    let x = layout.level(level)?;
    (x.rooms.iter().position(|r| r.spots.contains(&at)))
        .or_else(|| x.rooms.iter().position(|r| r.rect.contains(at)))
}

/// The graph node of the objective's room.
fn goal_of(g: &VenueGraph, layout: &VenueLayout, objective: Placed) -> Result<usize, String> {
    room_holding(layout, objective.level, objective.at)
        .and_then(|room| g.room(objective.level, room))
        .ok_or_else(|| "the objective stands in no kept room".to_string())
}

/// The spots of room `room` on `level` that nothing of the layer takes yet.
fn free_spots(
    layout: &VenueLayout,
    level: i32,
    room: usize,
    used: &BTreeSet<(i32, Pos)>,
) -> Vec<Pos> {
    (layout.level(level).and_then(|x| x.rooms.get(room)))
        .map(|r| {
            r.spots
                .iter()
                .copied()
                .filter(|p| !used.contains(&(level, *p)))
                .collect()
        })
        .unwrap_or_default()
}

/// Whether `q` is open floor of room `room` before anything stands on it: inside its rect,
/// off the core, the pieces and beside them, the doors, the reserved tiles, and the rect
/// of any other room round it but one that holds it.
fn open_floor(x: &LevelLayout, room: usize, q: Pos) -> bool {
    let r = &x.rooms[room];
    r.rect.contains(q)
        && !x.core.is_some_and(|c| c.contains(q))
        && !x.pieces.iter().any(|p| p.rect.grow(1).contains(q))
        && !x.doors.iter().any(|d| d.at == q)
        && !x.reserved.contains(&q)
        && !(x.rooms.iter().enumerate()).any(|(i, o)| {
            i != room && o.rect.intersection(r.rect) != Some(r.rect) && o.rect.grow(1).contains(q)
        })
}

/// The fewest locks the base way holding `keys` can't pass yet from each node to one of
/// `to`; u32::MAX where none gets there.
fn locks_to(g: &VenueGraph, to: &[usize], keys: &Keys) -> Vec<u32> {
    let mut into = vec![Vec::new(); g.nodes.len()];
    for (i, e) in g.edges.iter().enumerate() {
        into[e.to].push(i);
    }
    let mut left = vec![u32::MAX; g.nodes.len()];
    let mut queue = VecDeque::new();
    for &n in to {
        left[n] = 0;
        queue.push_back(n);
    }
    while let Some(n) = queue.pop_front() {
        for e in into[n].iter().map(|&i| &g.edges[i]) {
            let cost = u32::from(!passes(e.lock.as_ref(), Ways::default(), keys));
            if left[n] + cost < left[e.from] {
                left[e.from] = left[n] + cost;
                if cost == 0 {
                    queue.push_front(e.from);
                } else {
                    queue.push_back(e.from);
                }
            }
        }
    }
    left
}

/// The fewest edges from node `from` to each node, any lock and either way.
fn hops_from(g: &VenueGraph, from: usize) -> Vec<u32> {
    let mut near = vec![Vec::new(); g.nodes.len()];
    for e in &g.edges {
        near[e.from].push(e.to);
        near[e.to].push(e.from);
    }
    let mut hops = vec![u32::MAX; g.nodes.len()];
    hops[from] = 0;
    let mut queue = VecDeque::from([from]);
    while let Some(n) = queue.pop_front() {
        for &m in &near[n] {
            if hops[m] == u32::MAX {
                hops[m] = hops[n] + 1;
                queue.push_back(m);
            }
        }
    }
    hops
}

/// The objective's spot: one of the spots of its rooms (on its role's levels when it
/// names one), the rng's pick.
fn objective(
    layout: &VenueLayout,
    def: &ObjectiveDef,
    rng: &mut ChaCha8Rng,
) -> Result<Placed, String> {
    let spots: Vec<Placed> = (layout.levels.iter())
        .filter(|x| def.level.as_ref().is_none_or(|role| *role == x.role))
        .flat_map(|x| {
            (x.rooms.iter().filter(|r| r.name == def.room))
                .flat_map(move |r| r.spots.iter().map(move |&at| Placed { level: x.level, at }))
        })
        .collect();
    if spots.is_empty() {
        return Err(format!("no {} for the objective", def.room));
    }
    Ok(spots[rng.random_range(0..spots.len())])
}
