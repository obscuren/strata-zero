# Grenades and Abilities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Thrown grenades with an area cursor, a blast engine with falloff and cover, three blast looks and two throw arcs, a fused blast the goon steps out of, four new abilities (Shove, Redline, Jolt, Deadeye) each with its own look, Static on ICE, and one `ACT` list on the number keys.

**Architecture:** Rules and content live in `game` (`Blast`, `Use::Throw`, the new `Fx` shapes, the grown `Active`, `blast_damage`, `Action::Throw`). The app's `combat.rs` gains the blast engine (`footprint`, `detonate`, `throw`, live blasts) and the three ability casts beside `strike`; `foes.rs` learns to step out of a live footprint; `net.rs` lends the piece-hit path to strikes; `explore.rs` arms the cursor and builds the `ACT` list; `hud.rs` draws it; `render/path.rs` shades the footprint and draws live blasts; `render/people.rs` draws the mark; `render/fx.rs` draws the six new shapes from the mock.

**Tech Stack:** Rust 2024 workspace (`citygen`, `sim`, `game`, `strata-zero`), ratatui + crossterm, RON content, `cargo test` with `TestBackend` buffers.

**Spec:** `docs/superpowers/specs/2026-10-20-grenades-abilities-design.md`; art reference `docs/superpowers/specs/2026-10-20-grenades-abilities-mock.html` (glyphs, colours, ticks; it wins over the text).

## Global Constraints

- Every glyph, colour and tick of a shape comes from spec §5 / the mock; copied below where a task needs them.
- Constants: `THROW_SPEED = 2`, `THROW_SETTLE = 2` (landing tick = `ceil(dist / 2) + 2`), `BLOOM_TICKS = 13`, `WHITEOUT_TICKS = 8`, `SHOCK_TICKS = 4`, `TRAIL_TICKS = 4`, `PULSE_TICKS = 7`, `SCORCH_TICKS = 3600`, `MARK_PULSE = 8` ticks, `STRIKE_NOISE = 3` (the attack program's `noise`).
- Blast damage: `damage · (radius + 1 − ring) / (radius + 1)`, floor, at least 1 when `damage > 0`; blasts never roll; every ring takes the full stun.
- Copy, verbatim: `not in a fight`, `not your turn`, `move first · space to stay`, `no AP left`, `none left`, `out of range`, `no line`, `no landing`, `pick a target`, `throw it from the map`, `use it from the map`, `not in here`, `once a fight`, `you're stunned`, `your turn again`, `nothing in it`, `you're in it`, `back`, `marked`, `fuse`, `ACT`, `PROGRAMS`, `ABILITIES`.
- Content numbers: frag range 8 / radius 2 / 12; flashbang 8 / 2 / 0 stun 2; EMP puck 7 / 2 / 3 stun 2 chrome; satchel 6 / 3 / 16 fuse 1. Jolt range 3 / 5 / stun 1 / `electronics` / Tech / ap 1 / chain 1, cooldown 180; Shove 1 / 3 / `athletics` / Body / ap 1 / push 2, cooldown 60; Redline `Boost(once: true)`, cooldown 900; Deadeye `Mark(range: 12, bonus: 20, ap: 1)`, cooldown 120.
- Keys on your act half on the street: `1`–`9` the `ACT` list (abilities then throwables), `space` fires or throws, `esc` disarms then drops the cursor, `t` next foe, `.` ends the turn. In a construct the programs keep `1`–`5` and the net-capable abilities take `6`–`8`.
- Wording comes from kinds (`item.name`, `ability.name`, `foe.name`), never a hard-coded "frag" or "goon" outside content, the lot and tests.
- Work stays in the `grenades-abilities` worktree; commits end with the attribution lines in the session reminder; `target/release/strata` is never rebuilt; smoke runs use `--target-dir target/smoke`.
- Before every commit: `cargo test -p game -p strata-zero` green (the failing-test steps excepted), `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt`.

## Review Focus

1. A throw aimed at the tile you stand on, or a radius that covers you: allowed, the readout red with `you're in it`, you take your ring's damage and stun, a death reads `your own frag`. (Task 3 tests `a_blast_hits_you_by_your_ring`, Task 6 tests `armed_readout_warns_when_you_are_in_it`.)
2. A blast with no tile but the centre (thrown into a doorway with walls all round): the footprint is one tile, nothing panics, the log reads `nothing in it` when no one stands there. (Task 3 tests `a_footprint_walled_in_is_the_centre_alone`.)
3. Shove on a foe with its back to a wall or another foe: it moves as far as it can (zero tiles is fine), the trail covers only the tiles left, damage still lands. (Task 5 tests `shove_stops_at_a_wall` and `shove_against_another_foe`.)
4. The number keys past the list's end, and a digit on a cooling ability or a throwable you ran out of: refused with the ability's cooldown line / `none left`, no AP spent, nothing armed. (Task 6 tests `act_keys_past_the_end_and_cooling`.)
5. A live satchel whose thrower's fight ends before its fuse: it counts calm beats and goes off, hitting whoever stands there, waking idle foes. (Task 3 tests `a_live_blast_counts_calm_beats`.)

---

### Task 1: The model — `Blast`, `Use::Throw`, the new shapes, the grown `Active`, `Action::Throw`

**Files:**
- Modify: `crates/game/src/model.rs` (`Use` ~458, `Item` ~463, `Active` ~531, `Fx` ~551, `impl Fx` ~582)
- Modify: `crates/game/src/run.rs` (beside `hit_chance` ~97)
- Modify: `crates/game/src/action.rs` (`Action` ~19, `Action::Use` arm ~269, `Action::Fire` arm ~531)
- Modify: `crates/game/src/content.rs` (`validate` ~173: the weapon/fx checks ~358–372, the ability fx check ~472)
- Modify: `crates/game/src/lib.rs` (re-exports)
- Test: the `tests` modules of the same files

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
  pub struct Blast { pub radius: i32, pub damage: u8, pub stun: u8, #[serde(default)] pub fuse: u8, #[serde(default)] pub chrome: bool }
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
  pub enum Arc { Lob, Shadow }
  pub enum Use { Heal(u32), Throw { range: i32, arc: Arc, blast: Blast } }
  // Item: #[serde(default)] pub fx: Vec<Fx>
  pub enum Fx { /* as today, plus: */ Throw, Bloom, Whiteout, Shock, Trail, Pulse { colour: (u8, u8, u8) } }
  impl Fx { pub fn is_blast(&self) -> bool /* Bloom | Whiteout | Shock */; pub fn is_shot(&self) -> bool /* grows: Trail, Pulse */ }
  pub enum Active { Scan{..}, Ping{..}, Trace{..},
      Strike { range: i32, damage: u8, stun: u8, skill: SkillId, ap: u8,
               #[serde(default = "Stat::net")] stat: Stat, #[serde(default)] push: u8, #[serde(default)] chain: u8 },
      Boost { once: bool },
      Mark { range: i32, bonus: u8, ap: u8 } }
  pub fn blast_damage(damage: u8, radius: i32, ring: i32) -> u8;   // run.rs
  pub enum Action { /* as today, plus: */ Throw(ItemId) }
  // Item: pub fn throw(&self) -> Option<(i32, Arc, Blast)>  (the Throw use, if any)
  ```
- `Stat::net()` is a free fn `fn net() -> Stat { Stat::Net }` for serde's default (name it `default_net` in model.rs if `Stat::net` reads wrong).

- [ ] **Step 1: Write the failing tests**

In `model.rs` tests (or `content.rs` tests where the parse goes through `Content`):

```rust
#[test]
fn a_throw_use_parses() {
    let item: Item = ron::from_str(r#"Item(id: "x", name: "X", kind: Consumable, weight: 0.4, value: 1,
        use: Some(Throw(range: 8, arc: Lob, blast: Blast(radius: 2, damage: 12, stun: 0))),
        fx: [Throw, Bloom, Chips, Number(tag: true)], desc: "")"#).unwrap();
    assert_eq!(item.throw(), Some((8, Arc::Lob, Blast { radius: 2, damage: 12, stun: 0, fuse: 0, chrome: false })));
    assert!(item.fx.iter().any(Fx::is_blast));
}
#[test]
fn a_strike_parses_with_its_defaults_and_its_extras() {
    // Static's entry, unchanged, reads stat Net, push 0, chain 0.
    // A new entry with `stat: Body, push: 2` reads them.
}
#[test]
fn boost_and_mark_parse() { /* Active(Boost(once: true)), Active(Mark(range: 12, bonus: 20, ap: 1)) */ }
```

In `run.rs` tests:

```rust
#[test]
fn blast_damage_falls_off_by_ring() {
    assert_eq!(blast_damage(12, 2, 0), 12);
    assert_eq!(blast_damage(12, 2, 1), 8);
    assert_eq!(blast_damage(12, 2, 2), 4);
    assert_eq!(blast_damage(16, 3, 3), 4);
    assert_eq!(blast_damage(2, 3, 3), 1);   // the floor
    assert_eq!(blast_damage(0, 2, 0), 0);   // stun-only
}
```

In `action.rs` tests:

```rust
#[test]
fn throw_consumes_one_and_use_refuses_a_throwable() {
    // give 2 frag; apply(Action::Throw(frag)) → count 1, no effects, message "threw `Frag`";
    // apply(Action::Use(frag)) → Err("throw it from the map"), count still 1;
    // apply(Action::Throw(stim_patch)) → Err("Stim patch is not thrown");
    // apply(Action::Throw(frag)) with 0 left → Err("none left").
}
#[test]
fn fire_books_boost_and_mark_cooldowns() { /* Action::Fire(redline) and Fire(deadeye) → Ok, `fired` set, no effects */ }
```

In `content.rs` tests, validation:

```rust
#[test]
fn validate_refuses_bad_throws() {
    // Throw on an Armor item → ItemInvalid; radius 0 → ItemInvalid; damage 0 and stun 0 → ItemInvalid;
    // two blast shapes in fx → FxInvalid; a blast shape on a Heal item → FxInvalid;
    // push on a Strike with range 0 → AbilityInvalid; Trail on a Mark ability → FxInvalid.
}
```

(`ContentError::ItemInvalid { id, why }` and `AbilityInvalid { id, why }` are new variants; reuse `WeaponInvalid`/`FxInvalid` where they already fit.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p game` — expected: compile errors on the new names.

- [ ] **Step 3: Implement the model**

Add the types above. `Item::throw` reads `self.on_use`. `blast_damage` per the formula. `Action::Throw(id)`: unknown item → `unknown item {id}`; no `Throw` use → `{name} is not thrown`; `count == 0` → `none left`; else `remove_from_pack(ch, &id, 1)` and `done(format!("threw `{}`", item.name))` with no effects. `Action::Use` on a `Throw` item → `Err("throw it from the map")` before anything is consumed. `Action::Fire` adds `Active::Boost { .. } | Active::Mark { .. }` to the no-effect arm. Validation: `Throw` only on `ItemKind::Consumable`; `range ≥ 1`, `radius ≥ 1`, `damage + stun ≥ 1`; an item's `fx` holds at most one `Throw` and at most one blast shape, and any `fx` at all only on a `Throw` item; `push`/`chain` need `range ≥ 1`; a `Boost`/`Mark` ability's fx may hold only `Kick`, `Pulse`, `Number`, `Mark`; a `Strike`'s may add `Trail` to today's shot shapes. Re-export `Blast`, `Arc`, `blast_damage` from `lib.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p game` — expected: all green, including every existing test (Static's entry parses unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/game flavor
git commit -m "feat(game): Blast and Use::Throw, the blast and trail shapes, Strike's stat/push/chain, Boost and Mark, Action::Throw"
```

---

### Task 2: Content — the four throwables, four abilities, four chrome pieces, the talent grants, the runner's kit

**Files:**
- Modify: `flavor/content.ron` (items after `stim_patch` ~274 and the cyberware ~26–41; abilities after `static` ~538; talents `iron_grip` ~398, `bullet_time` ~435, `solder_hand` ~445, `cold_read` (STREET tier 3))
- Modify: `crates/game/src/character.rs` (`combat_runner` ~393)
- Test: `crates/game/src/content.rs` tests, `character.rs` tests (~1569)

**Interfaces:**
- Produces content ids: items `frag`, `flashbang`, `emp_puck`, `satchel`, `piston_arm`, `adrenal_pump`, `arc_hand`, `target_lens`; abilities `jolt`, `shove`, `redline`, `deadeye`.
- Consumes: Task 1's types.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_throwables_and_the_abilities_are_in_content() {
    let c = Content::load().unwrap();
    let frag = c.item(&ItemId::new("frag")).unwrap();
    assert_eq!(frag.throw().unwrap().2, Blast { radius: 2, damage: 12, stun: 0, fuse: 0, chrome: false });
    assert_eq!(c.item(&ItemId::new("satchel")).unwrap().throw().unwrap().2.fuse, 1);
    assert!(c.item(&ItemId::new("emp_puck")).unwrap().throw().unwrap().2.chrome);
    assert_eq!(c.item(&ItemId::new("flashbang")).unwrap().fx.iter().filter(|f| f.is_blast()).next(), Some(&Fx::Whiteout));
    assert!(matches!(c.ability(&AbilityId::new("shove")).unwrap().kind, AbilityKind::Active(Active::Strike { push: 2, range: 1, stat: Stat::Body, .. })));
    assert!(matches!(c.ability(&AbilityId::new("jolt")).unwrap().kind, AbilityKind::Active(Active::Strike { chain: 1, stat: Stat::Tech, .. })));
    assert!(matches!(c.ability(&AbilityId::new("redline")).unwrap().kind, AbilityKind::Active(Active::Boost { once: true })));
    assert!(matches!(c.ability(&AbilityId::new("deadeye")).unwrap().kind, AbilityKind::Active(Active::Mark { range: 12, bonus: 20, ap: 1 })));
}
#[test]
fn the_talents_and_the_chrome_grant_the_abilities() {
    // iron_grip rank 1 → Grant(shove); bullet_time rank 1 → Grant(redline); solder_hand rank 1 → Grant(jolt); cold_read rank 1 → Grant(deadeye);
    // piston_arm grants shove, adrenal_pump redline, arc_hand jolt, target_lens deadeye.
}
#[test]
fn combat_runner_carries_the_kit() {
    let c = Content::load().unwrap();
    let d = Character::combat_runner(&c);
    assert_eq!(d.count(&ItemId::new("frag")), 3);
    assert_eq!(d.count(&ItemId::new("flashbang")), 2);
    assert_eq!(d.count(&ItemId::new("emp_puck")), 2);
    assert_eq!(d.count(&ItemId::new("satchel")), 1);
    for a in ["static", "jolt", "shove", "redline", "deadeye"] { assert!(d.has_ability(&c, &AbilityId::new(a)), "{a}"); }
    assert_eq!(d.equipped(Slot::Torso).map(|i| i.0.as_str()), Some("adrenal_pump"));
    assert_eq!(d.equipped(Slot::Eyes).map(|i| i.0.as_str()), Some("target_lens"));
}
```

(Check `equipped`'s return type in `character.rs` and match it.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p game content::tests::the_throwables character::tests::combat_runner` — expected: FAIL (unknown ids).

- [ ] **Step 3: Write the content**

The four items exactly as spec §1 (names `Frag`, `Flashbang`, `EMP puck`, `Satchel charge`; descs as given). The four abilities as spec §1. Chrome: `piston_arm` (Arms, 5.0 kg, 2800, perks `[Stat(Body, 1)]`, grants `["shove"]`), `adrenal_pump` (Torso, 0.6, 3400, grants `["redline"]`), `arc_hand` (Arms, 2.5, 2600, grants `["jolt"]`), `target_lens` (Eyes, 0.2, 2200, perks `[Sight(1)]`, grants `["deadeye"]`); write a one-line desc each in the content's voice. Talents: add `Grant("shove")` to `iron_grip`'s rank-1 list, `Grant("redline")` to `bullet_time`'s, `Grant("jolt")` to `solder_hand`'s, `Grant("deadeye")` to `cold_read`'s. `combat_runner`: `give` the four throwables with the counts, `give_item` + `wear` the `adrenal_pump` (Torso) and `target_lens` (Eyes), stowing what they replace as the neural port does.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p game` — expected: green; `Content::load().validate()` passes (a content test already loads everything).

- [ ] **Step 5: Commit**

```bash
git add flavor/content.ron crates/game/src/character.rs crates/game/src/content.rs
git commit -m "content: frag, flashbang, EMP puck, satchel; jolt, shove, redline, deadeye; their chrome and talent grants; the runner's kit"
```

---

### Task 3: The blast engine — `footprint`, `detonate`, `throw`, live blasts, the console

**Files:**
- Modify: `crates/strata-zero/src/combat.rs` (after `miss_impact` ~28; constants at the top)
- Modify: `crates/strata-zero/src/app.rs` (`FxInstance` ~101: `arc`; `AppState` ~65: `live`; `until` ~123; `enter_city` clears `live`)
- Modify: `crates/strata-zero/src/net.rs` (`end_turn` ~1736 where your turn's pools are set: the countdown; `beat` ~2211: the calm countdown; `NetState.stunned_you`)
- Modify: `crates/strata-zero/src/foes.rs` (`Foe.chrome`, `Foe.marked`; `goon`/`dummy` set `chrome: true`)
- Modify: `crates/strata-zero/src/screens/console.rs` (`"foe" | "dummy"` ~281: add `throw`, `live`)
- Test: `combat.rs` tests (use `foes::testutil::lot`), `net.rs` tests

**Interfaces:**
- Produces:
  ```rust
  pub const THROW_SPEED: i32 = 2; pub const THROW_SETTLE: u64 = 2; pub const SCORCH_TICKS: u64 = 3600;
  pub fn throw_ticks(from: Pos, to: Pos) -> u64;                       // ceil(dist / 2) + THROW_SETTLE, dist ≥ 1
  pub fn footprint(city: &City, place: Place, centre: Pos, radius: i32) -> Vec<(Pos, i32)>;
  pub struct Live { pub place: Place, pub pos: Pos, pub blast: Blast, pub fx: Vec<Fx>, pub name: String, pub turns: u8 }
  pub fn detonate(st: &mut AppState, place: Place, centre: Pos, blast: Blast, fx: &[Fx], name: &str, at: u64, tint: Option<(u8,u8,u8)>);
  pub fn throw(st: &mut AppState, id: &ItemId, at: Pos) -> Result<(), String>;
  pub fn tick_live(st: &mut AppState);                                 // one turn off every Live on your place; zero detonates
  pub fn live_footprints(st: &AppState, place: Place) -> Vec<(Pos, i32)>;  // every live blast's footprint tiles with rings (for foes and the overlay)
  // app.rs: FxInstance.arc: Option<game::Arc>; AppState.live: Vec<Live>; NetState.stunned_you: u8
  // foes.rs: Foe.chrome: bool, Foe.marked: bool
  ```
- Consumes: `foes::blocks_sight`, `citygen::pathing::bresenham`, `citygen::sight::sees` (as `your_chance`'s line check uses), `net::spend_ap`, `net::Phase`, `game::blast_damage`, `wound_foe`/`wound_you` (make `wound_you` take the killer's name: `wound_you(st, by: &str, damage)`, and keep the foe callers passing the foe's name).

- [ ] **Step 1: Write the failing tests** in `combat.rs`:

```rust
#[test] fn footprint_on_open_ground_is_the_square_by_ring() { /* lot, centre (20,8), radius 2 → 25 tiles, ring 0 first, 8 at ring 1, 16 at ring 2 */ }
#[test] fn footprint_stops_at_walls_and_behind_them() { /* centre beside the lot's wall ring: wall tiles absent; a tile behind the wall absent; the centre present even on a foe's tile */ }
#[test] fn a_footprint_walled_in_is_the_centre_alone() { /* City::empty with one Road tile ringed by Wall → one tile */ }
#[test] fn detonate_hits_every_standing_foe_by_ring_and_wakes_them() { /* goon at ring 1 of a 12/2 frag: hp 30→22, Hunting; a second goon at ring 2: 26; a down foe untouched; numbers "-8" "-4"; Chips per victim at `at + ring`; the Bloom instance with 25 targets and radius 2; log "frag · goon 8 · goon 4" */ }
#[test] fn detonate_spares_the_dummy_and_the_chromeless() { /* dummy hp unchanged, its Number still pushed; a foe with chrome: false under an EMP (chrome: true) takes 0 and no stun; a stun-only blast's label reads "stun" */ }
#[test] fn a_blast_hits_you_by_your_ring() { /* you at ring 0 of a frag: hp −12, Number on you, log ends "· you 12"; a stun 2 flashbang sets net.stunned_you 2; hp to 0 → net.death == Some("your own frag") */ }
#[test] fn throw_refusals_in_order() { /* calm → "not in a fight"; engaged on the move half → "move first · space to stay"; act half, ap 0 → "no AP left"; no frag → "none left"; 9 tiles with range 8 → "out of range"; a tile behind a wall → "no line"; a wall tile → "no landing"; nothing consumed, no AP spent in every case */ }
#[test] fn throw_lands_and_blasts_on_the_landing_tick() { /* act half, 2 frag, target 6 tiles east: Ok, 1 frag, 1 AP gone, a Throw instance with arc Lob, origin you, target the tile, started now; the Bloom started now + 5 (ceil(6/2)+2); the goon's damage already taken */ }
#[test] fn a_fused_throw_goes_live_and_counts_your_turns() { /* satchel: no blast instance, st.live has one with turns 1, log "satchel · 1 turn"; idle goon in it → Hunting; tick_live → turns 0 → detonated: live empty, Bloom pushed, goon hurt */ }
#[test] fn a_live_blast_counts_calm_beats() { /* throw with console-style call while calm (call `detonate`/`Live` through `throw_free`, below), then net::beat twice → gone off */ }
```

`throw_free(st, id, at)` is the console's door: the same as `throw` without the engagement refusals and the AP (`pub fn throw_free`). Tests set `st.net.engaged` through `net::engage(st, STREET, true)` and move to the act half with `net::stay(st)`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p strata-zero combat::` — expected: compile errors.

- [ ] **Step 3: Implement**

`footprint` per spec §2 (ring then y then x; the centre always first; a blocking tile never; a tile whose Bresenham line from the centre crosses a blocking tile never). `detonate` per spec §2 steps 1–5: the blast shape instance (`origin = centre`, `radius`, `targets` = the footprint tiles in order, `started = at`; `until` = `at + <shape ticks> + SCORCH_TICKS` for Bloom/Whiteout with damage > 0, `at + <shape ticks>` otherwise); per victim the impact shapes from `fx` (`Chips`, `Tear`, `Spark`; `Spark` plays on a blast's victims too) at `at + ring`, `hit: true`, and a `Number` with the label; victims' columns offset by index (`targets[0].x + index`, as the burst's `k` does today). Wake: `FoeMode::Hunting { seen: you, lost: 0 }` on any standing non-inert victim that was idle. You: ring from the footprint; `chrome` counts you when `st.character.worn_items()` (whatever today's "any cyberware worn" query is; `Character::equipped` over `Slot::ALL` filtered to `ItemKind::Cyberware`) is non-empty. `throw` per spec §2: the refusals in order, then `spend_ap`, `game::apply(Action::Throw)`, the `Throw` instance, and either `detonate` at the landing tick or a `Live`. `tick_live`: called at the start of your turn (in `end_turn` right after the pools are set for `Actor::You`, before `start_turn`) and from `net::beat` when `st.net.engaged.is_none()` and you are on foot (before the construct early return). Also in `end_turn` for `Actor::You`: if `stunned_you > 0` → `e.ap = 0`, `say("you're stunned")`, `stunned_you -= 1`. `enter_city` clears `live`. Console: `throw <item> [n]` → `throw_free` at `n` tiles east (default 4), `live` → one line per `Live` (`satchel at 20,8 · 1 turn`).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p strata-zero` — expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/combat.rs crates/strata-zero/src/app.rs crates/strata-zero/src/net.rs crates/strata-zero/src/foes.rs crates/strata-zero/src/screens/console.rs
git commit -m "feat(combat): the blast engine — footprint by sight, detonate by ring, throw, live blasts on your turns and calm beats"
```

---

### Task 4: Foes step out of a live footprint

**Files:**
- Modify: `crates/strata-zero/src/foes.rs` (`step` ~256, before the `in_reach` check)
- Test: `foes.rs` tests

**Interfaces:**
- Consumes: `combat::live_footprints(st, place)`, `combat::Live`.
- Produces: no new names; `step` gains the rule.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn a_foe_in_a_live_footprint_steps_out_before_anything_else() { /* hunting goon at ring 1 of a live satchel (radius 3), you in its range: step(can_move, can_hit) → Moved to a tile outside every footprint, no shot fired */ }
#[test] fn it_picks_the_out_tile_nearest_its_seen() { /* two out tiles reachable; the one nearer `seen` wins */ }
#[test] fn with_no_way_out_it_takes_the_greatest_ring() { /* walled so every neighbour is in the footprint: Moved to the neighbour with the largest ring */ }
#[test] fn it_fires_once_out() { /* after the step, a second call with can_hit → Hit */ }
#[test] fn a_foe_with_no_move_in_a_footprint_fires_as_today() { /* can_move false → in reach fires */ }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p strata-zero foes::` — expected: FAIL.

- [ ] **Step 3: Implement**

In `step`, after the hunting check and before `in_reach`: when `can_move` and `live_footprints` holds `foe.pos`, pick among the 8 neighbours that are walkable, not you, not another standing foe: those outside every live footprint, by Chebyshev distance to `seen`, then by index; with none, the neighbour with the greatest ring; move there and return `Step::Moved`. (An idle foe never reaches here; Task 3's wake covers it.)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p strata-zero foes::` — expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/foes.rs
git commit -m "feat(foes): a foe standing in a live blast's footprint spends its move leaving it"
```

---

### Task 5: The abilities — Shove's push, Jolt's chain, Redline, Deadeye, Static on ICE

**Files:**
- Modify: `crates/strata-zero/src/combat.rs` (`strike` ~450, `your_chance` ~97, `volley` ~308; new `boost`, `mark`, `strike_ice`)
- Modify: `crates/strata-zero/src/net.rs` (`step_run` ~640–700: extract `hit_piece`; `Engagement.boosted`; `disengage` ~1639 clears marks and `boosted`)
- Modify: `crates/strata-zero/src/foes.rs` (`wound`/down clears `marked`)
- Modify: `crates/strata-zero/src/actions.rs` (~75–85: the deck's refusal grows to `Boost`/`Mark`: `use it from the map`; `Action::Use` on a throwable surfaces `throw it from the map` from `apply` unchanged)
- Test: `combat.rs`, `net.rs` tests

**Interfaces:**
- Produces:
  ```rust
  pub fn boost(st: &mut AppState, id: &AbilityId) -> Result<(), String>;
  pub fn mark(st: &mut AppState, id: &AbilityId, foe: usize) -> Result<(), String>;
  pub fn strike_ice(st: &mut AppState, id: &AbilityId, b: BuildingId, i: usize) -> Result<(), String>;
  // net.rs
  pub fn hit_piece(st: &mut AppState, b: BuildingId, index: usize, damage: u8, stun: u8, now: u64) -> Option<String>;  // the attack arm of step_run: integrity, awake, break path; returns the line
  pub const STRIKE_NOISE: u8 = 3;
  // Engagement.boosted: bool
  ```
- Consumes: Task 1's `Active` fields; Task 3's `Foe.marked`, `net.stunned_you`; `render/fx` shapes exist only as `Fx` variants here (Task 8 draws them).

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn strike_rolls_with_its_own_stat() { /* Shove with Body 10 vs Net 1: `force_hit` off, the chance read via a new `strike_chance(st, id, foe)` equals hit_chance(body, skilled, dist, range) */ }
#[test] fn shove_pushes_two_tiles_with_a_trail() { /* adjacent goon east of you, force_hit true: goon.pos.x += 2, two Trail instances on the tiles it left started at the impact tick (the second a tick later), log "shove · goon 3 · back 2" */ }
#[test] fn shove_stops_at_a_wall() { /* goon one tile from the wall: moves 1, log "back 1"; against the wall: moves 0, "back 0", damage still lands */ }
#[test] fn shove_against_another_foe() { /* a second foe behind the first: no push past it */ }
#[test] fn jolt_chains_to_the_nearest_within_two() { /* goon A at 3 tiles, goon B 2 tiles from A with a clear line, goon C 3 tiles from A: A takes 5 and stun 1 on a hit, B takes 2 and stun 1 without a roll, C nothing; B's shapes start 2 ticks after A's impact with origin A; a miss on A chains nothing */ }
#[test] fn redline_refills_once_a_fight() { /* act half, moves 0, ap 0: boost → Ok, moves/ap full, phase Move, boosted, log "redline · your turn again", a Number "+ turn" on you with a tint, Kick and Pulse instances; a second boost → "once a fight"; calm → "not in a fight"; disengage clears boosted */ }
#[test] fn deadeye_marks_one_foe_and_lifts_the_chance() { /* mark → goon.marked, 1 AP, log "deadeye · goon marked"; your_chance on it = +20 before the clamp; marking another clears the first; down clears it; calm clears it; out of range 13 → "out of range"; no cursor foe → "pick a target" */ }
#[test] fn deck_use_refuses_boost_mark_and_throwables() { /* perform(Action::Fire(redline)) from the deck → "use it from the map"; Fire(deadeye) → same; Action::Use(frag) → "throw it from the map" */ }
#[test] fn static_on_a_piece_is_an_attack() { /* net lot (net::start_in_net_lot), act half, grey ICE in reach: strike_ice(static) with force hit → integrity −6, stunned 1, awake, heat +STRIKE_NOISE, the shot instances tinted ATTACK_TINT with no Flash; at integrity 0 the break line "grey ICE broken"; the dummy ICE keeps its integrity and the number lifts; shove in a construct → "not in here" */ }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p strata-zero combat:: net::` — expected: compile errors.

- [ ] **Step 3: Implement**

Extract `hit_piece` from `step_run`'s attack arm (everything from `let (grade, label, inert) = …` through the broken/left lines) and call it from both. `strike`: read `stat`, `push`, `chain`; the roll's stat is `st.character.stat(&st.content, stat)`; after `volley`, on any hit: push (the direction `(sign(foe.x − you.x), sign(foe.y − you.y))`, up to `push` steps while the next tile is walkable and free of you and standing foes; a `Trail` instance per tile left with `origin` the tile, `targets = [the tile after it]` (for the arrow's direction), `started` at the impact tick + index); chain (`chain` hops: the nearest other standing foe within 2 of the last victim with `line_clear` from it, `blast_damage`-free half damage `max(1, damage / 2)`, the stun, `wound_foe`, and `play_round` with `from` the last victim's tile, `started + 2`). In a construct, `strike` delegates to `strike_ice` when the cursor's enemy is a piece (explore passes `Enemy::Ice`); `Shove`/`Mark` there → `not in here`. `boost` and `mark` per spec §1 with the refusal order of `strike` (turn, phase, AP where it costs, then `game::apply(Action::Fire)`). `your_chance`: `+ bonus` when `foes[foe].marked`, before the clamp (expose the bonus through the mark's content value: find the character's `Mark` ability's `bonus`; one mark at a time so the first `Mark` ability found is right).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p strata-zero` — expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/combat.rs crates/strata-zero/src/net.rs crates/strata-zero/src/foes.rs crates/strata-zero/src/actions.rs
git commit -m "feat(combat): Shove pushes, Jolt chains, Redline refills, Deadeye marks, Static hits ICE"
```

---

### Task 6: The `ACT` list, arming, the keys and the readout (`explore.rs`, `hud.rs`)

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs` (`MapScreen` ~147: `armed: Option<ItemId>`; `handle_firing` ~413; `actives` ~553; `slots` ~802; `footer` ~681; `turn_prompt` ~700; `aim_readout`)
- Modify: `crates/strata-zero/src/ui/hud.rs` (`Footer` ~332: `act: Option<Vec<ActEntry>>`, `flash_pools: bool`; `footer_rows` row 4)
- Test: `explore.rs` tests (the lot through `foes::testutil::lot` + `MapScreen`), `hud.rs` tests (~930)

**Interfaces:**
- Produces:
  ```rust
  // hud.rs
  pub struct ActEntry { pub key: char, pub name: String, pub count: Option<u32>, pub state: ActState }
  pub enum ActState { Ready, Cooling, Refused }
  // Footer: pub act: Option<Vec<ActEntry>>  (Some on the act half: row 4 reads ` ACT  1 static 2 jolt …`), pub flash_pools: bool
  pub fn act_row(entries: &[ActEntry], right: &str, width: usize) -> Line<'static>;   // drops whole entries off the end with `…`
  // explore.rs
  pub enum Act { Ability(AbilityId), Throw(ItemId) }
  fn act_list(st: &AppState) -> Vec<Act>;   // street: fight abilities (Strike/Boost/Mark, content order) then throwables (pack order); construct: Static, Jolt, Redline only, keyed from 6
  fn act_key(st: &AppState, n: u32) -> Option<Act>;
  ```
- Consumes: Task 3's `throw`, Task 5's `boost`/`mark`/`strike`/`strike_ice`; `hud::footer_rows`.

- [ ] **Step 1: Write the failing tests**

```rust
// hud.rs
#[test] fn the_act_row_numbers_abilities_then_throwables_and_cuts_with_an_ellipsis() {
    // nine entries at 120 columns: row starts " ACT  1 static 2 jolt 3 shove 4 redline 5 deadeye 6 frag×3 7 flashbang×2 8 emp puck×2 9 satchel charge×1", right "AP ●● 2/2  MOVE 4/4";
    // at 100 columns the row ends with "…" and the right side is intact; a Cooling entry draws dim with " ◌"; width == the terminal's.
}
// explore.rs
#[test] fn act_list_on_the_street_and_in_a_construct() { /* combat_runner on the lot: [static, jolt, shove, redline, deadeye, frag, flashbang, emp_puck, satchel]; in the net lot: keys 6,7,8 → static, jolt, redline; 1–5 → None (programs) */ }
#[test] fn a_throwable_key_arms_the_cursor_and_space_throws() { /* act half: '6' → armed Some(frag), readout " frag · 6 tiles · blast 2 · goon 8 "; ' ' → combat::throw ran (frag count 2), armed None; esc with armed → None but cursor kept; a second esc drops the cursor */ }
#[test] fn armed_readout_warns_when_you_are_in_it() { /* cursor 1 tile off: readout ends "· you're in it" and the overlay's `you_in_it` is true */ }
#[test] fn armed_readout_names_the_refusal() { /* 9 tiles: " frag · 9 tiles · out of range "; behind a wall: "no line"; a wall: "no landing" */ }
#[test] fn act_keys_past_the_end_and_cooling() { /* '9' with eight entries → nothing, no AP; a cooling jolt → its cooldown line, no AP; '6' with 0 frag → entry absent (the list is from the pack), key → nothing */ }
#[test] fn boost_and_mark_keys() { /* '4' → redline fired (phase Move); '5' with the cursor on the goon → marked; '5' off a foe → "pick a target" */ }
#[test] fn the_prompt_rows() { /* act half "aim" / "hjkl target · t next · space fire · 1-9 act · . end turn"; armed "throw" / "hjkl aim · space throw · esc back" */ }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p strata-zero hud:: explore::` — expected: compile errors.

- [ ] **Step 3: Implement**

`act_list`/`act_key` per spec §4 and the Interfaces block. In `handle_firing`'s digit arm, route by `act_key`: `Act::Throw` arms (`self.armed = Some(id)`), `Act::Ability` by kind: `Strike` → `strike`/`strike_ice` at the cursor's enemy, `Mark` → `mark`, `Boost` → `boost`; anything else keeps today's fall-through (the net's programs on 1–5 stay in `program_key`). `space` with `armed` → `combat::throw(st, &id, cursor)` then `armed = None` (even on a refusal the item stays armed only when the refusal was `out of range`/`no line`/`no landing`; a spent AP disarms). `esc` disarms first. `footer`: on your act half `act: Some(entries)` (states: `Cooling` when `cooling(..)` is `Some`, `Refused` for Shove/Deadeye in a construct), `flash_pools` when a `Pulse` instance on you is live. `hud::footer_rows`: when `act` is `Some`, row 4 is `act_row`; otherwise as today. Prompts per spec §4. `aim_readout` grows an armed branch: ` {name} · {d} tiles · blast {r} · {foe} {dmg} …` with `{foe} stun` when the damage is 0, nearest first, `· you're in it` appended, and the refusal forms; the overlay gets the readout's colour from a new `AimOverlay.warn: bool`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p strata-zero` — expected: green (fix the existing footer tests that pin the `ABILITIES` row only where the act half is on).

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/screens/explore.rs crates/strata-zero/src/ui/hud.rs
git commit -m "feat(app): the ACT list on the act half, arming a throwable, space throws, the armed readout"
```

---

### Task 7: The cursor's `×`, the footprint shading, live blasts and the mark on the map (`render/path.rs`, `render/people.rs`)

**Files:**
- Modify: `crates/strata-zero/src/render/path.rs` (`AimOverlay` ~95 and its `draw` ~109; new `LiveOverlay`)
- Modify: `crates/strata-zero/src/render/people.rs` (the foe loop ~57)
- Modify: `crates/strata-zero/src/screens/explore.rs` (the overlay site ~2613: pass `footprint`, `you_in_it`, `warn`; push a `LiveOverlay`)
- Test: `path.rs` tests, `people.rs` tests, an explore draw test

**Interfaces:**
- Produces:
  ```rust
  // AimOverlay gains: pub footprint: &'a [(Pos, i32)], pub you: Option<Pos>, pub warn: bool
  pub const RING_BG: [Color; 3] = [Rgb(120,30,30), Rgb(80,22,22), Rgb(48,16,16)];
  pub const YOU_IN_IT_BG: Color = Rgb(170,30,30);
  pub const WARN_FG: Color = Rgb(255,80,80); pub const WARN_BG: Color = Rgb(26,0,0);
  pub struct LiveOverlay<'a> { pub live: &'a [combat::Live], pub footprints: &'a [(Pos, i32)] }
  pub const LIVE_RING_BG: [Color; 3] = [Rgb(60,18,18), Rgb(44,14,14), Rgb(30,10,10)];
  // people.rs: MARK_BRIGHT = Rgb(140,0,105), MARK_DIM = Rgb(70,0,52), MARK_PULSE = 8, the `⌖` above a marked foe
  ```
- Consumes: Task 3's `Live`, `live_footprints`; Task 6's readout and `warn`.

- [ ] **Step 1: Write the failing tests** (buffer tests as `path.rs` has today):

```rust
#[test] fn the_cursor_off_a_foe_is_a_cyan_block_with_a_cross() { /* cell char '×', fg black, bg CYAN, bold; on a foe: inverted as before */ }
#[test] fn the_footprint_shades_by_ring_and_lights_your_tile() { /* ring 0/1/2 bgs; ring 3 takes RING_BG[2]; glyphs kept; your tile YOU_IN_IT_BG; the fire line's dots under the cursor */ }
#[test] fn a_warned_readout_is_red() { /* WARN_FG on WARN_BG */ }
#[test] fn a_live_blast_blinks_and_shades() { /* even tick 'o' (190,190,140) bold, odd tick (255,80,80); footprint LIVE_RING_BG by ring; " fuse 1 " two rows above in cyan on READOUT_BG */ }
#[test] fn a_marked_foe_pulses_with_a_reticle() { /* tick 0..8 bg MARK_BRIGHT and '⌖' bold magenta above; tick 8..16 MARK_DIM and plain */ }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p strata-zero render::` — expected: FAIL.

- [ ] **Step 3: Implement**

Per spec §4 "The cursor's look" and §2 "Live blasts". The footprint shading goes under the line's dots and the cursor; the readout clamps inside the area as today. `LiveOverlay` draws every `Live` on the map's place. The marked foe's cell above draws `⌖` only when that cell is on screen and not the readout's row.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p strata-zero` — expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/render/path.rs crates/strata-zero/src/render/people.rs crates/strata-zero/src/screens/explore.rs
git commit -m "feat(render): the × cursor, the blast footprint under the aim, live blasts on the map, the marked foe's reticle"
```

---

### Task 8: Drawing the six shapes and the scorch (`render/fx.rs`)

**Files:**
- Modify: `crates/strata-zero/src/render/fx.rs` (constants ~21–31; the dispatch in `draw`; new fns `throw`, `bloom`, `whiteout`, `shock`, `trail`, `pulse`, `scorch`)
- Modify: `crates/strata-zero/src/app.rs` (`until` ~123: the new arms)
- Modify: `crates/strata-zero/src/ui/hud.rs` (the pools flash: white bold on `(0,90,110)` while `flash_pools`)
- Test: `fx.rs` tests (buffer at a fixed tick, as the shot shapes' tests do), `app.rs` `until_per_shot_shape`

**Interfaces:**
- Produces: `pub const BLOOM_TICKS: u64 = 13; WHITEOUT_TICKS = 8; SHOCK_TICKS = 4; TRAIL_TICKS = 4; PULSE_TICKS = 7;` `pub fn throw_ticks(origin, target) -> u64` lives in `combat.rs` (Task 3) and `until` uses it.
- Consumes: `FxInstance.arc`, `.radius`, `.targets` (the footprint), `.tint`, `.label`.

- [ ] **Step 1: Write the failing tests**, one per shape at its first, a middle and its last tick, asserting the mock's glyphs and colours:

```rust
#[test] fn throw_lob_rises_and_lands() { /* 10-tile throw east: e=2 glyph 'o' (190,190,140) bold at x+5, 1 row up (2.2·4·s(1−s) rounded at s=0.5 → 2 rows at mid-flight); trail '·' behind; landing tick '●' (255,240,180) bold at L; +1 'o' one past L a row up; +2 'o' at L; after that nothing */ }
#[test] fn throw_shadow_grows_on_the_line() { /* same throw with Arc::Shadow: s<0.3 '·', mid 'O' bold white-mixed, never off the row */ }
#[test] fn bloom_by_tick() { /* e0 '✺' black on (255,240,180); e1 ring0 '✺' white on (255,120,40), ring1 '*' (255,230,120) on (200,60,20); e2 ring2 '*' on (90,30,10); e3 ring0 '▓' (90,90,90) on (50,40,36); e6 some '░' smoke risen 0 rows, e9 risen 1; e13.. the scorch: L '▒' (45,40,36) on (18,16,15), ring1 '░' (38,34,30) on (20,17,16); no cell outside the footprint touched */ }
#[test] fn whiteout_by_tick() { /* e0 all footprint black on white bold; e1 (60,0,0) on (255,160,60) inside ring 2 / (255,110,50) beyond; e2 scorch '▓' '▒' '░' (30,27,25) on (16,14,13) and debris at distance 3; e4 debris at 4 in (e−2)/5 mix; a damage-0 instance draws no scorch after e8 */ }
#[test] fn shock_by_tick() { /* e0 '◉' cyan bold on (0,60,75); e1 ring1 '≋' cyan bold on (0,40,50), L '○'; e2 ring2 '≋' (0,180,200) on (0,30,38), ring1 '·' (0,120,140); e3 ring2 '·'; e4 nothing */ }
#[test] fn trail_arrows_fade() { /* east push: '»' (255,120,120) for 2 ticks then (180,60,60); north '∧'; a diagonal east-north with dx > dy → '»' */ }
#[test] fn pulse_rings_out() { /* e0 origin black on the colour bold; e1 the 8 at distance 1 '·' in the colour; e3 distance 2 at 2/3; e5 distance 3 at 1/3; e7 nothing */ }
#[test] fn a_tinted_tag_sits_on_the_cyan_field() { /* Number tag with tint: bg (0,90,110) */ }
#[test] fn until_per_new_shape() { /* Throw: started + throw_ticks; Bloom with damage: +13+3600; Whiteout no damage: +8; Shock +4; Trail +4; Pulse +7 */ }
```

(`FxInstance` needs to know a blast's damage for the scorch: use `label` non-empty? No: give the blast instance `hit: damage > 0` and read that.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p strata-zero render::fx::` — expected: FAIL.

- [ ] **Step 3: Implement** each shape per spec §5, reusing `paint`, `glow`, `line_glyph`, `shot_line` and the hash from `tear`. The smoke's hash is `citygen::rng::hash64` over `(x, y, e / 2)` as the mock's `hash(x, y, a>>1)`. Rings come from `origin.chebyshev(t)` per footprint tile. Actors draw over the scorch because the fx layer draws under people (check the overlay order in `explore.rs`; if fx draws over people, skip the tiles that hold a standing foe or you for the scorch only).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p strata-zero` — expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/render/fx.rs crates/strata-zero/src/app.rs crates/strata-zero/src/ui/hud.rs
git commit -m "feat(render): the throw, bloom, whiteout, shock, trail and pulse shapes, the scorch, the pools flash"
```

---

### Task 9: Shots, smoke, README and the spec's status

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs` (beside `combat_*`)
- Modify: `README.md` (the combat keys: `1-9 act`, throwables)
- Modify: `docs/superpowers/specs/2026-10-20-grenades-abilities-design.md` (status line)
- Create: smoke script in the session's scratchpad (not committed)

- [ ] **Step 1: Add the ignored shots** `combat_armed_frag` (the act half with the frag armed on the goon: the footprint, the readout), `combat_bloom_mid` (a frag blast at e2), `combat_whiteout_mid` (e1), `combat_shock_mid` (e1), `combat_live_satchel` (a live satchel with the goon beside it), `combat_jolt_chain` (e3), `combat_marked` (the mark on the goon). Each a fixed `st.tick`.

- [ ] **Step 2: Render and look at them**

Run: `SHOT_DIR=/private/tmp/claude-501/-Users-jeffrey-dev-cyberpunk/3e458f3b-3c49-4afe-bc44-11f42fad0b6b/scratchpad/shots cargo test -p strata-zero shots::combat_ -- --ignored`, then the scratchpad's `shot.py` (or the earlier sessions' SVG→PNG step) and view the PNGs against the mock. Fix drift in Tasks 7–8's code.

- [ ] **Step 3: Smoke**

Build `cargo build --release --target-dir target/smoke -p strata-zero` and drive `target/smoke/release/strata` with the session's smoke driver (as the ranged-combat session did; see `smoke/` under the previous sessions' scratchpads if present, else write a pty driver in the scratchpad): the COMBAT start, walk east, `e`, space to stay, `6`, a readout, space, the bloom, `.`; `9` the satchel, the fuse, the goon stepping out; `3` shove, `2` jolt, `4` redline, `5` deadeye. Save screenshots to the scratchpad.

- [ ] **Step 4: README and status**

README: the combat section lists the `ACT` keys and the throwables; the spec's status line reads "built".

- [ ] **Step 5: Full verification and commit**

Run: `cargo test -p game -p citygen -p sim -p strata-zero`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` — expected: green, clean. Commit:

```bash
git add crates/strata-zero/src/screens/shots.rs README.md docs/superpowers/specs/2026-10-20-grenades-abilities-design.md
git commit -m "test(shots): the armed cursor, each blast mid-way, the live satchel, the chain and the mark; README and spec status"
```
