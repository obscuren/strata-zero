//! Backdoors with lifetimes: keys planted from a shell, Net backdoors cut into a
//! server, the route of a run from the rig, the trace that walks it home, and the
//! raid at the end. Pure over game minutes; the app reads and writes it.

use citygen::BuildingId;
use std::collections::{BTreeMap, BTreeSet};

pub const SLOTS: usize = 3;
pub const KEY_LIFE: u64 = 120;
pub const NET_LIFE: u64 = 3 * 24 * 60;
pub const COST_SSH: u64 = 10;
pub const COST_LAND: u64 = 20;
pub const COST_LAND_NET: u64 = 60;
pub const COST_WRITE: u64 = 5;
pub const HOLD_KEY: u64 = 30;
pub const HOLD_NET: u64 = 60;
pub const LAST_LEG: u64 = 20;
pub const PATCH_MINUTES: u64 = 24 * 60;
pub const SEIZED_MINUTES: u64 = 24 * 60;
pub const QUIET_MINUTES: u64 = 24 * 60;
pub const PLANT_NOISE: u8 = 15;
pub const HONEYPOT_NOISE: u8 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Key { user: String },
    Net { rating: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Decay,
    Trace,
    You,
    Raid,
    Honeypot,
}

impl Why {
    pub fn label(self) -> &'static str {
        match self {
            Why::Decay => "found",
            Why::Trace => "the trace took it",
            Why::You => "by you",
            Why::Raid => "the raid",
            Why::Honeypot => "a honeypot",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Live,
    Honeypot,
    Burned { at: u64, why: Why },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backdoor {
    pub building: BuildingId,
    pub kind: Kind,
    pub planted: u64,
    pub nominal: u64,
    pub spent: u64,
    /// Hidden: 70..=130, the real life as a percentage of the nominal.
    pub factor: u8,
    pub state: State,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    Stable,
    Degrading,
    LikelyBurned,
    Burned,
}

impl Health {
    pub fn label(self) -> &'static str {
        match self {
            Health::Stable => "STABLE",
            Health::Degrading => "DEGRADING",
            Health::LikelyBurned => "LIKELY BURNED",
            Health::Burned => "BURNED",
        }
    }
}

impl Backdoor {
    /// What the nominal clock says; the hidden factor plays no part.
    pub fn health(&self, now: u64) -> Health {
        if matches!(self.state, State::Burned { .. }) {
            return Health::Burned;
        }
        let gone = now.saturating_sub(self.planted) + self.spent;
        let left = self.nominal.saturating_sub(gone);
        if left == 0 {
            Health::LikelyBurned
        } else if left <= self.nominal / 2 {
            Health::Degrading
        } else {
            Health::Stable
        }
    }

    /// The minute it really stops working.
    fn real_end(&self) -> u64 {
        (self.planted + self.nominal * self.factor as u64 / 100).saturating_sub(self.spent)
    }

    pub fn hold(&self) -> u64 {
        match self.kind {
            Kind::Key { .. } => HOLD_KEY,
            Kind::Net { rating } if rating >= 2 => HOLD_NET * 2,
            Kind::Net { .. } => HOLD_NET,
        }
    }

    pub fn user_label(&self) -> String {
        match &self.kind {
            Kind::Key { user } => user.clone(),
            Kind::Net { .. } => "root".to_string(),
        }
    }

    fn burned(&self) -> bool {
        matches!(self.state, State::Burned { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    Works,
    Refused,
    Trap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Seized,
    Patched(u64),
    Full,
    Already,
}

impl Refusal {
    pub fn words(&self) -> String {
        match self {
            Refusal::Seized => "the rig is seized · no line home".to_string(),
            Refusal::Patched(_) => "hole patched · try again tomorrow".to_string(),
            Refusal::Full => format!("the rig holds {SLOTS} lines · burn one first"),
            Refusal::Already => "already planted here".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Burnt {
    pub cut_trace: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Route {
    pub relays: Vec<BuildingId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomeTrace {
    /// Nearest the target first.
    pub relays: Vec<BuildingId>,
    /// Index of the relay it sits on; `relays.len()` on the last leg.
    pub at: usize,
    pub moves_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Raid {
    pub at: u64,
    pub mailed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    RelayBurned(BuildingId),
    LastLeg,
    Raid,
    RigReturned,
}

#[derive(Default)]
pub struct Backdoors {
    pub list: Vec<Backdoor>,
    pub route: Option<Route>,
    pub trace: Option<HomeTrace>,
    pub patched: BTreeMap<BuildingId, u64>,
    pub rig_seized_until: Option<u64>,
    pub fixer_quiet_until: Option<u64>,
    pub raided: Vec<Raid>,
    /// Logins through a backdoor, for the host's auth.log: (minute, building, user).
    pub logins: Vec<(u64, BuildingId, String)>,
    /// A honeypot login in progress: the user it still lets in, read once by `user_for`.
    pub trap_login: Option<(BuildingId, String)>,
    last_minute: u64,
}

fn factor_for(seed: u64, b: BuildingId, planted: u64) -> u8 {
    70 + (citygen::rng::hash64(seed, b.index() as u64 ^ planted) % 61) as u8
}

impl Backdoors {
    pub fn live(&self, b: BuildingId) -> Option<&Backdoor> {
        self.list.iter().find(|d| d.building == b && !d.burned())
    }

    pub fn held(&self, b: BuildingId) -> Option<&Backdoor> {
        self.live(b)
    }

    fn held_mut(&mut self, b: BuildingId) -> Option<&mut Backdoor> {
        self.list
            .iter_mut()
            .find(|d| d.building == b && !d.burned())
    }

    pub fn root_on(&self, b: BuildingId) -> bool {
        self.live(b)
            .is_some_and(|d| matches!(d.kind, Kind::Net { .. }))
    }

    pub fn roots(&self) -> BTreeSet<BuildingId> {
        self.list
            .iter()
            .filter(|d| !d.burned() && matches!(d.kind, Kind::Net { .. }))
            .map(|d| d.building)
            .collect()
    }

    pub fn slots_used(&self) -> usize {
        self.list.iter().filter(|d| !d.burned()).count()
    }

    pub fn can_plant(&self, b: BuildingId, now: u64) -> Result<(), Refusal> {
        if self.seized(now) {
            return Err(Refusal::Seized);
        }
        if let Some(until) = self.patched.get(&b).filter(|u| now < **u) {
            return Err(Refusal::Patched(*until));
        }
        if self.slots_used() >= SLOTS {
            return Err(Refusal::Full);
        }
        if self.held(b).is_some() {
            return Err(Refusal::Already);
        }
        Ok(())
    }

    fn plant(&mut self, b: BuildingId, kind: Kind, nominal: u64, seed: u64, now: u64) {
        self.list.push(Backdoor {
            building: b,
            kind,
            planted: now,
            nominal,
            spent: 0,
            factor: factor_for(seed, b, now),
            state: State::Live,
        });
    }

    pub fn plant_key(&mut self, b: BuildingId, user: &str, security: u8, seed: u64, now: u64) {
        let nominal = if security >= citygen::construct::GREY_THRESHOLD {
            KEY_LIFE / 2
        } else {
            KEY_LIFE
        };
        self.plant(
            b,
            Kind::Key {
                user: user.to_string(),
            },
            nominal,
            seed,
            now,
        );
    }

    pub fn plant_net(&mut self, b: BuildingId, rating: u8, seed: u64, now: u64) {
        let nominal = if rating >= 2 { NET_LIFE * 2 } else { NET_LIFE };
        self.plant(b, Kind::Net { rating }, nominal, seed, now);
    }

    pub fn spend(&mut self, b: BuildingId, minutes: u64) {
        if let Some(d) = self.held_mut(b) {
            d.spent += minutes;
        }
    }

    /// What using the backdoor on `b` does now: it works, it is refused (and the
    /// journal learns it is burned), or it is bait and springs. At the hidden
    /// burn moment a grey-or-worse host may turn the key into a honeypot: that
    /// use still works, the host's `auth.log` shows the audit line from then on,
    /// and the next use springs it.
    pub fn check(&mut self, b: BuildingId, security: u8, seed: u64, now: u64) -> Use {
        let Some(d) = self.held_mut(b) else {
            return Use::Refused;
        };
        match d.state {
            State::Honeypot => {
                d.state = State::Burned {
                    at: now,
                    why: Why::Honeypot,
                };
                Use::Trap
            }
            State::Live if now < d.real_end() => Use::Works,
            State::Live => {
                let bait = security >= citygen::construct::GREY_THRESHOLD
                    && citygen::rng::hash64(seed, b.index() as u64 ^ 0xB00B).is_multiple_of(3);
                if bait {
                    d.state = State::Honeypot;
                    Use::Works
                } else {
                    d.state = State::Burned {
                        at: now,
                        why: Why::Decay,
                    };
                    Use::Refused
                }
            }
            State::Burned { .. } => Use::Refused,
        }
    }

    pub fn burn(&mut self, b: BuildingId, why: Why, now: u64) -> Burnt {
        let Some(d) = self.held_mut(b) else {
            return Burnt { cut_trace: false };
        };
        d.state = State::Burned { at: now, why };
        if matches!(why, Why::You | Why::Trace) {
            self.patched.insert(b, now + PATCH_MINUTES);
        }
        let on_it = self
            .trace
            .as_ref()
            .is_some_and(|t| t.relays.get(t.at) == Some(&b));
        if why == Why::You && on_it {
            self.trace = None;
            return Burnt { cut_trace: true };
        }
        Burnt { cut_trace: false }
    }

    pub fn start_route(&mut self, b: Option<BuildingId>) {
        self.route = Some(Route {
            relays: b.into_iter().collect(),
        });
    }

    pub fn hop(&mut self, b: BuildingId) {
        if let Some(r) = self.route.as_mut() {
            r.relays.push(b);
        }
    }

    pub fn end_route(&mut self) {
        self.route = None;
    }

    /// The trace starts at the relay nearest the target and walks home.
    /// A trace already walking keeps its countdown.
    pub fn start_trace(&mut self, now: u64) {
        if self.trace.is_some() {
            return;
        }
        let relays: Vec<BuildingId> = self
            .route
            .as_ref()
            .map(|r| {
                r.relays
                    .iter()
                    .rev()
                    .copied()
                    .filter(|b| self.held(*b).is_some())
                    .collect()
            })
            .unwrap_or_default();
        let moves_at = now + relays.first().map_or(LAST_LEG, |b| self.hold_of(*b));
        self.trace = Some(HomeTrace {
            relays,
            at: 0,
            moves_at,
        });
    }

    fn hold_of(&self, b: BuildingId) -> u64 {
        self.held(b).map_or(HOLD_KEY, |d| d.hold())
    }

    /// `TRACE ▸ <name> · N min` while a trace runs; `name` turns a building into
    /// the words the HUD uses for it.
    pub fn trace_line(&self, now: u64, name: &dyn Fn(BuildingId) -> String) -> Option<String> {
        let t = self.trace.as_ref()?;
        let left = t.moves_at.saturating_sub(now);
        let at = match t.relays.get(t.at) {
            Some(b) => name(*b),
            None => "your door".to_string(),
        };
        Some(format!("TRACE ▸ {at} · {left} min"))
    }

    pub fn seized(&self, now: u64) -> bool {
        self.rig_seized_until.is_some_and(|u| now < u)
    }

    pub fn fixer_quiet(&self, now: u64) -> bool {
        self.fixer_quiet_until.is_some_and(|u| now < u)
    }

    /// The game minutes up to `now` pass: the trace moves and burns, the raid lands,
    /// the rig comes back. Idempotent for a minute already passed.
    pub fn minute(&mut self, now: u64) -> Vec<Event> {
        let mut events = Vec::new();
        if now <= self.last_minute {
            return events;
        }
        self.last_minute = now;
        // The trace: one relay per hold; several may fall in one long step.
        while let Some(t) = self.trace.clone()
            && now >= t.moves_at
        {
            match t.relays.get(t.at).copied() {
                Some(b) => {
                    // A relay already burned (checked, honeypot, raid) costs the trace nothing.
                    if self.held(b).is_some() {
                        self.burn(b, Why::Trace, t.moves_at);
                        events.push(Event::RelayBurned(b));
                    }
                    let at = t.at + 1;
                    let next = match t.relays.get(at) {
                        Some(n) => self.hold_of(*n),
                        None => {
                            events.push(Event::LastLeg);
                            LAST_LEG
                        }
                    };
                    self.trace = Some(HomeTrace {
                        relays: t.relays,
                        at,
                        moves_at: t.moves_at + next,
                    });
                }
                None => {
                    self.trace = None;
                    self.raid(t.moves_at);
                    events.push(Event::Raid);
                }
            }
        }
        if let Some(until) = self.rig_seized_until
            && now >= until
            && let Some(r) = self.raided.iter_mut().find(|r| !r.mailed)
        {
            r.mailed = true;
            events.push(Event::RigReturned);
        }
        events
    }

    fn raid(&mut self, at: u64) {
        for d in self.list.iter_mut().filter(|d| !d.burned()) {
            d.state = State::Burned { at, why: Why::Raid };
        }
        self.rig_seized_until = Some(at + SEIZED_MINUTES);
        self.fixer_quiet_until = Some(at + QUIET_MINUTES);
        self.raided.push(Raid { at, mailed: false });
    }
}

/// `zenbo ▸ a ▸ b`.
pub fn route_line(names: &[String]) -> String {
    let mut s = "zenbo".to_string();
    for n in names {
        s.push_str(" ▸ ");
        s.push_str(n);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(i: u32) -> BuildingId {
        BuildingId(i)
    }

    #[test]
    fn health_follows_the_nominal_clock_not_the_factor() {
        let mut bd = Backdoors::default();
        bd.plant_key(b(1), "mara", 100, 7, 0);
        let k = bd.held(b(1)).unwrap();
        assert_eq!(k.nominal, KEY_LIFE);
        assert!((70..=130).contains(&k.factor));
        assert_eq!(k.health(0), Health::Stable);
        assert_eq!(k.health(KEY_LIFE / 2 - 1), Health::Stable);
        assert_eq!(k.health(KEY_LIFE / 2), Health::Degrading);
        assert_eq!(k.health(KEY_LIFE - 1), Health::Degrading);
        assert_eq!(k.health(KEY_LIFE), Health::LikelyBurned);
        assert_eq!(k.health(KEY_LIFE * 10), Health::LikelyBurned);
        assert_eq!(Health::LikelyBurned.label(), "LIKELY BURNED");
        // use spends life: half gone at once is DEGRADING now
        bd.spend(b(1), KEY_LIFE / 2);
        assert_eq!(bd.held(b(1)).unwrap().health(0), Health::Degrading);
    }

    #[test]
    fn lives_by_kind_security_and_rating() {
        let mut bd = Backdoors::default();
        bd.plant_key(b(1), "mara", citygen::construct::GREY_THRESHOLD, 7, 0);
        assert_eq!(bd.held(b(1)).unwrap().nominal, KEY_LIFE / 2);
        bd.plant_net(b(2), 1, 7, 0);
        assert_eq!(bd.held(b(2)).unwrap().nominal, NET_LIFE);
        bd.plant_net(b(3), 2, 7, 0);
        assert_eq!(bd.held(b(3)).unwrap().nominal, NET_LIFE * 2);
        assert_eq!(bd.held(b(3)).unwrap().hold(), HOLD_NET * 2);
        assert_eq!(bd.held(b(1)).unwrap().hold(), HOLD_KEY);
        assert!(bd.root_on(b(2)));
        assert!(!bd.root_on(b(1)));
        assert_eq!(bd.roots(), [b(2), b(3)].into_iter().collect());
    }

    #[test]
    fn check_works_until_the_real_moment_then_burns_or_baits() {
        // a white host never baits
        let mut bd = Backdoors::default();
        bd.plant_key(b(1), "mara", 100, 7, 0);
        let k = bd.held(b(1)).unwrap();
        let real = k.nominal * k.factor as u64 / 100;
        assert_eq!(bd.check(b(1), 100, 7, real - 1), Use::Works);
        assert_eq!(bd.check(b(1), 100, 7, real), Use::Refused);
        assert_eq!(bd.held(b(1)), None);
        assert_eq!(bd.list[0].health(real), Health::Burned);
        assert!(matches!(
            bd.list[0].state,
            State::Burned {
                why: Why::Decay,
                ..
            }
        ));
        assert_eq!(bd.check(b(1), 100, 7, real + 5), Use::Refused);
        // a grey host baits one in three, by hash; find a seed that does
        let seed = (0..200u64)
            .find(|s| citygen::rng::hash64(*s, 5u64 ^ 0xB00B).is_multiple_of(3))
            .unwrap();
        let mut bd = Backdoors::default();
        bd.plant_key(b(5), "mara", 200, seed, 0);
        let real = bd.held(b(5)).unwrap().nominal * bd.held(b(5)).unwrap().factor as u64 / 100;
        assert_eq!(bd.check(b(5), 200, seed, real), Use::Works);
        assert_eq!(bd.list[0].state, State::Honeypot);
        assert!(bd.live(b(5)).is_some());
        assert_eq!(bd.check(b(5), 200, seed, real + 1), Use::Trap);
        assert!(matches!(
            bd.list[0].state,
            State::Burned {
                why: Why::Honeypot,
                ..
            }
        ));
        assert_eq!(bd.check(b(5), 200, seed, real + 2), Use::Refused);
        // a honeypot still showed as whatever the clock said before it sprang
        let mut bd = Backdoors::default();
        bd.plant_key(b(5), "mara", 200, seed, 0);
        bd.list[0].state = State::Honeypot;
        assert_eq!(bd.list[0].health(0), Health::Stable);
        assert!(bd.live(b(5)).is_some());
    }

    #[test]
    fn the_cap_and_the_refusals_in_order() {
        let mut bd = Backdoors::default();
        for i in 1..=SLOTS as u32 {
            assert_eq!(bd.can_plant(b(i), 0), Ok(()));
            bd.plant_key(b(i), "u", 100, 1, 0);
        }
        assert_eq!(bd.slots_used(), SLOTS);
        assert_eq!(bd.can_plant(b(9), 0), Err(Refusal::Full));
        assert_eq!(bd.can_plant(b(1), 0), Err(Refusal::Full));
        bd.burn(b(1), Why::You, 10);
        assert_eq!(bd.slots_used(), SLOTS - 1);
        // burned by you: patched for a day
        assert_eq!(
            bd.can_plant(b(1), 10),
            Err(Refusal::Patched(10 + PATCH_MINUTES))
        );
        assert_eq!(bd.can_plant(b(1), 10 + PATCH_MINUTES), Ok(()));
        assert_eq!(bd.can_plant(b(2), 10), Err(Refusal::Already));
        bd.rig_seized_until = Some(100);
        assert_eq!(bd.can_plant(b(1), 50), Err(Refusal::Seized));
        assert_eq!(
            Refusal::Full.words(),
            "the rig holds 3 lines · burn one first"
        );
        assert_eq!(Refusal::Seized.words(), "the rig is seized · no line home");
        assert_eq!(
            Refusal::Patched(5).words(),
            "hole patched · try again tomorrow"
        );
        assert_eq!(Refusal::Already.words(), "already planted here");
    }

    #[test]
    fn a_route_then_a_trace_that_walks_it_home() {
        let mut bd = Backdoors::default();
        bd.plant_key(b(1), "u", 100, 1, 0);
        bd.plant_net(b(2), 1, 1, 0);
        bd.plant_net(b(3), 1, 1, 0);
        bd.start_route(Some(b(1)));
        bd.hop(b(2));
        assert_eq!(bd.route.as_ref().unwrap().relays, vec![b(1), b(2)]);
        bd.start_trace(100);
        bd.end_route();
        assert!(bd.route.is_none());
        let t = bd.trace.as_ref().unwrap();
        // nearest the target first: the last hop
        assert_eq!(t.relays, vec![b(2), b(1)]);
        assert_eq!(t.at, 0);
        assert_eq!(t.moves_at, 100 + HOLD_NET);
        assert_eq!(
            bd.trace_line(110, &|_| "x".to_string()),
            Some(format!("TRACE ▸ x · {} min", HOLD_NET - 10))
        );
        // nothing happens before the hold runs out
        assert!(bd.minute(100 + HOLD_NET - 1).is_empty());
        let ev = bd.minute(100 + HOLD_NET);
        assert_eq!(ev, vec![Event::RelayBurned(b(2))]);
        assert!(matches!(
            bd.list[1].state,
            State::Burned {
                why: Why::Trace,
                ..
            }
        ));
        assert!(bd.patched.contains_key(&b(2)));
        let t = bd.trace.as_ref().unwrap();
        assert_eq!((t.at, t.moves_at), (1, 100 + HOLD_NET + HOLD_KEY));
        let ev = bd.minute(100 + HOLD_NET + HOLD_KEY);
        assert_eq!(ev, vec![Event::RelayBurned(b(1)), Event::LastLeg]);
        let t = bd.trace.as_ref().unwrap();
        assert_eq!(t.moves_at, 100 + HOLD_NET + HOLD_KEY + LAST_LEG);
        assert_eq!(
            bd.trace_line(t.moves_at - 7, &|_| String::new()),
            Some("TRACE ▸ your door · 7 min".to_string())
        );
        // the same minute twice does nothing more
        assert!(bd.minute(100 + HOLD_NET + HOLD_KEY).is_empty());
        let raid_at = 100 + HOLD_NET + HOLD_KEY + LAST_LEG;
        let ev = bd.minute(raid_at);
        assert_eq!(ev, vec![Event::Raid]);
        assert!(bd.trace.is_none());
        // the raid takes everything you still hold, without patching
        assert!(matches!(
            bd.list[2].state,
            State::Burned { why: Why::Raid, .. }
        ));
        assert!(!bd.patched.contains_key(&b(3)));
        assert_eq!(bd.rig_seized_until, Some(raid_at + SEIZED_MINUTES));
        assert_eq!(bd.fixer_quiet_until, Some(raid_at + QUIET_MINUTES));
        assert!(bd.seized(raid_at + 1) && !bd.seized(raid_at + SEIZED_MINUTES));
        assert!(bd.fixer_quiet(raid_at + 1));
        assert_eq!(bd.raided.len(), 1);
        // the rig comes back once, with the mail's cue
        assert!(bd.minute(raid_at + SEIZED_MINUTES - 1).is_empty());
        assert_eq!(
            bd.minute(raid_at + SEIZED_MINUTES),
            vec![Event::RigReturned]
        );
        assert!(bd.minute(raid_at + SEIZED_MINUTES + 1).is_empty());
    }

    #[test]
    fn burning_the_relay_the_trace_sits_on_cuts_it() {
        let mut bd = Backdoors::default();
        bd.plant_key(b(1), "u", 100, 1, 0);
        bd.plant_key(b(2), "u", 100, 1, 0);
        bd.start_route(Some(b(1)));
        bd.hop(b(2));
        bd.start_trace(0);
        // burning the one it has not reached yet changes nothing
        assert_eq!(bd.burn(b(1), Why::You, 1), Burnt { cut_trace: false });
        assert!(bd.trace.is_some());
        assert_eq!(bd.burn(b(2), Why::You, 2), Burnt { cut_trace: true });
        assert!(bd.trace.is_none());
        // a second burn of a burned one is a no-op
        assert_eq!(bd.burn(b(2), Why::You, 3), Burnt { cut_trace: false });
        assert_eq!(Why::You.label(), "by you");
    }

    #[test]
    fn a_running_trace_is_never_reset() {
        let mut bd = Backdoors::default();
        bd.plant_key(b(1), "u", 100, 1, 0);
        bd.start_route(Some(b(1)));
        bd.start_trace(0);
        let before = bd.trace.as_ref().unwrap().moves_at;
        bd.start_trace(50);
        assert_eq!(bd.trace.as_ref().unwrap().moves_at, before);
    }

    #[test]
    fn a_trace_with_no_relays_goes_straight_to_the_door() {
        let mut bd = Backdoors::default();
        bd.start_route(None);
        bd.start_trace(50);
        let t = bd.trace.as_ref().unwrap();
        assert!(t.relays.is_empty());
        assert_eq!(t.moves_at, 50 + LAST_LEG);
        // relays already burned are left out too
        let mut bd = Backdoors::default();
        bd.plant_key(b(1), "u", 100, 1, 0);
        bd.start_route(Some(b(1)));
        bd.burn(b(1), Why::Honeypot, 1);
        bd.start_trace(1);
        assert!(bd.trace.as_ref().unwrap().relays.is_empty());
        assert_eq!(bd.minute(1 + LAST_LEG), vec![Event::Raid]);
    }

    #[test]
    fn the_trace_walks_past_a_relay_that_burned_already() {
        let mut bd = Backdoors::default();
        bd.plant_net(b(1), 1, 1, 0);
        bd.plant_key(b(2), "u", 100, 1, 0);
        bd.start_route(Some(b(1)));
        bd.hop(b(2));
        bd.start_trace(100);
        // the relay it reaches first is burned before the trace gets there
        bd.burn(b(2), Why::Honeypot, 101);
        assert!(
            bd.minute(100 + HOLD_KEY).is_empty(),
            "no burn event, no patch"
        );
        assert!(!bd.patched.contains_key(&b(2)));
        // the next relay's hold runs from the same moment
        let at = 100 + HOLD_KEY + HOLD_NET;
        assert_eq!(bd.trace.as_ref().unwrap().moves_at, at);
        assert_eq!(
            bd.minute(at),
            vec![Event::RelayBurned(b(1)), Event::LastLeg]
        );
    }

    #[test]
    fn a_route_and_its_line() {
        assert_eq!(
            route_line(&["hollis-court".to_string(), "meridian".to_string()]),
            "zenbo ▸ hollis-court ▸ meridian"
        );
        assert_eq!(route_line(&[]), "zenbo");
        let mut bd = Backdoors::default();
        bd.plant_net(b(2), 1, 1, 0);
        assert_eq!(bd.held(b(2)).unwrap().user_label(), "root");
        bd.plant_key(b(3), "mara-voss", 100, 1, 0);
        assert_eq!(bd.held(b(3)).unwrap().user_label(), "mara-voss");
    }

    #[test]
    #[ignore = "slow: three cities"]
    fn plant_land_hop_and_burn_across_three_cities() {
        use citygen::{GenParams, Generator, SizePreset};
        let (mut landed, mut hopped) = (0, 0);
        for seed in [3u64, 8, 21] {
            let mut st = crate::app::AppState::new();
            st.enter_city(
                Generator::run(GenParams {
                    seed,
                    size: SizePreset::Medium,
                    districts: 7,
                    ..Default::default()
                })
                .unwrap(),
            );
            let Some(rig) = st.home else { continue };
            let mut p = rig
                .wake(
                    st.city.as_ref().unwrap(),
                    &mut st.interiors,
                    &st.constructs,
                    0,
                )
                .unwrap();
            let at = {
                let (_, floor) = st
                    .interiors
                    .floor(rig.home.building, rig.home.level)
                    .unwrap();
                rig.home
                    .rig
                    .neighbors8()
                    .find(|q| rig.home.rect.contains(*q) && floor.tiles[*q].is_walkable())
                    .unwrap()
            };
            p.pos = at;
            st.player = Some(p);
            crate::credentials::move_in(&mut st);
            let district = st
                .city
                .as_ref()
                .unwrap()
                .building(rig.home.building)
                .district;
            let targets = crate::access::scannable(&st, district);
            let city_seed = st.city.as_ref().unwrap().seed;
            let mut planted = Vec::new();
            for b in targets.iter().copied() {
                if st.backdoors.can_plant(b, 0).is_ok() {
                    st.backdoors.plant_key(b, "user", 100, city_seed, 0);
                    planted.push(b);
                }
                assert!(st.backdoors.slots_used() <= SLOTS);
            }
            assert!(!planted.is_empty(), "seed {seed}: nothing planted");
            for b in planted.iter().copied() {
                if crate::constructs::city_gate(st.city.as_ref().unwrap(), b).is_none() {
                    continue;
                }
                if crate::net::land_at(&mut st, b).is_err() {
                    continue;
                }
                landed += 1;
                if let Some(other) = planted.iter().copied().find(|o| *o != b)
                    && crate::constructs::enter_construct(&mut st, b).is_ok()
                    && crate::net::hop(&mut st, other).is_ok()
                {
                    hopped += 1;
                }
                assert!(st.backdoors.slots_used() <= SLOTS);
                let _ = crate::net::jack_out(&mut st);
            }
            for b in planted {
                st.backdoors.burn(b, Why::You, 1);
            }
            assert_eq!(st.backdoors.slots_used(), 0);
        }
        assert!(landed > 0, "never landed");
        eprintln!("landed {landed}, hopped {hopped}");
    }
}
