//! The Net's state this session: every construct's trace, the hacks that landed,
//! the backdoors, the deck's cooling after a dump, and the program in flight.

use crate::app::{AppState, FxInstance};
use crate::constructs::Constructs;
use crate::render::map::MapRef;
use citygen::pathing::{astar, bresenham};
use citygen::rng::hash64;
use citygen::{BuildingId, Construct, IceGrade, NodeId, NodeKind, Place, Pos, Terrain};
use game::{
    ALERT_MINUTES, COOLING_MINUTES, Fx, HACK_MINUTES, HackTarget, ItemId, Op, Run, Target, Trace,
    TraceEvent, applies, difficulty, power, succeeds,
};
use std::collections::{BTreeMap, BTreeSet};

/// A hack that landed: on what, with which op, when, and until when (`None` forever).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hack {
    pub building: BuildingId,
    pub target: HackTarget,
    pub kind: Target,
    pub op: Op,
    pub at: u64,
    pub until: Option<u64>,
}

/// Turns an awake piece waits at the last seen tile before it gives up.
pub const LOST_TURNS: u8 = 3;
/// Minutes the deck cools after the failsafe pulls you out.
pub const FAILSAFE_COOLING: u64 = 30;
/// Turns a mask holds once raised.
pub const MASK_TURNS: u8 = 6;

/// What a piece of ICE is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IceMode {
    Asleep,
    Awake { seen: Pos, lost: u8 },
}

/// A piece of ICE's state this session: its mode, what is left of it, and where it
/// is on its beat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ice {
    pub mode: IceMode,
    pub integrity: u8,
    pub step: usize,
    pub forward: bool,
}

pub struct NetState {
    /// One state per guard of every construct entered.
    pub ice: BTreeMap<BuildingId, Vec<Ice>>,
    /// Turns the mask still holds.
    pub mask: u8,
    /// The mask was raised this turn: it does not wear on the turn that raised it.
    mask_fresh: bool,
    /// Shield charges: each soaks one hit.
    pub shield: u8,
    /// The deck pulls you out at a quarter of your HP; off, black ICE can kill you.
    pub failsafe: bool,
    /// Set when black ICE killed: what did it. The screen reads and clears it.
    pub death: Option<String>,
    pub traces: BTreeMap<BuildingId, Trace>,
    pub hacks: Vec<Hack>,
    pub backdoors: BTreeSet<BuildingId>,
    /// The game minute the dumped deck jacks in again.
    pub cooling_until: Option<u64>,
    /// The program in flight.
    pub run: Option<Run>,
    last_minute: u64,
    /// Every roll comes out this way, for tests.
    #[cfg(test)]
    pub force_roll: Option<u8>,
}

impl Default for NetState {
    fn default() -> Self {
        NetState {
            ice: BTreeMap::new(),
            mask: 0,
            mask_fresh: false,
            shield: 0,
            failsafe: true,
            death: None,
            traces: BTreeMap::new(),
            hacks: Vec::new(),
            backdoors: BTreeSet::new(),
            cooling_until: None,
            run: None,
            last_minute: 0,
            #[cfg(test)]
            force_roll: None,
        }
    }
}

impl NetState {
    /// The game minute now.
    pub fn now(&self, st: &AppState) -> u64 {
        st.clock_tick() / sim::TICKS_PER_MINUTE
    }

    pub fn trace(&mut self, b: BuildingId) -> &mut Trace {
        self.traces.entry(b).or_default()
    }

    pub fn alert(&self, b: BuildingId, now: u64) -> bool {
        self.traces.get(&b).is_some_and(|t| t.alert(now))
    }

    fn live(&self, now: u64) -> impl Iterator<Item = &Hack> {
        self.hacks
            .iter()
            .filter(move |h| h.until.is_none_or(|u| now < u))
    }

    /// Records a landed hack; a backdoor is forever.
    pub fn land(&mut self, b: BuildingId, target: HackTarget, kind: Target, op: Op, now: u64) {
        // A hack lasts an hour, or until the building's alert ends, whichever is sooner.
        let alert_end = self
            .traces
            .get(&b)
            .and_then(|t| t.alert_until)
            .filter(|u| now < *u);
        let until = (op != Op::Backdoor)
            .then(|| alert_end.map_or(now + HACK_MINUTES, |u| u.min(now + HACK_MINUTES)));
        if op == Op::Backdoor {
            self.backdoors.insert(b);
        }
        self.hacks.push(Hack {
            building: b,
            target,
            kind,
            op,
            at: now,
            until,
        });
    }

    /// Whether the building's private rooms stand open: a lock cracked or spoofed,
    /// and no alert.
    pub fn unlocked(&self, b: BuildingId, now: u64) -> bool {
        !self.alert(b, now)
            && self.live(now).any(|h| {
                h.building == b
                    && matches!(h.op, Op::Crack | Op::Spoof)
                    && h.kind == Target::Node(NodeKind::DoorLock)
            })
    }

    /// Whether the lifts answer: always off alert; on alert only with a spoof on the
    /// lift control that landed after the alarm.
    pub fn lifts_answer(&self, b: BuildingId, now: u64) -> bool {
        let Some(t) = self.traces.get(&b).filter(|t| t.alert(now)) else {
            return true;
        };
        let since = t.alert_until.unwrap_or(0).saturating_sub(ALERT_MINUTES);
        self.live(now).any(|h| {
            h.building == b
                && h.op == Op::Spoof
                && h.kind == Target::Node(NodeKind::LiftCtrl)
                && h.at >= since
        })
    }

    pub fn blinded(&self, node: NodeId, now: u64) -> bool {
        self.live(now)
            .any(|h| h.target == HackTarget::Node(node) && h.op == Op::Blind)
    }

    pub fn copied(&self, node: NodeId, now: u64) -> bool {
        self.live(now)
            .any(|h| h.target == HackTarget::Node(node) && h.op == Op::Copy)
    }

    /// Minutes until the dumped deck jacks in again.
    pub fn cooling(&self, now: u64) -> Option<u64> {
        self.cooling_until.filter(|u| now < *u).map(|u| u - now)
    }

    /// The alarm: every hack on the building but a backdoor expires now.
    pub fn alarm(&mut self, b: BuildingId, now: u64) {
        let _ = now;
        self.hacks
            .retain(|h| h.building != b || h.op == Op::Backdoor);
    }

    /// Sets every guard of a generated construct standing or lifted by the hacks.
    pub fn sync_guards(&self, constructs: &mut Constructs, b: BuildingId, now: u64) {
        let backdoor = self.backdoors.contains(&b);
        let lifted: Vec<usize> = self
            .live(now)
            .filter(|h| matches!(h.op, Op::Crack | Op::Attack))
            .filter_map(|h| match h.target {
                HackTarget::Guard { building, index } if building == b => Some(index),
                _ => None,
            })
            .collect();
        let Some(k) = constructs.get_mut(b) else {
            return;
        };
        for i in 0..k.guards.len() {
            let standing = !(i == 0 && backdoor) && !lifted.contains(&i);
            k.set_guard(i, standing);
        }
    }

    /// The game minutes up to `now` pass: hacks expire, every trace the player is
    /// not inside cools, alerts end and their guards stand again.
    pub fn minute(&mut self, now: u64, inside: Option<BuildingId>, constructs: &mut Constructs) {
        if now <= self.last_minute {
            return;
        }
        let from = self.last_minute + 1;
        self.last_minute = now;
        let buildings: Vec<BuildingId> = self.traces.keys().copied().collect();
        for m in from..=now {
            for b in &buildings {
                let backdoor = self.backdoors.contains(b);
                let t = self.traces.get_mut(b).expect("traced");
                let ending = t.alert_until.is_some_and(|u| m >= u);
                if ending || inside != Some(*b) {
                    t.cool(m, backdoor);
                }
            }
        }
        self.hacks.retain(|h| h.until.is_none_or(|u| now < u));
        for b in buildings {
            self.sync_guards(constructs, b, now);
        }
    }

    /// The ICE states of a construct, made on first ask.
    pub fn ice_of(&mut self, k: &Construct) -> &mut Vec<Ice> {
        let states = self.ice.entry(k.building).or_default();
        while states.len() < k.guards.len() {
            let g = &k.guards[states.len()];
            // A piece starts on its beat where its post is: the gate guard at the near
            // end walking forward, the vault guard at the far end walking back.
            let step = g.beat.iter().position(|p| *p == g.pos).unwrap_or(0);
            states.push(Ice {
                mode: IceMode::Asleep,
                integrity: g.grade.integrity(),
                step,
                forward: step + 1 < g.beat.len(),
            });
        }
        states
    }

    /// Every piece of a construct wakes and heads for a tile.
    pub fn wake_all(&mut self, b: BuildingId, seen: Pos) {
        if let Some(states) = self.ice.get_mut(&b) {
            for i in states {
                i.mode = IceMode::Awake { seen, lost: 0 };
            }
        }
    }

    fn roll(&self, st: &AppState, seed: u64) -> u8 {
        #[cfg(test)]
        if let Some(r) = self.force_roll {
            return r;
        }
        (hash64(st.clock_tick(), seed) % 10 + 1) as u8
    }
}

fn building_name(st: &AppState, b: BuildingId) -> String {
    st.city
        .as_ref()
        .and_then(|c| c.building_opt(b))
        .map(|x| x.name.clone().unwrap_or_else(|| x.kind.label().to_string()))
        .unwrap_or_else(|| "the building".to_string())
}

/// The building whose construct or floor the player is in.
pub fn building_here(st: &AppState) -> Option<BuildingId> {
    match st.player.as_ref()?.place {
        Place::Construct(b) | Place::Floor { building: b, .. } => Some(b),
        Place::Stratum(_) => None,
    }
}

/// What a hack target is, in kind.
pub fn target_kind(st: &AppState, b: BuildingId, target: HackTarget) -> Option<Target> {
    match target {
        HackTarget::Node(id) => Some(Target::Node(st.city.as_ref()?.net.node(id).kind)),
        HackTarget::Deck => Some(Target::Deck),
        HackTarget::Guard { building, index } => {
            if building != b {
                return None;
            }
            let k = st.constructs.get(building)?;
            Some(Target::Guard(k.guards.get(index)?.grade))
        }
    }
}

/// Starts a program against a target in the construct the player stands in.
pub fn start_run(st: &mut AppState, program: ItemId, target: HackTarget) -> Result<String, String> {
    let Some(Place::Construct(b)) = st.player.as_ref().map(|p| p.place) else {
        return Err("not in a construct".to_string());
    };
    if st.character.deck(&st.content).is_none() {
        return Err("no deck jacked".to_string());
    }
    if !st.character.loaded.contains(&program) {
        return Err("that program is not loaded".to_string());
    }
    let item = st
        .content
        .item(&program)
        .ok_or_else(|| format!("unknown program {program}"))?;
    let spec = item.program.ok_or("that is not a program")?;
    let kind = target_kind(st, b, target).ok_or("nothing to run against")?;
    if !applies(spec.op, kind) {
        return Err(format!("{} does nothing there", item.name));
    }
    let now = st.net.now(st);
    if spec.op == Op::Copy
        && st
            .net
            .copied(node_of(target).unwrap_or(NodeId(u32::MAX)), now)
    {
        return Err("copied already".to_string());
    }
    if let HackTarget::Guard { building, index } = target
        && st.constructs.get(building).is_none_or(|k| {
            k.guards
                .get(index)
                .is_none_or(|g| k.guard_at(g.pos).is_none())
        })
    {
        return Err("nothing stands there".to_string());
    }
    let dropped = st.net.run.replace(Run {
        program: program.clone(),
        target,
        done: 0,
        work: spec.work,
    });
    let starts = format!("{} starts · {} turns", spec.op.label(), spec.work);
    Ok(match dropped {
        Some(_) => format!("run aborted · {starts}"),
        None => starts,
    })
}

fn node_of(target: HackTarget) -> Option<NodeId> {
    match target {
        HackTarget::Node(id) => Some(id),
        HackTarget::Guard { .. } | HackTarget::Deck => None,
    }
}

/// One turn of the program in flight: the roll, the heat, the alarm or the dump,
/// and the hack landing when the work is done. Logs what happened.
pub fn step_run(st: &mut AppState) -> Option<String> {
    let run = st.net.run.clone()?;
    let Some(Place::Construct(b)) = st.player.as_ref().map(|p| p.place) else {
        st.net.run = None;
        return None;
    };
    let spec = st.content.item(&run.program)?.program?;
    let kind = target_kind(st, b, run.target)?;
    let security = st.constructs.get(b)?.security;
    let pw = power(&st.character, &st.content, &spec, kind);
    let df = difficulty(security);
    let seed = match run.target {
        HackTarget::Node(id) => id.index() as u64,
        HackTarget::Guard { index, .. } => 1000 + index as u64,
        HackTarget::Deck => 2000,
    };
    let roll = st.net.roll(st, seed);
    // A program on yourself never fails.
    let ok = kind == Target::Deck || succeeds(roll, pw, df);
    let now = st.net.now(st);
    let noise = if ok { spec.noise } else { spec.noise + 2 };
    let was_alert = st.net.alert(b, now);
    let event = st.net.trace(b).add(noise, now);
    let heat = st.net.traces[&b].heat;
    match event {
        TraceEvent::Dump => {
            if !was_alert {
                let line = raise_alarm(st, b);
                st.warn(line);
            }
            dump(st, "the trace found you");
            return Some("DUMPED".to_string());
        }
        TraceEvent::Alarm => {
            let line = raise_alarm(st, b);
            st.warn(line);
        }
        TraceEvent::None => {}
    }
    let label = spec.op.label();
    // An attack is a hit a turn until the piece breaks; the run stays up.
    if spec.op == Op::Attack {
        let HackTarget::Guard { index, .. } = run.target else {
            st.net.run = None;
            return None;
        };
        // The piece must still stand beside you.
        let in_reach = st
            .player
            .as_ref()
            .zip(st.constructs.get(b))
            .is_some_and(|(p, k)| {
                k.guards
                    .get(index)
                    .is_some_and(|g| g.standing && g.pos.chebyshev(p.pos) <= 1)
            });
        if !in_reach {
            st.net.run = None;
            let line = "attack · out of reach".to_string();
            st.say(line.clone());
            return Some(line);
        }
        let line = if ok {
            let mut damage = 2 * spec.rating;
            if st.character.flags(&st.content).contains("black_ice") {
                damage += 1;
            }
            let grade = st.constructs.get(b).map(|k| k.guards[index].grade)?;
            let seen = st.player.as_ref()?.pos;
            let left = {
                let states = st.net.ice.get_mut(&b)?;
                let ice = states.get_mut(index)?;
                ice.integrity = ice.integrity.saturating_sub(damage);
                // A hit piece turns on you.
                ice.mode = IceMode::Awake { seen, lost: 0 };
                ice.integrity
            };
            if left == 0 {
                st.net.run = None;
                st.net.land(b, run.target, kind, Op::Attack, now);
                st.net.sync_guards(&mut st.constructs, b, now);
                kick(st, 6);
                format!("{} ICE broken", grade.label())
            } else {
                format!(
                    "attack · {} ICE {left}/{}",
                    grade.label(),
                    grade.integrity()
                )
            }
        } else {
            format!("{label} slips · heat {heat}")
        };
        st.say(line.clone());
        return Some(line);
    }
    let line = if ok {
        let done = run.done + 1;
        if done >= run.work {
            st.net.run = None;
            let line = land(st, b, run.target, kind, spec.op, now);
            st.say(line.clone());
            kick(st, 6);
            return Some(line);
        }
        st.net.run = Some(Run { done, ..run });
        format!("{label} {done}/{}", run.work)
    } else {
        format!("{label} slips · heat {heat}")
    };
    st.say(line.clone());
    Some(line)
}

/// A hack lands: the state remembers it, the building feels it, the log says so.
fn land(
    st: &mut AppState,
    b: BuildingId,
    target: HackTarget,
    kind: Target,
    op: Op,
    now: u64,
) -> String {
    st.net.land(b, target, kind, op, now);
    let name = building_name(st, b);
    let line = match (op, kind) {
        (Op::Crack, Target::Guard(_)) => "ICE cracked · the way is open".to_string(),
        (Op::Crack | Op::Spoof, Target::Node(NodeKind::DoorLock)) => {
            let verb = if op == Op::Crack {
                "cracked"
            } else {
                "spoofed"
            };
            format!("lock {verb} · `{name}`'s locked doors open for an hour")
        }
        (Op::Spoof, Target::Node(NodeKind::LiftCtrl)) => {
            "lift control spoofed · the lifts answer you".to_string()
        }
        (Op::Blind, _) => "camera blinded".to_string(),
        (Op::Copy, Target::Node(NodeKind::DataVault)) => {
            st.character.give(ItemId::new("vault_data"), 1);
            "vault copied · a `Vault dump` in the pack".to_string()
        }
        (Op::Copy, _) => {
            st.character.give(ItemId::new("paydata"), 1);
            "server copied · a `Paydata shard` in the pack".to_string()
        }
        (Op::Backdoor, _) => format!("backdoor planted · `{name}`'s gate is yours"),
        (Op::Shield, _) => {
            st.net.shield = (st.net.shield + 1).min(2);
            let n = st.net.shield;
            format!(
                "shield raised · {n} charge{}",
                if n == 1 { "" } else { "s" }
            )
        }
        (Op::Mask, _) => {
            st.net.mask = MASK_TURNS;
            st.net.mask_fresh = true;
            format!("mask raised · {MASK_TURNS} turns")
        }
        (Op::Crack, Target::Node(k)) => format!("{} cracked", k.label()),
        (op, _) => format!("{} landed", op.label()),
    };
    st.net.sync_guards(&mut st.constructs, b, now);
    line
}

fn kick(st: &mut AppState, ticks: u64) {
    let Some((place, origin)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return;
    };
    st.fx.push(FxInstance {
        fx: Fx::Kick(ticks),
        place,
        origin,
        radius: 0,
        started: st.tick,
        revealed: Vec::new(),
        targets: Vec::new(),
    });
}

/// The alarm: the building locks down, its hacks expire, its ICE wakes.
pub fn raise_alarm(st: &mut AppState, b: BuildingId) -> String {
    let now = st.net.now(st);
    st.net.alarm(b, now);
    st.net.sync_guards(&mut st.constructs, b, now);
    if let Some(p) = st
        .player
        .as_ref()
        .filter(|p| p.place == Place::Construct(b))
    {
        let pos = p.pos;
        if let Some(k) = st.constructs.get(b) {
            st.net.ice_of(k);
        }
        st.net.wake_all(b, pos);
    }
    format!("ALARM · `{}` locks down", building_name(st, b))
}

/// Dumped: back to the body hard, the focus gone, the deck cooling.
pub fn dump(st: &mut AppState, why: &str) {
    dump_for(st, why, COOLING_MINUTES);
}

/// Dumped, with the deck cooling for a given stretch.
pub fn dump_for(st: &mut AppState, why: &str, cooling: u64) {
    let bonus = st.character.sight_bonus(&st.content);
    let now = st.net.now(st);
    st.net.run = None;
    st.net.cooling_until = Some(now + cooling);
    st.character.focus = 0;
    if let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_mut())
        && player.jack_out(city, bonus).is_ok()
        && let Some(map) = MapRef::for_place(city, &st.interiors, &st.constructs, player.place)
    {
        player.update_fov(&map, bonus);
    }
    kick(st, 20);
    st.warn(format!("DUMPED · {why}"));
    st.repaint = true;
}

/// Whether a piece at `from` sees a player: on the spine, in its row or column,
/// within its sight (halved under a mask), with no wall between.
pub fn sees(k: &Construct, from: Pos, grade: IceGrade, player: Pos, masked: bool) -> bool {
    let Some(t) = k.tiles.get(player) else {
        return false;
    };
    if !matches!(t.terrain, Terrain::NetLane | Terrain::NetGate) {
        return false;
    }
    if from.x != player.x && from.y != player.y {
        return false;
    }
    let range = if masked {
        grade.sight() / 2
    } else {
        grade.sight()
    };
    if from.chebyshev(player) > range {
        return false;
    }
    bresenham(from, player)
        .into_iter()
        .filter(|p| *p != from && *p != player)
        .all(|p| {
            k.tiles
                .get(p)
                .is_some_and(|t| t.terrain != Terrain::NetWall)
        })
}

/// One step along the shortest walkable path from `from` to `goal`, never onto
/// `avoid` (the player's tile) and never onto another standing piece.
fn step_toward(k: &Construct, from: Pos, goal: Pos, avoid: Pos) -> Option<Pos> {
    if from == goal {
        return None;
    }
    let path = astar(k.tiles.size(), from, goal, |_, to| {
        // The player's own tile may be the goal (the piece stops beside it) but is
        // never crossed on the way somewhere else.
        if to == avoid && to != goal {
            return None;
        }
        let t = k.tiles.get(to)?;
        (t.is_walkable() || (to == goal && t.feature.is_none())).then_some(1)
    })?;
    path.into_iter().find(|p| *p != from)
}

/// The ICE's turn in the construct the player stands in: every standing piece
/// looks, then acts: walks its beat asleep, hunts awake, hits beside the player.
pub fn ice_turn(st: &mut AppState) {
    let Some(player) = st.player.as_ref() else {
        return;
    };
    let Place::Construct(b) = player.place else {
        return;
    };
    let pos = player.pos;
    // A held mask is a turn's noise, and it wears off.
    let masked = st.net.mask > 0;
    if masked && std::mem::take(&mut st.net.mask_fresh) {
        // Raised this turn: it starts wearing next turn.
    } else if masked {
        st.net.mask -= 1;
        let now = st.net.now(st);
        let was_alert = st.net.alert(b, now);
        match st.net.trace(b).add(1, now) {
            TraceEvent::Dump => {
                if !was_alert {
                    let line = raise_alarm(st, b);
                    st.warn(line);
                }
                dump(st, "the trace found you");
                return;
            }
            TraceEvent::Alarm => {
                let line = raise_alarm(st, b);
                st.warn(line);
            }
            TraceEvent::None => {}
        }
        if st.net.mask == 0 {
            st.say("the mask is gone");
        }
    }
    let Some(k) = st.constructs.get(b) else {
        return;
    };
    let n = k.guards.len();
    st.net.ice_of(k);
    for i in 0..n {
        let Some(k) = st.constructs.get(b) else {
            return;
        };
        let g = k.guards[i].clone();
        if k.guard_at(g.pos).is_none() {
            continue;
        }
        let mut ice = st.net.ice[&b][i].clone();
        if ice.integrity == 0 {
            ice.integrity = g.grade.integrity();
        }
        if sees(k, g.pos, g.grade, pos, masked) {
            ice.mode = IceMode::Awake { seen: pos, lost: 0 };
        }
        let mut moved = None;
        let mut hits = false;
        match ice.mode {
            IceMode::Asleep => {
                if g.beat.len() >= 2 {
                    let home = g.beat[ice.step.min(g.beat.len() - 1)];
                    if g.pos != home {
                        moved = step_toward(k, g.pos, home, pos);
                    } else {
                        let last = g.beat.len() - 1;
                        if ice.forward && ice.step >= last {
                            ice.forward = false;
                        } else if !ice.forward && ice.step == 0 {
                            ice.forward = true;
                        }
                        let next = if ice.forward {
                            ice.step + 1
                        } else {
                            ice.step - 1
                        };
                        let target = g.beat[next];
                        if target != pos && k.tiles[target].is_walkable() {
                            moved = Some(target);
                            ice.step = next;
                        }
                    }
                }
            }
            IceMode::Awake { seen, lost } => {
                if g.pos.chebyshev(pos) <= 1 {
                    hits = true;
                } else if g.pos == seen || step_toward(k, g.pos, seen, pos).is_none() {
                    let lost = lost + 1;
                    ice.mode = if lost >= LOST_TURNS {
                        IceMode::Asleep
                    } else {
                        IceMode::Awake { seen, lost }
                    };
                } else {
                    moved = step_toward(k, g.pos, seen, pos);
                }
            }
        }
        if let Some(to) = moved
            && let Some(k) = st.constructs.get_mut(b)
        {
            k.move_guard(i, to);
        }
        if let Some(states) = st.net.ice.get_mut(&b)
            && let Some(slot) = states.get_mut(i)
        {
            *slot = ice;
        }
        if hits {
            hit(st, b, i);
        }
        if st.net.death.is_some()
            || st
                .player
                .as_ref()
                .is_none_or(|p| p.place != Place::Construct(b))
        {
            return;
        }
    }
}

/// A standing piece beside the player hits them: white adds heat, grey drains
/// focus and dumps at zero, black takes HP through the jack. A shield charge soaks
/// a hit; the deck's rating comes off grey and black hits.
fn hit(st: &mut AppState, b: BuildingId, i: usize) {
    let Some((grade, security)) = st
        .constructs
        .get(b)
        .map(|k| (k.guards[i].grade, k.security))
    else {
        return;
    };
    if st.net.shield > 0 {
        st.net.shield -= 1;
        st.say("shield absorbs the hit");
        return;
    }
    let armour = st
        .character
        .deck(&st.content)
        .map(|(_, d)| d.rating as u32)
        .unwrap_or(0);
    let now = st.net.now(st);
    match grade {
        IceGrade::White => {
            let was_alert = st.net.alert(b, now);
            let event = st.net.trace(b).add(5, now);
            let heat = st.net.traces[&b].heat;
            st.warn(format!("white ICE hits · heat {heat}"));
            match event {
                TraceEvent::Dump => {
                    if !was_alert {
                        let line = raise_alarm(st, b);
                        st.warn(line);
                    }
                    dump(st, "the trace found you");
                }
                TraceEvent::Alarm => {
                    let line = raise_alarm(st, b);
                    st.warn(line);
                }
                TraceEvent::None => {}
            }
        }
        IceGrade::Grey => {
            let drain = 10u32.saturating_sub(armour).max(1);
            st.character.focus = st.character.focus.saturating_sub(drain);
            let focus = st.character.focus;
            st.warn(format!("grey ICE hits · focus {focus}"));
            if focus == 0 {
                dump(st, "grey ICE drained you");
            }
        }
        IceGrade::Black => {
            let damage = (6 + security.saturating_sub(230) as u32 / 5)
                .saturating_sub(armour)
                .max(1);
            st.character.hp = st.character.hp.saturating_sub(damage);
            let hp = st.character.hp;
            st.warn(format!("black ICE hits · hp {hp}"));
            let quarter = st.character.hp_max(&st.content) / 4;
            if st.net.failsafe {
                if hp <= quarter {
                    dump_for(st, "the failsafe pulled you out", FAILSAFE_COOLING);
                }
            } else if hp == 0 {
                st.net.run = None;
                st.net.death = Some(format!("black ICE at {}", building_name(st, b)));
            }
        }
    }
}

/// The HERE lines for the ICE of a construct the player can see.
pub fn ice_lines(st: &AppState, b: BuildingId, seen: impl Fn(Pos) -> bool) -> Vec<String> {
    let Some(k) = st.constructs.get(b) else {
        return Vec::new();
    };
    let Some(states) = st.net.ice.get(&b) else {
        return Vec::new();
    };
    k.guards
        .iter()
        .zip(states)
        .filter(|(g, _)| k.guard_at(g.pos).is_some() && seen(g.pos))
        .map(|(g, i)| {
            let mode = match i.mode {
                IceMode::Asleep => "asleep",
                IceMode::Awake { .. } => "hunting",
            };
            format!(
                "▣ {} ICE · {mode} · {}/{}",
                g.grade.label(),
                i.integrity,
                g.grade.integrity()
            )
        })
        .collect()
}

/// Why the port refuses to jack in right now: the deck cooling, or a lockdown.
pub fn jack_in_refusal(st: &AppState) -> Option<String> {
    let now = st.net.now(st);
    if let Some(left) = st.net.cooling(now) {
        return Some(format!("deck cooling · {left} min"));
    }
    let city = st.city.as_ref()?;
    let player = st.player.as_ref()?;
    let port = player.terminal_in_reach(city)?;
    let locked = city
        .buildings
        .iter()
        .any(|b| b.footprint.grow(1).contains(port) && st.net.alert(b.id, now));
    locked.then(|| "port locked down".to_string())
}

/// The node on or beside a tile in a construct, and the standing guard beside it.
pub fn targets_near(st: &AppState, b: BuildingId, pos: citygen::Pos) -> Vec<(HackTarget, String)> {
    let mut out = vec![(HackTarget::Deck, "yourself".to_string())];
    let (Some(city), Some(k)) = (st.city.as_ref(), st.constructs.get(b)) else {
        return out;
    };
    for p in std::iter::once(pos).chain(pos.neighbors8()) {
        if let Some(id) = k.node_at(p) {
            out.push((
                HackTarget::Node(id),
                city.net.node(id).kind.label().to_string(),
            ));
        }
        if let Some(i) = k.guard_at(p) {
            out.push((
                HackTarget::Guard {
                    building: b,
                    index: i,
                },
                format!("ICE · {}", k.guards[i].grade.label()),
            ));
        }
    }
    out
}

/// The HERE section's trace lines inside a construct.
pub fn trace_lines(net: &NetState, b: BuildingId, now: u64) -> Vec<String> {
    let Some(t) = net.traces.get(&b) else {
        return vec!["TRACE ▱▱▱▱▱▱▱▱▱▱ 0".to_string()];
    };
    let filled = (t.heat / 10).min(10) as usize;
    let mut out = vec![format!(
        "TRACE {}{} {}",
        "▰".repeat(filled),
        "▱".repeat(10 - filled),
        t.heat
    )];
    if let Some(until) = t.alert_until.filter(|u| now < *u) {
        out.push(format!("ALERT · {} min", until - now));
    }
    if net.shield > 0 {
        out.push(format!("SHIELD ×{}", net.shield));
    }
    if net.mask > 0 {
        out.push(format!("MASK {}", net.mask));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{BuildingId, IceGrade, NodeId, NodeKind};
    use game::{Op, Target};

    fn b(i: u32) -> BuildingId {
        BuildingId(i)
    }

    #[test]
    fn queries_read_the_landed_hacks_and_the_alert() {
        let mut net = NetState::default();
        let lock = HackTarget::Node(NodeId(7));
        net.land(b(1), lock, Target::Node(NodeKind::DoorLock), Op::Crack, 10);
        assert!(net.unlocked(b(1), 10) && net.unlocked(b(1), 69));
        assert!(!net.unlocked(b(1), 70), "an hour, then it relocks");
        assert!(!net.unlocked(b(2), 10));
        assert!(net.lifts_answer(b(1), 10));
        net.land(
            b(1),
            HackTarget::Node(NodeId(8)),
            Target::Node(NodeKind::Camera),
            Op::Blind,
            10,
        );
        assert!(net.blinded(NodeId(8), 20) && !net.blinded(NodeId(7), 20));
        net.land(
            b(1),
            HackTarget::Node(NodeId(9)),
            Target::Node(NodeKind::DataVault),
            Op::Copy,
            10,
        );
        assert!(net.copied(NodeId(9), 20));
        // The alarm: the lock relocks, the lifts stop, a spoof after the alarm frees them.
        assert_eq!(
            net.trace(b(1)).add(game::ALARM_HEAT, 20),
            game::TraceEvent::Alarm
        );
        net.alarm(b(1), 20);
        assert!(net.alert(b(1), 20) && !net.unlocked(b(1), 20) && !net.lifts_answer(b(1), 20));
        assert!(net.hacks.is_empty(), "every hack expired with the alarm");
        net.land(
            b(1),
            HackTarget::Node(NodeId(10)),
            Target::Node(NodeKind::LiftCtrl),
            Op::Spoof,
            25,
        );
        assert!(net.lifts_answer(b(1), 25));
        // A backdoor is forever and survives the alarm.
        net.land(
            b(1),
            HackTarget::Node(NodeId(11)),
            Target::Node(NodeKind::Server),
            Op::Backdoor,
            30,
        );
        assert!(net.backdoors.contains(&b(1)));
        net.alarm(b(1), 30);
        assert!(net.backdoors.contains(&b(1)));
        // A hack landed during the alert ends with the alert, a backdoor does not.
        let mut net = NetState::default();
        net.trace(b(1)).add(game::ALARM_HEAT, 20);
        net.alarm(b(1), 20);
        net.land(
            b(1),
            HackTarget::Guard {
                building: b(1),
                index: 1,
            },
            Target::Guard(IceGrade::Grey),
            Op::Crack,
            25,
        );
        assert_eq!(net.hacks[0].until, Some(20 + game::ALERT_MINUTES));
        let mut constructs = crate::constructs::Constructs::default();
        net.minute(20 + game::ALERT_MINUTES, None, &mut constructs);
        assert!(net.hacks.is_empty(), "{:?}", net.hacks);
        // Cooling.
        assert_eq!(net.cooling(0), None);
        net.cooling_until = Some(15);
        assert_eq!(net.cooling(5), Some(10));
        assert_eq!(net.cooling(15), None);
        let _ = IceGrade::White;
    }
}
