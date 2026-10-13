# Items: perks, abilities, actions and the deck menus — design

**Status:** approved in chat on 2026-10-13 (sub-project A of two). Sub-project B,
`2026-10-13-world-interact-design.md`, puts items on the map, adds the world
interact key, the abilities key and the dev console. Both share the key rule
below: **Space is the one interact key**, in the deck and in the world.

## Goal

Items become real game objects. A definition carries modifiers to stats or
skills and may grant an ability while equipped. Consumables do something when
used. The deck acts on items the way Caves of Qud does: Space on an inventory
row opens a verb popup (inspect, equip or unequip, use, drop) with a hotkey
letter per verb; Space on a gear slot opens a picker of the items that fit; Esc
backs out one level. Everything a menu does is an `Action` applied by one
function in the `game` crate, so the rules are unit-tested without a terminal.

## Scope

In: content model (perks, grants, use, abilities, starter abilities) and its
validation; character values derived from equipment; the ground model (piles of
stacks per stratum and position); `Action` and `apply`; the generic popup
screen; deck interactions (slot picker, verb popup, inspect card, ABILITIES
tab, status strip, capacity readout); the starter kit updated to use all of it.

Out (sub-project B): drawing ground items, Space and `a` in the world, the
console. Out (later rounds): saving the character or the ground, loot
generation, combat numbers, firing actives from the deck.

## Content model (`crates/game/src/model.rs`, `flavor/content.ron`)

```rust
pub enum Perk { Stat(Stat, i8), Skill(SkillId, i8), Sight(i8), Carry(f32), HpMax(i32) }
pub enum Use { Heal(u32) }
pub struct Item { …, #[serde(default)] perks: Vec<Perk>, #[serde(default)] grants: Vec<AbilityId>,
                  #[serde(default, rename = "use")] on_use: Option<Use> }
pub enum Active { Scan { radius: i32 }, Ping { radius: i32, ticks: u64 } }
pub enum AbilityKind { Passive, Active(Active) }
pub struct Ability { id: AbilityId, name: String, desc: String, cooldown: u64, kind: AbilityKind }
pub struct Content { …, abilities: Vec<Ability> }
pub struct Starter { …, abilities: Vec<AbilityId> }
```

- `AbilityId` joins the `id_type!` family and the global duplicate-id check.
- Perks apply while the item is equipped. `Sight` adds to the walker's field of
  view radius, `Carry` to carry capacity in kilograms, `HpMax` to maximum HP,
  `Stat` and `Skill` to the named stat or skill. `cooldown` is in ticks (30
  per second); `0` means none.
- Validation adds: perk skills exist; granted abilities exist; `use` only on
  `Consumable` items; `perks` and `grants` only on items with a slot; starter
  abilities exist and are not duplicated.
- Starter kit changes: optics `Sight(4)` and grant `lowlight`; pawnshop deck
  `Skill(hacking, 1)` and grant `netscan`; neural port grants `ping`; grip arm
  `Carry(12.0)` and `Stat(Body, 1)`; salvage jacket `HpMax(5)`; hardhat
  `HpMax(2)`; cargo pants `Carry(4.0)`; monowire shiv `Skill(blades, 1)`; stim
  patch `use: Heal(15)`; soy-noodle cup `use: Heal(3)`. Abilities: `netscan`
  (active Scan radius 12, cooldown 300), `ping` (active Ping radius 24 for 180
  ticks, cooldown 240), `lowlight` (passive), `street_sense` (passive, innate:
  the starter's `abilities: ["street_sense"]`).

## Character (`crates/game/src/character.rs`)

Nothing is cached; every derived value scans the equipment on read.

- `hp_max` becomes a field `hp_base` plus `hp_max(&self, c) -> u32` =
  `max(1, hp_base + Σ HpMax)`. `from_starter` starts `hp` at `hp_max`.
- `perks(&self, c) -> impl Iterator<Item = &Perk>` over equipped items.
- `stat(&self, c, Stat) -> u8` = base + Σ `Stat` perks, clamped to 0..=10.
  `Stats::get(Stat) -> u8` is added.
- `skill_level(&self, c, id) -> u8` = base + Σ `Skill` perks, floor 0
  (signature gains `c`; `skill_base(id)` keeps the raw level).
- `sight_bonus(&self, c) -> i32` = Σ `Sight`. The walker's `update_fov` takes
  a `bonus: i32` added to its stratum radius; every caller passes
  `character.sight_bonus(&content)`.
- `carry_cap(&self, c) -> f32` = `8.0 + 2.0 × body` (base stat) + Σ `Carry`.
  `overburdened(&self, c)` = `weight > carry_cap`.
- `abilities(&self, c) -> Vec<(AbilityId, Source)>` with `Source::Innate |
  Source::Item(ItemId)`: innate first, then granted in `Slot::ALL` order,
  deduplicated by id (first source wins).
- `fired: BTreeMap<AbilityId, u64>` records the tick an active last fired;
  `cooling(&self, c, id, tick) -> Option<u64>` returns the ticks left.

## Ground (`crates/game/src/ground.rs`)

`Ground` holds piles: `HashMap<(StratumKind, Pos), Vec<Stack>>` (the `game`
crate gains a dependency on `citygen` for `Pos` and `StratumKind`, which must
derive `Hash`; citygen does not depend on `game`). API: `add(stratum, pos,
item, qty)` merges into an existing stack; `take(stratum, pos, item, qty) ->
u32` returns what was actually taken and drops empty stacks and piles; `at(stratum,
pos) -> &[Stack]`; `clear(stratum, pos)`; `on(stratum) -> impl Iterator<Item =
(Pos, &[Stack])>`.

## Actions (`crates/game/src/action.rs`)

```rust
pub enum Action { Equip(ItemId), Unequip(Slot), Drop { item: ItemId, qty: u32 }, Use(ItemId),
                  Take { item: ItemId, qty: u32 }, TakeAll, Fire(AbilityId) }
pub struct At { pub stratum: StratumKind, pub pos: Pos }
pub enum Effect { Reveal { radius: i32 }, Ping { radius: i32, ticks: u64 } }
pub struct Outcome { pub message: String, pub effects: Vec<Effect> }
pub fn apply(ch: &mut Character, ground: &mut Ground, c: &Content, at: At, tick: u64,
             action: Action) -> Result<Outcome, String>
```

Rules, each a refusal (`Err`) with a plain reason when it fails:

- **Equip**: the item is in the inventory and has a slot. One leaves the stack;
  the slot's previous occupant, if any, returns to the inventory. Message
  `equipped Ratchet SMG` or `swapped Hardhat for Salvage jacket`.
- **Unequip**: the slot is occupied; the item returns to the inventory.
- **Drop**: `qty` clamps to what is carried and must end above zero; the stack
  moves to the ground at `at`. Message `dropped 3× Copper coil`.
- **Use**: the item is carried and has a `use`. `Heal(n)` raises `hp` by up to
  `n`, capped at `hp_max`; one is consumed. Message `used Stim patch · +15 hp`
  with the amount actually healed.
- **Take**: the pile at `at` has the item. Refused when already overburdened
  (`over capacity · 18.2/16.0 kg`); a take may carry the character over.
  **TakeAll** takes every stack at `at` under the same rule, message `took 3
  items`.
- **Fire**: the ability is in `abilities()` and active; refused while cooling
  (`Netscan cooling down · 7s`). Records the tick, returns the effect as data.
- After any equipment change `hp` clamps to the new `hp_max`.

The app wraps this in `cyberpunk::actions::perform(st: &mut AppState, action)
-> Result<String, String>`: `at` is the player's stratum and position (refusal
`no body in the city` without a player), the outcome's message goes to the
status strip, and effects run at once: `Reveal` marks every tile within the
radius on the player's stratum `Remembered` unless already `Visible`; `Ping`
stores `AppState::ping = Some(Ping { stratum, pos, radius, until })` for
sub-project B's overlay. `AppState` gains `ground: Ground` and `ping`.

## The popup (`crates/cyberpunk/src/screens/popup.rs`)

One generic screen pushed over whatever opened it.

```rust
pub enum Pick { Act(Action), Card(Vec<Line<'static>>), Close }
pub struct Choice { pub key: char, pub label: String, pub pick: Pick, pub enabled: bool }
pub struct Popup { title: String, choices: Vec<Choice>, selected: usize, anchor: Option<(u16, u16)>,
                   opened: u64, card: Option<Vec<Line<'static>>> }
impl Popup { pub fn verbs(title, choices, tick) -> Self; pub fn card(title, lines, tick) -> Self;
             pub fn anchored(self, x: u16, y: u16) -> Self }
```

- Keys: `↑↓`/`j k` move over enabled rows; `Enter` or `Space` fires the
  selected row; a row's letter fires that row; `Esc`/`q` close. A card closes
  on any of `Esc`, `q`, `Enter`, `Space`.
- Firing `Act` calls `perform`; success or refusal lands in the status strip
  and the popup pops with `TransitionKind::None`. `Card` replaces the popup's
  body with the lines (a second Esc returns to the verbs). `Close` pops.
- Layout: width = longest row + 6, min 24, max 48; height = rows + 3 (title,
  rule, rows) + 2 frame rows. Anchored at `(x, y)` when given, clamped inside
  the terminal, else centred. The panel is the deck's glass over whatever is
  below, the deck's heavy rules and corners, the title in cyan bold on the top
  rule, the hotkey letter in magenta, the label in TEXT (DIM and no letter when
  disabled), the selected row on `BAR_BG` with a `▸`. Rows reveal with the
  deck's typewriter over 10 ticks.
- The screen stack washes everything under a pushed screen; the popup's glass
  tints only its own rect, so the deck stays readable behind it.

## Deck changes (`crates/cyberpunk/src/screens/deck.rs`)

- Seven tabs: ABILITIES joins after SKILLS; `1-7` jump.
- A status strip takes the row above the hints: `st.status_text()` in cyan
  with the typewriter, magenta when the status is a warning. `AppState` gains
  `set_warning(msg, ttl)` and `status_is_warning()`; the map's status bar uses
  the same flag.
- `Space` or `Enter` on a row interacts:
  - Gear slot: picker popup titled with the slot. Rows: one per inventory stack
    whose item fits the slot (`a`, `b`, …, label = item name), then when the
    slot is occupied `u unequip` and `i inspect`. An empty slot with nothing
    fitting opens no popup; the status strip says `nothing in the pack fits
    <slot>`.
  - Inventory item: verbs `i inspect`, `e equip` when the item has a slot, `u
    use` when it has a use, `d drop one`, `D drop all` when the stack is
    larger than one. Inspect is a card: the item's name, kind and slot, weight
    and value, each perk (`+4 sight`, `+1 hacking`, `+5 max hp`, `+12.0 kg
    carry`, `+1 body`), `grants Netscan`, `equipped` when it is, the
    description.
  - Any other row: nothing.
  The popup anchors to the right of the list pane on the selected row.
- Readout: `¤ 320 · hp 40/45 · 11.6/16.0 kg`; the weight part turns magenta
  when overburdened.
- Skills rows use the effective level; the detail pane adds `base 3 · +1
  Pawnshop deck` when a perk applies.
- Gear and inventory detail panes list perks and grants as the card does.
- ABILITIES rows: `◈ Netscan      active · ready`, `◈ Ping         active ·
  cooling 7s` (Warn tone), `· Low-light    passive` (Dim). Detail: name, kind,
  source (`innate` or `from Pawnshop deck`), cooldown, the effect in words
  (`scan: paints the map within 12 tiles`, `ping: lights terminals and stairs
  within 24 tiles for 6s`), description.
- Hints row: `←→ tabs · ↑↓ select · 1-7 jump · space act · esc close`.

## Keys

| Where | Key | Does |
|---|---|---|
| Deck | `Space` / `Enter` | interact with the selected row |
| Deck | `1`-`7` | jump to a tab |
| Popup | `↑↓` `j k` | move |
| Popup | letter, `Enter`, `Space` | fire a verb |
| Popup | `Esc` `q` | back out one level |

## Testing

`game`: new fields parse with defaults; every new validator rejection; derived
values with and without equipment (sight, carry, hp max, stat, skill); ability
sources and dedup; cooldown; `Ground` merge, partial take, empty cleanup;
`apply` for every action and refusal, swap on equip, hp clamp on unequip, heal
cap, overburdened take; the embedded content still passes and the starter
starts at full hp with 45 max.

`cyberpunk`: `perform` reveals memory and stores a ping; the popup renders
title, letters and rows at 120x35 and 100x30, moves only over enabled rows,
fires by letter and by Enter, cards close; the deck's Gear picker lists only
fitting items and equips on pick; an inventory verb popup shows the right
verbs for a stim patch and for a coil stack; `d` moves one coil to the ground
at the player's feet and the row count drops; `u` on a damaged character
raises hp; ABILITIES renders ready and cooling states; the readout shows
capacity and goes magenta when overburdened; a refusal shows in the status
strip; width test covers every new glyph.
