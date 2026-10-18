//! The Net's state this session: every construct's trace, the hacks that landed,
//! the deck's cooling after a dump, and the program in flight.

use crate::app::{AppState, FxInstance};
use crate::constructs::Constructs;
use crate::render::map::MapRef;
use citygen::pathing::{astar, bresenham};
use citygen::rng::hash64;
use citygen::{
    BuildingId, City, Construct, IceGrade, NodeId, NodeKind, Place, Pos, StratumKind, Terrain,
};
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
/// How far from a gate the footer still reads that construct's trace on the grid.
pub const TRACE_REACH: i32 = 12;
/// Ticks between beats in a calm construct: the run rolls, asleep ICE patrols.
pub const BEAT_TICKS: u64 = 45;
/// Tiles you may walk in a turn.
pub const BASE_MOVE: u8 = 4;
/// Actions you may take in a turn.
pub const BASE_AP: u8 = 2;
/// Ticks between one piece's actions in its turn, so you watch it come.
pub const PACE_TICKS: u64 = 8;
/// Ticks between your own steps on an aimed walk.
pub const WALK_TICKS: u64 = 4;
/// Ticks a bolt takes from a roll or a hit to its target.
pub const ROLL_TICKS: u64 = 12;
/// How far a program reaches a piece of ICE in sight.
pub const ATTACK_RANGE: i32 = 6;
/// Ticks a slip's or a shield's mark sits on its tile.
pub const MARK_TICKS: u64 = 20;
/// A program's roll, cyan; an attack's, white; an ICE hit, magenta; black ICE, red.
pub const BOLT_ROLL: (u8, u8, u8) = (0, 229, 255);
pub const BOLT_ATTACK: (u8, u8, u8) = (240, 240, 240);
pub const BOLT_HIT: (u8, u8, u8) = (255, 0, 200);
pub const BOLT_BLACK: (u8, u8, u8) = (255, 40, 40);
/// A slip marks its target magenta; a shield that absorbs marks you cyan.
pub const MARK_SLIP: (u8, u8, u8) = (255, 0, 200);
pub const MARK_SHIELD: (u8, u8, u8) = (0, 229, 255);

/// Who acts in an engagement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    You,
    Ice(usize),
}

/// Your turn in two halves: you move (or stay), then you act.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Move,
    Act,
}

/// A construct on turns: the order rolled once, whose turn it is, and the pools
/// left to the actor acting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engagement {
    pub building: BuildingId,
    pub round: u32,
    pub order: Vec<Actor>,
    pub turn: usize,
    pub moves: u8,
    pub ap: u8,
    /// Where your turn stands: moving first, acting after.
    pub phase: Phase,
    /// Switched on by `e`; it does not end when the ICE calms.
    pub manual: bool,
    /// The tick the current ICE action started, for the pace.
    pub acting_since: u64,
}

impl Engagement {
    pub fn actor(&self) -> Actor {
        self.order.get(self.turn).copied().unwrap_or(Actor::You)
    }
}

/// What a piece did with one action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Moved(Pos),
    Hit,
    Nothing,
}
/// Minutes the deck cools after the failsafe pulls out a runner in the deep end.
pub const DEEP_END_COOLING: u64 = 10;
/// Heat a ping makes inside a construct.
pub const PING_NOISE: u8 = 2;
/// Heat an arc trace makes inside a construct.
pub const TRACE_NOISE: u8 = 5;
/// Turns a piece stands stunned after an arc trace.
pub const STUN_TURNS: u8 = 2;

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
    /// Turns left standing stunned: it neither sees, moves nor hits.
    pub stunned: u8,
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
    /// Constructs whose trace has already been routed away once this session.
    pub null_routed: BTreeSet<BuildingId>,
    pub hacks: Vec<Hack>,
    /// The game minute the dumped deck jacks in again.
    pub cooling_until: Option<u64>,
    /// The program in flight.
    pub run: Option<Run>,
    /// The tick of the last beat in a calm construct.
    pub beat: u64,
    /// The construct on turns, if any.
    pub engaged: Option<Engagement>,
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
            null_routed: BTreeSet::new(),
            hacks: Vec::new(),
            cooling_until: None,
            run: None,
            beat: 0,
            engaged: None,
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

    /// The construct whose gate is nearest within `TRACE_REACH`, with its heat; a
    /// construct never entered reads 0 and no state is made for it.
    pub fn nearest_trace(&self, city: &City, pos: Pos) -> Option<(BuildingId, u8)> {
        city.buildings
            .iter()
            .filter(|b| {
                let r = b.footprint;
                pos.x >= r.x - TRACE_REACH
                    && pos.x < r.right() + TRACE_REACH
                    && pos.y >= r.y - TRACE_REACH
                    && pos.y < r.bottom() + TRACE_REACH
            })
            .filter_map(|b| {
                let gate = crate::constructs::city_gate(city, b.id)?;
                let d = gate.chebyshev(pos);
                (d <= TRACE_REACH).then_some((d, b.id))
            })
            .min_by_key(|(d, _)| *d)
            .map(|(_, b)| (b, self.traces.get(&b).map_or(0, |t| t.heat)))
    }

    /// Heat on a construct's trace. With the null route, the first dump in a
    /// construct is turned away: the heat falls to the alarm line and the event is
    /// the alarm if it had not tripped yet. Returns the event and whether it routed.
    pub fn add_heat(
        &mut self,
        b: BuildingId,
        noise: u8,
        now: u64,
        null_route: bool,
    ) -> (TraceEvent, bool) {
        let was_alert = self.trace(b).alert(now);
        let event = self.trace(b).add(noise, now);
        if event == TraceEvent::Dump && null_route && !self.null_routed.contains(&b) {
            self.trace(b).heat = game::ALARM_HEAT;
            self.null_routed.insert(b);
            let event = if was_alert {
                TraceEvent::None
            } else {
                TraceEvent::Alarm
            };
            return (event, true);
        }
        (event, false)
    }

    fn live(&self, now: u64) -> impl Iterator<Item = &Hack> {
        self.hacks
            .iter()
            .filter(move |h| h.until.is_none_or(|u| now < u))
    }

    /// Records a landed hack; a backdoor's hack is logged without an end.
    pub fn land(&mut self, b: BuildingId, target: HackTarget, kind: Target, op: Op, now: u64) {
        // A hack lasts an hour, or until the building's alert ends, whichever is
        // sooner; a broken piece stays down the whole hour whatever the alert does.
        let alert_end = self
            .traces
            .get(&b)
            .and_then(|t| t.alert_until)
            .filter(|u| now < *u && op != Op::Attack);
        let until = (op != Op::Backdoor)
            .then(|| alert_end.map_or(now + HACK_MINUTES, |u| u.min(now + HACK_MINUTES)));
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
    /// The alarm resets the building's locks, cameras and lifts; a backdoor holds,
    /// and a broken piece of ICE stays broken for its hour.
    pub fn alarm(&mut self, b: BuildingId, now: u64) {
        let _ = now;
        self.hacks
            .retain(|h| h.building != b || matches!(h.op, Op::Backdoor | Op::Attack));
    }

    /// Sets every guard of a generated construct standing or lifted by the hacks.
    pub fn sync_guards(
        &self,
        constructs: &mut Constructs,
        b: BuildingId,
        now: u64,
        roots: &BTreeSet<BuildingId>,
    ) {
        let backdoor = roots.contains(&b);
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
    pub fn minute(
        &mut self,
        now: u64,
        inside: Option<BuildingId>,
        constructs: &mut Constructs,
        roots: &BTreeSet<BuildingId>,
    ) {
        if now <= self.last_minute {
            return;
        }
        let from = self.last_minute + 1;
        self.last_minute = now;
        let buildings: Vec<BuildingId> = self.traces.keys().copied().collect();
        for m in from..=now {
            for b in &buildings {
                let backdoor = roots.contains(b);
                let t = self.traces.get_mut(b).expect("traced");
                let ending = t.alert_until.is_some_and(|u| m >= u);
                if ending || inside != Some(*b) {
                    t.cool(m, backdoor);
                }
            }
        }
        self.hacks.retain(|h| h.until.is_none_or(|u| now < u));
        for b in buildings {
            self.sync_guards(constructs, b, now, roots);
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
                stunned: 0,
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

pub(crate) fn building_name(st: &AppState, b: BuildingId) -> String {
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

/// Why a Backdoor program may not be planted here, in the shell's words.
pub fn plant_net_refusal(st: &AppState, b: BuildingId) -> Option<String> {
    let now = st.net.now(st);
    st.backdoors.can_plant(b, now).err().map(|r| r.words())
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
    if spec.op == Op::Backdoor
        && let Some(why) = plant_net_refusal(st, b)
    {
        return Err(why);
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
    // The same program at the same target is the run already armed: it goes on
    // where it stands, nothing is dropped.
    if let Some(run) = st.net.run.as_ref()
        && run.program == program
        && run.target == target
    {
        return Ok(format!(
            "{} goes on · {}/{}",
            spec.op.label(),
            run.done,
            run.work
        ));
    }
    let dropped = st.net.run.replace(Run {
        program: program.clone(),
        target,
        done: 0,
        work: spec.work,
    });
    // Pointing a program at a piece is picking the fight.
    if let HackTarget::Guard { .. } = target {
        engage(st, b, false);
    }
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
    // The picture of the roll: a bolt to the target, a mark where it slipped.
    if let Some((me, at)) = st
        .player
        .as_ref()
        .map(|p| p.pos)
        .zip(target_pos(st, b, run.target))
    {
        if at == me {
            mark(st, at, BOLT_ROLL, ROLL_TICKS);
        } else {
            let colour = if spec.op == Op::Attack {
                BOLT_ATTACK
            } else {
                BOLT_ROLL
            };
            bolt(st, me, at, colour, ROLL_TICKS);
        }
        if !ok {
            mark(st, at, MARK_SLIP, MARK_TICKS);
        }
    }
    // Deep dive: a vault gives up its contents quietly; a slip is still loud.
    let quiet = ok
        && spec.op == Op::Copy
        && kind == Target::Node(NodeKind::DataVault)
        && st.character.flags(&st.content).contains("deep_dive");
    let noise = if quiet {
        0
    } else if ok {
        spec.noise
    } else {
        spec.noise + 2
    };
    if noise_on(st, b, noise) {
        return Some("DUMPED".to_string());
    }
    let heat = st.net.traces[&b].heat;
    let label = spec.op.label();
    // An attack is a hit a turn until the piece breaks; the run stays up.
    if spec.op == Op::Attack {
        let HackTarget::Guard { index, .. } = run.target else {
            st.net.run = None;
            return None;
        };
        // The piece must still stand in reach: beside you or in sight at range.
        if !piece_in_reach(st, b, index) {
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
                let roots = st.backdoors.roots();
                st.net.sync_guards(&mut st.constructs, b, now, &roots);
                kick(st, 6);
                let line = format!("{} ICE broken", grade.label());
                st.say(line.clone());
                alarm_clears(st, b);
                return Some(line);
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
            let line = land(st, b, run.target, kind, spec.op, spec.rating, now);
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
    rating: u8,
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
            let base = "server copied · a `Paydata shard` in the pack";
            match crate::credentials::copy_codes(st, b, now) {
                Some(n) if n > 0 => format!("{base} · `{name}`'s door codes are in the journal"),
                Some(_) => format!("{base} · the door codes were already yours"),
                None => base.to_string(),
            }
        }
        (Op::Backdoor, _) => {
            let seed = st.city.as_ref().map_or(0, |c| c.seed);
            st.backdoors.plant_net(b, rating, seed, now);
            format!(
                "backdoor planted · `{name}`'s gate is yours · {} of {}",
                st.backdoors.slots_used(),
                crate::backdoors::SLOTS
            )
        }
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
    let roots = st.backdoors.roots();
    st.net.sync_guards(&mut st.constructs, b, now, &roots);
    line
}

/// Where a hack target sits: a node's tile, a piece's tile, your own for the deck.
pub fn target_pos(st: &AppState, b: BuildingId, target: HackTarget) -> Option<Pos> {
    match target {
        HackTarget::Node(id) => st
            .constructs
            .get(b)?
            .nodes
            .iter()
            .find(|(n, _)| *n == id)
            .map(|(_, p)| *p),
        HackTarget::Guard { index, .. } => st.constructs.get(b)?.guards.get(index).map(|g| g.pos),
        HackTarget::Deck => st.player.as_ref().map(|p| p.pos),
    }
}

/// A bolt from one tile to another on the player's map.
pub fn bolt(st: &mut AppState, from: Pos, to: Pos, colour: (u8, u8, u8), ticks: u64) {
    let Some(place) = st.player.as_ref().map(|p| p.place) else {
        return;
    };
    st.fx.push(FxInstance {
        fx: Fx::Bolt { colour, ticks },
        place,
        origin: from,
        radius: 0,
        started: st.tick,
        revealed: Vec::new(),
        targets: vec![to],
    });
}

/// A mark on a tile of the player's map.
pub fn mark(st: &mut AppState, at: Pos, colour: (u8, u8, u8), ticks: u64) {
    let Some(place) = st.player.as_ref().map(|p| p.place) else {
        return;
    };
    st.fx.push(FxInstance {
        fx: Fx::Mark { colour, ticks },
        place,
        origin: at,
        radius: 0,
        started: st.tick,
        revealed: Vec::new(),
        targets: vec![at],
    });
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

/// The last standing piece broken: the alarm has nobody left to answer it, so the
/// alert ends and the trace falls back under the line. Escaping clears nothing.
pub fn alarm_clears(st: &mut AppState, b: BuildingId) {
    let standing = st
        .constructs
        .get(b)
        .is_some_and(|k| k.guards.iter().any(|g| k.guard_at(g.pos).is_some()));
    if standing {
        return;
    }
    let Some(t) = st.net.traces.get_mut(&b) else {
        return;
    };
    if t.alert_until.is_none() {
        return;
    }
    t.alert_until = None;
    t.heat = t.heat.min(game::ALARM_HEAT / 2);
    st.say("ALARM cleared · no ICE stands");
}

/// The alarm: the building locks down, its hacks expire, its ICE wakes.
pub fn raise_alarm(st: &mut AppState, b: BuildingId) -> String {
    let now = st.net.now(st);
    st.net.alarm(b, now);
    let roots = st.backdoors.roots();
    st.net.sync_guards(&mut st.constructs, b, now, &roots);
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
    // Inside with ICE standing, the board goes to turns; with every piece broken
    // there is nobody to fight and the beat runs on.
    if st
        .player
        .as_ref()
        .is_some_and(|p| p.place == Place::Construct(b))
        && any_awake(st, b)
    {
        engage(st, b, false);
        seat_standing(st, b);
    }
    format!("ALARM · `{}` locks down", building_name(st, b))
}

/// Heat on the construct's trace with everything that follows: the alarm raised,
/// the null route taken once, the dump. Returns whether the player was dumped.
pub(crate) fn noise_on(st: &mut AppState, b: BuildingId, noise: u8) -> bool {
    let now = st.net.now(st);
    let was_alert = st.net.alert(b, now);
    let null_route = st.character.flags(&st.content).contains("null_route");
    let (event, routed) = st.net.add_heat(b, noise, now, null_route);
    if routed {
        st.warn("null route · the trace loses you");
    }
    match event {
        TraceEvent::Dump => {
            if !was_alert {
                let line = raise_alarm(st, b);
                st.warn(line);
            }
            dump(st, "the trace found you");
            true
        }
        TraceEvent::Alarm => {
            let line = raise_alarm(st, b);
            st.warn(line);
            false
        }
        TraceEvent::None => false,
    }
}

/// Dumped: back to the body hard, the focus gone, the deck cooling.
pub fn dump(st: &mut AppState, why: &str) {
    dump_for(st, why, COOLING_MINUTES);
}

/// Dumped, with the deck cooling for a given stretch.
pub fn dump_for(st: &mut AppState, why: &str, cooling: u64) {
    st.net.engaged = None;
    let now = st.net.now(st);
    st.net.run = None;
    st.net.cooling_until = Some(now + cooling);
    st.character.focus = 0;
    let _ = jack_out(st);
    kick(st, 20);
    st.warn(format!("DUMPED · {why}"));
    st.repaint = true;
}

/// Back to the body, wherever it waits, seeing around it again.
pub fn jack_out(st: &mut AppState) -> Result<(), &'static str> {
    let bonus = st.sight_bonus();
    let city = st.city.as_ref().ok_or("no body in the city")?;
    let player = st.player.as_mut().ok_or("no body in the city")?;
    player.jack_out(city, bonus)?;
    if let Some(map) = MapRef::for_place(city, &st.interiors, &st.constructs, player.place) {
        player.update_fov(&map, bonus);
    }
    // A run in flight stays in the Net.
    if st.net.run.take().is_some() {
        st.say("run aborted");
    }
    Ok(())
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

/// The tiles you would walk from `from` to `to`, after your own: the shortest way
/// over the floor with standing pieces in the way. `None` when nothing reaches it.
pub fn path_to(k: &Construct, from: Pos, to: Pos) -> Option<Vec<Pos>> {
    if from == to {
        return Some(Vec::new());
    }
    let path = astar(k.tiles.size(), from, to, |_, step| {
        let t = k.tiles.get(step)?;
        (t.is_walkable() && k.guard_at(step).is_none()).then_some(1)
    })?;
    Some(path.into_iter().filter(|p| *p != from).collect())
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
    mask_wears(st, b);
    let Some(k) = st.constructs.get(b) else {
        return;
    };
    let n = k.guards.len();
    st.net.ice_of(k);
    for i in 0..n {
        if !ice_look(st, b, i) {
            continue;
        }
        ice_step(st, b, i, true, true);
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

/// A held mask is a turn's noise, and it wears off.
fn mask_wears(st: &mut AppState, b: BuildingId) {
    let masked = st.net.mask > 0;
    if masked && std::mem::take(&mut st.net.mask_fresh) {
        // Raised this turn: it starts wearing next turn.
    } else if masked {
        st.net.mask -= 1;
        if noise_on(st, b, 1) {
            return;
        }
        if st.net.mask == 0 {
            st.say("the mask is gone");
        }
    }
}

/// A piece looks at the start of its turn: integrity back if it stood again, the
/// stun counting down, and sight waking it. Returns whether it acts this turn.
pub fn ice_look(st: &mut AppState, b: BuildingId, i: usize) -> bool {
    let Some(k) = st.constructs.get(b) else {
        return false;
    };
    let Some(g) = k.guards.get(i).cloned() else {
        return false;
    };
    if k.guard_at(g.pos).is_none() {
        return false;
    }
    let Some(pos) = st.player.as_ref().map(|p| p.pos) else {
        return false;
    };
    let masked = st.net.mask > 0;
    st.net.ice_of(k);
    let Some(ice) = st.net.ice.get_mut(&b).and_then(|v| v.get_mut(i)) else {
        return false;
    };
    if ice.integrity == 0 {
        ice.integrity = g.grade.integrity();
    }
    // Stunned: the turn passes it by.
    if ice.stunned > 0 {
        ice.stunned -= 1;
        return false;
    }
    if sees(k, g.pos, g.grade, pos, masked) {
        ice.mode = IceMode::Awake { seen: pos, lost: 0 };
    }
    true
}

/// One action of a standing piece: a patrol step asleep, a step toward where it saw
/// you awake, a hit when beside you. `can_move` and `can_hit` are the turn's pools.
pub fn ice_step(st: &mut AppState, b: BuildingId, i: usize, can_move: bool, can_hit: bool) -> Step {
    let Some(k) = st.constructs.get(b) else {
        return Step::Nothing;
    };
    let Some(g) = k.guards.get(i).cloned() else {
        return Step::Nothing;
    };
    if k.guard_at(g.pos).is_none() {
        return Step::Nothing;
    }
    let Some(pos) = st.player.as_ref().map(|p| p.pos) else {
        return Step::Nothing;
    };
    let Some(mut ice) = st.net.ice.get(&b).and_then(|v| v.get(i)).cloned() else {
        return Step::Nothing;
    };
    let mut moved = None;
    let mut hits = false;
    match ice.mode {
        IceMode::Asleep => {
            if can_move && g.beat.len() >= 2 {
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
                hits = can_hit;
            } else if !can_move {
                // Out of steps: it waits where it stands.
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
        return Step::Hit;
    }
    match moved {
        Some(to) => Step::Moved(to),
        None => Step::Nothing,
    }
}

/// Any standing piece of the construct awake.
pub fn any_awake(st: &AppState, b: BuildingId) -> bool {
    let (Some(k), Some(states)) = (st.constructs.get(b), st.net.ice.get(&b)) else {
        return false;
    };
    k.guards
        .iter()
        .zip(states)
        .any(|(g, i)| k.guard_at(g.pos).is_some() && matches!(i.mode, IceMode::Awake { .. }))
}

/// A piece's turn: the tiles it may walk and the hits it may land, by grade.
pub fn budgets(grade: IceGrade) -> (u8, u8) {
    match grade {
        IceGrade::White => (0, 1),
        IceGrade::Grey => (2, 1),
        IceGrade::Black => (3, 2),
    }
}

/// The order of an engagement: you by your Net, each standing piece by its grade,
/// higher first, ties to you, then by index.
pub fn initiative(st: &AppState, b: BuildingId) -> Vec<Actor> {
    let mut scored: Vec<(i32, u8, Actor)> = vec![(
        st.character.stat(&st.content, game::Stat::Net) as i32,
        0,
        Actor::You,
    )];
    if let Some(k) = st.constructs.get(b) {
        for (i, g) in k.guards.iter().enumerate() {
            if k.guard_at(g.pos).is_none() {
                continue;
            }
            let score = match g.grade {
                IceGrade::Black => 3,
                IceGrade::Grey => 2,
                IceGrade::White => 1,
            };
            scored.push((score, 1 + i as u8, Actor::Ice(i)));
        }
    }
    scored.sort_by(|a, c| c.0.cmp(&a.0).then(a.1.cmp(&c.1)));
    scored.into_iter().map(|(_, _, a)| a).collect()
}

fn pools_for(st: &AppState, b: BuildingId, actor: Actor) -> (u8, u8) {
    match actor {
        Actor::You => (BASE_MOVE, BASE_AP),
        Actor::Ice(i) => st
            .constructs
            .get(b)
            .and_then(|k| k.guards.get(i))
            .map(|g| budgets(g.grade))
            .unwrap_or((0, 0)),
    }
}

/// The construct goes to turns: the order is rolled, you or the fastest piece acts
/// first. Nothing happens if it already is.
pub fn engage(st: &mut AppState, b: BuildingId, manual: bool) {
    if st.net.engaged.is_some() {
        return;
    }
    if let Some(k) = st.constructs.get(b) {
        st.net.ice_of(k);
    }
    let order = initiative(st, b);
    let (moves, ap) = pools_for(st, b, order[0]);
    st.net.engaged = Some(Engagement {
        building: b,
        round: 1,
        order,
        turn: 0,
        moves,
        ap,
        phase: Phase::Move,
        manual,
        acting_since: st.clock_tick(),
    });
    st.warn(if manual {
        "engaged"
    } else {
        "ENGAGED · the ICE is on you"
    });
    start_ice_turn(st);
}

/// Every standing piece of the construct that the order does not hold yet takes
/// its place at the end: a piece that stood again mid-fight.
pub fn seat_standing(st: &mut AppState, b: BuildingId) {
    let standing: Vec<usize> = match st.constructs.get(b) {
        Some(k) => k
            .guards
            .iter()
            .enumerate()
            .filter(|(_, g)| k.guard_at(g.pos).is_some())
            .map(|(i, _)| i)
            .collect(),
        None => return,
    };
    if let Some(e) = st.net.engaged.as_mut()
        && e.building == b
    {
        for i in standing {
            if !e.order.contains(&Actor::Ice(i)) {
                e.order.push(Actor::Ice(i));
            }
        }
    }
}

/// Back to the beat.
pub fn disengage(st: &mut AppState) {
    if st.net.engaged.take().is_some() {
        st.net.beat = st.clock_tick();
        st.say("calm");
    }
}

/// The half of your turn you are in, on your turn.
pub fn phase(st: &AppState) -> Option<Phase> {
    let e = st.net.engaged.as_ref()?;
    (e.actor() == Actor::You).then_some(e.phase)
}

/// You stay where you are: the move half is over, the act half opens. True when
/// that happened.
pub fn stay(st: &mut AppState) -> bool {
    if let Some(e) = st.net.engaged.as_mut()
        && e.actor() == Actor::You
        && e.phase == Phase::Move
    {
        e.phase = Phase::Act;
        return true;
    }
    false
}

/// How far an aim reaches: the moves left on turns, a turn's worth in calm.
pub fn aim_cap(st: &AppState) -> usize {
    match st.net.engaged.as_ref() {
        Some(e) if e.actor() == Actor::You => e.moves as usize,
        _ => BASE_MOVE as usize,
    }
}

pub fn your_turn(st: &AppState) -> bool {
    st.net
        .engaged
        .as_ref()
        .is_none_or(|e| e.actor() == Actor::You)
}

/// One action point of yours; free in calm.
pub fn spend_ap(st: &mut AppState) -> Result<(), String> {
    let Some(e) = st.net.engaged.as_mut() else {
        return Ok(());
    };
    if e.actor() != Actor::You {
        return Err("not your turn".to_string());
    }
    if e.phase == Phase::Move {
        return Err("move first · space to stay".to_string());
    }
    if e.ap == 0 {
        return Err("no AP left".to_string());
    }
    e.ap -= 1;
    Ok(())
}

/// One tile of yours; free in calm.
pub fn spend_move(st: &mut AppState) -> Result<(), String> {
    let Some(e) = st.net.engaged.as_mut() else {
        return Ok(());
    };
    if e.actor() != Actor::You {
        return Err("not your turn".to_string());
    }
    if e.phase == Phase::Act {
        return Err("moves are done".to_string());
    }
    if e.moves == 0 {
        return Err("no moves left".to_string());
    }
    e.moves -= 1;
    Ok(())
}

/// The next actor; a new round when the order wraps, which ends the engagement when
/// nothing is awake and nobody asked for turns.
pub fn end_turn(st: &mut AppState) {
    let Some((b, len, manual, turn, round)) = st
        .net
        .engaged
        .as_ref()
        .map(|e| (e.building, e.order.len(), e.manual, e.turn, e.round))
    else {
        return;
    };
    let mut turn = turn + 1;
    let mut round = round;
    if turn >= len {
        turn = 0;
        round += 1;
        // A round passed: a held mask wears (and may dump you through the trace),
        // and any piece that stood again since takes its seat.
        seat_standing(st, b);
        mask_wears(st, b);
        if st.net.engaged.is_none() {
            return;
        }
        if !manual && !any_awake(st, b) {
            disengage(st);
            return;
        }
    }
    let Some(actor) = st
        .net
        .engaged
        .as_ref()
        .and_then(|e| e.order.get(turn).copied())
    else {
        return;
    };
    let (moves, ap) = pools_for(st, b, actor);
    let now = st.clock_tick();
    if let Some(e) = st.net.engaged.as_mut() {
        e.turn = turn;
        e.round = round;
        e.moves = moves;
        e.ap = ap;
        e.phase = Phase::Move;
        e.acting_since = now;
    }
    start_ice_turn(st);
}

/// A piece's turn begins: it looks; a lifted, broken or stunned piece passes.
fn start_ice_turn(st: &mut AppState) {
    let Some(e) = st.net.engaged.as_ref() else {
        return;
    };
    let Actor::Ice(i) = e.actor() else {
        return;
    };
    let b = e.building;
    if !ice_look(st, b, i) {
        end_turn(st);
    }
}

/// From the frame loop while a piece acts: one action every `PACE_TICKS`.
pub fn ice_pace(st: &mut AppState) {
    let Some(e) = st.net.engaged.as_ref() else {
        return;
    };
    let Actor::Ice(i) = e.actor() else {
        return;
    };
    if st.clock_tick() < e.acting_since + PACE_TICKS {
        return;
    }
    let b = e.building;
    let (moves, ap) = (e.moves, e.ap);
    let step = ice_step(st, b, i, moves > 0, ap > 0);
    let now = st.clock_tick();
    // The hit may have dumped or killed: the engagement is gone then.
    let Some(e) = st.net.engaged.as_mut() else {
        return;
    };
    e.acting_since = now;
    match step {
        Step::Moved(_) => {
            e.moves = e.moves.saturating_sub(1);
            // A step may bring it into sight of you; it looks again.
            ice_look(st, b, i);
        }
        Step::Hit => e.ap = e.ap.saturating_sub(1),
        Step::Nothing => end_turn(st),
    }
    if let Some(e) = st.net.engaged.as_ref()
        && e.actor() == Actor::Ice(i)
        && e.moves == 0
        && e.ap == 0
    {
        end_turn(st);
    }
}

/// The footer's turn line: `TURN 3  YOU › grey ICE › black ICE`, the actor acting first.
pub fn turn_readout(st: &AppState) -> Option<String> {
    let e = st.net.engaged.as_ref()?;
    let k = st.constructs.get(e.building)?;
    let name = |a: Actor| match a {
        Actor::You => "YOU".to_string(),
        Actor::Ice(i) => k
            .guards
            .get(i)
            .map(|g| format!("{} ICE", g.grade.label()))
            .unwrap_or_else(|| "ICE".to_string()),
    };
    let mut parts = Vec::new();
    for n in 0..e.order.len() {
        let a = e.order[(e.turn + n) % e.order.len()];
        let mut s = name(a);
        if n == 0
            && let Actor::Ice(_) = a
        {
            if e.moves > 0 {
                s.push_str(&format!(" moves {}", e.moves));
            } else if e.ap > 0 {
                s.push_str(" hits");
            }
        }
        parts.push(s);
    }
    Some(format!("TURN {}  {}", e.round, parts.join(" › ")))
}

/// A standing piece beside the player hits them: white adds heat, grey drains
/// focus and dumps at zero, black takes HP through the jack. A shield charge soaks
/// a hit; the deck's rating comes off grey and black hits.
fn hit(st: &mut AppState, b: BuildingId, i: usize) {
    let Some((grade, security, from)) = st
        .constructs
        .get(b)
        .map(|k| (k.guards[i].grade, k.security, k.guards[i].pos))
    else {
        return;
    };
    if let Some(me) = st.player.as_ref().map(|p| p.pos) {
        let colour = if grade == IceGrade::Black {
            BOLT_BLACK
        } else {
            BOLT_HIT
        };
        bolt(st, from, me, colour, ROLL_TICKS);
        if st.net.shield > 0 {
            mark(st, me, MARK_SHIELD, MARK_TICKS);
        }
    }
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
    match grade {
        IceGrade::White => {
            let heat = st
                .net
                .traces
                .get(&b)
                .map_or(0, |t| t.heat)
                .saturating_add(5);
            st.warn(format!(
                "white ICE hits · heat {}",
                heat.min(game::DUMP_HEAT)
            ));
            noise_on(st, b, 5);
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
            // The deep end: half the damage through the jack, a failsafe that waits
            // for an eighth, and a short cooling after.
            let deep = st.character.flags(&st.content).contains("deep_end");
            let mut damage = (6 + security.saturating_sub(230) as u32 / 5)
                .saturating_sub(armour)
                .max(1);
            if deep {
                damage = (damage / 2).max(1);
            }
            st.character.hp = st.character.hp.saturating_sub(damage);
            let hp = st.character.hp;
            st.warn(format!("black ICE hits · hp {hp}"));
            let hp_max = st.character.hp_max(&st.content);
            let line = if deep { hp_max / 8 } else { hp_max / 4 };
            let cooling = if deep {
                DEEP_END_COOLING
            } else {
                FAILSAFE_COOLING
            };
            if st.net.failsafe {
                if hp <= line {
                    dump_for(st, "the failsafe pulled you out", cooling);
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
    // Signal ghost: an awake piece is felt through the walls.
    let ghost = st
        .character
        .has_ability(&st.content, &game::AbilityId::new("signal_ghost"));
    k.guards
        .iter()
        .zip(states)
        .filter(|(g, i)| {
            k.guard_at(g.pos).is_some()
                && (seen(g.pos) || (ghost && matches!(i.mode, IceMode::Awake { .. })))
        })
        .map(|(g, i)| {
            let mode = match i.mode {
                IceMode::Asleep => "asleep",
                IceMode::Awake { .. } => "hunting",
            };
            let stunned = if i.stunned > 0 {
                format!(" · stunned {}", i.stunned)
            } else {
                String::new()
            };
            format!(
                "▣ {} ICE · {mode} · {}/{}{stunned}",
                g.grade.label(),
                i.integrity,
                g.grade.integrity()
            )
        })
        .collect()
}

/// Every node tile of a construct, for a ping's glow.
pub fn construct_nodes(st: &AppState, b: BuildingId) -> Vec<Pos> {
    st.constructs
        .get(b)
        .map(|k| k.nodes.iter().map(|(_, p)| *p).collect())
        .unwrap_or_default()
}

/// An arc trace inside a construct: every standing piece within `radius` of the
/// origin is stunned for `STUN_TURNS`. Returns their tiles, for the bolts.
pub fn stun_pieces(st: &mut AppState, b: BuildingId, origin: Pos, radius: i32) -> Vec<Pos> {
    let Some(k) = st.constructs.get(b).cloned() else {
        return Vec::new();
    };
    let states = st.net.ice_of(&k);
    let mut hit = Vec::new();
    for (i, g) in k.guards.iter().enumerate() {
        if k.guard_at(g.pos).is_none() || g.pos.chebyshev(origin) > radius {
            continue;
        }
        if let Some(ice) = states.get_mut(i) {
            ice.stunned = STUN_TURNS;
            hit.push(g.pos);
        }
    }
    hit
}

/// The awake standing pieces a signal ghost feels through the walls; none without
/// the ability.
pub fn ghost_pieces(st: &AppState, b: BuildingId) -> Vec<Pos> {
    if !st
        .character
        .has_ability(&st.content, &game::AbilityId::new("signal_ghost"))
    {
        return Vec::new();
    }
    let (Some(k), Some(states)) = (st.constructs.get(b), st.net.ice.get(&b)) else {
        return Vec::new();
    };
    k.guards
        .iter()
        .zip(states)
        .filter(|(g, i)| k.guard_at(g.pos).is_some() && matches!(i.mode, IceMode::Awake { .. }))
        .map(|(g, _)| g.pos)
        .collect()
}

/// The district a Net tile sits over, or the city's name.
pub(crate) fn district_name(city: &City, pos: Pos) -> String {
    city.tile(StratumKind::Net, pos)
        .and_then(|t| t.district)
        .or_else(|| city.tile(StratumKind::Street, pos).and_then(|t| t.district))
        .and_then(|d| city.district_opt(d))
        .map(|d| d.name.clone())
        .unwrap_or_else(|| city.name.clone())
}

/// Ten cells of trace: a filled cell per ten heat.
pub fn trace_bar(heat: u8) -> String {
    let filled = (heat / 10).min(10) as usize;
    format!("{}{}", "▰".repeat(filled), "▱".repeat(10 - filled))
}

/// One beat of a calm construct: the run in flight rolls once, then every piece of
/// ICE looks and takes one step or one hit.
pub fn beat(st: &mut AppState) {
    step_run(st);
    // The roll may have tripped the alarm or dumped you: the fight (or the street)
    // takes over from here, nothing acts outside the pace.
    let Some(Place::Construct(b)) = st.player.as_ref().map(|p| p.place) else {
        return;
    };
    if st.net.engaged.is_some() {
        return;
    }
    ice_turn(st);
    if st
        .player
        .as_ref()
        .is_some_and(|p| p.place == Place::Construct(b))
        && st.net.death.is_none()
        && any_awake(st, b)
    {
        engage(st, b, false);
    }
}

/// `▓▓▓▓░░░░`: how far the clock is toward the next beat.
pub fn beat_bar(st: &AppState, cells: usize) -> String {
    let phase = st.clock_tick().saturating_sub(st.net.beat).min(BEAT_TICKS);
    let filled = ((phase * cells as u64 + BEAT_TICKS / 2) / BEAT_TICKS) as usize;
    format!("{}{}", "▓".repeat(filled), "░".repeat(cells - filled))
}

/// The footer's run line: `RUN  crack · door lock  ▰▰▱ 2/3 · 40%   ▓▓▓▓░░░░  next roll`.
/// `crack · door lock`: the run's program and target, as the footer names them.
pub fn run_label(st: &AppState, run: &Run) -> Option<String> {
    let city = st.city.as_ref()?;
    let spec = st.content.item(&run.program)?.program?;
    let target = match run.target {
        HackTarget::Node(id) => city.net.node(id).kind.label().to_string(),
        HackTarget::Guard { building, index } => st
            .constructs
            .get(building)
            .and_then(|k| k.guards.get(index))
            .map(|g| format!("{} ICE", g.grade.label()))
            .unwrap_or_else(|| "ICE".to_string()),
        HackTarget::Deck => "deck".to_string(),
    };
    Some(format!("{} · {target}", spec.op.label()))
}

pub fn run_readout(st: &AppState, b: BuildingId, run: &Run) -> Option<String> {
    let spec = st.content.item(&run.program)?.program?;
    let kind = target_kind(st, b, run.target)?;
    let label = run_label(st, run)?;
    let security = st.constructs.get(b).map_or(0, |k| k.security);
    let pct = if kind == Target::Deck {
        100
    } else {
        game::chance(
            power(&st.character, &st.content, &spec, kind),
            difficulty(security),
        )
    };
    let tail = if st.net.engaged.is_some() {
        String::new()
    } else {
        format!("   {}  next roll", beat_bar(st, 8))
    };
    Some(format!(
        "RUN  {label}  {}{} {}/{} · {pct}%{tail}",
        "▰".repeat(run.done as usize),
        "▱".repeat(run.work.saturating_sub(run.done) as usize),
        run.done,
        run.work
    ))
}

/// An awake piece stands in the player's sight memory.
pub fn hunted(st: &AppState) -> bool {
    let Some(p) = st.player.as_ref() else {
        return false;
    };
    let Place::Construct(b) = p.place else {
        return false;
    };
    let (Some(k), Some(states)) = (st.constructs.get(b), st.net.ice.get(&b)) else {
        return false;
    };
    let vis = p.vis();
    k.guards.iter().zip(states).any(|(g, i)| {
        g.standing
            && matches!(i.mode, IceMode::Awake { .. })
            && vis
                .get(g.pos)
                .is_some_and(|v| *v != crate::render::Visibility::Unseen)
    })
}

/// Why the port refuses to jack in right now: the deck cooling, or a lockdown.
pub fn jack_in_refusal(st: &AppState) -> Option<String> {
    let now = st.net.now(st);
    if let Some(left) = st.net.cooling(now) {
        return Some(format!("deck cooling · {left} min"));
    }
    let city = st.city.as_ref()?;
    let player = st.player.as_ref()?;
    let port = player.terminal_in_reach(city, st.home.as_ref())?;
    // The rig at home is your own line: no building's lockdown reaches it.
    if st.home.is_some_and(|h| h.place() == player.place) {
        return None;
    }
    let locked = city
        .buildings
        .iter()
        .any(|b| b.footprint.grow(1).contains(port) && st.net.alert(b.id, now));
    locked.then(|| "port locked down".to_string())
}

/// Jacks in at the terminal in reach: the refusal when the port refuses, else the
/// line the log reads.
pub fn jack_in_here(st: &mut AppState) -> Result<String, String> {
    if let Some(why) = jack_in_refusal(st) {
        return Err(why);
    }
    let bonus = st.sight_bonus();
    let home = st.home;
    let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_mut()) else {
        return Err("no body in the city".to_string());
    };
    let from = player.place;
    player
        .jack_in(city, home.as_ref(), bonus)
        .map(|port| crate::walker::jacked_in_line(city, from, port))
        .map_err(|e| e.to_string())
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
    }
    // Pieces: beside you or in sight within range, nearest first.
    let mut pieces: Vec<(i32, usize)> = (0..k.guards.len())
        .filter(|i| piece_in_reach(st, b, *i))
        .map(|i| (k.guards[i].pos.chebyshev(pos), i))
        .collect();
    pieces.sort();
    for (_, i) in pieces {
        out.push((
            HackTarget::Guard {
                building: b,
                index: i,
            },
            format!("ICE · {}", k.guards[i].grade.label()),
        ));
    }
    out
}

/// A standing piece a program reaches: beside you, or in your sight within
/// `ATTACK_RANGE`.
pub fn piece_in_reach(st: &AppState, b: BuildingId, index: usize) -> bool {
    let (Some(player), Some(k)) = (st.player.as_ref(), st.constructs.get(b)) else {
        return false;
    };
    let Some(g) = k.guards.get(index) else {
        return false;
    };
    if k.guard_at(g.pos) != Some(index) {
        return false;
    }
    let d = g.pos.chebyshev(player.pos);
    d <= 1
        || (d <= ATTACK_RANGE
            && player.vis().get(g.pos) == Some(&crate::render::Visibility::Visible))
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

    /// A Medium city with a home, the player woken up beside the bed and stepped to
    /// the rig.
    fn at_the_rig(seed: u64) -> (AppState, crate::home::HomeRig, citygen::Pos) {
        let mut st = AppState::new();
        st.enter_city(
            citygen::Generator::run(citygen::GenParams {
                seed,
                size: citygen::SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        let rig = st.home.unwrap();
        let city = st.city.as_ref().unwrap();
        let mut p = rig
            .wake(city, &mut st.interiors, &st.constructs, 0)
            .unwrap();
        let (_, floor) = st
            .interiors
            .floor(rig.home.building, rig.home.level)
            .unwrap();
        let at = rig
            .home
            .rig
            .neighbors8()
            .find(|q| rig.home.rect.contains(*q) && floor.tiles[*q].is_walkable())
            .unwrap();
        p.pos = at;
        st.player = Some(p);
        (st, rig, at)
    }

    #[test]
    fn the_home_rig_jacks_in_through_a_lockdown() {
        // Medium seed 2: a street Jack hugs the home megablock.
        let (mut st, rig, _) = at_the_rig(2);
        let b = rig.home.building;
        let now = st.net.now(&st);
        st.net.trace(b).add(game::ALARM_HEAT, now);
        st.net.alarm(b, now);
        assert!(st.net.alert(b, now));
        assert_eq!(jack_in_refusal(&st), None);
        // The deck cooling still refuses.
        st.net.cooling_until = Some(now + 5);
        assert_eq!(
            jack_in_refusal(&st),
            Some("deck cooling · 5 min".to_string())
        );
        st.net.cooling_until = None;
        // A street Jack beside the same megablock is locked down.
        let city = st.city.as_ref().unwrap().clone();
        let fp = city.building(b).footprint;
        let street = citygen::StratumKind::Street;
        let beside_jack = city
            .connectors
            .iter()
            .filter(|k| {
                k.kind == citygen::ConnectorKind::Jack
                    && k.serves.contains(&street)
                    && fp.grow(1).contains(k.pos)
            })
            .find_map(|k| {
                std::iter::once(k.pos)
                    .chain(k.pos.neighbors8())
                    .find(|q| city.walkable(street, *q))
            })
            .expect("a street Jack beside the megablock");
        let p = st.player.as_mut().unwrap();
        p.place = citygen::Place::Stratum(street);
        p.pos = beside_jack;
        assert_eq!(jack_in_refusal(&st), Some("port locked down".to_string()));
    }

    #[test]
    fn a_dump_from_the_home_rig_wakes_at_the_rig() {
        let (mut st, rig, at) = at_the_rig(8);
        let city = st.city.as_ref().unwrap();
        let p = st.player.as_mut().unwrap();
        assert_eq!(p.jack_in(city, Some(&rig), 0), Ok(rig.landing));
        dump_for(&mut st, "test", 5);
        let p = st.player.as_ref().unwrap();
        let home = citygen::Place::Floor {
            building: rig.home.building,
            level: rig.home.level,
        };
        assert_eq!((p.place, p.pos, p.body), (home, at, None));
        assert_eq!(
            p.vis().get(rig.home.rig),
            Some(&crate::render::Visibility::Visible)
        );
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
        net.alarm(b(1), 30);
        assert!(net.hacks.iter().any(|h| h.op == Op::Backdoor));
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
        net.minute(
            20 + game::ALERT_MINUTES,
            None,
            &mut constructs,
            &BTreeSet::new(),
        );
        assert!(net.hacks.is_empty(), "{:?}", net.hacks);
        // Cooling.
        assert_eq!(net.cooling(0), None);
        net.cooling_until = Some(15);
        assert_eq!(net.cooling(5), Some(10));
        assert_eq!(net.cooling(15), None);
        let _ = IceGrade::White;
    }
}
