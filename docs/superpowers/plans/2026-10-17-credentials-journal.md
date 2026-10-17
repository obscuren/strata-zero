# Credentials and the Journal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Locked flats open for a keycard in the pack or a door code in the journal beside the Net's lock crack; the JOURNAL tab lists codes, cards and backdoors by building; every resident has a stable flat.

**Architecture:** A pure door-code hash in `citygen`, a `Homes` assignment cached in `Interiors` that placement and labels read, and a new `credentials` module in the app crate with one `opens` question, a `Journal` of learned codes, keycard catalogue entries generated per household, and the hooks the Net copy, the lobby drop and the deck tab call. Cards come from the inventory and backdoors from the Net state, so the journal tab is a view.

**Tech Stack:** Rust 2024, ratatui, the workspace's own crates (`citygen`, `sim`, `game`, `strata-zero`). Tests with `cargo test -p <crate>`; lint with `cargo clippy --workspace --all-targets`.

**Spec:** `docs/superpowers/specs/2026-10-17-credentials-journal-design.md`

## Global Constraints

- Never rebuild `target/release/strata` (Jeffrey may be playing it); test builds use the default target dir, which is fine.
- Commit messages in the repo's voice: `feat(app): ...`, `fix(citygen): ...`, lower case, a sentence.
- Log lines mark names with backticks; the log strips them (`messages::parse_marks`).
- New enum variants go at the end of their enums (postcard saves).
- Door codes are four digits, zero-padded, from the city seed; the same flat always has the same code.
- Keycard item ids are `keycard:<building index>:<household index>`; the player's own is `keycard:home`.
- No lifetime on any credential in this slice.

## Review Focus

1. A residential building whose plan has no corridor floors with flats (all residents, no flats) must give an empty `Homes` and never panic on modulo by zero. Pinned in Task 2.
2. A door bumped from inside its own room still opens (the old `rect.contains(here)` rule) whatever the credentials say. Pinned in Task 4.
3. Loading a saved city or starting a second new game must not leave the previous city's keycards in the catalogue nor stack a second home keycard. Pinned in Tasks 3 and 8.
4. Re-entering a lobby must not drop a second lost card, and picking the card up must not make it reappear. Pinned in Task 5.
5. A server copy on a residential building whose homes were never computed (the player never walked in) must still write every code: `copy_codes` prepares the building itself. Pinned in Task 6.

---

### Task 1: The door code hash

**Files:**
- Modify: `crates/citygen/src/rng.rs`
- Modify: `crates/citygen/src/lib.rs` (re-export if `rng` items are re-exported there; check `grep -n "pub use rng" crates/citygen/src/lib.rs`)

**Interfaces:**
- Produces: `pub fn door_code(seed: u64, building: BuildingId, level: i32, number: u16) -> u16` in `citygen::rng`, value in `0..=9999`.

- [ ] **Step 1: Write the failing test** in `crates/citygen/src/rng.rs` tests module (create one if absent):

```rust
#[test]
fn door_codes_are_four_digits_stable_and_differ_between_flats() {
    use crate::ids::BuildingId;
    let a = door_code(7, BuildingId(3), 12, 1204);
    assert!(a <= 9999);
    assert_eq!(a, door_code(7, BuildingId(3), 12, 1204));
    assert_ne!(a, door_code(7, BuildingId(3), 12, 1205));
    assert_ne!(a, door_code(8, BuildingId(3), 12, 1204));
}
```

- [ ] **Step 2: Run** `cargo test -p citygen door_codes` — expected: compile error, `door_code` not found.

- [ ] **Step 3: Implement** below `hash64`:

```rust
/// A flat's four-digit door code, fixed by the seed, the building and the flat.
pub fn door_code(seed: u64, building: BuildingId, level: i32, number: u16) -> u16 {
    let h = hash64(
        hash64(seed, 0xD00Ru64 + building.index() as u64),
        ((level as i64 as u64) << 16) ^ number as u64,
    );
    (h % 10_000) as u16
}
```

Import `crate::ids::BuildingId` at the top of `rng.rs`.

- [ ] **Step 4: Run** `cargo test -p citygen door_codes` — expected: PASS.

- [ ] **Step 5: Commit** `feat(citygen): a door code for every flat, from the seed`.

---

### Task 2: Stable homes

**Files:**
- Modify: `crates/strata-zero/src/interiors.rs` (the `Interiors` struct, a new `Homes` type, `Placement::new`)
- Modify: `crates/strata-zero/src/screens/explore.rs:525-545` (the placement refresh) and the test at `:4282` that builds a `Placement`

**Interfaces:**
- Produces:
  - `pub struct Homes` with `pub fn flat_of(&self, household: HouseholdId) -> Option<(i32, u16)>`, `pub fn household_of(&self, level: i32, number: u16) -> Option<HouseholdId>`, `pub fn owner_of(&self, level: i32, number: u16) -> Option<PersonId>`, `pub fn households(&self) -> impl Iterator<Item = (HouseholdId, (i32, u16))>`, `pub fn flats(&self) -> impl Iterator<Item = (i32, u16)>`, `pub fn is_empty(&self) -> bool`.
  - `Interiors::ensure_homes(&mut self, city: &City, world: &World, b: BuildingId) -> bool` (true when the building has flats) and `Interiors::homes_of(&self, b: BuildingId) -> Option<&Homes>`.
  - `Placement::new(plan, floor, world, homes: &Homes)`.

- [ ] **Step 1: Write the failing tests** in `interiors.rs` tests:

```rust
#[test]
fn every_resident_has_a_stable_flat_and_flats_are_not_shared_while_free() {
    let city = Generator::run(GenParams { seed: 6, size: SizePreset::Custom { w: 120, h: 60 }, districts: 8, ..Default::default() }).unwrap();
    let people = sim::People::default();
    let world = sim::World::seed(&city, &people, &sim::SimParams::default(), crate::ui::hud::START_MINUTE_OF_DAY, 0);
    let b = city.buildings.iter().find(|b| matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock)
        && world.people().iter().any(|p| p.home == Some(b.id))).expect("a residential building with residents").id;
    let mut a = Interiors::default();
    assert!(a.ensure_homes(&city, &world, b));
    let mut c = Interiors::default();
    assert!(c.ensure_homes(&city, &world, b));
    let (ha, hc) = (a.homes_of(b).unwrap(), c.homes_of(b).unwrap());
    let mut seen = std::collections::BTreeMap::new();
    for p in world.people().iter().filter(|p| p.home == Some(b)) {
        let flat = ha.flat_of(p.household).expect("a flat");
        assert_eq!(Some(flat), hc.flat_of(p.household), "stable");
        seen.entry(flat).or_insert_with(Vec::new).push(p.household);
    }
    let households = seen.values().flatten().collect::<std::collections::BTreeSet<_>>().len();
    let flats = ha.flats().count();
    if households <= flats {
        assert!(seen.values().all(|hs| hs.iter().collect::<std::collections::BTreeSet<_>>().len() == 1), "no sharing while flats remain");
    }
    let (level, number) = ha.flats().next().unwrap();
    if let Some(h) = ha.household_of(level, number) {
        assert_eq!(ha.owner_of(level, number).map(|p| world.people()[p.0 as usize].household), Some(h));
    }
}

#[test]
fn a_building_without_flats_has_empty_homes() {
    let city = Generator::run(GenParams { seed: 6, size: SizePreset::Custom { w: 120, h: 60 }, districts: 8, ..Default::default() }).unwrap();
    let people = sim::People::default();
    let world = sim::World::seed(&city, &people, &sim::SimParams::default(), crate::ui::hud::START_MINUTE_OF_DAY, 0);
    let b = city.buildings.iter().find(|b| b.kind == BuildingKind::Office).unwrap().id;
    let mut i = Interiors::default();
    assert!(!i.ensure_homes(&city, &world, b));
    assert!(i.homes_of(b).is_none_or(|h| h.is_empty()));
}
```

Check how existing tests in `interiors.rs` build a world (`grep -n "World::seed" crates/strata-zero/src/interiors.rs`) and copy that form; `sim::People::default()` may need to be whatever those tests use.

- [ ] **Step 2: Run** `cargo test -p strata-zero interiors::tests::every_resident` — expected: compile error.

- [ ] **Step 3: Implement `Homes` and `ensure_homes`** in `interiors.rs`:

```rust
use sim::HouseholdId;
use std::collections::BTreeMap;

/// Which household lives in which flat of one building: stable, in or out.
#[derive(Clone, Debug, Default)]
pub struct Homes {
    by_household: BTreeMap<HouseholdId, (i32, u16)>,
    /// The households in a flat, the first is the owner.
    residents: BTreeMap<(i32, u16), Vec<HouseholdId>>,
    /// The first member of each household, for naming the owner.
    heads: BTreeMap<HouseholdId, PersonId>,
}

impl Homes {
    fn new(seed: u64, b: BuildingId, flats: Vec<(i32, u16)>, world: &World) -> Homes {
        let mut homes = Homes::default();
        if flats.is_empty() {
            return homes;
        }
        let mut households: BTreeMap<HouseholdId, PersonId> = BTreeMap::new();
        for p in world.people().iter().filter(|p| p.home == Some(b)) {
            households.entry(p.household).or_insert(p.id);
        }
        let start = (hash64(seed, 0x40E5 + b.index() as u64) % flats.len() as u64) as usize;
        for (i, (h, head)) in households.into_iter().enumerate() {
            let flat = flats[(start + i) % flats.len()];
            homes.by_household.insert(h, flat);
            homes.residents.entry(flat).or_default().push(h);
            homes.heads.insert(h, head);
        }
        homes
    }
    pub fn flat_of(&self, household: HouseholdId) -> Option<(i32, u16)> { self.by_household.get(&household).copied() }
    pub fn household_of(&self, level: i32, number: u16) -> Option<HouseholdId> { self.residents.get(&(level, number)).and_then(|v| v.first().copied()) }
    pub fn owner_of(&self, level: i32, number: u16) -> Option<PersonId> { self.household_of(level, number).and_then(|h| self.heads.get(&h).copied()) }
    pub fn households(&self) -> impl Iterator<Item = (HouseholdId, (i32, u16))> + '_ { self.by_household.iter().map(|(h, f)| (*h, *f)) }
    pub fn flats(&self) -> impl Iterator<Item = (i32, u16)> + '_ { self.residents.keys().copied() }
    pub fn is_empty(&self) -> bool { self.by_household.is_empty() }
}
```

Add `homes: HashMap<BuildingId, Homes>` to `Interiors` and:

```rust
/// Works out who lives where in a residential building, once; generates its
/// corridor floors to count the flats. True when the building has flats.
pub fn ensure_homes(&mut self, city: &City, world: &World, b: BuildingId) -> bool {
    if let Some(h) = self.homes.get(&b) {
        return !h.is_empty();
    }
    let Some(plan) = self.plan(city, b).cloned() else { return false };
    let mut flats = Vec::new();
    for level in plan.corridor_floors() {
        if let Some(floor) = self.ensure(city, b, level) {
            flats.extend(floor.rooms.iter().filter(|r| r.kind == RoomKind::Flat).map(|r| (level, r.number)));
        }
    }
    flats.sort();
    let homes = Homes::new(city.seed, b, flats, world);
    let has = !homes.is_empty();
    self.homes.insert(b, homes);
    has
}
pub fn homes_of(&self, b: BuildingId) -> Option<&Homes> { self.homes.get(&b) }
```

Note `corridor_floors()` filters `FloorType::Corridor`; check whether atrium floors (`FloorType::Atrium`) hold `RoomKind::Flat` rooms too (`grep -n "Atrium" crates/citygen/src/interior/plan.rs`). If they do, iterate `plan.levels().filter(|l| matches!(plan.floor_type(*l), FloorType::Corridor | FloorType::Atrium))` instead, and use the same list in `Placement::new` for residents.

- [ ] **Step 4: Rewrite the residents arm of `Placement::new`** to take `homes: &Homes`. Replace the `OccupantRole::Living | OccupantRole::Guest` arm with:

```rust
OccupantRole::Living => {
    // Residents own their flat whether in or out; see below.
}
OccupantRole::Guest => {
    if corridor.is_empty() || hotel_rooms.is_empty() { continue; }
    let level = corridor[(h % corridor.len() as u64) as usize];
    if level != floor.level { continue; }
    let r = hotel_rooms[(hash64(h, 1) % hotel_rooms.len() as u64) as usize];
    if let Some(d) = r.door { owners.entry(d).or_insert(o.id); }
}
```

and before the occupants loop fill owners from homes:

```rust
for r in &flats {
    if let (Some(d), Some(owner)) = (r.door, homes.owner_of(floor.level, r.number)) {
        owners.insert(d, owner);
    }
}
```

Update callers: `explore.rs:541` becomes

```rust
let homes = st.interiors.homes_of(building).cloned().unwrap_or_default();
...
crate::interiors::Placement::new(plan, floor, world, &homes),
```

(the clone is a few maps per game minute), and the test at `explore.rs:4282` calls `st.interiors.ensure_homes(&city, st.world.as_ref().unwrap(), b)` before its loop and passes `&st.interiors.homes_of(b).cloned().unwrap()` to `Placement::new`. Fix every other `Placement::new(` call (`grep -rn "Placement::new(" crates/strata-zero/src`).

- [ ] **Step 5: Run** `cargo test -p strata-zero interiors:: explore::tests::a_locked_flat_names_its_resident explore::tests::locked_doors` — expected: PASS. Then `cargo test -p strata-zero` — expected: all green.

- [ ] **Step 6: Commit** `feat(app): every resident has a stable flat`.

---

### Task 3: The credentials module and the journal store

**Files:**
- Create: `crates/strata-zero/src/credentials.rs`
- Modify: `crates/strata-zero/src/main.rs` or `lib.rs` (add `mod credentials;` next to `mod net;`)
- Modify: `crates/strata-zero/src/app.rs:28-70` (`journal: credentials::Journal` field, init, reset in `enter_city`, keycard purge)

**Interfaces:**
- Produces:
  - `pub fn keycard_id(b: BuildingId, h: HouseholdId) -> ItemId`, `pub const HOME_KEYCARD: &str = "keycard:home"`, `pub fn parse_keycard(id: &ItemId) -> Option<(BuildingId, HouseholdId)>`.
  - `pub enum Source { ServerCopy, Home }`, `pub struct Code { building, level, number, household: Option<HouseholdId>, code: u16, source: Source, at: u64 }`.
  - `pub struct Journal { codes: Vec<Code>, pub lost_cards: BTreeSet<BuildingId> }` with `learn(&mut self, Code) -> bool`, `code_for(&self, b, level, number) -> Option<&Code>`, `codes(&self) -> &[Code]`.
  - `pub fn stock(st: &mut AppState, b: BuildingId)`: appends keycard catalogue entries for the building's households (idempotent).
  - `pub fn prepare(st: &mut AppState, b: BuildingId)`: ensures homes, stocks cards, drops the lost card (Task 5 fills the drop).
  - `pub enum How { Cracked, Keycard, Code(u16) }`, `pub fn opens(st: &AppState, b: BuildingId, level: i32, number: u16) -> Option<How>`.

- [ ] **Step 1: Write the failing tests** at the bottom of `credentials.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{BuildingKind, GenParams, Generator, SizePreset};

    fn state() -> (crate::app::AppState, BuildingId) {
        let mut st = crate::app::AppState::new();
        st.enter_city(Generator::run(GenParams { seed: 6, size: SizePreset::Custom { w: 120, h: 60 }, districts: 8, ..Default::default() }).unwrap());
        let city = st.city.as_ref().unwrap();
        let world = st.world.as_ref().unwrap();
        let b = city.buildings.iter().find(|b| matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock)
            && world.people().iter().any(|p| p.home == Some(b.id))).unwrap().id;
        (st, b)
    }

    #[test]
    fn keycard_ids_round_trip() {
        let id = keycard_id(BuildingId(4), sim::HouseholdId(9));
        assert_eq!(id.as_str(), "keycard:4:9");
        assert_eq!(parse_keycard(&id), Some((BuildingId(4), sim::HouseholdId(9))));
        assert_eq!(parse_keycard(&ItemId::new("paydata")), None);
    }

    #[test]
    fn a_code_is_learned_once() {
        let mut j = Journal::default();
        let c = Code { building: BuildingId(1), level: 3, number: 304, household: None, code: 1234, source: Source::ServerCopy, at: 5 };
        assert!(j.learn(c.clone()));
        assert!(!j.learn(c));
        assert_eq!(j.codes().len(), 1);
        assert_eq!(j.code_for(BuildingId(1), 3, 304).map(|c| c.code), Some(1234));
    }

    #[test]
    fn stocking_makes_one_card_per_household_and_never_twice() {
        let (mut st, b) = state();
        prepare(&mut st, b);
        let homes = st.interiors.homes_of(b).unwrap().clone();
        let n = homes.households().count();
        assert!(n > 0);
        let cards = |st: &crate::app::AppState| st.content.items.iter().filter(|i| i.id.as_str().starts_with(&format!("keycard:{}:", b.index()))).count();
        assert_eq!(cards(&st), n);
        prepare(&mut st, b);
        assert_eq!(cards(&st), n, "idempotent");
        let (h, (level, number)) = homes.households().next().unwrap();
        let item = st.content.item(&keycard_id(b, h)).unwrap();
        assert!(item.name.starts_with("Keycard · "), "{}", item.name);
        assert!(item.name.contains(&number.to_string()));
        assert_eq!(item.kind, game::ItemKind::Component);
        let _ = level;
    }

    #[test]
    fn a_new_city_drops_the_old_cards() {
        let (mut st, b) = state();
        prepare(&mut st, b);
        assert!(st.content.items.iter().any(|i| i.id.as_str().starts_with("keycard:")));
        st.enter_city(Generator::run(GenParams { seed: 9, size: SizePreset::Custom { w: 120, h: 60 }, districts: 8, ..Default::default() }).unwrap());
        assert!(!st.content.items.iter().any(|i| i.id.as_str().starts_with("keycard:")));
        assert!(st.journal.codes().is_empty());
    }

    #[test]
    fn opens_in_order_crack_then_card_then_code() {
        let (mut st, b) = state();
        prepare(&mut st, b);
        let homes = st.interiors.homes_of(b).unwrap().clone();
        let (h, (level, number)) = homes.households().next().unwrap();
        assert_eq!(opens(&st, b, level, number), None);
        let seed = st.city.as_ref().unwrap().seed;
        st.journal.learn(Code { building: b, level, number, household: Some(h), code: citygen::rng::door_code(seed, b, level, number), source: Source::ServerCopy, at: 0 });
        assert!(matches!(opens(&st, b, level, number), Some(How::Code(_))));
        st.character.give(keycard_id(b, h), 1);
        assert_eq!(opens(&st, b, level, number), Some(How::Keycard));
        let now = st.net.now(&st);
        let lock = st.city.as_ref().unwrap().net.nodes.iter().find(|n| n.building == Some(b) && n.kind == citygen::NodeKind::DoorLock).map(|n| n.id).expect("a door lock");
        st.net.land(b, game::HackTarget::Node(lock), game::Target::Node(citygen::NodeKind::DoorLock), game::Op::Crack, now);
        assert_eq!(opens(&st, b, level, number), Some(How::Cracked));
    }
}
```

Check the real field names of a Net node (`grep -n "pub struct Node" -A8 crates/citygen/src/net.rs`) and adjust `n.building`/`n.kind`/`n.id`; `explore.rs:4594 node_of` shows how tests find one today.

- [ ] **Step 2: Run** `cargo test -p strata-zero credentials::` — expected: compile error.

- [ ] **Step 3: Implement** `credentials.rs`:

```rust
//! Credentials: keycards in the pack, door codes in the journal, and the one
//! question a locked flat asks. Cards are items; codes are knowledge; the Net's
//! lock crack is still the third way in.

use crate::app::AppState;
use citygen::{BuildingId, ItemKind as _, Place, Role};
use game::{Item, ItemId, ItemKind};
use sim::HouseholdId;
use std::collections::BTreeSet;

pub const HOME_KEYCARD: &str = "keycard:home";

pub fn keycard_id(b: BuildingId, h: HouseholdId) -> ItemId {
    ItemId::new(&format!("keycard:{}:{}", b.index(), h.0))
}

pub fn parse_keycard(id: &ItemId) -> Option<(BuildingId, HouseholdId)> {
    let rest = id.as_str().strip_prefix("keycard:")?;
    let (b, h) = rest.split_once(':')?;
    Some((BuildingId(b.parse().ok()?), HouseholdId(h.parse().ok()?)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source { ServerCopy, Home }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub building: BuildingId,
    pub level: i32,
    pub number: u16,
    pub household: Option<HouseholdId>,
    pub code: u16,
    pub source: Source,
    /// The game minute it was learned.
    pub at: u64,
}

#[derive(Default)]
pub struct Journal {
    codes: Vec<Code>,
    /// Buildings whose lost keycard already lies in the lobby.
    pub lost_cards: BTreeSet<BuildingId>,
}

impl Journal {
    /// Records a code; false when that flat's code was already known.
    pub fn learn(&mut self, code: Code) -> bool {
        if self.code_for(code.building, code.level, code.number).is_some() { return false; }
        self.codes.push(code);
        true
    }
    pub fn code_for(&self, b: BuildingId, level: i32, number: u16) -> Option<&Code> {
        self.codes.iter().find(|c| c.building == b && c.level == level && c.number == number)
    }
    pub fn codes(&self) -> &[Code] { &self.codes }
}

/// How a locked door opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How { Cracked, Keycard, Code(u16) }

/// Whether the player gets through this flat's door, and how.
pub fn opens(st: &AppState, b: BuildingId, level: i32, number: u16) -> Option<How> {
    if st.net.unlocked(b, st.net.now(st)) { return Some(How::Cracked); }
    let household = st.interiors.homes_of(b).and_then(|h| h.household_of(level, number));
    if household.is_some_and(|h| st.character.count(&keycard_id(b, h)) > 0) { return Some(How::Keycard); }
    st.journal.code_for(b, level, number).map(|c| How::Code(c.code))
}

/// A card's catalogue entry for one household.
fn card_item(st: &AppState, b: BuildingId, h: HouseholdId, level: i32, number: u16) -> Option<Item> {
    let homes = st.interiors.homes_of(b)?;
    let owner = homes.owner_of(level, number)?;
    let name = st.world.as_ref()?.people()[owner.0 as usize].name.clone();
    let building = crate::net::building_name(st, b);
    let _ = level;
    Some(Item {
        id: keycard_id(b, h),
        name: format!("Keycard · {name}, {building} {number}"),
        kind: ItemKind::Component,
        slot: None,
        weight: 0.01,
        value: 40,
        desc: format!("A plastic rectangle with a chip and a lanyard. It opens one door in the city, and that door is {number} at {building}."),
        perks: Vec::new(),
        grants: Vec::new(),
        on_use: None,
        program: None,
        deck: None,
    })
}

/// Puts the building's keycards in the catalogue, once.
pub fn stock(st: &mut AppState, b: BuildingId) {
    let Some(homes) = st.interiors.homes_of(b).cloned() else { return };
    for (h, (level, number)) in homes.households() {
        let id = keycard_id(b, h);
        if st.content.item(&id).is_some() { continue; }
        if let Some(item) = card_item(st, b, h, level, number) { st.content.items.push(item); }
    }
}

/// Everything a residential building needs once the player can reach it: its homes,
/// its keycards in the catalogue, and the lost card in its lobby.
pub fn prepare(st: &mut AppState, b: BuildingId) {
    let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_ref()) else { return };
    if !st.interiors.ensure_homes(city, world, b) { return; }
    stock(st, b);
    drop_lost_card(st, b); // Task 5
}

pub fn drop_lost_card(_st: &mut AppState, _b: BuildingId) {}

/// Drops every keycard of a previous city from the catalogue.
pub fn purge(content: &mut game::Content) {
    content.items.retain(|i| !i.id.as_str().starts_with("keycard:"));
}
```

If `Item` has fields this plan does not name, set them to their defaults; fix `ItemId::as_str` to whatever accessor exists (`grep -n "impl ItemId" -A10 crates/game/src/model.rs`; it may be `&id.0` or `id.to_string()`). `crate::net::building_name` is `pub(crate)` already. Drop the unused `citygen::...` imports clippy flags.

Add to `AppState`: `pub journal: crate::credentials::Journal,` with `journal: Default::default(),` in `new()`, and in `enter_city` add `self.journal = Default::default(); crate::credentials::purge(&mut self.content);`.

- [ ] **Step 4: Run** `cargo test -p strata-zero credentials::` — expected: PASS.

- [ ] **Step 5: Commit** `feat(app): keycards, door codes and the journal's store`.

---

### Task 4: Locked flats open for a card or a code

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs:1172-1220` (the `Place::Floor` arm of `doorway`)
- Modify: `crates/strata-zero/src/interiors.rs:95-220` (`enter`, `ride`, `climb` call `credentials::prepare` after `ensure`)
- Modify: `crates/strata-zero/src/screens/explore.rs:563-575` (`enter_walk`: prepare the home building after `wake`)

**Interfaces:**
- Consumes: `credentials::opens`, `credentials::prepare`, `Homes::owner_of`.

- [ ] **Step 1: Write the failing screen test** beside `a_locked_flat_names_its_resident` in `explore.rs`:

```rust
/// A residential building, a flat on one of its floors, its door, a tile outside it,
/// the household and the owner's name.
fn a_flat(st: &mut AppState) -> (citygen::BuildingId, i32, u16, Pos, Pos, sim::HouseholdId, String) {
    let city = st.city.as_ref().unwrap().clone();
    let b = {
        let world = st.world.as_ref().unwrap();
        city.buildings.iter().find(|b| matches!(b.kind, BuildingKind::Megablock | BuildingKind::Apartment)
            && world.people().iter().any(|p| p.home == Some(b.id))).expect("a residential block").id
    };
    crate::credentials::prepare(st, b);
    let homes = st.interiors.homes_of(b).unwrap().clone();
    for (h, (level, number)) in homes.households() {
        let (_, floor) = st.interiors.floor(b, level).unwrap();
        let Some(r) = floor.rooms.iter().find(|r| r.kind == citygen::RoomKind::Flat && r.number == number) else { continue };
        let Some(d) = r.door else { continue };
        let Some(outside) = d.neighbors4().find(|n| !r.rect.contains(*n) && floor.tiles[*n].is_walkable()) else { continue };
        let owner = homes.owner_of(level, number).unwrap();
        let name = st.world.as_ref().unwrap().people()[owner.0 as usize].name.clone();
        return (b, level, number, d, outside, h, name);
    }
    panic!("a flat with a door and a corridor outside");
}

#[test]
fn a_keycard_or_a_code_opens_a_locked_flat() {
    let mut st = state8();
    seed_world(&mut st);
    let mut s = MapScreen::explore(0);
    walking(&mut st, &mut s);
    let (b, level, number, door, outside, h, name) = a_flat(&mut st);
    let place = Place::Floor { building: b, level };
    // Without anything: locked.
    stand(&mut st, &mut s, place, outside);
    s.update(&mut st, &[key(dir_between(outside, door))]);
    assert_eq!(player(&st).1, outside);
    assert!(last(&st).is_some_and(|l| l.starts_with(&format!("locked · {name}'s flat {number}"))), "{:?}", last(&st));
    // With the code: in, and the log says you keyed it.
    let seed = st.city.as_ref().unwrap().seed;
    let code = citygen::rng::door_code(seed, b, level, number);
    st.journal.learn(crate::credentials::Code { building: b, level, number, household: Some(h), code, source: crate::credentials::Source::ServerCopy, at: 0 });
    s.update(&mut st, &[key(dir_between(outside, door))]);
    assert_eq!(player(&st).1, door);
    assert_eq!(last(&st), Some(format!("{name}'s flat {number} · you key in {code:04}").as_str()));
    // Back out, forget the code, hold the card instead: the card wins.
    stand(&mut st, &mut s, place, outside);
    st.journal = Default::default();
    st.character.give(crate::credentials::keycard_id(b, h), 1);
    s.update(&mut st, &[key(dir_between(outside, door))]);
    assert_eq!(player(&st).1, door);
    assert_eq!(last(&st), Some(format!("{name}'s flat {number} · the keycard opens it").as_str()));
    // From inside the room the door opens whatever you hold.
    st.character.inventory.clear();
    let inside = door.neighbors4().find(|n| st.interiors.floor(b, level).unwrap().1.door_of(door).unwrap().rect.contains(*n)).unwrap();
    stand(&mut st, &mut s, place, inside);
    s.update(&mut st, &[key(dir_between(inside, door))]);
    assert_eq!(player(&st).1, door);
}
```

`last(&st)` returns the log's last text without backticks (check `explore.rs:2273`); if it keeps the kind marker, compare with `ends_with`.

- [ ] **Step 2: Run** `cargo test -p strata-zero a_keycard_or_a_code_opens` — expected: FAIL (locked with the code).

- [ ] **Step 3: Implement** in `doorway`'s `Place::Floor` arm, replacing the `owner`/`open`/`locked` block:

```rust
let here = player.pos;
let room = floor.door_of(target).filter(|r| r.private && r.kind != citygen::RoomKind::Home && !r.rect.contains(here)).cloned();
if let Some(r) = room {
    let how = crate::credentials::opens(st, building, level, r.number);
    let owner = st.interiors.homes_of(building)
        .and_then(|h| h.owner_of(level, r.number))
        .or_else(|| self.placement.as_ref().filter(|(pl, _, _)| *pl == place).and_then(|(_, _, p)| p.owner_of(target)))
        .and_then(|id| st.world.as_ref().map(|w| w.people()[id.0 as usize].name.clone()));
    let mut what = r.kind.label().to_string();
    if r.number > 0 { what = format!("{what} {}", r.number); }
    let whose = match &owner { Some(name) => format!("{name}'s {what}"), None => what };
    match how {
        None => { st.warn(format!("locked · {whose}")); return Some(ScreenCmd::Stay); }
        Some(crate::credentials::How::Cracked) => {}
        Some(crate::credentials::How::Keycard) => st.say(format!("`{}`'s {} · the keycard opens it", owner.clone().unwrap_or_default(), what_only)),
        Some(crate::credentials::How::Code(c)) => st.say(format!("`{}`'s {} · you key in {c:04}", owner.clone().unwrap_or_default(), what_only)),
    }
}
```

Keep a `what_only` copy of `what` before `whose` moves it; when `owner` is `None` write the line without the possessive (`"{what} · the keycard opens it"`). Hotel rooms are `private` too but have no `Homes` entry, so `opens` returns `None` for them unless cracked, as today.

In `interiors.rs`, after each `ensure` in `enter`, `ride` and `climb`, add `crate::credentials::prepare(st, b);` (the borrow of `city` must end first: rebind `let city = st.city.as_ref()...` after, or move the `prepare` call to just before `land(...)`). In `explore.rs` `enter_walk`, after `st.player = Some(...)`, add:

```rust
if let Some(rig) = st.home { crate::credentials::prepare(st, rig.home.building); }
```

- [ ] **Step 4: Run** `cargo test -p strata-zero a_keycard_or_a_code_opens explore::tests::a_locked_flat explore::tests::locked_doors explore::tests::a_new_game_wakes` — expected: PASS. Then `cargo test -p strata-zero` — green.

- [ ] **Step 5: Commit** `feat(app): a keycard or a door code opens a locked flat`.

---

### Task 5: The lost card at the lobby desk, and who carries theirs

**Files:**
- Modify: `crates/strata-zero/src/credentials.rs` (`drop_lost_card`, `carries`)
- Modify: `crates/strata-zero/src/screens/explore.rs` (the look card for a person adds the carried line; find where `inspector_lines(` is called with a person, `grep -n "inspector_lines(" crates/strata-zero/src/screens/explore.rs`)
- Modify: `crates/strata-zero/src/ui/panel.rs:14` (`kv` becomes `pub(crate) fn kv`)

**Interfaces:**
- Produces: `pub fn lost_household(st: &AppState, b: BuildingId) -> Option<HouseholdId>`, `pub fn drop_lost_card(st: &mut AppState, b: BuildingId)`, `pub fn carries(st: &AppState, person: sim::PersonId) -> Vec<ItemId>`.

- [ ] **Step 1: Write the failing tests** in `credentials.rs`:

```rust
#[test]
fn one_lost_card_lies_in_the_lobby_once_and_its_owner_goes_without() {
    let (mut st, b) = state();
    prepare(&mut st, b);
    let lost = lost_household(&st, b).expect("a household lost its card");
    let id = keycard_id(b, lost);
    let lobby = Place::Floor { building: b, level: 0 };
    let on_floor = |st: &crate::app::AppState| st.ground.on(lobby).map(|(_, s)| s.iter().filter(|s| s.item == id).map(|s| s.qty).sum::<u32>()).sum::<u32>();
    assert_eq!(on_floor(&st), 1);
    let (pos, _) = st.ground.on(lobby).find(|(_, s)| s.iter().any(|s| s.item == id)).unwrap();
    let (_, floor) = st.interiors.floor(b, 0).unwrap();
    assert!(floor.tiles[pos].is_walkable());
    prepare(&mut st, b);
    assert_eq!(on_floor(&st), 1, "no second card");
    st.ground.take(lobby, pos, &id, 1);
    prepare(&mut st, b);
    assert_eq!(on_floor(&st), 0, "picked up stays picked up");
    let homes = st.interiors.homes_of(b).unwrap().clone();
    let world = st.world.as_ref().unwrap();
    let loser = world.people().iter().find(|p| p.household == lost).unwrap().id;
    assert!(carries(&st, loser).is_empty());
    let other = homes.households().map(|(h, _)| h).find(|h| *h != lost).map(|h| world.people().iter().find(|p| p.household == h).unwrap().id);
    if let Some(o) = other { assert_eq!(carries(&st, o), vec![keycard_id(b, world.people()[o.0 as usize].household)]); }
}
```

- [ ] **Step 2: Run** `cargo test -p strata-zero one_lost_card` — expected: FAIL.

- [ ] **Step 3: Implement**:

```rust
/// The household that left its card at the lobby desk.
pub fn lost_household(st: &AppState, b: BuildingId) -> Option<HouseholdId> {
    let homes = st.interiors.homes_of(b)?;
    let hs: Vec<HouseholdId> = homes.households().map(|(h, _)| h).collect();
    if hs.is_empty() { return None; }
    let seed = st.city.as_ref()?.seed;
    Some(hs[(citygen::rng::hash64(seed, b.index() as u64 ^ 0xCA2D) % hs.len() as u64) as usize])
}

/// Lays the lost card on the lobby floor the first time the lobby exists.
pub fn drop_lost_card(st: &mut AppState, b: BuildingId) {
    if st.journal.lost_cards.contains(&b) { return; }
    let Some(h) = lost_household(st, b) else { return };
    let Some((_, floor)) = st.interiors.floor(b, 0) else { return };
    let tile = floor.stations.iter().find(|s| s.role == Role::Reception)
        .or_else(|| floor.stations.iter().find(|s| s.role == Role::Table))
        .map(|s| s.pos)
        .or_else(|| floor.tiles.iter().find(|(_, t)| t.is_walkable() && t.feature.is_none()).map(|(p, _)| p));
    let Some(tile) = tile else { return };
    st.ground.add(Place::Floor { building: b, level: 0 }, tile, keycard_id(b, h), 1);
    st.journal.lost_cards.insert(b);
}

/// What a person has on them: their flat's keycard, unless it is the lost one.
pub fn carries(st: &AppState, person: sim::PersonId) -> Vec<ItemId> {
    let Some(world) = st.world.as_ref() else { return Vec::new() };
    let p = &world.people()[person.0 as usize];
    let Some(b) = p.home else { return Vec::new() };
    let Some(homes) = st.interiors.homes_of(b) else { return Vec::new() };
    if homes.flat_of(p.household).is_none() || lost_household(st, b) == Some(p.household) { return Vec::new(); }
    vec![keycard_id(b, p.household)]
}
```

A station tile may hold furniture; check `Tile::is_walkable` for a `Reception` station (the receptionist stands on it, so it is). The floor-0 check: `prepare` runs after `ensure` of any level, and an atrium building generates level 0 with any upper level; an ordinary apartment only gets its lobby when the player enters at the street, which is the normal way in. `Ground::take` signature is `take(place, pos, &ItemId, qty) -> u32`.

In the look card: where `inspector_lines(...)` is built for a person in `explore.rs`, append `crate::ui::panel::kv("carries", "a keycard on a lanyard")` when `!crate::credentials::carries(st, id).is_empty()` (the `id` is the `PersonId` the card came from).

- [ ] **Step 4: Run** `cargo test -p strata-zero credentials::` then `cargo test -p strata-zero` — expected: PASS.

- [ ] **Step 5: Commit** `feat(app): a lost keycard at the lobby desk; residents carry theirs`.

---

### Task 6: A server copy writes the building's door codes

**Files:**
- Modify: `crates/strata-zero/src/credentials.rs` (`copy_codes`)
- Modify: `crates/strata-zero/src/net.rs:675-678` (the `(Op::Copy, _)` arm of `land`)

**Interfaces:**
- Produces: `pub fn copy_codes(st: &mut AppState, b: BuildingId, now: u64) -> Option<usize>`: `None` for a building without flats, `Some(n)` new codes.

- [ ] **Step 1: Write the failing tests** in `credentials.rs`:

```rust
#[test]
fn a_server_copy_learns_every_flats_code_once() {
    let (mut st, b) = state();
    // Never walked in: copy_codes prepares the building itself.
    assert!(st.interiors.homes_of(b).is_none());
    let n = copy_codes(&mut st, b, 7).expect("a residential building");
    let flats = st.interiors.homes_of(b).unwrap().households().count();
    assert_eq!(n, flats);
    assert_eq!(st.journal.codes().len(), flats);
    assert!(st.journal.codes().iter().all(|c| c.source == Source::ServerCopy && c.at == 7 && c.household.is_some()));
    assert_eq!(copy_codes(&mut st, b, 8), Some(0));
    let office = st.city.as_ref().unwrap().buildings.iter().find(|x| x.kind == BuildingKind::Office).unwrap().id;
    assert_eq!(copy_codes(&mut st, office, 9), None);
}
```

And extend the explore test that ends with "A server copy gives paydata" (`explore.rs:5034`): after the paydata assertion, if the building `b` there is residential assert `st.journal.codes().len() > 0` and `last(&st).unwrap().contains("door codes are in the journal")`; if it is not, assert the line is exactly `"server copied · a Paydata shard in the pack"`. Read what `b` is in that test (`grep -n "fn .*paydata\|let b =" around line 4980`) and write the one branch that applies.

- [ ] **Step 2: Run** `cargo test -p strata-zero a_server_copy_learns` — expected: FAIL.

- [ ] **Step 3: Implement**:

```rust
/// A server copy on a residential building: every flat's code into the journal.
/// `None` when the building has no flats, else how many codes were new.
pub fn copy_codes(st: &mut AppState, b: BuildingId, now: u64) -> Option<usize> {
    prepare(st, b);
    let homes = st.interiors.homes_of(b).filter(|h| !h.is_empty())?.clone();
    let seed = st.city.as_ref()?.seed;
    let mut new = 0;
    for (h, (level, number)) in homes.households() {
        if st.journal.learn(Code { building: b, level, number, household: Some(h), code: citygen::rng::door_code(seed, b, level, number), source: Source::ServerCopy, at: now }) {
            new += 1;
        }
    }
    Some(new)
}
```

In `net.rs` `land`:

```rust
(Op::Copy, _) => {
    st.character.give(ItemId::new("paydata"), 1);
    let base = "server copied · a `Paydata shard` in the pack";
    match crate::credentials::copy_codes(st, b, now) {
        Some(n) if n > 0 => format!("{base} · `{name}`'s door codes are in the journal"),
        Some(_) => format!("{base} · the door codes were already yours"),
        None => base.to_string(),
    }
}
```

`name` is already bound above the match in `land`.

- [ ] **Step 4: Run** `cargo test -p strata-zero credentials:: explore::tests::` — expected: PASS.

- [ ] **Step 5: Commit** `feat(app): a server copy writes the building's door codes to the journal`.

---

### Task 7: The JOURNAL tab

**Files:**
- Modify: `crates/strata-zero/src/credentials.rs` (`entries`, `Entry`, `stamp`)
- Modify: `crates/strata-zero/src/screens/deck.rs` (`Row` gains `spans`, `Detail::Journal(usize)`, `journal_rows`, `detail`, the draw loop)
- Modify: `crates/strata-zero/src/ui/hud.rs:26` (a `day_at(tick) -> u32` beside `clock_at`)

**Interfaces:**
- Produces: `pub struct Entry { pub building: BuildingId, pub kind: EntryKind, pub text: String, pub at: Option<u64> }`, `pub enum EntryKind { Code(Code), Card(ItemId), Backdoor }`, `pub fn entries(st: &AppState) -> Vec<(BuildingId, String, Vec<Entry>)>` grouped per building with the header text, `pub fn stamp(minute: u64) -> String` as `day N HH:MM`.

- [ ] **Step 1: Write the failing tests**:

In `credentials.rs`:

```rust
#[test]
fn journal_entries_group_by_building_and_say_whose() {
    let (mut st, b) = state();
    copy_codes(&mut st, b, 130).unwrap();
    let homes = st.interiors.homes_of(b).unwrap().clone();
    let (h, (level, number)) = homes.households().next().unwrap();
    st.character.give(keycard_id(b, h), 1);
    let now = st.net.now(&st);
    let server = st.city.as_ref().unwrap().net.nodes.iter().find(|n| n.building == Some(b) && n.kind == citygen::NodeKind::Server).map(|n| n.id).unwrap();
    st.net.land(b, game::HackTarget::Node(server), game::Target::Node(citygen::NodeKind::Server), game::Op::Backdoor, now);
    let groups = entries(&st);
    assert_eq!(groups.len(), 1);
    let (gb, header, es) = &groups[0];
    assert_eq!(*gb, b);
    let name = crate::net::building_name(&st, b);
    assert!(header.starts_with(&name.to_uppercase()), "{header}");
    assert!(header.contains(" · "), "{header}");
    let owner = st.world.as_ref().unwrap().people()[homes.owner_of(level, number).unwrap().0 as usize].name.clone();
    let code = es.iter().find(|e| matches!(&e.kind, EntryKind::Code(c) if c.number == number)).unwrap();
    assert!(code.text.starts_with("door code "), "{}", code.text);
    assert!(code.text.contains(&format!("`{owner}`'s flat `{number}`, level {level}")), "{}", code.text);
    assert!(code.text.ends_with("copied off the server, day 1 00:10"), "{}", code.text);
    let card = es.iter().find(|e| matches!(e.kind, EntryKind::Card(_))).unwrap();
    assert!(card.text.starts_with("keycard · ") && card.text.ends_with("in your pack"), "{}", card.text);
    let door = es.iter().find(|e| matches!(e.kind, EntryKind::Backdoor)).unwrap();
    assert!(door.text.starts_with(&format!("backdoor · `{name}`'s gate is yours · planted day")), "{}", door.text);
    assert_eq!(stamp(0), "day 1 22:00");
    assert_eq!(stamp(120), "day 2 00:00");
}
```

(`stamp(130)` = 22:00 + 130 min = day 2 00:10, so the code line above ends with `day 2 00:10`; fix the literal to match.)

In `deck.rs` tests:

```rust
#[test]
fn the_journal_tab_lists_what_you_know_with_lit_names() {
    let mut st = state();
    st.enter_city(st.city.take().unwrap());
    walking(&mut st);
    let city = st.city.as_ref().unwrap();
    let world = st.world.as_ref().unwrap();
    let b = city.buildings.iter().find(|b| matches!(b.kind, BuildingKind::Apartment | BuildingKind::Megablock) && world.people().iter().any(|p| p.home == Some(b.id))).unwrap().id;
    crate::credentials::copy_codes(&mut st, b, 5).unwrap();
    let mut d = Deck::open(&st);
    d.set_tab(Tab::Journal, &st);
    let rows = d.rows(&st);
    assert!(rows[0].detail.is_none(), "a header first");
    assert!(rows[0].text.starts_with(&crate::net::building_name(&st, b).to_uppercase()));
    assert!(rows.iter().any(|r| r.text.starts_with("door code ") && r.spans.is_some()));
    let text = draw(&d, &st, 120, 40);
    assert!(text.contains("door code"), "{text}");
    assert!(!text.contains('`'), "marks are stripped: {text}");
    let lines = d.detail(&st, &rows);
    assert!(lines.iter().any(|l| l.to_string().contains("flat")), "{lines:?}");
}

#[test]
fn an_empty_journal_says_so() {
    let st = state();
    let mut d = Deck::open(&st);
    d.set_tab(Tab::Journal, &st);
    assert_eq!(d.rows(&st)[0].text, "nothing known yet");
}
```

Use the real constructor (`grep -n "pub fn open\|pub fn new" crates/strata-zero/src/screens/deck.rs`).

- [ ] **Step 2: Run** `cargo test -p strata-zero journal` — expected: compile error.

- [ ] **Step 3: Implement.** In `hud.rs`:

```rust
/// The day at `tick`, counting from day 1 at tick 0.
pub fn day_at(tick: u64) -> u32 {
    ((START_MINUTE + tick / TICKS_PER_MINUTE) / (24 * 60)) as u32 + 1
}
```

In `credentials.rs`:

```rust
/// `day N HH:MM` for a game minute.
pub fn stamp(minute: u64) -> String {
    let tick = minute * sim::TICKS_PER_MINUTE;
    let (h, m) = crate::ui::hud::clock_at(tick);
    format!("day {} {h:02}:{m:02}", crate::ui::hud::day_at(tick))
}

#[derive(Clone, Debug, PartialEq)]
pub enum EntryKind { Code(Code), Card(ItemId), Backdoor }

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub building: BuildingId,
    pub kind: EntryKind,
    /// One line, names in backticks.
    pub text: String,
    pub at: Option<u64>,
}

fn owner_name(st: &AppState, b: BuildingId, level: i32, number: u16) -> Option<String> {
    let owner = st.interiors.homes_of(b)?.owner_of(level, number)?;
    Some(st.world.as_ref()?.people()[owner.0 as usize].name.clone())
}

/// The journal, grouped per building: (building, header, entries).
pub fn entries(st: &AppState) -> Vec<(BuildingId, String, Vec<Entry>)> {
    let mut by: BTreeMap<BuildingId, Vec<Entry>> = BTreeMap::new();
    for c in st.journal.codes() {
        let whose = owner_name(st, c.building, c.level, c.number).map(|n| format!("`{n}`'s flat")).unwrap_or_else(|| "a flat".to_string());
        let from = match c.source { Source::ServerCopy => format!("copied off the server, {}", stamp(c.at)), Source::Home => "your own door".to_string() };
        by.entry(c.building).or_default().push(Entry { building: c.building, kind: EntryKind::Code(c.clone()), text: format!("door code {:04} · {whose} `{}`, level {} · {from}", c.code, c.number, c.level), at: Some(c.at) });
    }
    for s in &st.character.inventory {
        if s.item.as_str() == HOME_KEYCARD {
            if let Some(rig) = st.home {
                by.entry(rig.home.building).or_default().push(Entry { building: rig.home.building, kind: EntryKind::Card(s.item.clone()), text: format!("keycard · your flat `{}`, level {} · in your pack", rig.home.number, rig.home.level), at: None });
            }
            continue;
        }
        let Some((b, h)) = parse_keycard(&s.item) else { continue };
        let Some((level, number)) = st.interiors.homes_of(b).and_then(|x| x.flat_of(h)) else { continue };
        let whose = owner_name(st, b, level, number).map(|n| format!("`{n}`'s flat")).unwrap_or_else(|| "a flat".to_string());
        by.entry(b).or_default().push(Entry { building: b, kind: EntryKind::Card(s.item.clone()), text: format!("keycard · {whose} `{number}`, level {level} · in your pack"), at: None });
    }
    for b in &st.net.backdoors {
        let at = st.net.hacks.iter().find(|h| h.building == *b && h.op == game::Op::Backdoor).map(|h| h.at);
        let name = crate::net::building_name(st, *b);
        let when = at.map(|a| format!(" · planted {}", stamp(a))).unwrap_or_default();
        by.entry(*b).or_default().push(Entry { building: *b, kind: EntryKind::Backdoor, text: format!("backdoor · `{name}`'s gate is yours{when}"), at });
    }
    let mut groups: Vec<(BuildingId, String, Vec<Entry>)> = by.into_iter().map(|(b, es)| {
        let name = crate::net::building_name(st, b).to_uppercase();
        let district = st.city.as_ref().and_then(|c| c.building_opt(b)).map(|x| c_district(st, x.district)).unwrap_or_default();
        (b, format!("{name} · {district}"), es)
    }).collect();
    groups.sort_by_key(|(b, _, es)| (std::cmp::Reverse(es.iter().filter_map(|e| e.at).max()), b.index()));
    groups
}

fn c_district(st: &AppState, d: citygen::DistrictId) -> String {
    st.city.as_ref().map(|c| c.district(d).name.clone()).unwrap_or_default()
}
```

(Inline `c_district` into the closure if the borrow is simpler; `Reverse(None)` sorts `None` last, which is the "cards only after" rule.) Within a group keep codes first, cards next, the backdoor last, which the three loops already give. `st.net.hacks` is `pub`; `home.number` and `home.level` exist on `citygen::Home`.

In `deck.rs`:
- `Row` gains `pub spans: Option<Line<'static>>` (default `None`; update every `Row { .. }` literal, or add a `Row::plain(text, tone, detail)` helper and use it where convenient).
- `Detail::Journal(usize)` at the end of the enum.
- `journal_rows(st) -> Vec<Row>`:

```rust
fn journal_rows(st: &AppState) -> Vec<Row> {
    let groups = crate::credentials::entries(st);
    if groups.is_empty() {
        return vec![Row { text: "nothing known yet".into(), tone: Tone::Dim, detail: None, spans: None }];
    }
    let lit = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
    let plain = Style::default().fg(TEXT);
    let mut rows = Vec::new();
    let mut i = 0;
    for (_, header, es) in groups {
        rows.push(Row { text: header, tone: Tone::Dim, detail: None, spans: None });
        for e in es {
            let (text, marks) = crate::messages::parse_marks(&e.text);
            let mut spans = Vec::new();
            let chars: Vec<char> = text.chars().collect();
            let mut at = 0;
            for (a, b) in marks.iter().copied().chain(std::iter::once((chars.len(), chars.len()))) {
                if at < a { spans.push(Span::styled(chars[at..a].iter().collect::<String>(), plain)); }
                if a < b { spans.push(Span::styled(chars[a..b].iter().collect::<String>(), lit)); }
                at = b;
            }
            rows.push(Row { text, tone: Tone::Normal, detail: Some(Detail::Journal(i)), spans: Some(Line::from(spans)) });
            i += 1;
        }
    }
    rows
}
```

- In `rows()`, `Tab::Journal => Self::journal_rows(st)`.
- In `detail()`, `Some(Detail::Journal(i)) => Self::journal_card(st, *i)` where:

```rust
fn journal_card(st: &AppState, i: usize) -> Vec<Line<'static>> {
    let all: Vec<crate::credentials::Entry> = crate::credentials::entries(st).into_iter().flat_map(|(_, _, es)| es).collect();
    let Some(e) = all.get(i) else { return vec![warn("nothing here")] };
    let name = crate::net::building_name(st, e.building);
    let district = st.city.as_ref().and_then(|c| c.building_opt(e.building).map(|b| c.district(b.district).name.clone())).unwrap_or_default();
    let mut out = vec![head(&name), Line::from(district)];
    let (text, _) = crate::messages::parse_marks(&e.text);
    out.push(Line::from(text));
    match &e.kind {
        crate::credentials::EntryKind::Code(c) => {
            if let Some(owner) = st.interiors.homes_of(c.building).and_then(|h| h.owner_of(c.level, c.number)) {
                let p = &st.world.as_ref().unwrap().people()[owner.0 as usize];
                out.push(Line::from(format!("{}, {}", p.name, p.profession)));
            }
            out.push(Line::from(format!("flat {} on {}", c.number, citygen::FloorPlan::label(c.level))));
            out.push(Line::from(format!("code {:04}", c.code)));
        }
        crate::credentials::EntryKind::Card(id) => { out.extend(Self::item_card(st, id)); }
        crate::credentials::EntryKind::Backdoor => { out.push(Line::from("reachable from any port; the alarm does not shut it")); }
    }
    out
}
```

- In the draw loop, before the `board` branch: `if let Some(line) = &row.spans { ...same span loop as the board branch, with `line.spans`...; continue; }`. Factor the span loop into a small `put_spans(buf, x, y, spans, selected, right)` used by both.

Use the deck's existing `head`/`warn` helpers (`grep -n "^fn head\|^fn warn" crates/strata-zero/src/screens/deck.rs`).

- [ ] **Step 4: Run** `cargo test -p strata-zero journal deck::` — expected: PASS. Then `cargo test -p strata-zero`.

- [ ] **Step 5: Commit** `feat(app): the journal lists codes, cards and backdoors by building`.

---

### Task 8: Your own card and code at new game

**Files:**
- Modify: `crates/strata-zero/src/credentials.rs` (`move_in`)
- Modify: `crates/strata-zero/src/screens/explore.rs:563-575` (`enter_walk`)

**Interfaces:**
- Produces: `pub fn move_in(st: &mut AppState)`: with a home, learns its code with `Source::Home` and gives `keycard:home` once; the catalogue gets the `keycard:home` entry.

- [ ] **Step 1: Write the failing test** in `explore.rs` beside `a_new_game_wakes_up_at_home`:

```rust
#[test]
fn a_new_game_starts_with_your_own_card_and_code() {
    let mut st = state_home();
    let _ = MapScreen::walking(&mut st);
    let h = st.home.unwrap().home;
    let code = st.journal.code_for(h.building, h.level, h.number).expect("your own code");
    assert_eq!(code.source, crate::credentials::Source::Home);
    assert_eq!(code.code, citygen::rng::door_code(st.city.as_ref().unwrap().seed, h.building, h.level, h.number));
    let home_card = game::ItemId::new(crate::credentials::HOME_KEYCARD);
    assert_eq!(st.character.count(&home_card), 1);
    assert!(st.content.item(&home_card).unwrap().name.starts_with("Keycard · your flat"));
    // Waking again (a load) does not stack a second card.
    st.player = None;
    let _ = MapScreen::walking(&mut st);
    assert_eq!(st.character.count(&home_card), 1);
    let groups = crate::credentials::entries(&st);
    assert!(groups.iter().any(|(b, _, es)| *b == h.building && es.iter().any(|e| e.text.contains("your own door"))));
}
```

- [ ] **Step 2: Run** `cargo test -p strata-zero a_new_game_starts_with_your_own` — expected: FAIL.

- [ ] **Step 3: Implement**:

```rust
/// The player's own door: its code in the journal and its card in the pack, once.
pub fn move_in(st: &mut AppState) {
    let Some(rig) = st.home else { return };
    let Some(seed) = st.city.as_ref().map(|c| c.seed) else { return };
    let h = rig.home;
    let now = st.net.now(st);
    st.journal.learn(Code { building: h.building, level: h.level, number: h.number, household: None, code: citygen::rng::door_code(seed, h.building, h.level, h.number), source: Source::Home, at: now });
    let id = ItemId::new(HOME_KEYCARD);
    if st.content.item(&id).is_none() {
        let building = crate::net::building_name(st, h.building);
        st.content.items.push(Item { id: id.clone(), name: format!("Keycard · your flat, {building} {}", h.number), kind: ItemKind::Component, slot: None, weight: 0.01, value: 40, desc: format!("Your own door, {} at {building}. Do not lose it.", h.number), perks: Vec::new(), grants: Vec::new(), on_use: None, program: None, deck: None });
    }
    if st.character.count(&id) == 0 { st.character.give(id, 1); }
}
```

In `enter_walk`, right after the `prepare(st, rig.home.building)` line from Task 4, call `crate::credentials::move_in(st);`. `purge` already removes `keycard:home` on a new city (prefix match).

- [ ] **Step 4: Run** `cargo test -p strata-zero a_new_game explore::tests::` — expected: PASS.

- [ ] **Step 5: Commit** `feat(app): a new game starts with your own door code and keycard`.

---

### Task 9: Whole-workspace check, docs and the fuzz

**Files:**
- Modify: `README.md` (the paragraph on the flat and the Net: one or two sentences on keycards, door codes and the journal)
- Modify: `docs/superpowers/specs/2026-10-17-credentials-journal-design.md` status line to "built".

- [ ] **Step 1: Run** `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. Fix what fails.
- [ ] **Step 2: Run the ignored city fuzz** that enters every construct and the interiors fuzz if any: `cargo test -p strata-zero -- --ignored 2>&1 | tail -20` (expect them green; they may take a minute).
- [ ] **Step 3: README.** After the sentence about the flat's rig, add: "Locked flats open for their resident's keycard, an item you may find at a lobby desk, or for the flat's four-digit door code, which a server copy in the building's construct writes to the JOURNAL tab beside the keycards you hold and the backdoors you planted." Keep the README's voice.
- [ ] **Step 4: Commit** `docs: keycards, door codes and the journal`.
- [ ] **Step 5: Smoke by hand** if a smoke script exists under `smoke/` for interiors; otherwise run `cargo run --target-dir target/smoke -p strata-zero -- --help` only to confirm the binary builds; do not touch `target/release`.
