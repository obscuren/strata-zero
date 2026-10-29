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
pub const ATTACK_RANGE: i32 = 8;
/// Ticks a slip's or a shield's mark sits on its tile.
pub const MARK_TICKS: u64 = 20;
/// A program's roll and an attack, cyan; an ICE hit, magenta; black ICE, red.
pub const BOLT_ROLL: (u8, u8, u8) = (0, 229, 255);
pub const BOLT_HIT: (u8, u8, u8) = (255, 0, 200);
pub const BOLT_BLACK: (u8, u8, u8) = (255, 40, 40);
/// What an attack program looks like: a shot in the net's cyan (no muzzle, nothing
/// burns here), the tear where it lands in cyan and deep blue, the damage as a
/// blue tag; a slip sparks cyan. The whole shot carries `ATTACK_TINT`.
pub const ATTACK_FX: [Fx; 4] = [
    Fx::Streak { colour: BOLT_ROLL },
    Fx::Tear,
    Fx::Spark,
    Fx::Number { tag: true },
];
pub const ATTACK_TINT: (u8, u8, u8) = BOLT_ROLL;
/// A slip marks its target magenta; a shield that absorbs marks you cyan.
pub const MARK_SLIP: (u8, u8, u8) = (255, 0, 200);
pub const MARK_SHIELD: (u8, u8, u8) = (0, 229, 255);

/// Who acts in an engagement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    You,
    Ice(usize),
    Foe(usize),
}

/// Your turn in two halves: you move (or stay), then you act.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Move,
    Act,
}

/// A map on turns, a construct or the street: the order rolled once, whose turn it
/// is, and the pools left to the actor acting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engagement {
    pub place: Place,
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
    /// The tick your last action point went; the turn ends on its own a beat later.
    pub spent_at: Option<u64>,
    /// Redline has been spent this fight.
    pub boosted: bool,
}

/// Ticks after your last action point before your turn ends on its own.
pub const AUTO_END_TICKS: u64 = 20;

impl Engagement {
    pub fn actor(&self) -> Actor {
        self.order.get(self.turn).copied().unwrap_or(Actor::You)
    }

    /// The construct on turns, when it is one.
    pub fn construct(&self) -> Option<BuildingId> {
        match self.place {
            Place::Construct(b) => Some(b),
            _ => None,
        }
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
/// Heat a strike ability makes on a piece of ICE inside a construct.
pub const STRIKE_NOISE: u8 = 3;
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
    /// Your turns left stunned by a blast of your own.
    pub stunned_you: u8,
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
            stunned_you: 0,
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
        cut: u8,
        now: u64,
        null_route: bool,
    ) -> (TraceEvent, bool) {
        let was_alert = self.trace(b).alert(now);
        let event = self.trace(b).add_cut(noise, cut, now);
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

    /// Whether the server itself stands cracked: a live Crack on the server node.
    /// For its hour you are root on that building's host.
    pub fn server_cracked(&self, b: BuildingId, now: u64) -> bool {
        self.live(now).any(|h| {
            h.building == b && h.op == Op::Crack && h.kind == Target::Node(NodeKind::Server)
        })
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
    let city = st.city.as_ref();
    city.and_then(|c| c.building_opt(b))
        .map(|x| x.name.clone().unwrap_or_else(|| x.kind.label().to_string()))
        .unwrap_or_else(|| {
            if city.is_some_and(|c| c.is_lot()) {
                "the lot".to_string()
            } else {
                "the building".to_string()
            }
        })
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
        engage(st, Place::Construct(b), false);
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
    // The picture of the roll: a bolt to the target, a mark where it slipped; an
    // attack looks like a shot, with its number when it lands.
    if let Some((me, at)) = st
        .player
        .as_ref()
        .map(|p| p.pos)
        .zip(target_pos(st, b, run.target))
    {
        if at == me {
            mark(st, at, BOLT_ROLL, ROLL_TICKS);
        } else if spec.op == Op::Attack {
            let mut damage = 2 * spec.rating;
            if st.character.flags(&st.content).contains("black_ice") {
                damage += 1;
            }
            crate::combat::play_shot(
                st,
                Place::Construct(b),
                &ATTACK_FX,
                me,
                at,
                ok,
                damage,
                Some(ATTACK_TINT),
            );
        } else {
            bolt(st, me, at, BOLT_ROLL, ROLL_TICKS);
        }
        if !ok {
            // A slipped attack marks in its own cyan; every other slip in magenta.
            let colour = if spec.op == Op::Attack {
                ATTACK_TINT
            } else {
                MARK_SLIP
            };
            mark(st, at, colour, MARK_TICKS);
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
        if ok {
            let mut damage = 2 * spec.rating;
            if st.character.flags(&st.content).contains("black_ice") {
                damage += 1;
            }
            return hit_piece(st, b, index, label, damage, 0, now);
        }
        let line = format!("{label} slips · heat {heat}");
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

/// A hit lands on a piece of ICE: its integrity falls (a dummy takes it and keeps
/// standing), it wakes, `stun` turns hold it, and at zero the piece breaks with the
/// break path of an attack, ending a run on it. Logs, under the name of what hit
/// it (`attack · grey ICE 4/6`, `static · grey ICE 3/6`), and returns the line.
pub fn hit_piece(
    st: &mut AppState,
    b: BuildingId,
    index: usize,
    by: &str,
    damage: u8,
    stun: u8,
    now: u64,
) -> Option<String> {
    let target = HackTarget::Guard { building: b, index };
    let kind = target_kind(st, b, target)?;
    let (grade, label, inert) = st.constructs.get(b).map(|k| {
        (
            k.guards[index].grade,
            k.guards[index].label(),
            k.guards[index].inert,
        )
    })?;
    let seen = st.player.as_ref()?.pos;
    let left = {
        let states = st.net.ice.get_mut(&b)?;
        let ice = states.get_mut(index)?;
        // A dummy takes the hit and keeps standing; a piece turns on you.
        if !inert {
            ice.integrity = ice.integrity.saturating_sub(damage);
            ice.mode = IceMode::Awake { seen, lost: 0 };
            ice.stunned = ice.stunned.max(stun);
        }
        ice.integrity
    };
    if left == 0 {
        // A run on the broken piece ends with it; one on anything else goes on.
        if st.net.run.as_ref().is_some_and(|r| r.target == target) {
            st.net.run = None;
        }
        st.net.land(b, target, kind, Op::Attack, now);
        let roots = st.backdoors.roots();
        st.net.sync_guards(&mut st.constructs, b, now, &roots);
        kick(st, 6);
        let line = format!("{label} ICE broken");
        st.say(line.clone());
        alarm_clears(st, b);
        return Some(line);
    }
    let line = format!("{by} · {label} ICE {left}/{}", grade.integrity());
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
            crate::jobs::send(
                st,
                crate::jobs::Event::Copied {
                    building: b,
                    vault: true,
                },
            );
            "vault copied · a `Vault dump` in the pack".to_string()
        }
        (Op::Copy, _) => {
            st.character.give(ItemId::new("paydata"), 1);
            crate::jobs::send(
                st,
                crate::jobs::Event::Copied {
                    building: b,
                    vault: false,
                },
            );
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
            crate::jobs::send(st, crate::jobs::Event::Planted { building: b });
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
        label: String::new(),
        hit: false,
        tint: None,
        arc: None,
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
        label: String::new(),
        hit: false,
        tint: None,
        arc: None,
    });
}

/// A number lifting off a tile of the player's map from `started`, in a colour
/// (the net's bolts); `None` leaves the street's own.
pub fn number(
    st: &mut AppState,
    at: Pos,
    label: impl Into<String>,
    colour: Option<(u8, u8, u8)>,
    started: u64,
) {
    let Some(place) = st.player.as_ref().map(|p| p.place) else {
        return;
    };
    st.fx.push(FxInstance {
        fx: Fx::Number { tag: false },
        place,
        origin: at,
        radius: 0,
        started,
        revealed: Vec::new(),
        targets: vec![at],
        label: label.into(),
        hit: true,
        tint: colour,
        arc: None,
    });
}

pub(crate) fn kick(st: &mut AppState, ticks: u64) {
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
        label: String::new(),
        hit: false,
        tint: None,
        arc: None,
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
        engage(st, Place::Construct(b), false);
        seat_standing(st, b);
    }
    format!("ALARM · `{}` locks down", building_name(st, b))
}

/// Heat on the construct's trace with everything that follows: the alarm raised,
/// the null route taken once, the dump. Returns whether the player was dumped.
pub(crate) fn noise_on(st: &mut AppState, b: BuildingId, noise: u8) -> bool {
    let cut = st.character.trace_cut(&st.content);
    heat_on(st, b, noise, cut)
}

/// Heat the deck's hardware cannot cut: ICE hitting you is not your own trace.
pub(crate) fn ice_heat(st: &mut AppState, b: BuildingId, noise: u8) -> bool {
    heat_on(st, b, noise, 0)
}

fn heat_on(st: &mut AppState, b: BuildingId, noise: u8, cut: u8) -> bool {
    let now = st.net.now(st);
    let was_alert = st.net.alert(b, now);
    let null_route = st.character.flags(&st.content).contains("null_route");
    let (event, routed) = st.net.add_heat(b, noise, cut, now, null_route);
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
    // A dump on a rig run starts the trace home from the relay nearest you.
    if st.backdoors.route.is_some() {
        let running = st.backdoors.trace.is_some();
        st.backdoors.start_trace(now);
        let first = st
            .backdoors
            .trace
            .as_ref()
            .and_then(|t| t.relays.first().copied());
        match first {
            _ if running => st.warn("trace running · already on its way"),
            Some(b) => st.warn(format!(
                "trace running · it starts at `{}`",
                building_name(st, b)
            )),
            None => st.warn("trace running · straight to your door"),
        }
    }
    let _ = jack_out(st);
    kick(st, 20);
    st.warn(format!("DUMPED · {why}"));
    st.repaint = true;
}

/// The backdoor clock: the trace walks, relays burn, the raid lands, the rig returns.
pub fn tick_backdoors(st: &mut AppState) {
    let now = st.net.now(st);
    for ev in st.backdoors.minute(now) {
        match ev {
            crate::backdoors::Event::RelayBurned(b) => {
                let line = format!(
                    "trace passes `{}` · the backdoor is gone",
                    building_name(st, b)
                );
                st.warn(line);
            }
            crate::backdoors::Event::LastLeg => {
                st.warn("trace · nothing left between them and your door");
            }
            crate::backdoors::Event::Raid => {
                // The route ends first, so the dump starts no second trace.
                st.backdoors.end_route();
                if st.player.as_ref().is_some_and(|p| p.jacked_in()) {
                    dump_for(st, "the line went dead", COOLING_MINUTES);
                }
                st.story("they kicked your door in · the rig is evidence · every backdoor is gone");
                kick(st, 20);
            }
            crate::backdoors::Event::RigReturned => crate::credentials::fixer_mail(st),
        }
    }
}

/// Back to the body, wherever it waits, seeing around it again.
pub fn jack_out(st: &mut AppState) -> Result<(), &'static str> {
    let bonus = st.sight_bonus();
    let minute = st.minute_of_day();
    let city = st.city.as_ref().ok_or("no body in the city")?;
    let player = st.player.as_mut().ok_or("no body in the city")?;
    player.jack_out(city, bonus, minute)?;
    if let Some(map) = MapRef::for_place(city, &st.interiors, &st.constructs, player.place, minute)
    {
        player.update_fov(&map, bonus);
    }
    // A run in flight stays in the Net.
    if st.net.run.take().is_some() {
        st.say("run aborted");
    }
    st.backdoors.end_route();
    Ok(())
}

/// The building's trace at the alarm line, its gate on alert, its line warned.
pub fn lock_down(st: &mut AppState, b: BuildingId) {
    let now = st.net.now(st);
    let t = st.net.trace(b);
    t.heat = t.heat.max(game::ALARM_HEAT);
    if t.alert_until.is_none() {
        t.alert_until = Some(now + game::ALERT_MINUTES);
    }
    let line = raise_alarm(st, b);
    st.warn(line);
}

/// The not-burned backdoors as landing rows: `Hollis Court · root · STABLE`.
pub fn landings(st: &AppState) -> Vec<(BuildingId, String)> {
    let now = st.net.now(st);
    st.backdoors
        .standing()
        .map(|d| {
            (
                d.building,
                format!(
                    "{} · {} · {}",
                    building_name(st, d.building),
                    d.user_label(),
                    d.health(now).label()
                ),
            )
        })
        .collect()
}

/// Whether Space on the rig should offer the landing list rather than jack in.
pub fn wants_landing_list(st: &AppState) -> bool {
    jack_in_refusal(st).is_none()
        && st
            .home
            .is_some_and(|h| st.player.as_ref().is_some_and(|p| p.place == h.place()))
        && !landings(st).is_empty()
}

/// The landing list: your block, then each backdoor you hold.
pub fn landing_popup(st: &AppState) -> crate::screens::popup::Popup {
    use crate::screens::popup::{Choice, Pick, Popup, WorldAct};
    let mut choices = vec![Choice::new(
        'a',
        "your block",
        Pick::World(WorldAct::JackIn),
    )];
    let mut letters = "bcdefgimnoprsuvwxyz".chars();
    for (b, label) in landings(st) {
        let Some(k) = letters.next() else { break };
        choices.push(Choice::new(k, label, Pick::World(WorldAct::Land(b))));
    }
    Popup::verbs("JACK IN", choices, st.tick)
}

/// A rig run begins: an empty route from your block, or one relay when you landed
/// through a backdoor. A street jack-in leaves the route alone.
pub fn after_jack_in(st: &mut AppState, from: Place, via: Option<BuildingId>) {
    if st.home.is_some_and(|h| h.place() == from) {
        st.backdoors.start_route(via);
    }
}

/// Checks and spends a use of the backdoor on `b`; whether it was a honeypot.
fn use_backdoor(st: &mut AppState, b: BuildingId) -> Result<bool, String> {
    let now = st.net.now(st);
    let seed = st.city.as_ref().map_or(0, |c| c.seed);
    let security = crate::access::security_of(st, b);
    match st.backdoors.check(b, security, seed, now) {
        crate::backdoors::Use::Refused => Err("connection refused · key rejected".to_string()),
        crate::backdoors::Use::Trap => Ok(true),
        crate::backdoors::Use::Works => {
            let cost = match st.backdoors.held(b).map(|d| &d.kind) {
                Some(crate::backdoors::Kind::Net { .. }) => crate::backdoors::COST_LAND_NET,
                _ => crate::backdoors::COST_LAND,
            };
            st.backdoors.spend(b, cost);
            Ok(false)
        }
    }
}

/// Springs a honeypot you just landed on: the alarm, the heat, a trace with that
/// relay already gone.
fn spring(st: &mut AppState, b: BuildingId) {
    let now = st.net.now(st);
    lock_down(st, b);
    let _ = st
        .net
        .add_heat(b, crate::backdoors::HONEYPOT_NOISE, 0, now, false);
    st.backdoors.start_trace(now);
    st.warn("honeypot · they were waiting · trace running");
}

/// Jack in from the rig and land at a backdoor's gate.
pub fn land_at(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    if let Some(why) = jack_in_refusal(st) {
        return Err(why);
    }
    // Everything that can stop the landing is settled before the key is used.
    let home = st.home;
    let city = st.city.as_ref().ok_or("no city")?;
    let player = st.player.as_ref().ok_or("no body")?;
    if player.jacked_in() {
        return Err("already jacked in".to_string());
    }
    if !home.is_some_and(|h| h.place() == player.place) {
        return Err("only the rig lands you at a backdoor".to_string());
    }
    let port = player
        .terminal_in_reach(city, home.as_ref())
        .ok_or("no terminal within reach")?;
    if !city.walkable(StratumKind::Net, port) {
        return Err("the port is dead".to_string());
    }
    let gate = crate::constructs::city_gate(city, b).ok_or("that gate is gone")?;
    let trap = use_backdoor(st, b)?;
    let bonus = st.sight_bonus();
    let city = st.city.as_ref().ok_or("no city")?;
    let player = st.player.as_mut().ok_or("no body")?;
    let from = player.place;
    player
        .jack_in(city, home.as_ref(), bonus)
        .map_err(|e| e.to_string())?;
    player.pos = gate;
    player.update_fov(&MapRef::stratum_of(city, StratumKind::Net), bonus);
    after_jack_in(st, from, Some(b));
    st.repaint = true;
    if trap {
        spring(st, b);
    }
    Ok(format!(
        "jacked in · `{}`'s gate · via your backdoor",
        building_name(st, b)
    ))
}

/// From the server of a construct you hold, onto another backdoor's gate.
pub fn hop(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    if st.backdoors.route.is_none() {
        return Err("no route home".to_string());
    }
    let gate = st
        .city
        .as_ref()
        .and_then(|c| crate::constructs::city_gate(c, b))
        .ok_or("that gate is gone")?;
    let trap = use_backdoor(st, b)?;
    if st.net.run.take().is_some() {
        st.say("run aborted");
    }
    crate::constructs::land(st, Place::Stratum(StratumKind::Net), gate);
    st.backdoors.hop(b);
    if trap {
        spring(st, b);
    }
    Ok(format!("hop · `{}`'s gate", building_name(st, b)))
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
    line_clear_in(k, from, player)
}

/// No wall of the construct on the line between two tiles, the ends excluded:
/// the construct's own clear line, as `foes::line_clear` is a stratum's.
pub fn line_clear_in(k: &Construct, from: Pos, to: Pos) -> bool {
    bresenham(from, to)
        .into_iter()
        .filter(|p| *p != from && *p != to)
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
    // A dummy never looks, never wakes, never acts.
    if g.inert {
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
                hits = can_hit && !g.inert;
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

/// The order of an engagement. In a construct: you by your Net, each standing
/// piece by its grade. On foot: you by your Reflex, each standing foe in your sight
/// memory by its own. Higher first, ties to you, then by index.
pub fn initiative(st: &AppState, place: Place) -> Vec<Actor> {
    let mut scored: Vec<(i32, u8, Actor)> = Vec::new();
    match place {
        Place::Construct(b) => {
            scored.push((
                st.character.attribute(&st.content, game::Attribute::Net) as i32,
                0,
                Actor::You,
            ));
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
        }
        _ => {
            scored.push((
                st.character.attribute(&st.content, game::Attribute::Reflex) as i32,
                0,
                Actor::You,
            ));
            for i in crate::foes::in_memory(st) {
                scored.push((st.foes[i].reflex as i32, 1 + i as u8, Actor::Foe(i)));
            }
        }
    }
    scored.sort_by(|a, c| c.0.cmp(&a.0).then(a.1.cmp(&c.1)));
    scored.into_iter().map(|(_, _, a)| a).collect()
}

/// Tiles you may walk in a turn at `place`: on foot your chrome's move counts, at
/// least one; in a construct your avatar isn't your meat, and walks `BASE_MOVE`.
pub fn you_move(st: &AppState, place: Place) -> u8 {
    match place {
        Place::Construct(_) => BASE_MOVE,
        _ => {
            let moves = BASE_MOVE as i32 + st.character.move_bonus(&st.content);
            moves.clamp(1, u8::MAX as i32) as u8
        }
    }
}

fn pools_for(st: &AppState, place: Place, actor: Actor) -> (u8, u8) {
    match (actor, place) {
        (Actor::You, _) => (you_move(st, place), BASE_AP),
        (Actor::Ice(i), Place::Construct(b)) => st
            .constructs
            .get(b)
            .and_then(|k| k.guards.get(i))
            .map(|g| budgets(g.grade))
            .unwrap_or((0, 0)),
        (Actor::Ice(_), _) => (0, 0),
        (Actor::Foe(_), _) => (crate::foes::GOON_MOVE, crate::foes::GOON_AP),
    }
}

/// The map goes to turns: the order is rolled, you or the fastest acts first.
/// Nothing happens if it already is.
pub fn engage(st: &mut AppState, place: Place, manual: bool) {
    if st.net.engaged.is_some() {
        return;
    }
    if let Place::Construct(b) = place
        && let Some(k) = st.constructs.get(b)
    {
        st.net.ice_of(k);
    }
    let order = initiative(st, place);
    let (moves, ap) = pools_for(st, place, order[0]);
    st.net.engaged = Some(Engagement {
        place,
        round: 1,
        order,
        turn: 0,
        moves,
        ap,
        phase: Phase::Move,
        manual,
        acting_since: st.clock_tick(),
        spent_at: None,
        boosted: false,
    });
    let line = if manual {
        "engaged".to_string()
    } else if let Place::Construct(_) = place {
        "ENGAGED · the ICE is on you".to_string()
    } else {
        let name = crate::foes::in_memory(st)
            .into_iter()
            .find(|i| st.foes[*i].hunting())
            .map(|i| st.foes[i].name.clone())
            .unwrap_or_else(|| "foe".to_string());
        format!("ENGAGED · the {name} has you")
    };
    st.warn(line);
    start_turn(st);
}

/// Every standing foe in sight memory the order does not hold yet takes its place
/// at the end: one that came hunting mid-fight.
pub fn seat_foes(st: &mut AppState) {
    let known = crate::foes::in_memory(st);
    if let Some(e) = st.net.engaged.as_mut() {
        for i in known {
            if !e.order.contains(&Actor::Foe(i)) {
                e.order.push(Actor::Foe(i));
            }
        }
    }
}

/// Something still hunts you on the map on turns: awake ICE in a construct, a
/// hunting foe on foot.
pub fn hunting(st: &AppState) -> bool {
    match st.net.engaged.as_ref().map(|e| e.place) {
        Some(Place::Construct(b)) => any_awake(st, b),
        Some(place) => crate::foes::any_hunting(st, place),
        None => false,
    }
}

/// Engaged on foot: a stratum or a floor on turns.
pub fn engaged_on_foot(st: &AppState) -> bool {
    st.net
        .engaged
        .as_ref()
        .is_some_and(|e| !matches!(e.place, Place::Construct(_)))
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
        && e.place == Place::Construct(b)
    {
        for i in standing {
            if !e.order.contains(&Actor::Ice(i)) {
                e.order.push(Actor::Ice(i));
            }
        }
    }
}

/// Back to the beat. The fight's marks and your stun end with it.
pub fn disengage(st: &mut AppState) {
    for f in &mut st.foes {
        f.marked = false;
    }
    st.net.stunned_you = 0;
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
        Some(e) => you_move(st, e.place) as usize,
        None => st
            .player
            .as_ref()
            .map_or(BASE_MOVE, |p| you_move(st, p.place)) as usize,
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
    let now = st.clock_tick();
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
    if e.ap == 0 {
        e.spent_at = Some(now);
    }
    Ok(())
}

/// From the frame loop: your turn ends on its own a beat after the last action
/// point went, so the numbers can be read before the board moves on.
pub fn auto_end(st: &mut AppState) {
    let now = st.clock_tick();
    let due = st.net.engaged.as_ref().is_some_and(|e| {
        e.actor() == Actor::You
            && e.ap == 0
            && e.spent_at.is_some_and(|at| now >= at + AUTO_END_TICKS)
    });
    if due {
        end_turn(st);
    }
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
    let Some((place, len, manual, turn, round)) = st
        .net
        .engaged
        .as_ref()
        .map(|e| (e.place, e.order.len(), e.manual, e.turn, e.round))
    else {
        return;
    };
    let mut turn = turn + 1;
    let mut round = round;
    if turn >= len {
        turn = 0;
        round += 1;
        match place {
            Place::Construct(b) => {
                // A round passed: a held mask wears (and may dump you through the
                // trace), and any piece that stood again since takes its seat.
                seat_standing(st, b);
                mask_wears(st, b);
            }
            _ => seat_foes(st),
        }
        if st.net.engaged.is_none() {
            return;
        }
        if !manual && !hunting(st) {
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
    let (moves, ap) = pools_for(st, place, actor);
    let now = st.clock_tick();
    if let Some(e) = st.net.engaged.as_mut() {
        e.turn = turn;
        e.round = round;
        e.moves = moves;
        e.ap = ap;
        e.phase = Phase::Move;
        e.acting_since = now;
        e.spent_at = None;
    }
    if actor == Actor::You {
        // Fuses burn down as your turn opens; a blast of your own may be the end of you.
        crate::blast::tick_live(st);
        // A stun empties the turn's action points, not the turn: the moves are
        // still yours, and you end it.
        if st.net.stunned_you > 0
            && let Some(e) = st.net.engaged.as_mut()
        {
            e.ap = 0;
            st.net.stunned_you -= 1;
            st.say("you're stunned");
        }
    }
    start_turn(st);
}

/// An enemy's turn begins: it looks; a lifted, broken, down or stunned one passes.
fn start_turn(st: &mut AppState) {
    let Some(e) = st.net.engaged.as_ref() else {
        return;
    };
    let acts = match (e.actor(), e.place) {
        (Actor::Ice(i), Place::Construct(b)) => ice_look(st, b, i),
        (Actor::Foe(i), _) => crate::foes::turn_start(st, i),
        (Actor::Ice(_), _) => false,
        (Actor::You, _) => return,
    };
    if !acts {
        end_turn(st);
    }
}

/// From the frame loop while an enemy acts: one action every `PACE_TICKS`.
pub fn pace(st: &mut AppState) {
    let Some(e) = st.net.engaged.as_ref() else {
        return;
    };
    let actor = e.actor();
    if actor == Actor::You {
        return;
    }
    if st.clock_tick() < e.acting_since + PACE_TICKS {
        return;
    }
    let place = e.place;
    let (moves, ap) = (e.moves, e.ap);
    let step = match (actor, place) {
        (Actor::Ice(i), Place::Construct(b)) => ice_step(st, b, i, moves > 0, ap > 0),
        (Actor::Foe(i), _) => crate::foes::step(st, i, moves > 0, ap > 0),
        _ => Step::Nothing,
    };
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
            match (actor, place) {
                (Actor::Ice(i), Place::Construct(b)) => {
                    ice_look(st, b, i);
                }
                (Actor::Foe(i), _) => {
                    crate::foes::look(st, i);
                }
                _ => {}
            }
        }
        Step::Hit => e.ap = e.ap.saturating_sub(1),
        Step::Nothing => end_turn(st),
    }
    if let Some(e) = st.net.engaged.as_ref()
        && e.actor() == actor
        && e.moves == 0
        && e.ap == 0
    {
        end_turn(st);
    }
}

/// A standing piece's card for the fire cursor: `grey ICE`, `4/6`, where it stands.
pub fn ice_card(st: &AppState, b: BuildingId, i: usize) -> Option<(String, String, Pos)> {
    let k = st.constructs.get(b)?;
    let g = k.guards.get(i)?;
    if k.guard_at(g.pos) != Some(i) {
        return None;
    }
    let ice = st.net.ice.get(&b)?.get(i)?;
    Some((
        format!("{} ICE", g.label()),
        format!("{}/{}", ice.integrity, g.grade.integrity()),
        g.pos,
    ))
}

/// The COMBAT start on the Net: the lot's construct (a lane with a grey ICE at the
/// far end and a dummy near the gate), the body left on the street, jacked in and
/// landed on the gate.
pub fn start_in_net_lot(st: &mut AppState) -> Result<String, String> {
    let k = citygen::lot_construct();
    let (b, entry) = (k.building, k.entry);
    st.constructs.insert(k);
    if let Some(k) = st.constructs.get(b) {
        st.net.ice_of(k);
    }
    let p = st.player.as_mut().ok_or("no body")?;
    p.body = Some((p.place, p.pos));
    p.place = Place::Stratum(StratumKind::Net);
    p.pos = entry;
    st.net.beat = st.clock_tick();
    crate::constructs::land(st, Place::Construct(b), entry);
    Ok("Entering `the lot`".to_string())
}

/// The turn column's view: every standing actor in the rolled order with its chip,
/// your pools on your turn, the acting enemy's plan on theirs.
pub fn turn_view(st: &AppState) -> Option<crate::render::turns::TurnView> {
    use crate::render::turns::{Chip, TurnView};
    let e = st.net.engaged.as_ref()?;
    let k = e.construct().and_then(|b| st.constructs.get(b));
    let states = e.construct().and_then(|b| st.net.ice.get(&b));
    let actor = e.actor();
    let mut chips = Vec::new();
    for a in &e.order {
        let chip = match *a {
            Actor::You => Chip {
                name: "YOU".to_string(),
                hp: String::new(),
                acting: actor == Actor::You,
            },
            Actor::Ice(i) => {
                let Some((g, ice)) = k
                    .and_then(|k| k.guards.get(i).filter(|g| k.guard_at(g.pos).is_some()))
                    .zip(states.and_then(|s| s.get(i)))
                else {
                    continue;
                };
                Chip {
                    name: format!("{} ICE", g.label()),
                    hp: format!("{}/{}", ice.integrity, g.grade.integrity()),
                    acting: actor == *a,
                }
            }
            Actor::Foe(i) => {
                let Some(f) = st.foes.get(i).filter(|f| f.standing()) else {
                    continue;
                };
                Chip {
                    name: f.name.clone(),
                    hp: f.hp.to_string(),
                    acting: actor == *a,
                }
            }
        };
        chips.push(chip);
    }
    let yours = (actor == Actor::You).then_some((
        e.moves,
        you_move(st, e.place),
        e.ap,
        BASE_AP,
        e.phase == Phase::Act,
    ));
    let theirs = match actor {
        Actor::You => None,
        Actor::Ice(_) | Actor::Foe(_) => {
            let verb = if matches!(actor, Actor::Ice(_)) {
                "hits"
            } else {
                "fires"
            };
            Some(match (e.moves, e.ap) {
                (0, 0) => "done".to_string(),
                (0, _) => verb.to_string(),
                (m, 0) => format!("moves {m}"),
                (m, _) => format!("moves {m} › {verb}"),
            })
        }
    };
    Some(TurnView {
        round: e.round,
        chips,
        yours,
        theirs,
    })
}

/// The footer's turn line: `TURN 3  YOU › grey ICE › black ICE`, the actor acting first.
pub fn turn_readout(st: &AppState) -> Option<String> {
    let e = st.net.engaged.as_ref()?;
    let k = e.construct().and_then(|b| st.constructs.get(b));
    let name = |a: Actor| match a {
        Actor::You => "YOU".to_string(),
        Actor::Ice(i) => k
            .and_then(|k| k.guards.get(i))
            .map(|g| format!("{} ICE", g.label()))
            .unwrap_or_else(|| "ICE".to_string()),
        Actor::Foe(i) => st
            .foes
            .get(i)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "foe".to_string()),
    };
    let mut parts = Vec::new();
    for n in 0..e.order.len() {
        let a = e.order[(e.turn + n) % e.order.len()];
        let mut s = name(a);
        if n == 0 {
            match a {
                Actor::Ice(_) => {
                    if e.moves > 0 {
                        s.push_str(&format!(" moves {}", e.moves));
                    } else if e.ap > 0 {
                        s.push_str(" hits");
                    }
                }
                Actor::Foe(_) => {
                    if e.moves > 0 {
                        s.push_str(&format!(" moves {}", e.moves));
                    } else if e.ap > 0 {
                        s.push_str(" fires");
                    }
                }
                Actor::You => {}
            }
        }
        parts.push(s);
    }
    Some(format!("TURN {}  {}", e.round, parts.join(" › ")))
}

/// A standing piece beside the player hits them: white adds heat, grey drains
/// focus and dumps at zero, black takes HP through the jack. A shield charge soaks
/// a hit; the deck's rating comes off grey and black hits, then your feedback
/// resistance cuts what is left.
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
    // Your feedback resistance takes its share after the deck's rating.
    let feedback = st.character.resist(&st.content, game::Harm::Feedback);
    // The number lifts off you when the bolt lands, in its colour.
    let me = st.player.as_ref().map(|p| p.pos);
    let landed = st.tick + ROLL_TICKS;
    let colour = if grade == IceGrade::Black {
        BOLT_BLACK
    } else {
        BOLT_HIT
    };
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
            if let Some(me) = me {
                number(st, me, "+5", Some(colour), landed);
            }
            ice_heat(st, b, 5);
        }
        IceGrade::Grey => {
            let drain = game::resisted(10u32.saturating_sub(armour).max(1), feedback);
            st.character.focus = st.character.focus.saturating_sub(drain);
            let focus = st.character.focus;
            st.warn(format!("grey ICE hits · focus {focus}"));
            if let Some(me) = me {
                number(st, me, format!("-{drain}"), Some(colour), landed);
            }
            if focus == 0 {
                dump(st, "grey ICE drained you");
            }
        }
        IceGrade::Black => {
            // The deep end: half the damage through the jack, a failsafe that waits
            // for an eighth, and a short cooling after.
            let deep = st.character.flags(&st.content).contains("deep_end");
            let mut damage = game::resisted(
                (6 + security.saturating_sub(230) as u32 / 5)
                    .saturating_sub(armour)
                    .max(1),
                feedback,
            );
            if deep {
                damage = (damage / 2).max(1);
            }
            st.character.hp = st.character.hp.saturating_sub(damage);
            let hp = st.character.hp;
            st.warn(format!("black ICE hits · hp {hp}"));
            if let Some(me) = me {
                number(st, me, format!("-{damage}"), Some(colour), landed);
            }
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
                g.label(),
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
    // On foot and calm, fuses burn down on the beat.
    if st.net.engaged.is_none()
        && st
            .player
            .as_ref()
            .is_some_and(|p| !matches!(p.place, Place::Construct(_)))
    {
        crate::blast::tick_live(st);
    }
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
        engage(st, Place::Construct(b), false);
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
            .map(|g| format!("{} ICE", g.label()))
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

/// Why the port refuses to jack in right now: a fight on foot, the deck cooling, or
/// a lockdown.
pub fn jack_in_refusal(st: &AppState) -> Option<String> {
    if engaged_on_foot(st) {
        return Some("not with a gun on you".to_string());
    }
    let now = st.net.now(st);
    if let Some(left) = st.net.cooling(now) {
        return Some(format!("deck cooling · {left} min"));
    }
    let city = st.city.as_ref()?;
    let player = st.player.as_ref()?;
    if st.home.is_some_and(|h| h.place() == player.place) && st.backdoors.seized(now) {
        return Some("the rig is gone · evidence".to_string());
    }
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
    let line = player
        .jack_in(city, home.as_ref(), bonus)
        .map(|port| crate::walker::jacked_in_line(city, from, port))
        .map_err(|e| e.to_string())?;
    // From the rig, the run has a route home: empty, until you hop.
    after_jack_in(st, from, None);
    Ok(line)
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
            format!("ICE · {}", k.guards[i].label()),
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

    const STREET: Place = Place::Stratum(StratumKind::Street);

    fn logged(st: &AppState, needle: &str) -> bool {
        st.log.iter().any(|e| e.text.contains(needle))
    }

    /// The lot's construct with you on its gate, as the COMBAT · NET start opens.
    fn net_lot() -> AppState {
        let mut st = AppState::new();
        st.enter_city(citygen::City::lot(1));
        st.player = Some(crate::walker::Player::at(STREET, Pos::new(6, 8)));
        start_in_net_lot(&mut st).unwrap();
        st
    }

    #[test]
    fn the_dummy_ice_never_wakes_hits_or_breaks() {
        let mut st = net_lot();
        let b = BuildingId(0);
        let dummy = st.constructs.get(b).unwrap().guards[1].pos;
        assert_eq!(dummy, Pos::new(6, 3));
        // Stand beside it and run the attack at it with a sure roll.
        st.player.as_mut().unwrap().pos = Pos::new(5, 3);
        st.net.force_roll = Some(10);
        st.character
            .unload(&st.content, &ItemId::new("copy"))
            .unwrap();
        st.character.give(ItemId::new("attack"), 1);
        st.character
            .load(&st.content, &ItemId::new("attack"))
            .unwrap();
        let target = HackTarget::Guard {
            building: b,
            index: 1,
        };
        start_run(&mut st, ItemId::new("attack"), target).unwrap();
        assert!(st.net.engaged.is_some());
        let line = step_run(&mut st).unwrap();
        assert_eq!(line, "attack · dummy ICE 6/6");
        assert_eq!(st.net.ice[&b][1].integrity, 6, "nothing gets through");
        assert_eq!(st.net.ice[&b][1].mode, IceMode::Asleep, "it never wakes");
        assert!(
            st.fx
                .iter()
                .any(|i| matches!(i.fx, Fx::Number { tag: true })),
            "the number still lifts"
        );
        // Its turn passes at once, and standing beside you it never hits.
        let hp = st.character.hp;
        let focus = st.character.focus;
        for _ in 0..3 {
            end_turn(&mut st);
            for _ in 0..40 {
                if your_turn(&st) {
                    break;
                }
                st.tick += PACE_TICKS;
                if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
                    world.advance(city, world.tick() + PACE_TICKS);
                }
                pace(&mut st);
            }
        }
        assert_eq!((st.character.hp, st.character.focus), (hp, focus));
        assert!(!logged(&st, "ICE hits"));
        assert!(
            ice_lines(&st, b, |_| true).contains(&"▣ dummy ICE · asleep · 6/6".to_string()),
            "{:?}",
            ice_lines(&st, b, |_| true)
        );
    }

    #[test]
    fn feedback_cuts_ice() {
        let mut st = net_lot();
        let b = BuildingId(0);
        st.character
            .chrome
            .insert(game::Site::Neural, ItemId::new("feedback_fuse"));
        assert_eq!(st.character.resist(&st.content, game::Harm::Feedback), 25);
        let armour = st.character.deck(&st.content).map(|(_, d)| d.rating as u32);
        assert_eq!(armour, Some(1), "the deck's rating comes off first");
        let set = |st: &mut AppState, grade: IceGrade| {
            let k = st.constructs.get_mut(b).unwrap();
            k.security = 255;
            k.guards[0].grade = grade;
        };
        // Black: 6 + (255 − 230) / 5 = 11, less the rating 1, then FBK 25: 8.
        set(&mut st, IceGrade::Black);
        let hp = st.character.hp;
        hit(&mut st, b, 0);
        assert_eq!(hp - st.character.hp, game::resisted(10, 25));
        assert_eq!(game::resisted(10, 25), 8);
        assert!(logged(&st, &format!("black ICE hits · hp {}", hp - 8)));
        // The deep end halves what the fuse let through: 4.
        st.character
            .talents
            .insert(game::TalentId::new("deep_end"), 1);
        let hp = st.character.hp;
        hit(&mut st, b, 0);
        assert_eq!(hp - st.character.hp, 4);
        // Grey: 10 less the rating, then FBK 25: 9 drains 7.
        set(&mut st, IceGrade::Grey);
        let focus = st.character.focus;
        hit(&mut st, b, 0);
        assert_eq!(focus - st.character.focus, game::resisted(9, 25));
        assert_eq!(game::resisted(9, 25), 7);
        // White: the heat is the heat.
        set(&mut st, IceGrade::White);
        let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
        hit(&mut st, b, 0);
        assert_eq!(st.net.traces[&b].heat, heat + 5);
    }

    #[test]
    fn move_reads_chrome() {
        use game::Site;
        let fit = |st: &mut AppState, site: Site, id: &str| {
            st.character.chrome.insert(site, ItemId::new(id));
        };
        // The spine: five tiles a fight's turn on foot.
        let mut st = crate::foes::testutil::lot(Pos::new(14, 8));
        fit(&mut st, Site::Spine, "redline_spine");
        assert_eq!(you_move(&st, STREET), 5);
        engage(&mut st, STREET, true);
        let e = st.net.engaged.as_ref().unwrap();
        assert_eq!((e.actor(), e.moves), (Actor::You, 5));
        assert_eq!(aim_cap(&st), 5);
        assert_eq!(
            turn_view(&st).unwrap().yours,
            Some((5, 5, BASE_AP, BASE_AP, false))
        );
        // The next turn of yours opens with five again (the goon down, it comes
        // straight back).
        st.foes[0].down = true;
        st.net.engaged.as_mut().unwrap().moves = 0;
        end_turn(&mut st);
        let e = st.net.engaged.as_ref().unwrap();
        assert_eq!((e.actor(), e.round, e.moves), (Actor::You, 2, 5));
        // Redline's refill fills to five.
        fit(&mut st, Site::Organs, "adrenal_pump");
        st.net.engaged.as_mut().unwrap().moves = 0;
        crate::casts::boost(&mut st, &game::AbilityId::new("redline")).unwrap();
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, 5);
        // The plating alone: three.
        let mut st = crate::foes::testutil::lot(Pos::new(14, 8));
        fit(&mut st, Site::Skin, "ceramic_plating");
        engage(&mut st, STREET, true);
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, 3);
        // In a construct your avatar walks the base, spine or not.
        let mut st = net_lot();
        let b = BuildingId(0);
        fit(&mut st, Site::Spine, "redline_spine");
        assert_eq!(you_move(&st, Place::Construct(b)), BASE_MOVE);
        engage(&mut st, Place::Construct(b), true);
        let e = st.net.engaged.as_ref().unwrap();
        assert_eq!((e.actor(), e.moves), (Actor::You, BASE_MOVE));
        assert_eq!(BASE_MOVE, 4);
        assert_eq!(turn_view(&st).unwrap().yours.map(|y| y.1), Some(BASE_MOVE));
    }

    #[test]
    fn initiative_on_the_street_is_by_reflex_with_ties_to_you() {
        let mut st = crate::foes::testutil::lot(Pos::new(14, 8));
        st.character.attributes.reflex = 5;
        assert_eq!(initiative(&st, STREET), vec![Actor::You, Actor::Foe(0)]);
        st.character.attributes.reflex = 4;
        assert_eq!(initiative(&st, STREET), vec![Actor::Foe(0), Actor::You]);
        // A foe out of sight memory is not seated.
        st.player
            .as_mut()
            .unwrap()
            .set_vis(Pos::new(14, 8), crate::render::Visibility::Unseen);
        assert_eq!(initiative(&st, STREET), vec![Actor::You]);
    }

    #[test]
    fn pools_for_a_foe() {
        let mut st = crate::foes::testutil::lot(Pos::new(14, 8));
        st.character.attributes.reflex = 3;
        st.foes[0].mode = crate::foes::FoeMode::Hunting {
            seen: Pos::new(6, 8),
            lost: 0,
        };
        engage(&mut st, STREET, true);
        let e = st.net.engaged.clone().unwrap();
        assert_eq!(e.place, STREET);
        assert_eq!(e.actor(), Actor::Foe(0));
        assert_eq!(
            (e.moves, e.ap),
            (crate::foes::GOON_MOVE, crate::foes::GOON_AP)
        );
        assert!(logged(&st, "engaged"));
    }

    #[test]
    fn a_hunting_foe_engages_the_street_with_the_magenta_line() {
        let mut st = crate::foes::testutil::lot(Pos::new(14, 8));
        assert!(crate::foes::look_all(&mut st));
        engage(&mut st, STREET, false);
        let e = st.net.engaged.clone().unwrap();
        assert_eq!((e.place, e.manual), (STREET, false));
        assert_eq!(e.order, vec![Actor::You, Actor::Foe(0)]);
        let last = st.log.latest().unwrap();
        assert_eq!(last.text, "ENGAGED · the goon has you");
        assert_eq!(last.kind, crate::messages::Kind::Warn);
        assert!(hunting(&st) && engaged_on_foot(&st));
        assert!(!hunted(&st), "HUNTED is the net's word");
    }

    #[test]
    fn a_round_with_no_hunter_calms_unless_manual() {
        let mut st = crate::foes::testutil::lot(Pos::new(18, 8));
        engage(&mut st, STREET, false);
        assert_eq!(st.net.engaged.as_ref().unwrap().order.len(), 2);
        // Your turn, then the idle foe's (it passes), then the round ends.
        end_turn(&mut st);
        assert!(st.net.engaged.is_none(), "{:?}", st.net.engaged);
        assert!(logged(&st, "calm"));
        engage(&mut st, STREET, true);
        end_turn(&mut st);
        assert!(st.net.engaged.is_some());
        assert_eq!(st.net.engaged.as_ref().unwrap().round, 2);
    }

    #[test]
    fn jack_in_and_doors_refused_while_engaged() {
        let mut st = crate::foes::testutil::lot(Pos::new(14, 8));
        engage(&mut st, STREET, true);
        assert_eq!(
            jack_in_refusal(&st).as_deref(),
            Some("not with a gun on you")
        );
        assert_eq!(
            crate::interiors::enter(&mut st, b(0), (StratumKind::Street, Pos::new(6, 8)))
                .unwrap_err(),
            "not now"
        );
        disengage(&mut st);
        assert_ne!(
            jack_in_refusal(&st).as_deref(),
            Some("not with a gun on you")
        );
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
    fn hardware_cuts_your_trace_not_ice() {
        let (mut st, rig, _) = at_the_rig(8);
        let b = rig.home.building;
        st.character = game::Character::debug_runner(&st.content);
        let heat = |st: &AppState| st.net.traces.get(&b).map_or(0, |t| t.heat);
        // Bare: exactly the noise.
        assert!(!noise_on(&mut st, b, 4));
        assert_eq!(heat(&st), 4);
        // Coldfin vI: 36 tenths a time, 3 then 7.
        st.net.traces.clear();
        let at = st
            .character
            .inventory
            .iter()
            .position(|s| s.item == game::ItemId::new("coldfin_1"))
            .unwrap();
        let piece = st.character.inventory.remove(at);
        st.character.worn.get_mut(&game::Slot::Deck).unwrap().fitted[0] = Some(piece);
        noise_on(&mut st, b, 4);
        noise_on(&mut st, b, 4);
        assert_eq!(heat(&st), 7);
        // ICE is not your trace: the full 5.
        ice_heat(&mut st, b, 5);
        assert_eq!(heat(&st), 12);
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

    #[test]
    fn a_stunned_you_loses_the_turns_ap_and_a_fuse_burns_as_it_opens() {
        use crate::blast::Live;
        let street = Place::Stratum(citygen::StratumKind::Street);
        let mut st = crate::foes::testutil::lot(Pos::new(14, 8));
        engage(&mut st, street, true);
        st.foes[0].down = true;
        st.net.stunned_you = 1;
        let frag = st.content.item(&game::ItemId::new("frag")).unwrap();
        let (_, _, blast) = frag.throw().unwrap();
        st.live.push(Live {
            place: street,
            pos: Pos::new(30, 8),
            blast,
            fx: Vec::new(),
            name: "Frag".to_string(),
            turns: 2,
            lands: 0,
            wakes: None,
        });
        // Your turn passes to the idle goon and back.
        end_turn(&mut st);
        let e = st.net.engaged.as_ref().unwrap();
        assert_eq!(e.actor(), Actor::You);
        assert_eq!(
            (e.ap, e.moves),
            (0, BASE_MOVE),
            "stunned: no AP, the moves kept"
        );
        assert!(e.spent_at.is_none(), "the turn stays open");
        assert_eq!(st.net.stunned_you, 0);
        assert!(st.log.iter().any(|m| m.text.contains("you're stunned")));
        assert_eq!(st.live[0].turns, 1, "a fuse burned as the turn opened");
        // It does not end on its own: you end it.
        st.tick += AUTO_END_TICKS * 4;
        auto_end(&mut st);
        assert_eq!(st.net.engaged.as_ref().unwrap().actor(), Actor::You);
        // And the next turn is whole again.
        end_turn(&mut st);
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, BASE_AP);
        assert!(st.live.is_empty(), "the second turn set it off");
    }

    #[test]
    fn a_stun_taken_as_the_last_goon_falls_ends_with_the_fight() {
        let street = Place::Stratum(citygen::StratumKind::Street);
        let mut st = crate::foes::testutil::lot(Pos::new(8, 8));
        st.character = game::Character::combat_runner(&st.content);
        st.foes[0].hp = 1;
        engage(&mut st, street, false);
        assert!(stay(&mut st), "your turn first");
        st.tick = 500;
        // An EMP between you and the goon: it falls, your chrome takes the stun.
        crate::blast::throw(&mut st, &game::ItemId::new("emp_puck"), Pos::new(7, 8)).unwrap();
        assert!(st.foes[0].down);
        assert_eq!(st.net.stunned_you, 2);
        // The round runs out with nothing hunting: calm, and the stun with it.
        end_turn(&mut st);
        assert!(st.net.engaged.is_none(), "calm");
        assert_eq!(st.net.stunned_you, 0);
        // The next fight opens whole, through every turn of yours.
        st.foes
            .push(crate::foes::Foe::goon(street, Pos::new(12, 8)));
        engage(&mut st, street, true);
        for _ in 0..4 {
            end_turn(&mut st);
            if st.net.engaged.as_ref().unwrap().actor() == Actor::You {
                break;
            }
        }
        let e = st.net.engaged.as_ref().unwrap();
        assert_eq!((e.actor(), e.ap), (Actor::You, BASE_AP));
    }
}
