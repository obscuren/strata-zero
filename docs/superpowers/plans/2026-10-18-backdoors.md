# Backdoors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Backdoors get lifetimes, two kinds (a shell-planted key and the Net
program's), landings and hops on the Net from the rig, a home trace that walks
the route back and ends in a raid, a slot cap of three, honeypots, and words for
all of it in the journal and HELIX.

**Architecture:** One new pure module `backdoors.rs` owns every backdoor, the
route of a rig run, the home trace, the patched hosts and the raid's
after-effects; it replaces the `NetState.backdoors` set. `net.rs` grows the
landing and hop actions and starts routes and traces at the jack boundaries.
HELIX gets three programs (`backdoor`, `burn`, `route`) and runs `ssh` through
the backdoor check. The map screen, HUD, journal and terminal read the module.

**Tech Stack:** Rust 2024, ratatui, the crate's own test helpers (`AppState::new`
+ `enter_city` on a generated city; `screens::explore` tests build a `MapScreen`).

**Spec:** `docs/superpowers/specs/2026-10-18-backdoors-design.md`

## Global Constraints

- Nothing saved to disk: `Backdoors` is session state, reset in `enter_city`.
- Lower-case shell replies except the period-voice lines in UPPER CASE already
  used by programs (`CONNECTED TO …`, `DOOR LOCKS RELEASED`); new program replies
  follow that: `KEY PLANTED · …`, `BURNED · …`, `ROUTE  …`; errors are
  `program: message` lower case with `status 1`.
- Log lines use backticks around names (`messages::parse_marks` lights them).
- Health labels are exactly `STABLE`, `DEGRADING`, `LIKELY BURNED`, `BURNED`.
- No countdown of the hidden factor anywhere on screen.
- Constants: `SLOTS 3`, `KEY_LIFE 120`, `NET_LIFE 4320`, `COST_SSH 10`,
  `COST_LAND 20`, `COST_LAND_NET 60`, `COST_WRITE 5`, `HOLD_KEY 30`,
  `HOLD_NET 60`, `LAST_LEG 20`, `PATCH_MINUTES 1440`, `SEIZED_MINUTES 1440`,
  `QUIET_MINUTES 1440`, `PLANT_NOISE 15`, `HONEYPOT_NOISE 60`.
- The whole suite stays green: `cargo test -p strata-zero` (and the workspace).
- Run `cargo fmt` and `cargo clippy --all-targets` clean before each commit.
- Commit messages in the repo's voice: `feat(app): …` / `fix(app): …`, present
  tense, with the attribution trailer the session carries.
- Another session may be building the Space-window redesign in `explore.rs` /
  `popup.rs`. Keep edits there small and additive; do not rename existing rows.

## Review Focus

1. **A backdoor planted at minute 0 of a session** (`planted == 0`): health
   arithmetic must not underflow or treat `0` as unset. Pinned in Task 1's
   health test with `planted = 0`.
2. **A key host whose user leaves the building** (tier drops to guest later):
   `user_for` must still log you in as the key's user while the key lives, and
   `tier_on` must stay `User`. Pinned in Task 2's `tier_on` test after dropping
   the card.
3. **Dump on a rig run with every relay already burned** (hopped through a
   honeypot, or the trace burned them): `start_trace` must not index an empty
   list; the trace goes straight to the last leg. Pinned in Task 1's trace test.
4. **The raid firing while the player is jacked in** must end the run cleanly
   (no dangling engagement or run). Pinned in Task 6's raid test.
5. **`rm -r ~/.ssh` and `rm ~/.ssh/authorized_keys` by the resident on a host
   where the key is already burned** must not panic or double-burn. Pinned in
   Task 3's rm test.

---

### Task 1: The `backdoors` module

**Files:**
- Create: `crates/strata-zero/src/backdoors.rs`
- Modify: `crates/strata-zero/src/main.rs` (module list), `crates/strata-zero/src/app.rs:28-72` (the field), `crates/strata-zero/src/app.rs:202-223` (`enter_city`)

**Interfaces:**
- Produces everything in the spec's §1: `Backdoors`, `Backdoor`, `Kind`, `State`,
  `Why`, `Health`, `Use`, `Refusal`, `Burnt`, `Route`, `HomeTrace`, `Raid`,
  `Event`, the constants, and these methods (exact signatures):

```rust
impl Backdoors {
    pub fn live(&self, b: BuildingId) -> Option<&Backdoor>;         // Live or Honeypot
    pub fn held(&self, b: BuildingId) -> Option<&Backdoor>;         // not Burned
    pub fn root_on(&self, b: BuildingId) -> bool;                    // a live Net backdoor
    pub fn roots(&self) -> BTreeSet<BuildingId>;                     // every root_on
    pub fn slots_used(&self) -> usize;
    pub fn can_plant(&self, b: BuildingId, now: u64) -> Result<(), Refusal>;
    pub fn plant_key(&mut self, b: BuildingId, user: &str, security: u8, seed: u64, now: u64);
    pub fn plant_net(&mut self, b: BuildingId, rating: u8, seed: u64, now: u64);
    pub fn spend(&mut self, b: BuildingId, minutes: u64);
    pub fn check(&mut self, b: BuildingId, security: u8, seed: u64, now: u64) -> Use;
    pub fn burn(&mut self, b: BuildingId, why: Why, now: u64) -> Burnt;
    pub fn start_route(&mut self, b: Option<BuildingId>);
    pub fn hop(&mut self, b: BuildingId);
    pub fn end_route(&mut self);
    pub fn start_trace(&mut self, now: u64);
    pub fn minute(&mut self, now: u64) -> Vec<Event>;
    pub fn seized(&self, now: u64) -> bool;
    pub fn fixer_quiet(&self, now: u64) -> bool;
}
impl Backdoor {
    pub fn health(&self, now: u64) -> Health;
    pub fn hold(&self) -> u64;                 // HOLD_KEY, or HOLD_NET (×2 at rating ≥ 2)
    pub fn user_label(&self) -> String;        // "root" or the key's user
}
impl Health { pub fn label(self) -> &'static str; }
impl Why { pub fn label(self) -> &'static str; }  // "by you", "found", "the trace took it", "the raid", "a honeypot"
impl Refusal { pub fn words(&self) -> String; }   // the four refusal lines after "backdoor: "
pub fn route_line(names: &[String]) -> String;    // "zenbo ▸ a ▸ b"
```

- [ ] **Step 1: Write the failing tests**

Create `crates/strata-zero/src/backdoors.rs` with only the test module first:

```rust
//! Backdoors with lifetimes: keys planted from a shell, Net backdoors cut into a
//! server, the route of a run from the rig, the trace that walks it home, and the
//! raid at the end. Pure over game minutes; the app reads and writes it.

use citygen::BuildingId;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests {
    use super::*;

    fn b(i: u32) -> BuildingId {
        BuildingId::new(i)
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
        assert!(matches!(bd.list[0].state, State::Burned { why: Why::Decay, .. }));
        assert_eq!(bd.check(b(1), 100, 7, real + 5), Use::Refused);
        // a grey host baits one in three, by hash; find a seed that does
        let seed = (0..200u64)
            .find(|s| citygen::rng::hash64(*s, 5u64 ^ 0xB00B) % 3 == 0)
            .unwrap();
        let mut bd = Backdoors::default();
        bd.plant_key(b(5), "mara", 200, seed, 0);
        let real = bd.held(b(5)).unwrap().nominal * bd.held(b(5)).unwrap().factor as u64 / 100;
        assert_eq!(bd.check(b(5), 200, seed, real), Use::Trap);
        assert!(matches!(bd.list[0].state, State::Burned { why: Why::Honeypot, .. }));
        assert_eq!(bd.check(b(5), 200, seed, real), Use::Refused);
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
        assert_eq!(bd.can_plant(b(1), 10), Err(Refusal::Patched(10 + PATCH_MINUTES)));
        assert_eq!(bd.can_plant(b(1), 10 + PATCH_MINUTES), Ok(()));
        assert_eq!(bd.can_plant(b(2), 10), Err(Refusal::Already));
        bd.rig_seized_until = Some(100);
        assert_eq!(bd.can_plant(b(1), 50), Err(Refusal::Seized));
        assert_eq!(Refusal::Full.words(), "the rig holds 3 lines · burn one first");
        assert_eq!(Refusal::Seized.words(), "the rig is seized · no line home");
        assert_eq!(Refusal::Patched(5).words(), "hole patched · try again tomorrow");
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
        assert_eq!(bd.trace_line(110, &|_| "x".to_string()), Some(format!("TRACE ▸ x · {} min", HOLD_NET - 10)));
        // nothing happens before the hold runs out
        assert!(bd.minute(100 + HOLD_NET - 1).is_empty());
        let ev = bd.minute(100 + HOLD_NET);
        assert_eq!(ev, vec![Event::RelayBurned(b(2))]);
        assert!(matches!(bd.list[1].state, State::Burned { why: Why::Trace, .. }));
        assert!(bd.patched.contains_key(&b(2)));
        let t = bd.trace.as_ref().unwrap();
        assert_eq!((t.at, t.moves_at), (1, 100 + HOLD_NET + HOLD_KEY));
        let ev = bd.minute(100 + HOLD_NET + HOLD_KEY);
        assert_eq!(ev, vec![Event::RelayBurned(b(1)), Event::LastLeg]);
        let t = bd.trace.as_ref().unwrap();
        assert_eq!(t.moves_at, 100 + HOLD_NET + HOLD_KEY + LAST_LEG);
        assert_eq!(bd.trace_line(t.moves_at - 7, &|_| String::new()), Some("TRACE ▸ your door · 7 min".to_string()));
        // the same minute twice does nothing more
        assert!(bd.minute(100 + HOLD_NET + HOLD_KEY).is_empty());
        let raid_at = 100 + HOLD_NET + HOLD_KEY + LAST_LEG;
        let ev = bd.minute(raid_at);
        assert_eq!(ev, vec![Event::Raid]);
        assert!(bd.trace.is_none());
        // the raid takes everything you still hold, without patching
        assert!(matches!(bd.list[2].state, State::Burned { why: Why::Raid, .. }));
        assert!(!bd.patched.contains_key(&b(3)));
        assert_eq!(bd.rig_seized_until, Some(raid_at + SEIZED_MINUTES));
        assert_eq!(bd.fixer_quiet_until, Some(raid_at + QUIET_MINUTES));
        assert!(bd.seized(raid_at + 1) && !bd.seized(raid_at + SEIZED_MINUTES));
        assert!(bd.fixer_quiet(raid_at + 1));
        assert_eq!(bd.raided.len(), 1);
        // the rig comes back once, with the mail's cue
        assert!(bd.minute(raid_at + SEIZED_MINUTES - 1).is_empty());
        assert_eq!(bd.minute(raid_at + SEIZED_MINUTES), vec![Event::RigReturned]);
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
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p strata-zero backdoors:: 2>&1 | tail -5`
Expected: compile errors (`Backdoors` not found).

- [ ] **Step 3: Write the module**

Above the test module in `backdoors.rs`:

```rust
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
        self.list
            .iter()
            .find(|d| d.building == b && !d.burned())
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
        self.plant(b, Kind::Key { user: user.to_string() }, nominal, seed, now);
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
    /// journal learns it is burned), or it is bait and springs.
    pub fn check(&mut self, b: BuildingId, security: u8, seed: u64, now: u64) -> Use {
        let Some(d) = self.held_mut(b) else {
            return Use::Refused;
        };
        match d.state {
            State::Honeypot => {
                d.state = State::Burned { at: now, why: Why::Honeypot };
                Use::Trap
            }
            State::Live if now < d.real_end() => Use::Works,
            State::Live => {
                let bait = security >= citygen::construct::GREY_THRESHOLD
                    && citygen::rng::hash64(seed, b.index() as u64 ^ 0xB00B) % 3 == 0;
                if bait {
                    d.state = State::Burned { at: now, why: Why::Honeypot };
                    Use::Trap
                } else {
                    d.state = State::Burned { at: now, why: Why::Decay };
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
    pub fn start_trace(&mut self, now: u64) {
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
                    self.burn(b, Why::Trace, t.moves_at);
                    events.push(Event::RelayBurned(b));
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
```

Note on `minute`: the `while let … && …` chain needs `let_chains`, which this
crate already uses (see `hosts.rs` `if let … && …`). If the `while let` form does
not compile, write it as `loop { let Some(t) = self.trace.clone() else { break }; if now < t.moves_at { break } … }`.

Add `pub mod backdoors;` to `crates/strata-zero/src/main.rs` beside `pub mod access;`
(the modules are listed alphabetically there; check with `grep -n "^pub mod\|^mod" crates/strata-zero/src/main.rs`).

In `app.rs`, add to `AppState` after `net`:

```rust
    /// Your backdoors, the route of a rig run, the trace home and the raid.
    pub backdoors: crate::backdoors::Backdoors,
```

initialise it in `AppState::new()` (find where `net: Default::default()` or
`net: crate::net::NetState::default()` is set and add `backdoors: Default::default(),`),
and in `enter_city` after `self.net = Default::default();` add
`self.backdoors = Default::default();`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero backdoors:: 2>&1 | tail -15`
Expected: 8 passed.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets -p strata-zero 2>&1 | tail -3
git add crates/strata-zero/src/backdoors.rs crates/strata-zero/src/main.rs crates/strata-zero/src/app.rs
git commit -m "feat(app): backdoors get a module, a lifetime, a route and a trace home"
```

---

### Task 2: The Net state reads `backdoors`

**Files:**
- Modify: `crates/strata-zero/src/net.rs:152,179,257-280,322-370` (`NetState`), `net.rs:445-509` (`start_run`), `net.rs:647-703` (`land`)
- Modify: `crates/strata-zero/src/access.rs:30-45` (`tier_on`)
- Modify: `crates/strata-zero/src/helix/hosts.rs:122-151` (`user_for`), `hosts.rs:1061-1062` (the test)
- Modify: `crates/strata-zero/src/credentials.rs:627-633,719-736` (entries), `credentials.rs:1075-1085` (test)
- Modify: `crates/strata-zero/src/screens/deck.rs:619-621`
- Modify: `crates/strata-zero/src/screens/explore.rs:1939` (the `minute` call), `explore.rs:5246-5290` (the backdoor test), `explore.rs:941-995` (`run_choices`)
- Modify: `crates/strata-zero/src/helix/bin.rs:675-700` (the test's backdoor landing)

**Interfaces:**
- Consumes Task 1.
- Produces: `NetState` has no `backdoors` field. `NetState::land` no longer
  records backdoors. `NetState::sync_guards(&self, constructs, b, now, roots: &BTreeSet<BuildingId>)`
  and `NetState::minute(&mut self, now, inside, constructs, roots: &BTreeSet<BuildingId>)`
  take the root set. `net::land` (the app function) plants through
  `st.backdoors.plant_net`. `credentials::EntryKind::Backdoor(usize)`.
  `net::plant_net_refusal(st, b) -> Option<String>` for the greyed row and `start_run`.

- [ ] **Step 1: Update the tests that pin the old behaviour**

In `net.rs` tests near line 1946 ("A backdoor is forever and survives the alarm"):
the test calls `net.land(.., Op::Backdoor, ..)` and asserts `net.backdoors.contains`.
Change it to assert through hacks only: after `land` with `Op::Backdoor`, assert
`net.hacks.iter().any(|h| h.op == Op::Backdoor)` still holds after `net.alarm(b(1), now)`,
and drop both `backdoors.contains` asserts. Any `sync_guards`/`minute` calls in
`net.rs` tests gain a last argument `&BTreeSet::new()` (or `&[b(1)].into_iter().collect()`
where the test wants the gate guard down).

In `credentials.rs` test at line 1077, change `EntryKind::Backdoor` to
`EntryKind::Backdoor(_)` and the expected text to start with
`"backdoor · root on `{name}` · planted day"`. Also, since `land` on `NetState`
no longer plants, that test must call `st.backdoors.plant_net(b, 1, seed, now)`
where `seed = st.city.as_ref().unwrap().seed` right after the `st.net.land(...)`
call (keep the `land` so the hack is still logged).

In `hosts.rs` line 1061-1062 (`st.net.backdoors.insert(b)`), replace with
`st.backdoors.plant_net(b, 1, 0, 0);`.

In `bin.rs` test `programs_keep_the_old_voice` after `st.net.land(.., Op::Backdoor, ..)`
add `st.backdoors.plant_net(b, 1, 0, now);`.

In `explore.rs` test `copy_backdoor_and_awake_guards` (line 5267):
`assert!(st.net.backdoors.contains(&b))` becomes `assert!(st.backdoors.root_on(b))`.
Every `st.net.minute(now, inside, &mut st.constructs)` in explore tests gains
`, &st.backdoors.roots()` as a fourth argument (there are about eight; `grep -n
"\.minute(" crates/strata-zero/src/screens/explore.rs`). Note the borrow: write
`let roots = st.backdoors.roots(); st.net.minute(now, inside, &mut st.constructs, &roots);`.

Add a new test in `access.rs`'s test module (find `mod tests` there) pinning
Review Focus 2:

```rust
    #[test]
    fn a_key_keeps_the_tier_and_the_user_after_the_card_is_gone() {
        let (mut st, b) = state_with_resident();
        // (use the module's existing fixture that gives a resident's keycard; if
        // there is none, build one as bin.rs's `devices_read_the_building…` does:
        // prepare(b), take the first household, give `keycard_id(b, h)`)
        let user = crate::helix::hosts::user_for(&st, HostId::Building(b));
        assert_ne!(user, "guest");
        st.backdoors.plant_key(b, &user, 100, 0, 0);
        st.character.inventory.retain(|s| crate::credentials::parse_keycard(&s.item).is_none());
        assert_eq!(tier_on(&st, b), Tier::User);
        assert_eq!(crate::helix::hosts::user_for(&st, HostId::Building(b)), user);
        st.backdoors.burn(b, crate::backdoors::Why::You, 1);
        assert_eq!(tier_on(&st, b), Tier::Guest);
        st.backdoors.plant_net(b, 1, 0, 2);
        assert_eq!(tier_on(&st, b), Tier::Admin);
        assert_eq!(crate::helix::hosts::user_for(&st, HostId::Building(b)), "root");
    }
```

- [ ] **Step 2: Run the suite to see the compile failures**

Run: `cargo test -p strata-zero 2>&1 | grep -E "^error" | head`
Expected: errors about `backdoors` on `NetState`, `EntryKind::Backdoor(_)`, argument counts.

- [ ] **Step 3: Remove the set and thread the roots**

In `net.rs`:
- Delete `pub backdoors: BTreeSet<BuildingId>,` and its `Default` line.
- `land`: delete the `if op == Op::Backdoor { self.backdoors.insert(b); }` block; keep `until = None` for backdoors.
- `sync_guards` gains `roots: &BTreeSet<BuildingId>`; `let backdoor = roots.contains(&b);`.
- `minute` gains `roots: &BTreeSet<BuildingId>`; `let backdoor = roots.contains(b);` and pass `roots` to `sync_guards`.
- Every internal caller of `sync_guards` (in `land` the app fn, `step_run`'s attack arm, `raise_alarm`) passes `&st.backdoors.roots()`; bind it to a local first when `st` is borrowed mutably in the same expression.
- The doc comment on `NetState` and the module header drop "the backdoors".

In `net.rs`'s app `land` function (line 647), add a last parameter `rating: u8`;
`step_run` passes `spec.rating`, any other caller (grep `land(st,`) passes `1`.
The `(Op::Backdoor, _)` arm becomes:

```rust
        (Op::Backdoor, _) => {
            let seed = st.city.as_ref().map_or(0, |c| c.seed);
            st.backdoors.plant_net(b, rating, seed, now);
            format!(
                "backdoor planted · `{name}`'s gate is yours · {} of {}",
                st.backdoors.slots_used(),
                crate::backdoors::SLOTS
            )
        }
```

Add near `start_run`:

```rust
/// Why a Backdoor program may not be planted here, in the shell's words.
pub fn plant_net_refusal(st: &AppState, b: BuildingId) -> Option<String> {
    let now = st.net.now(st);
    st.backdoors.can_plant(b, now).err().map(|r| r.words())
}
```

In `start_run`, after the `copied already` check:

```rust
    if spec.op == Op::Backdoor
        && let Some(why) = plant_net_refusal(st, b)
    {
        return Err(why);
    }
```

In `explore.rs` `run_choices`, after the `copied` disabled row, add the same for
backdoors:

```rust
                if spec.op == game::Op::Backdoor
                    && let Some(why) = crate::net::plant_net_refusal(st, b)
                {
                    choices.push(Choice::disabled(key, format!("{label}{on} · {why}")));
                    continue;
                }
```

In `explore.rs:1939`: `let roots = st.backdoors.roots(); st.net.minute(now, inside, &mut st.constructs, &roots);`.

In `access.rs` `tier_on`:

```rust
pub fn tier_on(st: &AppState, b: BuildingId) -> Tier {
    if st.backdoors.root_on(b) {
        return Tier::Admin;
    }
    let key = st
        .backdoors
        .live(b)
        .is_some_and(|d| matches!(d.kind, crate::backdoors::Kind::Key { .. }));
    let card = …; let code = …;   // as today
    if key || card || code { Tier::User } else { Tier::Guest }
}
```

In `hosts.rs` `user_for`, the `Tier::User` arm first checks for a key:

```rust
            Tier::User => {
                if let Some(crate::backdoors::Kind::Key { user }) =
                    st.backdoors.live(b).map(|d| d.kind.clone())
                {
                    return user;
                }
                … // card, coded as today
            }
```

In `credentials.rs`: `EntryKind::Backdoor` becomes `Backdoor(usize)`; replace the
`for b in &st.net.backdoors` loop with:

```rust
    for (i, d) in st.backdoors.list.iter().enumerate() {
        let name = crate::net::building_name(st, d.building);
        let who = match &d.kind {
            crate::backdoors::Kind::Net { .. } => format!("backdoor · root on `{name}`"),
            crate::backdoors::Kind::Key { user } => {
                let display = st
                    .helix
                    .hosts
                    .get(&crate::helix::shell::HostId::Building(d.building))
                    .and_then(|h| h.users.iter().find(|u| u.name == *user))
                    .map(|u| u.display.clone())
                    .unwrap_or_else(|| user.clone());
                format!("key · `{display}`'s login on `{name}`")
            }
        };
        let tail = match d.state {
            crate::backdoors::State::Burned { at, why } => {
                format!("BURNED {} · {}", stamp(at), why.label())
            }
            _ => format!(
                "planted {} · {}",
                stamp(d.planted),
                d.health(st.net.now(st)).label()
            ),
        };
        by.entry(d.building).or_default().push(Entry {
            building: d.building,
            kind: EntryKind::Backdoor(i),
            text: format!("{who} · {tail}"),
            at: Some(d.planted),
        });
    }
```

In `deck.rs:619`, `EntryKind::Backdoor(i)` pushes: the kind (`"a key" / "a Net backdoor"`),
`format!("login as {}", d.user_label())`, `format!("health {}", d.health(now).label())`,
and for a burned one `format!("burned · {}", why.label())`, reading `st.backdoors.list[*i]`.

- [ ] **Step 4: Run the suite**

Run: `cargo test -p strata-zero 2>&1 | grep -E "test result|FAILED|panicked" | head`
Expected: all green (436 + the new test).

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets -p strata-zero 2>&1 | tail -3
git add -A crates/strata-zero/src
git commit -m "feat(app): the Net reads backdoors from their module, with a cap and a journal line"
```

---

### Task 3: The `backdoor` program, the key file and `ssh` through a backdoor

**Files:**
- Modify: `crates/strata-zero/src/helix/bin.rs:20-23` (`PROGRAMS`), `bin.rs:125-182` (`ssh`), `bin.rs:300-337` (`program`), `bin.rs:349-351` (`on_read`; add `removed`)
- Modify: `crates/strata-zero/src/helix/shell.rs:51-77` (`Io::removed`), `shell.rs:695-716` (`rm`), `shell.rs:880-1044` (`PAGES`)
- Modify: `crates/strata-zero/src/access.rs:126-160` (`log_lines`)
- Modify: `crates/strata-zero/src/helix/hosts.rs` (a `logins: Vec<(u64, String)>` on `Host` is **not** needed: log lines come from `st.backdoors` and a new `st.net.logins`? No: keep it simple, see Step 3)

**Interfaces:**
- Consumes Task 1 and 2.
- Produces: programs `backdoor`; `Io::removed(&mut self, host: HostId, path: &str)`
  default no-op; `AppIo::key_path(user) -> String` (`/home/<u>/.ssh/authorized_keys`,
  `/root/.ssh/authorized_keys` for root); `access::log_lines` audit/login lines;
  `AppIo` records logins through a backdoor in `st.backdoors.logins: Vec<(u64, BuildingId, String)>`
  (add this `pub` field to `Backdoors`, default empty; it is display only).

- [ ] **Step 1: Write the failing shell tests** in `bin.rs`'s test module

```rust
    #[test]
    fn backdoor_plants_a_key_as_a_resident_and_refuses_in_order() {
        let (mut st, b, d) = state();
        // guest: denied
        let mut sh = new_shell(&mut st, HostId::Building(b));
        let out = go(&mut st, &mut sh, "backdoor");
        assert!(out.denied && text(&out) == "backdoor: Permission denied", "{out:?}");
        // a district host has no server
        let mut dh = new_shell(&mut st, HostId::District(d));
        assert_eq!(text(&go(&mut st, &mut dh, "backdoor")), "backdoor: no server here");
        // a resident plants
        crate::credentials::copy_codes(&mut st, b, 1);
        let mut sh = new_shell(&mut st, HostId::Building(b));
        let me = sh.user.clone();
        let out = go(&mut st, &mut sh, "backdoor");
        assert_eq!(
            text(&out),
            format!("KEY PLANTED · {me}@{} · the rig holds 1 of 3", hostname(&st, HostId::Building(b)))
        );
        let key = text(&go(&mut st, &mut sh, "cat ~/.ssh/authorized_keys"));
        assert!(key.starts_with("helix-ed25519 ") && key.ends_with("@zenbo"), "{key}");
        let ls = text(&go(&mut st, &mut sh, "ls -l ~/.ssh"));
        assert!(ls.contains(&format!("rw---- {me:<7}")) || ls.contains(&me), "{ls}");
        assert!(st.backdoors.held(b).is_some());
        assert!(st.net.traces.get(&b).is_some_and(|t| t.heat >= crate::backdoors::PLANT_NOISE));
        // again: already here
        assert_eq!(text(&go(&mut st, &mut sh, "backdoor")), "backdoor: already planted here");
        // fill the rig, then the cap
        st.backdoors.plant_net(citygen::BuildingId::new(9000), 1, 0, 0);
        st.backdoors.plant_net(citygen::BuildingId::new(9001), 1, 0, 0);
        st.backdoors.burn(b, crate::backdoors::Why::Decay, 0);
        let out = go(&mut st, &mut sh, "backdoor");
        assert_eq!(text(&out), "backdoor: the rig holds 3 lines · burn one first");
        assert_eq!(out.status, 1);
        // patched beats full; seized beats patched
        st.backdoors.burn(citygen::BuildingId::new(9000), crate::backdoors::Why::You, 0);
        st.backdoors.patched.insert(b, 10_000);
        assert_eq!(text(&go(&mut st, &mut sh, "backdoor")), "backdoor: hole patched · try again tomorrow");
        st.backdoors.rig_seized_until = Some(10_000);
        assert_eq!(text(&go(&mut st, &mut sh, "backdoor")), "backdoor: the rig is seized · no line home");
        // man knows it
        assert!(text(&go(&mut st, &mut sh, "man backdoor")).starts_with("backdoor - "));
    }

    #[test]
    fn rm_of_the_key_file_burns_the_key_once() {
        let (mut st, b, _) = state();
        crate::credentials::copy_codes(&mut st, b, 1);
        let mut sh = new_shell(&mut st, HostId::Building(b));
        go(&mut st, &mut sh, "backdoor");
        assert!(st.backdoors.held(b).is_some());
        assert_eq!(go(&mut st, &mut sh, "rm ~/.ssh/authorized_keys").status, 0);
        assert!(st.backdoors.held(b).is_none());
        assert!(matches!(
            st.backdoors.list[0].state,
            crate::backdoors::State::Burned { why: crate::backdoors::Why::You, .. }
        ));
        // the directory may go too, and nothing double-burns
        assert_eq!(go(&mut st, &mut sh, "rm -r ~/.ssh").status, 0);
        assert_eq!(st.backdoors.list.len(), 1);
        // patched: planting again is refused
        assert_eq!(text(&go(&mut st, &mut sh, "backdoor")), "backdoor: hole patched · try again tomorrow");
    }

    #[test]
    fn ssh_rides_a_key_from_the_rig_and_is_refused_once_it_burns() {
        let mut st = state_home();
        let home = st.home.unwrap().home.building;
        // a key on the home block as a resident other than you: use copy_codes then plant as that user
        crate::credentials::copy_codes(&mut st, home, 1);
        let user = crate::helix::hosts::user_for(&st, HostId::Building(home));
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_key(home, &user, 100, seed, 0);
        // forget the codes, so only the key gets us in
        st.journal = Default::default();
        crate::credentials::move_in(&mut st);
        let mut sh = new_shell(&mut st, HostId::Rig);
        let out = text(&go(&mut st, &mut sh, "ssh 1"));
        assert!(out.ends_with("· via your backdoor"), "{out}");
        assert_eq!(sh.user, user);
        assert_eq!(st.backdoors.held(home).unwrap().spent, crate::backdoors::COST_SSH);
        let log = text(&go(&mut st, &mut sh, "cat /var/log/auth.log"));
        assert!(log.contains(&format!("login  {user} via key")), "{log}");
        go(&mut st, &mut sh, "exit");
        // a device write through the key costs too
        go(&mut st, &mut sh, "ssh 1");
        let before = st.backdoors.held(home).unwrap().spent;
        go(&mut st, &mut sh, "door release");
        assert_eq!(st.backdoors.held(home).unwrap().spent, before + crate::backdoors::COST_WRITE);
        go(&mut st, &mut sh, "exit");
        // burned by time: refused, and the journal knows
        let d = st.backdoors.list.iter_mut().find(|d| d.building == home).unwrap();
        d.spent = d.nominal * 2;
        let out = go(&mut st, &mut sh, "ssh 1");
        assert_eq!(out.status, 1);
        assert_eq!(
            text(&out),
            format!("ssh: {}: connection refused · key rejected", hostname(&st, HostId::Building(home)))
        );
        assert_eq!(sh.host, HostId::Rig);
        assert_eq!(st.backdoors.list[0].health(0), crate::backdoors::Health::Burned);
    }

    #[test]
    fn a_honeypot_logs_you_in_and_rings_the_alarm() {
        let (mut st, b, d) = state();
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_net(b, 1, seed, 0);
        st.backdoors.list[0].state = crate::backdoors::State::Honeypot;
        let mut sh = new_shell(&mut st, HostId::District(d));
        // the clue is in the log before you use it
        let mut peek = new_shell(&mut st, HostId::Building(b));
        let log = text(&go(&mut st, &mut peek, "cat /var/log/auth.log"));
        assert!(log.contains("audit  key flagged · monitoring"), "{log}");
        let n = hostname(&st, HostId::Building(b));
        let out = text(&go(&mut st, &mut sh, &format!("ssh {n}")));
        assert!(out.contains("AUDIT · THIS SESSION IS BEING RECORDED"), "{out}");
        assert_eq!(sh.user, "root");
        let now = st.net.now(&st);
        assert!(st.net.alert(b, now));
        // from a street terminal no trace runs home
        assert!(st.backdoors.trace.is_none());
        // from the rig it does
        let mut st = state_home();
        let home = st.home.unwrap().home.building;
        let seed = st.city.as_ref().unwrap().seed;
        st.backdoors.plant_net(home, 1, seed, 0);
        st.backdoors.list[0].state = crate::backdoors::State::Honeypot;
        let mut sh = new_shell(&mut st, HostId::Rig);
        go(&mut st, &mut sh, "ssh 1");
        assert!(st.backdoors.trace.is_some());
        assert!(st.backdoors.trace.as_ref().unwrap().relays.is_empty());
    }
```

The `state()` fixture's building `b` is a residential building with a server;
`copy_codes(&mut st, b, 1)` learns its codes so `new_shell` logs in as a resident
(as `programs_keep_the_old_voice` does). `state_home()` has a home; `ssh 1` from
the rig reaches the home block.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero helix::bin::tests::backdoor 2>&1 | tail -5`
Expected: FAIL (`backdoor: command not found` etc.).

- [ ] **Step 3: Implement**

`shell.rs`: add to `Io`:

```rust
    /// Called by `rm` after it removed something: absolute path.
    fn removed(&mut self, _host: HostId, _path: &str) {}
```

In `rm`, on `Ok(())` after a successful `fs.remove`, compute the absolute path
before removing (`let abs = Vfs::normalize(&me.cwd, &me.home, path);` — `normalize`
is an associated fn used by `write`; make it `pub(super)` if it is private) and
call `io.removed(me.host, &abs)` in the `Ok(()) => {}` arm. Add three man pages
to `PAGES`:

```rust
    (
        "backdoor",
        "plant a key on this host",
        "backdoor writes your rig's key into this user's ~/.ssh/authorized_keys, so you can ssh in as them from home or any terminal for a while, and your rig can put you on the Net at this building's gate. The rig keeps three lines at most. rm the file to burn it.",
    ),
    (
        "burn",
        "burn one of your backdoors",
        "burn <host> destroys a backdoor of yours by its /net name, a prefix or its number. The hole is patched for a day. If the trace is sitting on it, the trace dies there and the building goes on alert.",
    ),
    (
        "route",
        "your route and the trace",
        "route prints the chain of backdoors between the rig and you while you are jacked in from home, each with its health, and the trace walking it back when there is one. Rig only.",
    ),
```

`bin.rs`:

- `PROGRAMS` gains `"backdoor", "burn", "route"` (`burn` and `route` are
  implemented in Task 4; register them now and answer `Err("burn: rig only")` /
  `Err("route: rig only")` until then).
- Helper:

```rust
fn key_path(user: &str) -> String {
    if user == ROOT {
        "/root/.ssh/authorized_keys".to_string()
    } else {
        format!("/home/{user}/.ssh/authorized_keys")
    }
}

fn key_line(st: &AppState) -> String {
    let seed = st.city.as_ref().map_or(0, |c| c.seed);
    let me = hosts::rig_user(st);   // make `rig_user` pub(super) in hosts.rs
    format!("helix-ed25519 {:016x} {me}@zenbo", citygen::rng::hash64(seed, 0x6B65))
}
```

- The program:

```rust
    fn backdoor(&mut self, shell: &Shell) -> Result<Output, String> {
        let HostId::Building(b) = shell.host else {
            return Err("backdoor: no server here".to_string());
        };
        if shell.user == GUEST {
            return Ok(Output { lines: vec!["backdoor: Permission denied".into()], status: 1, denied: true, ..Default::default() });
        }
        let now = self.st.net.now(self.st);
        if let Err(r) = self.st.backdoors.can_plant(b, now) {
            return Err(format!("backdoor: {}", r.words()));
        }
        let path = key_path(&shell.user);
        let line = key_line(self.st);
        let security = access::security_of(self.st, b);   // make `security` in access.rs pub as `security_of(st, b) -> u8`
        let seed = self.st.city.as_ref().map_or(0, |c| c.seed);
        {
            let fs = self.fs(shell.host);
            let dir = path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            if fs.resolve("/", "/", &dir).is_err() {
                fs.mkdir_as(&dir, &shell.user, Mode::RW_NONE);
            }
            let existing = match fs.read(ROOT, "/", "/", &path) {
                Ok(Kind::File(t)) => t.clone(),
                _ => String::new(),
            };
            fs.put(&path, &format!("{existing}{line}\n"), &shell.user, Mode::RW_NONE);
        }
        self.st.backdoors.plant_key(b, &shell.user, security, seed, now);
        let (event, _) = self.st.net.add_heat(b, crate::backdoors::PLANT_NOISE, now, false);
        if event == game::TraceEvent::Alarm {
            let line = crate::net::raise_alarm(self.st, b);
            self.st.warn(line);
        }
        Ok(one(format!(
            "KEY PLANTED · {}@{} · the rig holds {} of {}",
            shell.user,
            hostname(self.st, shell.host),
            self.st.backdoors.slots_used(),
            crate::backdoors::SLOTS
        )))
    }
```

(`Kind` and `Mode` come from `super::vfs`; `GUEST`/`ROOT` consts exist there.)

- `removed`: in `impl Io for AppIo`:

```rust
    fn removed(&mut self, host: HostId, path: &str) {
        let HostId::Building(b) = host else { return };
        let Some(d) = self.st.backdoors.held(b) else { return };
        let crate::backdoors::Kind::Key { user } = &d.kind else { return };
        let key = key_path(user);
        let dir = key.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        if path == key || path == dir {
            let now = self.st.net.now(self.st);
            self.st.backdoors.burn(b, crate::backdoors::Why::You, now);
        }
    }
```

- `ssh`: after the host is picked (`[i] => { let b = list[*i]; …`), before
  `login`:

```rust
                let via = self.st.backdoors.held(b).is_some();
                let mut audit = false;
                if via {
                    let now = self.st.net.now(self.st);
                    let seed = self.st.city.as_ref().map_or(0, |c| c.seed);
                    let security = access::security_of(self.st, b);
                    match self.st.backdoors.check(b, security, seed, now) {
                        crate::backdoors::Use::Refused => {
                            return Err(format!("ssh: {}: connection refused · key rejected", hostname(self.st, HostId::Building(b))));
                        }
                        crate::backdoors::Use::Trap => {
                            audit = true;
                            let line = crate::net::raise_alarm(self.st, b);
                            self.st.warn(line);
                            let _ = self.st.net.add_heat(b, crate::backdoors::HONEYPOT_NOISE, now, false);
                            if shell.origin == HostId::Rig {
                                self.st.backdoors.start_route(None);
                                self.st.backdoors.start_trace(now);
                                self.st.backdoors.end_route();
                                self.st.warn("honeypot · they were waiting · trace running");
                            }
                        }
                        crate::backdoors::Use::Works => {
                            self.st.backdoors.spend(b, crate::backdoors::COST_SSH);
                        }
                    }
                }
                login(self.st, shell, HostId::Building(b));
                if via {
                    let now = self.st.net.now(self.st);
                    self.st.backdoors.logins.push((now, b, shell.user.clone()));
                }
                let mut lines = vec![format!("CONNECTED TO {} · CLEARANCE {}{}", building_label(self.st, b), tier_on(self.st, b).label(), if via { " · via your backdoor" } else { "" })];
                if audit { lines.push("AUDIT · THIS SESSION IS BEING RECORDED".to_string()); }
                Ok(Output { lines, ..Output::default() })
```

Note the `Trap` case: the backdoor is burned by `check`, so `login` falls back
to whatever standing you have otherwise; a honeypot as root should still log you
in as root for the session. To keep that, in `user_for` (hosts.rs) treat
`st.backdoors.list.iter().rev().find(|d| d.building == b && matches!(d.state, Burned{why: Honeypot, at} if at == now))`
as live for this one minute. Simpler: record `self.st.backdoors.trap_login: Option<(BuildingId, String)>`
set by `ssh` before `login` to the burned backdoor's `user_label()` and read first
by `user_for`, cleared on `exit`/`login` to another host. Add that `pub trap_login`
field to `Backdoors` and clear it in `login()` after it is read.

- Device writes through a backdoor: in `AppIo::act` and `write_dev`, after a
  successful action, `if self.st.backdoors.logins.iter().any(|(_, lb, _)| *lb == b) && shell/host is that building { self.st.backdoors.spend(b, COST_WRITE) }`.
  Simplest rule: when `shell.host == HostId::Building(b)` and `st.backdoors.live(b)` is `Some` and the shell's user equals the backdoor's `user_label()`, spend `COST_WRITE`. Implement as `fn spend_write(&mut self, host: HostId, user: &str)` called from both.

- `access::log_lines`: before the hack rows, for `b`: if any backdoor on `b` has
  `State::Honeypot`, push `format!("{}  audit  key flagged · monitoring", stamp(now))`;
  then for each `(at, lb, user)` in `st.backdoors.logins` with `lb == b`, newest
  first, `format!("{}  login  {user} via key", stamp(*at))`.

- Make `access::security` public as `pub fn security_of(st: &AppState, b: BuildingId) -> u8`
  (wrapping the existing `security(city, b)`).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero helix:: 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: green, including the four new tests.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets -p strata-zero 2>&1 | tail -3
git add -A crates/strata-zero/src
git commit -m "feat(app): backdoor plants a key in authorized_keys, and ssh rides it"
```

---

### Task 4: The rig's side of HELIX: `/net`, `route`, `burn`, `net.log`, the seized boot

**Files:**
- Modify: `crates/strata-zero/src/helix/hosts.rs:155-178` (`ensure_host`), `hosts.rs:252-300` (`rig_disk`), `helix/mod.rs:24-35` (`Host.nets_placed: BTreeSet<BuildingId>`)
- Modify: `crates/strata-zero/src/helix/bin.rs` (`route`, `burn`, `read_dev` for `NetHost` and `NetLog` on the rig, `net_of` for the rig)
- Modify: `crates/strata-zero/src/access.rs:161-178` (`trace_lines`) and a new `access::line_row(st, b) -> String`
- Modify: `crates/strata-zero/src/screens/terminal.rs:115-143,298-355` (seized boot)

**Interfaces:**
- Consumes Tasks 1–3.
- Produces: `AppIo::net_of(HostId::Rig)` = home block + every not-burned backdoor's
  building (deduped, home first); `access::line_row(st, b) -> Option<String>`
  (`LINE  root  STABLE  planted day 2 03:12`); `access::rig_net_lines(st) -> Vec<String>`
  (route, trace, `LINES n of 3`); `Terminal::seized: bool`.

- [ ] **Step 1: Write the failing tests**

In `bin.rs` tests:

```rust
    #[test]
    fn the_rig_lists_lines_and_burns_them() {
        let mut st = state_home();
        let home = st.home.unwrap().home.building;
        let seed = st.city.as_ref().unwrap().seed;
        // a backdoor on some other scannable building
        let other = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .find(|x| x.id != home && x.kind.is_closed() && crate::access::scannable(&st, x.district).contains(&x.id))
            .map(|x| x.id)
            .unwrap();
        st.backdoors.plant_net(other, 1, seed, 0);
        let mut sh = new_shell(&mut st, HostId::Rig);
        let ls = text(&go(&mut st, &mut sh, "ls /net"));
        let other_name = hostname(&st, HostId::Building(other));
        assert!(ls.contains(&other_name), "{ls}");
        let cat = text(&go(&mut st, &mut sh, &format!("cat /net/{other_name}")));
        assert!(cat.contains("LINE  root  STABLE  planted day"), "{cat}");
        let route = text(&go(&mut st, &mut sh, "route"));
        assert!(route.contains("NO ROUTE · you are home") && route.contains("TRACE  none") && route.contains("LINES  1 of 3"), "{route}");
        let log = text(&go(&mut st, &mut sh, "cat /var/log/net.log"));
        assert_eq!(log, route);
        // burn by prefix
        let out = text(&go(&mut st, &mut sh, &format!("burn {}", &other_name[..3])));
        assert_eq!(out, format!("BURNED · {other_name}"));
        assert!(st.backdoors.held(other).is_none());
        let ls = text(&go(&mut st, &mut sh, "ls /net"));
        assert!(!ls.contains(&other_name), "{ls}");
        let bad = go(&mut st, &mut sh, "burn nope");
        assert_eq!((text(&bad).as_str(), bad.status), ("burn: nope: no such backdoor", 1));
        // a trace on the relay: burning cuts it and the building alarms
        st.backdoors.plant_net(other, 1, seed, 5_000);
        st.backdoors.patched.clear();
        st.backdoors.start_route(Some(other));
        st.backdoors.start_trace(5_000);
        st.backdoors.end_route();
        let route = text(&go(&mut st, &mut sh, "route"));
        assert!(route.contains(&format!("TRACE ▸ {other_name}")), "{route}");
        let out = text(&go(&mut st, &mut sh, &format!("burn {other_name}")));
        assert_eq!(out, format!("BURNED · {other_name} · TRACE CUT"));
        let now = st.net.now(&st);
        assert!(st.net.alert(other, now));
        assert!(st.backdoors.trace.is_none());
        // not on a building host
        let mut bh = new_shell(&mut st, HostId::Building(home));
        assert_eq!(go(&mut st, &mut bh, "route").status, 1);
    }
```

In `terminal.rs` tests (use the fixture its other tests use for the rig, see
`boots_with_motd_and_shows_a_unix_prompt` / line 714 where `t.shell.home` is set):

```rust
    #[test]
    fn a_seized_rig_boots_to_the_warrant_and_takes_no_commands() {
        let mut st = home_state();   // the module's rig fixture
        st.backdoors.rig_seized_until = Some(u64::MAX);
        let key = (st.home.unwrap().place(), st.home.unwrap().home.rig);
        let mut t = Terminal::open(&mut st, HostId::Rig, key);
        let text = t.text(st.tick + 100_000);
        assert!(text.iter().any(|l| l == "EVIDENCE · THIS UNIT IS HELD UNDER WARRANT"), "{text:?}");
        assert!(text.iter().any(|l| l == "CONTACT THE DISTRICT OFFICE"));
        assert!(!drawn(&t, &st).contains("@zenbo"));
        // typing does nothing; Esc leaves
        let cmd = t.update(&mut st, &[key_event(KeyCode::Char('l')), key_event(KeyCode::Enter)]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        assert!(t.text(st.tick + 200_000).iter().all(|l| !l.contains("ls")));
        let cmd = t.update(&mut st, &[key_event(KeyCode::Esc)]);
        assert!(matches!(cmd, ScreenCmd::Pop { .. }));
    }
```

Adapt helper names (`home_state`, `drawn`, `key_event`) to what the module's
tests already define; read the test module's top before writing.

- [ ] **Step 2: Run them to fail**

Run: `cargo test -p strata-zero the_rig_lists_lines a_seized_rig 2>&1 | tail -5`

- [ ] **Step 3: Implement**

`helix/mod.rs`: `Host` gains `pub nets_placed: BTreeSet<BuildingId>` (import
`citygen::BuildingId`, `std::collections::BTreeSet`).

`hosts.rs`:
- `rig_disk`: after the `/net` block, add `fs.mkdir_as("/var/log", &me, Mode::RW_R); fs.put_dev("/var/log/net.log", Dev::NetLog, &me, Mode::RW_R);`.
- `ensure_host`: after the mails block, when `id == HostId::Rig`, sync `/net`:

```rust
    if id == HostId::Rig {
        let want: BTreeSet<BuildingId> = st
            .home
            .map(|h| h.home.building)
            .into_iter()
            .chain(st.backdoors.list.iter().filter(|d| !matches!(d.state, crate::backdoors::State::Burned { .. })).map(|d| d.building))
            .collect();
        let names: Vec<(BuildingId, String)> = want.iter().map(|b| (*b, hostname(st, HostId::Building(*b)))).collect();
        let host = st.helix.hosts.get_mut(&id).expect("the rig");
        for (b, name) in &names {
            if !host.nets_placed.contains(b) {
                host.fs.put_dev(&format!("/net/{name}"), Dev::NetHost(*b), USERS, Mode::RW_R);
                host.nets_placed.insert(*b);
            }
        }
        let gone: Vec<BuildingId> = host.nets_placed.iter().copied().filter(|b| !want.contains(b)).collect();
        for b in gone {
            if let Some((_, name)) = names.iter().find(|(x, _)| *x == b) { let _ = name; }
            let name = hostname_of_cached(b);   // see note
            let _ = host.fs.remove(ROOT, "/", "/", &format!("/net/{name}"), false);
            host.nets_placed.remove(&b);
        }
    }
```

Note: `hostname(st, …)` borrows `st` immutably while `host` borrows `st.helix`
mutably; compute every name you need (both wanted and the ones to remove, i.e.
`nets_placed ∪ want`) into a `Vec<(BuildingId, String)>` *before* taking
`host`. Replace `hostname_of_cached` with a lookup in that vec. Put the whole
block in a `fn sync_rig_net(st: &mut AppState)` called from `ensure_host`.

`bin.rs`:
- `net_of(HostId::Rig)`: home block first, then each not-burned backdoor's
  building not already present, in `list` order.
- `read_dev` for `Dev::NetHost(b)`: after the scan row, when `host == HostId::Rig`,
  push `access::line_row(self.st, *b)` if `Some`.
- `read_dev` for `Dev::NetLog` when `host == HostId::Rig` (there is no block): `Ok(access::rig_net_lines(self.st))`. Move the `Dev::NetLog` arm above the `block(host)` check for the rig.
- `route`: `if shell.host != HostId::Rig { return Err("route: rig only") }`; `Ok(lines(access::rig_net_lines(self.st)))`.
- `burn <what>`: rig only (`burn: rig only`); resolve `what` against the rig's
  `net_of` names the way `ssh` does (number, exact, prefix; skip the home block
  unless it holds a backdoor); no match → `burn: {what}: no such backdoor`;
  then `let r = st.backdoors.burn(b, Why::You, now)`; reply `BURNED · {name}` plus
  ` · TRACE CUT` and `raise_alarm(b)` (warn the line) when `r.cut_trace`.

`access.rs`:

```rust
/// The rig's line on a backdoored host: `LINE  root  STABLE  planted day 2 03:12`.
pub fn line_row(st: &AppState, b: BuildingId) -> Option<String> {
    let d = st.backdoors.list.iter().rev().find(|d| d.building == b)?;
    let now = st.net.now(st);
    Some(format!(
        "LINE  {:<10} {:<14} planted {}",
        d.user_label(),
        d.health(now).label(),
        credentials::stamp(d.planted)
    ))
}

/// What `route` and the rig's net.log say: the route, the trace, the lines held.
pub fn rig_net_lines(st: &AppState) -> Vec<String> {
    let now = st.net.now(st);
    let name = |b: BuildingId| crate::helix::hosts::hostname(st, crate::helix::shell::HostId::Building(b));
    let mut out = Vec::new();
    match st.backdoors.route.as_ref() {
        Some(r) => {
            let names: Vec<String> = r.relays.iter().map(|b| name(*b)).collect();
            out.push(format!("ROUTE  {}", crate::backdoors::route_line(&names)));
            for b in &r.relays {
                if let Some(d) = st.backdoors.list.iter().rev().find(|d| d.building == *b) {
                    out.push(format!("  {:<16} {:<10} {}", name(*b), d.user_label(), d.health(now).label()));
                }
            }
        }
        None => out.push("NO ROUTE · you are home".to_string()),
    }
    out.push(st.backdoors.trace_line(now, &name).unwrap_or_else(|| "TRACE  none".to_string()));
    out.push(format!("LINES  {} of {}", st.backdoors.slots_used(), crate::backdoors::SLOTS));
    out
}
```

`terminal.rs`: `Terminal` gains `seized: bool`. In `with_look`, when
`origin == HostId::Rig && st.backdoors.seized(st.net.now(st))`: `seized = true`,
`typing = Some(Typing::new(vec!["EVIDENCE · THIS UNIT IS HELD UNDER WARRANT".into(), "CONTACT THE DISTRICT OFFICE".into()], st.tick, look.style.cps))` and the
boot/motd are skipped. In `update`, after the typing block: `if self.seized { return if keys.iter().any(|k| k.code == KeyCode::Esc) { pop() } else { ScreenCmd::Stay }; }`.
In `draw`, when `self.seized` push no prompt row (`tail` stays empty).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero helix:: screens::terminal:: 2>&1 | grep -E "test result|FAILED|panicked"`

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets -p strata-zero 2>&1 | tail -3
git add -A crates/strata-zero/src
git commit -m "feat(app): the rig lists its lines, burns them, and boots to a warrant when seized"
```

---

### Task 5: Landing and hopping on the Net

**Files:**
- Modify: `crates/strata-zero/src/screens/popup.rs:19-57` (`WorldAct::Land`, `Pick::Hop`), `popup.rs:160-220` (`world`), `popup.rs:300-345` (`fire`)
- Modify: `crates/strata-zero/src/screens/explore.rs:743-746` (the jack row), `explore.rs:941-995` (`run_choices` hop rows), `explore.rs:1721-1760` (Enter jack: route start)
- Modify: `crates/strata-zero/src/net.rs:845-874` (`dump_for`, `jack_out`), `net.rs:1709-1726` (`jack_in_refusal`), new `land_at`, `hop`, `landing_choices`

**Interfaces:**
- Consumes Tasks 1–4.
- Produces: `net::land_at(st, b) -> Result<String, String>`, `net::hop(st, b) -> Result<String, String>`,
  `net::landings(st) -> Vec<(BuildingId, String)>` (not-burned backdoors with
  their row text `Hollis Court · root · STABLE`), `net::after_jack_in(st, from: Place)`
  (starts the route when `from` is the home floor), `WorldAct::Land(BuildingId)`,
  `Pick::Hop(BuildingId)`.

- [ ] **Step 1: Write the failing tests** in `explore.rs`'s test module (use its
existing home fixture: grep `fn home_state\|HomeRig::find` in the tests; the
test `the_rig_jacks_in…` near line 3337/3906 shows how a player is put in the flat
and jacks in):

```rust
    #[test]
    fn the_rig_offers_landings_and_lands_at_a_gate() {
        let (mut st, mut screen) = home_screen();   // player in the flat beside the rig
        let home = st.home.unwrap();
        let seed = st.city.as_ref().unwrap().seed;
        let other = other_construct(&st, home.home.building);
        st.backdoors.plant_net(other, 1, seed, 0);
        // Space: jack in opens the landing list
        let cmd = press(&mut screen, &mut st, ' ');
        let popup = pushed(cmd);
        fire_row(&mut st, popup, "jack in");       // fires Pick::World(WorldAct::JackIn)
        // a second popup: the landings
        let text = draw_screen(&screen, &st, 120, 40);
        assert!(text.contains("JACK IN") && text.contains("your block"), "{text}");
        let row = format!("{} · root · STABLE", crate::net::building_name(&st, other));
        assert!(text.contains(&row), "{text}");
        // land
        crate::net::land_at(&mut st, other).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Stratum(StratumKind::Net));
        assert_eq!(p.pos, crate::constructs::city_gate(st.city.as_ref().unwrap(), other).unwrap());
        assert_eq!(p.body.map(|(pl, _)| pl), Some(home.place()));
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![other]);
        assert_eq!(st.backdoors.held(other).unwrap().spent, crate::backdoors::COST_LAND_NET);
        assert!(last(&st).contains("via your backdoor"));
        // jacking out ends the route
        crate::net::jack_out(&mut st).unwrap();
        assert!(st.backdoors.route.is_none());
        // a burned one is refused and the journal learns it
        let d = st.backdoors.list.iter_mut().find(|d| d.building == other).unwrap();
        d.spent = d.nominal * 2;
        assert_eq!(crate::net::land_at(&mut st, other), Err("connection refused · key rejected".to_string()));
        assert!(st.player.as_ref().unwrap().body.is_none());
        assert_eq!(st.backdoors.list[0].health(0), crate::backdoors::Health::Burned);
    }

    #[test]
    fn jacking_in_at_your_block_from_the_rig_starts_an_empty_route() {
        let (mut st, mut screen) = home_screen();
        jack_in_from_rig(&mut st, &mut screen);   // via the Space row or Enter, as the module's tests do
        assert_eq!(st.backdoors.route, Some(crate::backdoors::Route::default()));
        // the same at a street port starts none
        let (mut st, mut screen) = street_screen_by_a_terminal();
        jack_in_at_port(&mut st, &mut screen);
        assert!(st.backdoors.route.is_none());
    }

    #[test]
    fn a_hop_from_the_server_re_lands_at_another_gate() {
        let (mut st, _screen) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let home = st.home.unwrap().home.building;
        let a = other_construct(&st, home);
        let b2 = second_construct(&st, home, a);
        st.backdoors.plant_net(a, 1, seed, 0);
        st.backdoors.plant_net(b2, 1, seed, 0);
        crate::net::land_at(&mut st, a).unwrap();
        crate::constructs::enter_construct(&mut st, a).unwrap();
        // stand on the server
        let server = server_tile(&st, a);
        st.player.as_mut().unwrap().pos = server;
        let rows = MapScreen::run_choices(&st, a, server, None, &mut WORLD_LETTERS.chars());
        let hop = rows.iter().find(|c| c.label.starts_with("hop · ")).expect("a hop row");
        assert!(hop.label.contains(&crate::net::building_name(&st, b2)) && hop.enabled, "{}", hop.label);
        crate::net::hop(&mut st, b2).unwrap();
        let p = st.player.as_ref().unwrap();
        assert_eq!(p.place, Place::Stratum(StratumKind::Net));
        assert_eq!(p.pos, crate::constructs::city_gate(st.city.as_ref().unwrap(), b2).unwrap());
        assert_eq!(st.backdoors.route.as_ref().unwrap().relays, vec![a, b2]);
        // away from the server: no hop rows
        crate::constructs::enter_construct(&mut st, b2).unwrap();
        let entry = st.constructs.get(b2).unwrap().entry;
        let rows = MapScreen::run_choices(&st, b2, entry, None, &mut WORLD_LETTERS.chars());
        assert!(rows.iter().all(|c| !c.label.starts_with("hop")));
        // on a street run the rows are greyed
        crate::net::jack_out(&mut st).unwrap();
        st.backdoors.route = None;
        st.player.as_mut().unwrap().place = Place::Construct(a);
        st.player.as_mut().unwrap().pos = server;
        let rows = MapScreen::run_choices(&st, a, server, None, &mut WORLD_LETTERS.chars());
        let hop = rows.iter().find(|c| c.label.starts_with("hop · ")).unwrap();
        assert!(!hop.enabled && hop.label.ends_with("no route home"), "{}", hop.label);
    }

    #[test]
    fn a_dump_on_a_rig_run_starts_the_trace_and_a_street_dump_does_not() {
        let (mut st, _screen) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let a = other_construct(&st, st.home.unwrap().home.building);
        st.backdoors.plant_net(a, 1, seed, 0);
        crate::net::land_at(&mut st, a).unwrap();
        crate::net::dump(&mut st, "the trace found you");
        assert!(st.backdoors.route.is_none());
        let t = st.backdoors.trace.as_ref().expect("a trace");
        assert_eq!(t.relays, vec![a]);
        assert!(last(&st).contains("trace running"));
        // street
        let (mut st, mut screen) = street_screen_by_a_terminal();
        jack_in_at_port(&mut st, &mut screen);
        crate::net::dump(&mut st, "the trace found you");
        assert!(st.backdoors.trace.is_none());
    }

    #[test]
    fn a_seized_rig_refuses_to_jack_in() {
        let (mut st, _screen) = home_screen();
        st.backdoors.rig_seized_until = Some(u64::MAX);
        assert_eq!(crate::net::jack_in_refusal(&st), Some("the rig is gone · evidence".to_string()));
    }
```

Helpers `home_screen`, `other_construct`, `second_construct`, `server_tile`,
`press`, `pushed`, `fire_row`, `jack_in_from_rig`, `street_screen_by_a_terminal`,
`jack_in_at_port`, `last`, `draw_screen` are to be written or reused from the
module's existing helpers (several exist: `draw_screen`, `last`; the rig jack-in
test near line 3906 shows the Space rows). `other_construct(st, home)` picks a
scannable closed building with a construct (`st.constructs.ensure` succeeds)
that is not `home`; `second_construct` a different one. `server_tile` is
`st.constructs.get(a).nodes.iter().find(kind == Server).1`. `run_choices` is a
private associated fn: the tests live in the same module, so they can call it.

- [ ] **Step 2: Run them to fail**

- [ ] **Step 3: Implement**

`popup.rs`: `WorldAct::Land(citygen::BuildingId)`; `Pick::Hop(citygen::BuildingId)`.
In `world()`, before the `jack_in_refusal` check:

```rust
        if let WorldAct::Land(b) = act {
            return match crate::net::land_at(st, b) {
                Ok(line) => { st.say(line); TransitionKind::Glitch }
                Err(e) => { st.warn(e); TransitionKind::None }
            };
        }
        if act == WorldAct::JackIn && crate::net::jack_in_refusal(st).is_none() {
            let landings = crate::net::landings(st);
            if !landings.is_empty() && st.home.is_some_and(|h| st.player.as_ref().is_some_and(|p| p.place == h.place())) {
                // the popup replaces itself with the landing list
                self.replace_with = Some(crate::net::landing_popup(st, landings));
                return TransitionKind::None;
            }
        }
```

`Popup` cannot return a `Replace` from `world()` (it returns a `TransitionKind`),
so instead handle it in `fire()`: before `Pick::World(act) => …`, add
`Pick::World(WorldAct::JackIn) if crate::net::wants_landing_list(st) => ScreenCmd::Replace { to: Box::new(crate::net::landing_popup(st)), fx: TransitionKind::None }`.
Where `wants_landing_list(st)` = no refusal, player in the flat (`home.place()`),
and `landings(st)` non-empty. And `landing_popup(st) -> Popup`:

```rust
pub fn landing_popup(st: &AppState) -> crate::screens::popup::Popup {
    use crate::screens::popup::{Choice, Pick, Popup, WorldAct};
    let mut choices = vec![Choice::new('a', "your block", Pick::World(WorldAct::JackIn))];
    let mut letters = "bcdefgimnoprsuvwxyz".chars();
    for (b, label) in landings(st) {
        let Some(k) = letters.next() else { break };
        choices.push(Choice::new(k, label, Pick::World(WorldAct::Land(b))));
    }
    Popup::verbs("JACK IN", choices, st.tick)
}
```

`Popup::verbs` is `pub fn verbs(title: &str, choices: Vec<Choice>, tick: u64) -> Popup` (popup.rs:132). Firing `your block` goes through the existing `WorldAct::JackIn` path (no list the second time because `fire` on the landing popup must not loop: `wants_landing_list` also requires `self.title != "JACK IN"`; simplest: give `Popup` a `pub fn is_landing(&self) -> bool` checking the title, and `fire` checks `!self.is_landing()`).

In `net.rs`:

```rust
/// The not-burned backdoors as landing rows: `Hollis Court · root · STABLE`.
pub fn landings(st: &AppState) -> Vec<(BuildingId, String)> {
    let now = st.net.now(st);
    st.backdoors
        .list
        .iter()
        .filter(|d| !matches!(d.state, crate::backdoors::State::Burned { .. }))
        .map(|d| (d.building, format!("{} · {} · {}", building_name(st, d.building), d.user_label(), d.health(now).label())))
        .collect()
}

/// A rig run begins: an empty route from your block, or one relay when you landed
/// through a backdoor. A street jack-in leaves the route alone.
pub fn after_jack_in(st: &mut AppState, from: Place, via: Option<BuildingId>) {
    if st.home.is_some_and(|h| h.place() == from) {
        st.backdoors.start_route(via);
    }
}

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
    let line = raise_alarm(st, b);
    st.warn(line);
    let _ = st.net.add_heat(b, crate::backdoors::HONEYPOT_NOISE, now, false);
    st.backdoors.start_trace(now);
    st.warn("honeypot · they were waiting · trace running");
}

/// Jack in from the rig and land at a backdoor's gate.
pub fn land_at(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    if let Some(why) = jack_in_refusal(st) {
        return Err(why);
    }
    let trap = use_backdoor(st, b)?;
    let gate = st.city.as_ref().and_then(|c| crate::constructs::city_gate(c, b)).ok_or("that gate is gone")?;
    let bonus = st.sight_bonus();
    let home = st.home;
    let city = st.city.as_ref().ok_or("no city")?;
    let player = st.player.as_mut().ok_or("no body")?;
    let from = player.place;
    player.jack_in(city, home.as_ref(), bonus).map_err(|e| e.to_string())?;
    player.pos = gate;
    player.update_fov(&MapRef::stratum_of(city, StratumKind::Net), bonus);
    after_jack_in(st, from, Some(b));
    st.repaint = true;
    if trap {
        spring(st, b);
    }
    Ok(format!("jacked in · `{}`'s gate · via your backdoor", building_name(st, b)))
}

/// From the server of a construct you hold, onto another backdoor's gate.
pub fn hop(st: &mut AppState, b: BuildingId) -> Result<String, String> {
    if st.backdoors.route.is_none() {
        return Err("no route home".to_string());
    }
    let trap = use_backdoor(st, b)?;
    let gate = st.city.as_ref().and_then(|c| crate::constructs::city_gate(c, b)).ok_or("that gate is gone")?;
    if st.net.run.take().is_some() {
        st.say("run aborted");
    }
    if let Some(e) = st.net.engaged.as_mut() { e.ap = e.ap.saturating_sub(1); }
    crate::constructs::land(st, Place::Stratum(StratumKind::Net), gate);   // make `land` pub(crate)
    st.backdoors.hop(b);
    if trap { spring(st, b); }
    Ok(format!("hop · `{}`'s gate", building_name(st, b)))
}
```

`dump_for`: before `let _ = jack_out(st);`:

```rust
    if st.backdoors.route.is_some() {
        st.backdoors.start_trace(now);
        let first = st.backdoors.trace.as_ref().and_then(|t| t.relays.first().copied());
        match first {
            Some(b) => st.warn(format!("trace running · it starts at `{}`", building_name(st, b))),
            None => st.warn("trace running · straight to your door"),
        }
    }
```

`jack_out` (the app fn): `st.backdoors.end_route();` after `player.jack_out`
succeeds. `jack_in_refusal`: after the cooling check, `if st.home.is_some_and(|h| h.place() == player.place) && st.backdoors.seized(now) { return Some("the rig is gone · evidence".to_string()); }`
(move the `player`/`city` bindings up as needed).

Route start on ordinary rig jack-ins: in `explore.rs` Enter handler (line ~1753)
and `popup.rs` `WorldAct::JackIn` arm, after a successful `player.jack_in`, call
`crate::net::after_jack_in(st, from, None)` (the `from` is already bound in both).
Both sites borrow `st.player` mutably in a `match`; call `after_jack_in` after the
match arm has released the borrow (store the `Ok` in a local, then call).

Hop rows in `run_choices`, after the loop over targets:

```rust
        let on_server = st.constructs.get(b).is_some_and(|k| {
            k.nodes.iter().any(|(id, p)| {
                st.city.as_ref().is_some_and(|c| c.net.node(*id).kind == NodeKind::Server)
                    && (*p == pos || p.chebyshev(pos) <= 1)
            })
        });
        if on_server && st.backdoors.held(b).is_some() {
            let home_run = st.backdoors.route.is_some();
            for (other, label) in crate::net::landings(st) {
                if other == b { continue; }
                let Some(key) = letters.next() else { break };
                if home_run {
                    choices.push(Choice::new(key, format!("hop · {label}"), Pick::Hop(other)));
                } else {
                    choices.push(Choice::disabled(key, format!("hop · {label} · no route home")));
                }
            }
        }
```

`fire`: `Pick::Hop(b) => { let r = crate::net::hop(st, b); report(st, r); ScreenCmd::Pop { fx: TransitionKind::Glitch } }`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero screens::explore 2>&1 | grep -E "test result|FAILED|panicked"`

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets -p strata-zero 2>&1 | tail -3
git add -A crates/strata-zero/src
git commit -m "feat(app): the rig lands you at a backdoor's gate, and you hop between them"
```

---

### Task 6: The trace clock, the raid, the HUD, the journal's extra lines, the console

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs:1933-1939` (minute hook), `explore.rs:280-313` (footer trace)
- Modify: `crates/strata-zero/src/screens/terminal.rs:299` (minute hook in update)
- Modify: `crates/strata-zero/src/net.rs` (new `pub fn tick_backdoors(st)`)
- Modify: `crates/strata-zero/src/credentials.rs:660-760` (route/trace/seized/quiet lines), `credentials.rs:179-183` (`first_mail` quiet), new `fixer_mail(st)`
- Modify: `crates/strata-zero/src/ui/hud.rs:380-385` (home trace in magenta)
- Modify: `crates/strata-zero/src/screens/console.rs:14-18,21-60` (`trace`, `seize`, `backdoor`)

**Interfaces:**
- Consumes Tasks 1–5.
- Produces: `net::tick_backdoors(st)` (runs `st.backdoors.minute(now)` and turns
  events into log lines and effects); `Footer.home_trace: Option<String>`;
  `credentials::fixer_mail(st)`.

- [ ] **Step 1: Write the failing tests**

In `net.rs` tests (or `explore.rs`, wherever `home_screen` lives; put them beside Task 5's):

```rust
    #[test]
    fn the_trace_burns_relays_on_the_minute_and_the_raid_takes_the_rig() {
        let (mut st, _screen) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let home = st.home.unwrap().home.building;
        let a = other_construct(&st, home);
        st.backdoors.plant_net(a, 1, seed, 0);
        st.backdoors.plant_net(home, 1, seed, 0);   // one you keep, to lose in the raid
        crate::net::land_at(&mut st, a).unwrap();
        crate::net::dump(&mut st, "the trace found you");
        let now = st.net.now(&st);
        advance_minutes(&mut st, crate::backdoors::HOLD_NET);   // moves the world clock; the module's tests have a clock helper (grep "clock" in console.rs: `clock +N`)
        crate::net::tick_backdoors(&mut st);
        assert!(log_text(&st).contains("trace passes"), "{}", log_text(&st));
        assert!(st.backdoors.held(a).is_none());
        advance_minutes(&mut st, crate::backdoors::LAST_LEG);
        crate::net::tick_backdoors(&mut st);
        assert!(log_text(&st).contains("they kicked your door in"));
        assert!(st.backdoors.held(home).is_none());
        assert!(st.backdoors.seized(st.net.now(&st)));
        assert_eq!(crate::net::jack_in_refusal(&st), Some("the rig is gone · evidence".to_string()));
        let mails = st.journal.mails.len();
        advance_minutes(&mut st, crate::backdoors::SEIZED_MINUTES);
        crate::net::tick_backdoors(&mut st);
        assert_eq!(st.journal.mails.len(), mails + 1);
        let m = st.journal.mails.last().unwrap();
        assert_eq!(m.subject, "lose this number");
        assert!(!st.backdoors.seized(st.net.now(&st)));
        let _ = now;
    }

    #[test]
    fn the_raid_while_jacked_in_ends_the_run_cleanly() {
        let (mut st, _screen) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let a = other_construct(&st, st.home.unwrap().home.building);
        st.backdoors.plant_net(a, 1, seed, 0);
        // a trace already on the last leg, then jack back in from the rig
        st.backdoors.start_route(None);
        st.backdoors.start_trace(st.net.now(&st));
        st.backdoors.end_route();
        crate::net::land_at(&mut st, a).unwrap();
        crate::constructs::enter_construct(&mut st, a).unwrap();
        advance_minutes(&mut st, crate::backdoors::LAST_LEG);
        crate::net::tick_backdoors(&mut st);
        let p = st.player.as_ref().unwrap();
        assert!(p.body.is_none(), "dumped home");
        assert!(st.net.run.is_none() && st.net.engaged.is_none());
        assert!(log_text(&st).contains("the line went dead"));
    }

    #[test]
    fn the_footer_shows_the_home_trace_and_the_route() {
        let (mut st, screen) = home_screen();
        let seed = st.city.as_ref().unwrap().seed;
        let a = other_construct(&st, st.home.unwrap().home.building);
        st.backdoors.plant_net(a, 1, seed, 0);
        crate::net::land_at(&mut st, a).unwrap();
        let text = draw_screen(&screen, &st, 120, 40);
        let name = crate::helix::hosts::hostname(&st, crate::helix::shell::HostId::Building(a));
        assert!(text.contains(&format!("ROUTE zenbo ▸ {name}")), "{text}");
        crate::net::dump(&mut st, "the trace found you");
        let text = draw_screen(&screen, &st, 120, 40);
        assert!(text.contains(&format!("TRACE ▸ {name} ·")), "{text}");
    }
```

In `credentials.rs` tests:

```rust
    #[test]
    fn the_journal_lists_the_route_the_trace_and_the_seizure() {
        let (mut st, b) = state();
        st.backdoors.plant_net(b, 1, 0, 0);
        st.backdoors.start_route(Some(b));
        let lines = extra_lines(&st);
        let name = crate::helix::hosts::hostname(&st, crate::helix::shell::HostId::Building(b));
        assert_eq!(lines[0], format!("route · zenbo ▸ {name}"));
        st.backdoors.start_trace(0);
        st.backdoors.end_route();
        assert!(extra_lines(&st)[0].starts_with(&format!("trace ▸ {name} · ")));
        st.backdoors.rig_seized_until = Some(120);
        st.backdoors.fixer_quiet_until = Some(240);
        let lines = extra_lines(&st);
        assert!(lines.iter().any(|l| l == "rig seized · back day 2 00:00"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "fixers quiet until day 2 02:00"), "{lines:?}");
    }
```

- [ ] **Step 2: Run them to fail**

- [ ] **Step 3: Implement**

`net.rs`:

```rust
/// The backdoor clock: the trace walks, relays burn, the raid lands, the rig returns.
pub fn tick_backdoors(st: &mut AppState) {
    let now = st.net.now(st);
    for ev in st.backdoors.minute(now) {
        match ev {
            crate::backdoors::Event::RelayBurned(b) => {
                let line = format!("trace passes `{}` · the backdoor is gone", building_name(st, b));
                st.warn(line);
            }
            crate::backdoors::Event::LastLeg => st.warn("trace · nothing left between them and your door"),
            crate::backdoors::Event::Raid => {
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
```

`dump_for` must tolerate being called when the raid already burned the route's
relays: `start_trace` skips burned ones, and a raid clears `trace`; after the
raid's `dump_for`, `route` was `Some` (the run was from the rig) so a *new*
trace would start — guard: in `tick_backdoors`'s `Raid` arm call
`st.backdoors.end_route()` **before** `dump_for`, so no second trace starts.

`explore.rs:1939`: after `st.net.minute(...)`, `crate::net::tick_backdoors(st);`.
`terminal.rs` `update`: first line `crate::net::tick_backdoors(st);`.

`credentials.rs`:

```rust
/// The fixer's mail when the rig comes back from the raid.
pub fn fixer_mail(st: &mut AppState) {
    let from = st.journal.favour.as_ref().map(|f| f.fixer.clone()).unwrap_or_else(|| "Dez".to_string());
    let now = st.net.now(st);
    st.journal.mails.push(Mail {
        from,
        subject: "lose this number".to_string(),
        body: vec![
            "They came to your door. I don't know you for a while.".to_string(),
            "Don't call.".to_string(),
        ],
        read: false,
        at: now,
    });
}

/// The dim lines above the journal's groups: the route, the trace, the seizure.
pub fn extra_lines(st: &AppState) -> Vec<String> {
    let now = st.net.now(st);
    let name = |b: BuildingId| crate::helix::hosts::hostname(st, crate::helix::shell::HostId::Building(b));
    let mut out = Vec::new();
    if let Some(r) = st.backdoors.route.as_ref() {
        let names: Vec<String> = r.relays.iter().map(|b| name(*b)).collect();
        out.push(format!("route · {}", crate::backdoors::route_line(&names)));
    }
    if let Some(t) = st.backdoors.trace_line(now, &name) {
        out.push(t.replacen("TRACE", "trace", 1));
    }
    if let Some(u) = st.backdoors.rig_seized_until.filter(|u| now < *u) {
        out.push(format!("rig seized · back {}", stamp(u)));
    }
    if let Some(u) = st.backdoors.fixer_quiet_until.filter(|u| now < *u) {
        out.push(format!("fixers quiet until {}", stamp(u)));
    }
    out
}
```

`first_mail`: after the `favour.is_some()` return, `if st.backdoors.fixer_quiet(st.net.now(st)) { return; }`.

`deck.rs` `journal_rows`: before the groups, push one `Tone::Dim`, `detail: None`
row per `extra_lines` line; `access::journal_lines` likewise prefixes them. The
"nothing known yet" row is shown only when both are empty.

HUD: `Footer` gains `pub home_trace: Option<String>`; in `footer_rows` row 1,
after the construct trace span, if `Some`, push `Span::raw("  ")` and the line
styled `MAGENTA` bold. In `explore.rs::footer`: `home_trace: st.backdoors.trace_line(now, &|b| crate::helix::hosts::hostname(st, crate::helix::shell::HostId::Building(b)))`,
and the `trace` string, when `jacked && st.backdoors.route.is_some()`, gets
` · ROUTE {}` appended with `route_line` of the relays' hostnames. Update the
`Footer` literal in `hud.rs` tests (`home_trace: None`).

Console (`console.rs`): `COMMANDS` grows to 27 with `"trace", "seize", "backdoor"`:

```rust
        "trace" => {
            let now = st.net.now(st);
            if st.backdoors.route.is_none() { st.backdoors.start_route(None); }
            st.backdoors.start_trace(now);
            Ok("a trace runs home".to_string())
        }
        "seize" => {
            let now = st.net.now(st);
            st.backdoors.rig_seized_until = Some(now + crate::backdoors::SEIZED_MINUTES);
            Ok("the rig is seized for a day".to_string())
        }
        "backdoor" => {
            let what = args.first().ok_or("backdoor <host>")?;
            let b = st.city.as_ref().and_then(|c| c.buildings.iter().find(|x| crate::helix::hosts::hostname(st, crate::helix::shell::HostId::Building(x.id)).starts_with(what)).map(|x| x.id)).ok_or_else(|| format!("no host {what}"))?;
            let now = st.net.now(st);
            let seed = st.city.as_ref().map_or(0, |c| c.seed);
            st.backdoors.can_plant(b, now).map_err(|r| r.words())?;
            st.backdoors.plant_net(b, 1, seed, now);
            Ok(format!("backdoor on `{}`", crate::net::building_name(st, b)))
        }
```

(The `hostname` call borrows `st` while `c` borrows `st.city`: collect the
candidates into a `Vec<(BuildingId, String)>` first.) Add the three to the
`help` text list.

- [ ] **Step 4: Run the whole suite**

Run: `cargo test 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: all green.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets 2>&1 | tail -3
git add -A crates/strata-zero/src
git commit -m "feat(app): the trace walks your route home, and the raid takes the rig"
```

---

### Task 7: Fuzz, README and the spec's status

**Files:**
- Modify: `crates/strata-zero/src/backdoors.rs` (an ignored fuzz test at the end of its test module), `README.md:70-85`, `docs/superpowers/specs/2026-10-18-backdoors-design.md` (status line)

- [ ] **Step 1: The ignored fuzz test**

```rust
    #[test]
    #[ignore = "slow: three cities"]
    fn plant_land_hop_and_burn_across_three_cities() {
        use citygen::{GenParams, Generator, SizePreset};
        for seed in [3u64, 8, 21] {
            let mut st = crate::app::AppState::new();
            st.enter_city(Generator::run(GenParams { seed, size: SizePreset::Medium, districts: 7, ..Default::default() }).unwrap());
            let Some(rig) = st.home else { continue };
            st.player = rig.wake(st.city.as_ref().unwrap(), &mut st.interiors, &st.constructs, 0);
            crate::credentials::move_in(&mut st);
            let district = st.city.as_ref().unwrap().building(rig.home.building).district;
            let targets = crate::access::scannable(&st, district);
            let seed = st.city.as_ref().unwrap().seed;
            let mut planted = Vec::new();
            for b in targets.iter().copied() {
                if st.backdoors.can_plant(b, 0).is_ok() {
                    st.backdoors.plant_key(b, "user", 100, seed, 0);
                    planted.push(b);
                }
                assert!(st.backdoors.slots_used() <= SLOTS);
            }
            for b in planted.iter().copied() {
                if crate::constructs::city_gate(st.city.as_ref().unwrap(), b).is_none() { continue; }
                crate::net::land_at(&mut st, b).unwrap();
                if let Some(other) = planted.iter().copied().find(|o| *o != b) {
                    if st.constructs.ensure(st.city.as_ref().unwrap(), b).is_some() {
                        crate::constructs::enter_construct(&mut st, b).unwrap();
                        let _ = crate::net::hop(&mut st, other);
                    }
                }
                crate::net::jack_out(&mut st).unwrap();
            }
            for b in planted {
                st.backdoors.burn(b, Why::You, 1);
            }
            assert_eq!(st.backdoors.slots_used(), 0);
        }
    }
```

Run: `cargo test -p strata-zero plant_land_hop -- --ignored 2>&1 | tail -3`. Fix
whatever panics (the test is for finding them).

- [ ] **Step 2: README**

In the HELIX paragraph (README.md:70-85) after "a backdoor on the server makes
you `root`", add two sentences: "`backdoor`, run as a resident or root, writes
your rig's key into their `~/.ssh/authorized_keys`: for a couple of hours you can
`ssh` in from home, and the rig can jack you in at that building's gate on the
Net; the Backdoor program does the same for days, as `root`. Your rig keeps three
such lines (`ls /net`, `route`, `burn <host>`), and a run from home that gets
dumped sends a trace back along the backdoors you came through, burning each
one it passes; cut it with `burn`, or they kick your door in and take the rig
for a day."

- [ ] **Step 3: The spec's status**

Change the spec's `Status:` line to `built on worktree-backdoors; approved in chat by Jeffrey`
and note any deviation the build made, in the house style of the other specs
("Built as specified except: …").

- [ ] **Step 4: Full verification**

```bash
cargo fmt --check && cargo clippy --all-targets 2>&1 | tail -3 && cargo test 2>&1 | grep -E "test result|FAILED"
cargo test -p strata-zero plant_land_hop -- --ignored 2>&1 | tail -2
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs: the README and the spec speak backdoors; a fuzz run plants and burns"
```
