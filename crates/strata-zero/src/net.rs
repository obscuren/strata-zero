//! The Net's state this session: every construct's trace, the hacks that landed,
//! the backdoors, the deck's cooling after a dump, and the program in flight.

use crate::app::{AppState, FxInstance};
use crate::constructs::Constructs;
use crate::render::map::MapRef;
use citygen::rng::hash64;
use citygen::{BuildingId, NodeId, NodeKind, Place};
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

#[derive(Default)]
pub struct NetState {
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
            .filter(|h| h.op == Op::Crack)
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
    let security = st.constructs.get(b).map(|k| k.security).unwrap_or(0);
    if game::chance(
        power(&st.character, &st.content, &spec, kind),
        difficulty(security),
    ) == 0
    {
        return Err(format!("{} is out of your reach here", item.name));
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
    st.net.run = Some(Run {
        program: program.clone(),
        target,
        done: 0,
        work: spec.work,
    });
    Ok(format!("{} starts · {} turns", spec.op.label(), spec.work))
}

fn node_of(target: HackTarget) -> Option<NodeId> {
    match target {
        HackTarget::Node(id) => Some(id),
        HackTarget::Guard { .. } => None,
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
    };
    let roll = st.net.roll(st, seed);
    let ok = succeeds(roll, pw, df);
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
        (Op::Crack, Target::Node(NodeKind::DoorLock)) => {
            format!("lock cracked · {name}'s doors open for an hour")
        }
        (Op::Spoof, Target::Node(NodeKind::DoorLock)) => {
            format!("lock spoofed · {name}'s doors open for an hour")
        }
        (Op::Spoof, Target::Node(NodeKind::LiftCtrl)) => {
            "lift control spoofed · the lifts answer you".to_string()
        }
        (Op::Blind, _) => "camera blinded".to_string(),
        (Op::Copy, Target::Node(NodeKind::DataVault)) => {
            st.character.give(ItemId::new("vault_data"), 1);
            "vault copied · a vault dump in the pack".to_string()
        }
        (Op::Copy, _) => {
            st.character.give(ItemId::new("paydata"), 1);
            "server copied · a paydata shard in the pack".to_string()
        }
        (Op::Backdoor, _) => format!("backdoor planted · {name}'s gate is yours"),
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
    format!("ALARM · {} locks down", building_name(st, b))
}

/// Dumped: back to the body hard, the focus gone, the deck cooling.
pub fn dump(st: &mut AppState, why: &str) {
    let bonus = st.character.sight_bonus(&st.content);
    let now = st.net.now(st);
    st.net.run = None;
    st.net.cooling_until = Some(now + COOLING_MINUTES);
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

/// A turn ends in a construct: awake ICE beside the player dumps them.
pub fn turn_ends(st: &mut AppState) {
    let Some(player) = st.player.as_ref() else {
        return;
    };
    let Place::Construct(b) = player.place else {
        return;
    };
    let now = st.net.now(st);
    if !st.net.alert(b, now) {
        return;
    }
    let pos = player.pos;
    let caught = st.constructs.get(b).is_some_and(|k| {
        k.guards
            .iter()
            .any(|g| k.guard_at(g.pos).is_some() && g.pos.chebyshev(pos) <= 1)
    });
    if caught {
        dump(st, "ICE caught you");
    }
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
    let mut out = Vec::new();
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
