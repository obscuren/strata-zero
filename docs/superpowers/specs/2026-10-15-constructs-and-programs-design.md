# Constructs and programs — design (netrunning, spec 1 of 3)

Date: 2026-10-15. Status: approved in chat.

## Goal

The first playable run. A building's construct has an inside: a small Net map
of rooms off a spine, the building's nodes in the rooms, ICE standing at the
gate and at the vault, the vault deepest. A netrunner jacks in, crosses the
grid to the construct, passes the gate, and runs programs from their deck
against nodes, in turns. A program rolls skill against the subnet's security
and adds noise to the construct's trace; the trace raises the alarm and then
dumps you. What you hack changes the real building: locks open, cameras go
blind, lifts answer, vaults give paydata, a backdoor keeps the construct open.

## Decisions taken with Jeffrey

- **The run is a second map of the same job**; a solo dips in with a cheap
  deck, a netrunner lives there.
- **Graded ICE**: white traces and alarms, grey dumps, black (later) hurts
  the meat. Security says which grades a construct has.
- **Hacking is programs against nodes, in turns.** No minigame.
- **A construct's inside is a node map** with rooms, ICE on patrol later, the
  vault deepest.
- **Programs are items loaded into deck slots.** The pawnshop deck has two
  slots and ships with crack and copy. Netscan, ping and arc trace stay
  abilities.
- **The alarm is a lockdown**: an hour of world time in which cracked locks
  relock, lifts stop, the construct's ICE wakes, the gate closes and nearby
  ports refuse. Guards come with combat.

Assumptions carried in: ICE does not move in this spec (spec 2: patrols,
pursuit, ICE combat, black ICE, stealth); paydata is an item with a value and
nobody buys it yet (spec 3: the fixer, backdoors as jobs); constructs and the
Net's state are generated and kept per session like interiors, never saved;
nothing hurts the meat.

## Content

### Items

```ron
Item(id: "pawnshop_deck", kind: Deck, slot: Some(Deck), ..., deck: Some(DeckSpec(slots: 2, rating: 1)), ...),
Item(id: "crack", name: "Crack", kind: Program, weight: 0.0, value: 150,
     program: Some(Program(op: Crack, rating: 1, size: 1, noise: 4, work: 3)),
     desc: "Brute force with manners. Opens anything that answers, loudly."),
Item(id: "spoof", ..., program: Some(Program(op: Spoof, rating: 1, size: 1, noise: 2, work: 2))),
Item(id: "blind", ..., program: Some(Program(op: Blind, rating: 1, size: 1, noise: 2, work: 2))),
Item(id: "copy", ..., program: Some(Program(op: Copy, rating: 1, size: 1, noise: 3, work: 4))),
Item(id: "backdoor", ..., program: Some(Program(op: Backdoor, rating: 1, size: 2, noise: 2, work: 5))),
Item(id: "crack_2", name: "Crack II", ..., program: Some(Program(op: Crack, rating: 2, size: 2, noise: 3, work: 3))),
Item(id: "copy_2", name: "Copy II", ..., program: Some(Program(op: Copy, rating: 2, size: 2, noise: 2, work: 3))),
Item(id: "paydata", name: "Paydata shard", kind: Component, weight: 0.05, value: 300, desc: "..."),
Item(id: "vault_data", name: "Vault dump", kind: Component, weight: 0.05, value: 1500, desc: "..."),
```

- `ItemKind::Program` (no slot, never equipped; loaded instead).
- `Item.program: Option<Program>` with `Program { op: Op, rating: u8, size: u8,
  noise: u8, work: u8 }` and `Op::{Crack, Spoof, Blind, Copy, Backdoor}`.
- `Item.deck: Option<DeckSpec>` with `DeckSpec { slots: u8, rating: u8 }`.
- Validation: a Program item has `program` and no slot; a Deck item has `deck`;
  `size`, `work` and `rating` are at least one; nothing else has `program` or
  `deck`.
- The netrunner profession's kit and the VEX template gain `crack` and `copy`;
  the fixer's kit gains `spoof`. The content's `creation` is unchanged.

### What an op does, and where

| op | targets | skill | effect |
|---|---|---|---|
| Crack | any node, any ICE guard | hacking (ice_breaker on a guard) | a lock unlocks the building's private rooms; a guard stands down; a camera, lift, server, terminal or ad board counts as cracked (reads so, nothing more yet) |
| Spoof | DoorLock, LiftCtrl | hacking | the quiet unlock; a spoofed lift control makes the lifts answer through a lockdown |
| Blind | Camera | hacking | the camera is blind (reads so; stealth later) |
| Copy | DataVault, Server | vault_run | one `vault_data` (vault) or `paydata` (server) into the pack; once per node per window |
| Backdoor | Server | hacking | the construct is backdoored for the session: the gate guard does not stand, heat decays twice as fast |

Every landed hack but a backdoor lasts `HACK_MINUTES = 60` of world time or
until the building's alarm ends, whichever is sooner. A backdoor is forever
(this session).

## Rules (`game`, terminal-free)

### Loading programs

- `Character.loaded: Vec<ItemId>`: the programs loaded into the jacked deck,
  each also carried in the pack. `from_build` loads the kit's programs in kit
  order while they fit the deck's slots.
- `deck(c) -> Option<(&Item, DeckSpec)>` (the Deck slot's item with its spec),
  `loaded_size(c) -> u8`, `can_load(c, id) -> Result<(), String>` (reasons:
  "no deck jacked", "that is not a program", "not in your pack", "already
  loaded", "no room: N of M slots"), `load`, `unload`. Dropping or losing a
  loaded program unloads it; unequipping the deck keeps the list but nothing
  runs without a deck.
- `Action::Load(ItemId)` / `Action::Unload(ItemId)` with messages
  `"crack loaded · 2/2 slots"` / `"crack unloaded"`.

### Resolving a program (`game::run`)

```rust
pub struct Program { pub op: Op, pub rating: u8, pub size: u8, pub noise: u8, pub work: u8 }
pub enum Target { Node(NodeKind), Guard(IceGrade) }
pub fn applies(op: Op, target: Target) -> bool;
pub fn skill_for(op: Op, target: Target) -> SkillId;   // hacking, ice_breaker, vault_run
/// 2 × Net + 3 with the skill + 2 × rating + 2 with the talent flag (ice_pick for Crack, deep_dive for Copy).
pub fn power(ch: &Character, c: &Content, p: &Program, target: Target) -> i32;
pub fn difficulty(security: u8) -> i32;                 // security / 8
/// One turn of work: the roll (1..=10) plus power beats the difficulty.
pub fn succeeds(roll: u8, power: i32, difficulty: i32) -> bool;   // roll as i32 + power > difficulty
pub struct Run { pub program: ItemId, pub target: HackTarget, pub done: u8, pub work: u8 }
```

A failed turn still adds `noise + 2` heat; a successful one adds `noise`.
The roll is `hash64(clock_tick, target seed) % 10 + 1` in the app: a replay
of the same turn gives the same roll.

### The trace

```rust
pub const ALARM_HEAT: u8 = 60;
pub const DUMP_HEAT: u8 = 100;
pub const ALERT_MINUTES: u64 = 60;
pub const HACK_MINUTES: u64 = 60;
pub const COOLING_MINUTES: u64 = 10;
pub struct Trace { pub heat: u8, pub alert_until: Option<u64> }
impl Trace {
    pub fn add(&mut self, noise: u8, now: u64) -> TraceEvent;   // None | Alarm | Dump
    pub fn alert(&self, now: u64) -> bool;
    /// One game minute passes with nobody inside: heat falls by one (two with a backdoor); an alert that ends resets heat.
    pub fn cool(&mut self, now: u64, backdoor: bool);
}
```

- Heat crosses `ALARM_HEAT` while not alert: `Alarm`, `alert_until = now +
  ALERT_MINUTES`; every hack on that building but a backdoor expires now.
- Heat reaches `DUMP_HEAT`: `Dump`.
- Heat falls one a game minute while the player is not inside that construct;
  when the alert ends, heat is zero.

## Constructs (`citygen::construct`)

```rust
pub enum IceGrade { White, Grey }
pub struct Guard { pub pos: Pos, pub grade: IceGrade }
pub struct Construct {
    pub building: BuildingId,
    pub security: u8,
    pub size: Size,
    pub tiles: Grid<Tile>,       // NetWall, NetFloor, NetLane (the spine), NetGate (room doorways and the entry)
    pub light: Grid<u8>,
    pub entry: Pos,              // the NetGate on the border: in from the city gate, out to it
    pub rooms: Vec<Rect>,        // in order along the spine; the last holds the vault when there is one
    pub nodes: Vec<(NodeId, Pos)>,
    pub guards: Vec<Guard>,
}
pub fn generate_construct(city: &City, b: BuildingId) -> Option<Construct>;   // None for a building with no subnet
pub fn validate_construct(c: &Construct) -> Result<(), String>;
```

- Deterministic from `hash(hash(seed, building), "construct")`.
- Size: the footprint doubled, clamped to 14×9 .. 40×24. The spine is a
  two-wide `NetLane` along the long axis from the entry on a short side to the
  far wall; rooms of 4–7 by 3–5 open off both sides every few tiles, walled in
  `NetWall`, each with one `NetGate` doorway onto the spine; the far end of the
  spine ends in the last room (the vault room when the building has a vault,
  else the server room).
- Nodes: every `city.net` node with `building == Some(b)` whose kind is not
  `Ice`, placed one or two per room along the spine in this order: door locks,
  lift controls, cameras, terminals, ad boards, then the server in the
  second-to-last room and the vault in the last. A node is a `Feature::Node(kind)`
  on a `NetFloor` tile that is not a doorway.
- Guards: a gate guard on the first spine tile past the entry when the subnet's
  security is at least `ICE_THRESHOLD` (160): `White` below 200, `Grey` from
  200. A vault guard, `Grey`, on the vault room's doorway when the building has
  a vault and security is at least 200. A guard is `Feature::Ice(grade)` on its
  tile; the tile is not walkable while the guard stands.
- Light: 90 base plus the security glow of the Net stage (lanes 255).
- Validation: the entry is a `NetGate` on the border; every node tile and the
  far room are reachable from the entry over walkable tiles with the guards
  lifted; no two nodes share a tile; guards stand on tiles that would be
  walkable; the same seed gives the same construct.
- `Feature::Ice(IceGrade)` joins `Feature` (at the end; `ALL_FEATURES` grows);
  the tileset draws it as `▣` in signal magenta (grey ICE) or white, the
  inspector reads "ICE · white" / "ICE · grey, down".
- `Place::Construct(BuildingId)` joins `Place`. `MapRef::Construct` wraps it;
  sight radius `RADIUS_INSIDE`; memory per place as for floors.

## Moving between the Net and a construct (app)

- `AppState.constructs: Constructs` caches generated constructs, like
  `Interiors`; `AppState.net: NetState` holds `traces: BTreeMap<BuildingId,
  Trace>`, `hacks: Vec<Hack { building, target: HackTarget, op, until: Option<u64> }>`,
  `backdoors: BTreeSet<BuildingId>`, `cooling_until: Option<u64>`, `run:
  Option<Run>`. Session-only.
- On the Net stratum, stepping onto a construct's `NetGate` tile from the open
  grid enters the construct: the player lands on `entry`, the log reads `into
  Hextek Holdings` and the location line `Hextek Holdings · construct`. During
  that building's alert the gate refuses: `gate closed · Hextek Holdings on
  alert`. Stepping from `entry` outward (onto the border) leaves onto the city
  gate tile. Jacking out works from inside a construct (the body is where it
  was).
- Jacking in: refused while `cooling_until` is in the future (`deck cooling ·
  7 min`) and at a port whose Street tile lies inside, or 4-adjacent to, an
  alert building's footprint (`port locked down`).
- A guard's tile is not walkable while it stands; a cracked guard's tile is.

## Running a program (app)

- `space` on a construct with a node or a standing guard on or beside the
  player opens the RUN popup: one row per loaded program whose op applies to
  that target, `crack · 17 vs 13 · 3 turns · noise 4`, dim when the program
  cannot help (`copied already` for a copied vault, `at its best`...), plus
  `look` (the node's card: kind, what it does, hacked state). Picking a row
  starts `net.run`.
- A run steps on every turn the player passes standing still (`.`, or `space`
  → `continue`): a roll, `done` or not, heat added, the log line `crack 2/3`
  or `crack slips · heat 46`. Moving, jacking out, or a dump clears the run
  (`run aborted`). When `done == work` the hack lands: the effect above, the
  log line (`lock cracked · Hextek Holdings' doors open for an hour`), the
  `Fx::Kick(6)` glitch.
- Heat and alert: the sidebar's HERE section shows `TRACE ▰▰▰▱▱▱▱▱▱▱ 34` while
  the player is in a construct, `ALERT · 48 min` when alert. `TraceEvent::Alarm`
  logs `ALARM · Hextek Holdings locks down` in magenta. `TraceEvent::Dump`, or
  ending a turn 8-adjacent to a standing guard while the building is alert,
  dumps the player: `jack_out` hard, focus to zero, `cooling_until = now +
  COOLING_MINUTES`, the run cleared, `Fx::Kick(20)`, the log `DUMPED` in
  magenta.
- Lockdown on the street: the building's private rooms are locked whatever was
  cracked; `ride` refuses (`lifts locked down`) unless a Spoof on the lift
  control landed after the alarm; the HUD location line and the inspector add
  `· ALERT`. The lockdown ends with the alert.
- Effects on the real building: `NetState::unlocked(b, now)` is consulted where
  explore refuses a locked room; `blinded(camera node)` by the inspector and
  the tileset (a blind camera draws dim); `lifts_answer(b, now)`; copy puts the
  item in the pack and marks the node copied until the window ends.
- Heat decay runs with the world clock: on each game minute, every trace not
  containing the player cools.

## The CYBERDECK tab

- Rows: the deck (`Pawnshop deck · 1/2 slots · rating 1`), `── loaded ──`
  rows `◆ Crack · size 1`, `── in the pack ──` rows `◇ Spoof · size 1`, then
  `── programs ──` as today for the abilities. `space` on a program row loads
  or unloads it through `Action::Load`/`Unload`; the refusal lands in the strip.
  The detail reads the op, what it targets, rating, size, noise, work, and
  the power the character brings against a typical mid-city construct.
- The inventory tab shows programs grouped under `program`, with `loaded`
  as a marker like `equipped`.

## Dev console

`program <id>` spawns and loads a program; `heat <n>` sets the heat of the
construct the player stands in; `alarm` trips its alarm.

## Out of scope

ICE that moves or fights back, black ICE and meat damage, stealth programs,
selling paydata, buying programs, Net jobs, guards on the street, NPC
netrunners, saving the Net's state.

## Tests

game: item validation (program without spec, deck without spec); loading
rules in order; `from_build` loads the kit; `applies`, `skill_for`, `power`
with and without skill and flag, `difficulty`, `succeeds`; `Trace::add`
crossing alarm and dump, `cool` with and without backdoor, alert ending
resets; the actions. citygen: construct generation for every closed building
of the test cities validates; nodes all placed; guards by security; the
vault deepest; determinism; `Feature::Ice` round-trips. app: entering and
leaving a construct through the gate, the gate refusing on alert, jacking
out from inside; the RUN popup's rows and dim rows; a run stepping on wait,
aborted by a move, landing a crack that unlocks a flat; the alarm locking
the flat again and stopping the lift; a dump on heat; a dump beside an awake
guard; the port refusing while cooling and on lockdown; copy giving paydata
once; backdoor removing the gate guard; the cyberdeck tab loading and
unloading with the slots; the inventory marker; the trace meter and ALERT in
the sidebar; the location line; the console commands. A smoke run through a
whole run and screenshots of a construct and the RUN popup.
