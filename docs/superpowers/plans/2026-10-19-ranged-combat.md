# Ranged Combat Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The street gets the net's turn-based engagement, a goon with a pistol who hunts and shoots, a target cursor and a fire action with per-round rolls, damage numbers, the Static ability, a vocabulary of gun effects on content, and a NEW GAME (COMBAT) lot to try it all on.

**Architecture:** Rules and content live in `game` (weapons, the hit roll, the `Fx` shapes, the `Strike` ability). The app grows `foes.rs` (the goon's state, sight and turn) and `combat.rs` (a fire action resolved round by round, pushing `FxInstance`s with future start ticks); `net.rs`'s `Engagement` loses its construct binding so the same turn engine runs both maps; `render/fx.rs` draws the seven new shapes from the approved mock; `explore.rs` adds the target cursor and the street's engaged keys; `title.rs` adds the lot.

**Tech Stack:** Rust 2024 workspace (`citygen`, `sim`, `game`, `strata-zero`), ratatui + crossterm, RON content, `cargo test` with `TestBackend` buffers.

**Spec:** `docs/superpowers/specs/2026-10-19-ranged-combat-design.md`; art reference `docs/superpowers/specs/2026-10-19-ranged-combat-mock.html` (glyphs, colours, ticks).

## Global Constraints

- Every glyph, colour and tick of a shape comes from the spec §7 / the mock; they are copied below where a task needs them.
- Timing constants: `ROUND_GAP = 4`, `FLASH_TICKS = 2`, `STREAK_TICKS = 3`, `IMPACT_TICKS = 3`, `NUMBER_TICKS = 14`, `NUMBER_RISE = 4` (ticks per row), tracer `speed` from content (the scrap nine: 3).
- Goon numbers: `GOON_HP = 20`, `GOON_REFLEX = 4`, `GOON_SIGHT = 8`, `GOON_MOVE = 3`, `GOON_AP = 1`; `LOST_TURNS` is the net's 3.
- Hit chance: `50 + 4·reflex + (skilled ? 10 : 0) − 5·max(0, dist − range/2)`, clamped `5..=95`; a round hits when `roll (1..=100) <= chance`.
- Copy, verbatim: `nothing to shoot with`, `out of range`, `no line`, `pick a target`, `nothing to hit`, `nothing to engage`, `still hunted`, `not with a gun on you`, `not now`, `nothing to save on the lot`, `ENGAGED · the goon has you`, `goon down`, `goon stunned`, `ENGAGED`, `on foot · the lot`, `NEW GAME (COMBAT)`, `goon moves 2`, `goon fires`, `goon · stunned`.
- Keys on your act half on the street: `hjkl`/arrows move the target cursor, `tab` next foe, `space` fire, `1`–`5` a strike ability at the cursor's foe, `esc` drop the cursor, `.` end the turn.
- No file in the main checkout is touched; work is in the `ranged-combat` worktree, commits end with the attribution lines in the session reminder.
- The release binary is never rebuilt; smoke runs use `--target-dir target/smoke`.
- Wording comes from kinds (`foe.name`, `weapon.name`), never hard-coded "goon" outside the goon's constructor and the lot.

## Review Focus

1. A fire action with the cursor on a tile the player cannot see (the foe stepped out of sight memory): refused with `no line`, nothing rolls, no AP spent. (Task 9 tests `fire_refuses_without_a_line`.)
2. A miss with nothing behind the target before `range` runs out: the impact is the last walkable tile along the line inside `range`, and the spark draws there, not off the map. (Task 7 tests `a_miss_stops_at_range_on_open_ground`.)
3. The goon's turn when every path to you is blocked by walls or another foe: it stays, its turn ends on the pace, the game never spins. (Task 6 tests `a_foe_with_no_path_ends_its_turn`.)
4. `.` pressed during the foe's turn on the street, and `e` during it: refused with `not your turn`, the engagement unchanged. (Task 9 tests `street_keys_on_the_foes_turn`.)
5. The number keys on a non-strike ability (netscan) during a street fight: fires as today through `perform` and spends one AP; a strike ability with the cursor off a foe: `pick a target`, no AP spent. (Task 9 tests `number_keys_in_a_street_fight`.)

---

### Task 1: Weapons, the shot shapes and the strike ability in `game`

**Files:**
- Modify: `crates/game/src/model.rs` (`Fx` at ~539, `Active` at ~528, `Item` at ~463)
- Modify: `crates/game/src/content.rs` (`validate` at ~167, `ContentError` at ~11)
- Modify: `crates/game/src/run.rs` (beside `succeeds` ~91)
- Modify: `crates/game/src/lib.rs` (re-exports)
- Modify: `flavor/content.ron` (items `ratchet_smg`, new `scrap_nine`; ability `static`; `neural_port` grants)
- Test: the `tests` modules of the same files

**Interfaces:**
- Produces:
  ```rust
  pub struct Weapon { pub skill: SkillId, pub damage: u8, pub rounds: u8, pub range: i32, #[serde(default)] pub fx: Vec<Fx> }
  // Item: #[serde(default)] pub weapon: Option<Weapon>
  pub enum Fx { /* as today, plus: */
      Tracer { colour: (u8, u8, u8), speed: u8 }, Streak { colour: (u8, u8, u8) },
      Flash { big: bool }, Chips, Spark, Tear, Number { tag: bool } }
  impl Fx { pub fn is_flight(&self) -> bool; pub fn is_impact(&self) -> bool; pub fn is_shot(&self) -> bool /* any of the seven */ }
  pub enum Active { /* as today, plus: */ Strike { range: i32, damage: u8, stun: u8, skill: SkillId, ap: u8 } }
  pub fn hit_chance(reflex: u8, skilled: bool, dist: i32, range: i32) -> u8;   // run.rs
  pub fn round_hits(roll: u8, chance: u8) -> bool;                             // run.rs
  ContentError::WeaponInvalid { id: String, why: String }, ContentError::FxInvalid { where_: String, why: String }
  ```

- [ ] **Step 1: Write the failing tests** in `crates/game/src/content.rs` tests and `run.rs` tests:

```rust
#[test] fn the_ratchet_and_the_scrap_nine_are_weapons() {
    let c = Content::load().unwrap();
    let w = c.item(&ItemId::new("ratchet_smg")).unwrap().weapon.clone().unwrap();
    assert_eq!((w.skill.0.as_str(), w.damage, w.rounds, w.range), ("handguns", 4, 3, 10));
    assert!(matches!(w.fx[..], [Fx::Flash { big: true }, Fx::Streak { .. }, Fx::Chips, Fx::Spark, Fx::Number { tag: false }]));
    let p = c.item(&ItemId::new("scrap_nine")).unwrap().weapon.clone().unwrap();
    assert_eq!((p.damage, p.rounds, p.range), (5, 1, 8));
    assert!(matches!(p.fx[1], Fx::Tracer { speed: 3, .. }));
    assert!(c.item(&ItemId::new("shiv_blade")).unwrap().weapon.is_none());
}
#[test] fn static_is_a_strike_the_neural_port_grants() {
    let c = Content::load().unwrap();
    let a = c.ability(&AbilityId::new("static")).unwrap();
    assert!(matches!(a.kind, AbilityKind::Active(Active::Strike { range: 8, damage: 6, stun: 1, ap: 2, .. })));
    assert!(matches!(a.fx[..], [Fx::Flash { big: false }, Fx::Streak { .. }, Fx::Tear, Fx::Number { tag: true }]));
    assert!(c.item(&ItemId::new("neural_port")).unwrap().grants.contains(&AbilityId::new("static")));
}
#[test] fn weapon_validation() {
    // armour with a weapon → WeaponInvalid; rounds 0 → WeaponInvalid; unknown skill → UnknownSkill;
    // two flight shapes → FxInvalid; Chips on netscan → FxInvalid.
}
#[test] fn hit_chance_by_the_formula() {
    assert_eq!(hit_chance(10, true, 2, 10), 95);
    assert_eq!(hit_chance(10, true, 9, 10), 80);
    assert_eq!(hit_chance(4, true, 1, 8), 76);
    assert_eq!(hit_chance(0, false, 30, 8), 5);
    assert!(round_hits(80, 80) && !round_hits(81, 80));
}
```

- [ ] **Step 2: Run them to see them fail**: `cargo test -p game weapons static_is hit_chance weapon_validation` → compile errors on the missing types.

- [ ] **Step 3: Implement** the model (`Weapon`, `Item.weapon`, the seven `Fx` variants with the three predicates, `Active::Strike` — `Active` and `AbilityKind` derive `Copy` today and lose it, since `Strike` holds a `SkillId`; the by-value matches in `actions.rs` ~118–150 take borrows), the two error variants, `hit_chance`/`round_hits`, and the validation rules of spec §1: `weapon` only on `ItemKind::Weapon` with a `Primary`/`Secondary` slot; `rounds`, `range`, `damage ≥ 1`; the skill through the existing `UnknownSkill` check (`where_: "item ratchet_smg"`); at most one flight shape in a weapon's `fx`; a shot shape in an ability's `fx` only when its kind is `Strike`. Re-export `Weapon`, `hit_chance`, `round_hits` from `lib.rs`.

- [ ] **Step 4: Content**: the Ratchet's `weapon`, the new `scrap_nine` item (spec text), the `static` ability (spec text, `cooldown: 240`), `neural_port` `grants: ["ping", "static"]`. Keep every other field.

- [ ] **Step 5: Run** `cargo test -p game` → PASS (the existing `every ItemKind` / content snapshot tests may need the new item counted).

- [ ] **Step 6: Commit** `feat(game): weapons with rounds, range and fx; the shot shapes; Strike abilities; hit_chance`

---

### Task 2: `City::lot`

**Files:**
- Modify: `crates/citygen/src/world.rs` (beside `City::empty` ~394)
- Test: `world.rs` tests

**Interfaces:**
- Produces: `pub fn lot(seed: u64) -> City` — `City::empty(GenParams { seed, size: SizePreset::Custom { w: 41, h: 19 }, ..Default::default() })`, `name: "the lot"`, Street stratum: `Road` on `Rect (2,2) 36×14`, `Sidewalk` on the one-tile ring around it, `Wall` on the ring around that; `pub fn is_lot(&self) -> bool { self.districts.is_empty() }`.

- [ ] **Step 1: Test** `the_lot_is_a_walled_road_with_nothing_else`: `lot(1).validate()` is `Ok`; `tile(Street, (2,2))` is `Road`, `(1,1)` `Sidewalk`, `(0,0)` `Wall`, `(20,9)` `Road`, `(40,18)` `Wall`; `walkable(Street, (6,8))` and `(18,8)`; `!walkable(Street, (0,8))`; `districts`, `buildings`, `connectors` empty; `is_lot()`; `name == "the lot"`; `Generator`-made cities are `!is_lot()`.
- [ ] **Step 2: Run** `cargo test -p citygen the_lot` → fails (no `lot`).
- [ ] **Step 3: Implement** with `Grid::fill_rect` on the stratum's tiles (`Tile::new(terrain)`).
- [ ] **Step 4: Run** → PASS. **Step 5: Commit** `feat(citygen): City::lot, a walled road for the street fight`

---

### Task 3: `FxInstance` carries a label and a hit; `until` for the new shapes

**Files:**
- Modify: `crates/strata-zero/src/app.rs` (`FxInstance` ~94, `until` ~109)
- Modify: `crates/strata-zero/src/net.rs` (`bolt` ~774, `mark` ~790; add `number`)
- Modify: `crates/strata-zero/src/actions.rs` (the pushes in `perform` ~119–170 gain the two fields)
- Modify: `crates/strata-zero/src/render/fx.rs` (constants)

**Interfaces:**
- Produces: `FxInstance { ..., pub label: String, pub hit: bool }`; constants in `render/fx.rs`: `pub const FLASH_TICKS: u64 = 2; STREAK_TICKS = 3; IMPACT_TICKS = 3; NUMBER_TICKS = 14; NUMBER_RISE = 4;` and `pub fn tracer_ticks(origin: Pos, target: Pos, speed: u8) -> u64` (= `ceil(chebyshev / speed)`, at least 1); `pub fn number(st: &mut AppState, at: Pos, label: impl Into<String>, colour: (u8,u8,u8), started: u64)` in `net.rs` beside `bolt`.

- [ ] **Step 1: Tests** in `app.rs`: `until_per_shot_shape`: with `started = 100`, Flash → 102, Streak → 103, Chips/Spark/Tear → 103, Number → 114, Tracer `speed: 3` from (6,8) to (18,8) → 104; a `Tracer` with no target → `started + 1`.
- [ ] **Step 2: Run** → fails. **Step 3: Implement**; every existing constructor sets `label: String::new(), hit: false`. `perform` for a `Strike` ability: return `Err("needs a target · aim at a foe")` before `apply` (combat fires strikes, Task 7); its `radius` arm reads 0.
- [ ] **Step 4: Run** `cargo test -p strata-zero app:: actions::` → PASS. **Step 5: Commit** `feat(app): FxInstance labels and hits; until() for the shot shapes`

---

### Task 4: Drawing the seven shapes

**Files:**
- Modify: `crates/strata-zero/src/render/fx.rs` (the `match inst.fx` in `draw` ~70–115; new fns)
- Test: `render/fx.rs` tests (pattern on `bolt_grows_toward_its_targets` ~438: `aimed(fx, origin, targets)`, `render(...)`)

**Interfaces:**
- Consumes: Task 3's fields and constants.
- Produces: private `fn flash(inst, big, elapsed, origin, area, buf)`, `tracer(inst, colour, speed, elapsed, ...)`, `streak(inst, colour, elapsed, ...)`, `chips(inst, elapsed, ...)`, `spark(inst, elapsed, ...)`, `tear(inst, elapsed, tick, ...)`, `number(inst, tag, colour: Option<Rgb>, elapsed, ...)`; `pub fn line_glyph(came: (i32, i32), heavy: bool) -> char` (`─`/`━` horizontal, `│`/`┃` vertical, `╲` when `dx·dy > 0`, `╱` when `< 0`).

Values (spec §7, the mock): line = `bresenham(inst.origin, inst.targets[0])`.
- Flash: firer cell bg `(255,240,180)` then `(255,170,40)`, fg black, bold, glyph kept; `line[1]` `✦` then `*` `(255,230,120)` bold; `line[2]` `·` `(255,200,90)` on tick 0; `big`: tick 0 `` ` `` above and `,` below `line[1]` in `(255,200,90)`. Nothing after `FLASH_TICKS`.
- Tracer: head index `1 + speed·elapsed`, drawn while `< line.len() − 1`... the head stops at the impact: draw while head index `< line.len()`; head `•` `(255,240,200)` bold; cells head−1..head−3 (index > 0) `line_glyph(came, false)` in `(255,170,40)` bold, `(200,110,20)`, `(110,55,10)`.
- Streak: cells `1..line.len()−1`, `line_glyph(came, true)`, tick 0 `(240,240,240)` bold, tick 1 `colour`, tick 2 `(120,60,10)`.
- Chips: target cell ticks 0–1 fg white bg `(255,40,40)` then `(160,20,20)` bold (glyph kept); chips `[(1,−1,'\''),(1,0,'·'),(1,1,','),(2,−1,'·'),(2,1,'`'),(0,−1,'·')]`, x negated when `origin.x > target.x`, chip `i` shown when `i <= 2·elapsed + 1`, offset x by `elapsed/2` outward, `(255,120,120)` tick 0 else `(180,60,60)`.
- Spark: target `✶ (255,240,160)` bold, `× (255,170,40)`, `· (120,60,10)`; tick 0 `` ` `` at `(−back,−1)` and `,` at `(−back,+1)` `(255,220,120)`; tick 1 `·` at `(−2·back,+1)` `(200,110,20)`; `back = signum(target.x − origin.x)` (1 when 0).
- Tear: `dx ∈ −3..=3, dy ∈ −1..=1`, not `(0,0)`; `h = hash64(hash64(dx+7, dy+3), hash64(tick, inst.started))`; `h % 3 == 0` → glyph `SPARKS[h % 6]`, fg `CYAN` when `h % 2 == 1` else `MAGENTA (255,0,200)`.
- Number: at `(target.x, target.y − 1 − elapsed / NUMBER_RISE)`; `fade = elapsed >= 10`; plain: label bold, `(255,235,235)` / `(170,60,60)`; tag: `" {label} "` bold, white on `(170,20,20)` / `(200,120,120)` on `(80,10,10)`; `colour: Some(c)` (the net) replaces the plain foreground with `c` and its fade with `c.scale(0.6)`. The engine shifts the column (Task 7), not the renderer.
- A `Tracer`/`Streak`/`Flash`/`Spark`/`Chips`/`Tear`/`Number` with no target draws nothing; `started > tick` draws nothing (the filter in `draw` already does this).

- [ ] **Step 1: Tests**, one per shape, at its first, middle and last tick, asserting the glyph and colour of named cells (use `render` with the city from the existing tests at a street tile; the firer at `(40,30)`, the target at `(52,30)`): `flash_lights_the_muzzle_two_ticks`, `big_flash_adds_the_cells_above_and_below`, `tracer_head_moves_three_tiles_a_tick_with_a_fading_trail`, `tracer_on_a_diagonal_uses_the_slash_glyphs`, `streak_whitens_then_ambers_then_embers`, `chips_invert_the_target_and_fly_away_from_the_shooter`, `spark_at_the_impact_three_ticks`, `tear_scrambles_the_box_around_the_target`, `number_rises_a_row_every_four_ticks_and_fades`, `tag_draws_on_a_red_field`, `a_future_instance_draws_nothing`.
- [ ] **Step 2: Run** `cargo test -p strata-zero render::fx` → fail. **Step 3: Implement.** **Step 4: Run** → PASS. **Step 5: Commit** `feat(render): tracer, streak, flash, chips, spark, tear and number`

---

### Task 5: Foes

**Files:**
- Create: `crates/strata-zero/src/foes.rs`
- Modify: `crates/strata-zero/src/lib.rs` or `main.rs` (module list), `app.rs` (`AppState.foes`, cleared in `enter_city`), `render/people.rs` (`PeopleOverlay.foes`), `screens/explore.rs` (`bump` ~1148 blocks on a standing foe; `PeopleOverlay` construction ~2077), `screens/console.rs` (`foe`, `foes`)
- Test: `foes.rs` tests; `explore.rs` tests

**Interfaces:**
- Produces:
  ```rust
  pub const GOON_HP: u8 = 20; pub const GOON_REFLEX: u8 = 4; pub const GOON_SIGHT: i32 = 8; pub const GOON_MOVE: u8 = 3; pub const GOON_AP: u8 = 1;
  pub enum FoeMode { Idle, Hunting { seen: Pos, lost: u8 } }
  pub struct Foe { pub name: String, pub place: Place, pub pos: Pos, pub hp: u8, pub hp_max: u8, pub reflex: u8, pub weapon: ItemId, pub sight: i32, pub mode: FoeMode, pub stunned: u8, pub down: bool }
  impl Foe { pub fn goon(place: Place, pos: Pos) -> Foe; pub fn standing(&self) -> bool /* !down */; pub fn hunting(&self) -> bool }
  pub fn blocks_sight(t: &Tile) -> bool;                       // Wall | Door | Window | Mass
  pub fn line_clear(city: &City, place: Place, from: Pos, to: Pos) -> bool;   // ends excluded; false off a stratum
  pub fn sees(city: &City, foe: &Foe, to: Pos) -> bool;        // standing, same place assumed, chebyshev <= sight, line_clear
  pub fn foe_at(st: &AppState, place: Place, pos: Pos) -> Option<usize>;   // standing only
  pub fn standing(st: &AppState, place: Place) -> Vec<usize>;
  pub fn in_sight(st: &AppState) -> Vec<usize>;   // standing, on the player's place, vis Visible, nearest first
  pub fn in_memory(st: &AppState) -> Vec<usize>;  // vis != Unseen, nearest first
  pub fn any_hunting(st: &AppState, place: Place) -> bool;
  pub fn look(st: &mut AppState, i: usize) -> bool;   // Idle→Hunting on sight; Hunting refreshes `seen`; returns hunting
  pub fn look_all(st: &mut AppState) -> bool;         // every standing foe on the player's place; true when any hunts
  pub fn step_toward(city: &City, place: Place, from: Pos, goal: Pos, avoid: &[Pos]) -> Option<Pos>;  // astar, walkable, not onto avoid unless goal
  pub fn lines(st: &AppState, seen: impl Fn(Pos) -> bool) -> Vec<String>;  // "▣ goon · hunting · 14/20 · 9 tiles" (+ " · stunned 1"; "idle"; a down foe is left out)
  ```
- `PeopleOverlay { ..., pub foes: &'a [Foe] }`: after the walkers, for each foe on the drawn place: in sight → standing `@` `(255,80,80)` (bold while hunting, `.scale(0.5)` while stunned); down `%` `(120,30,30)` when seen or remembered.
- `bump` on the street: a standing foe on the target tile → `st.warn(format!("the {} is in the way", foe.name))`, `Err(ScreenCmd::Stay)`.
- Console: `foe [n]` → pushes `Foe::goon(player.place, player.pos + (n, 0))` (default 6) when that tile is walkable, else `Err("no room there")`; `foes` → one line per foe `"{i} {name} {idle|hunting→x,y lost n} {hp}/{hp_max} at x,y{ down}"`.

- [ ] **Step 1: Tests** in `foes.rs` (a lot state helper: `AppState::new()`, `enter_city_at(City::lot(1), 18*60+15)` — Task 8 adds `enter_city_at`; until then `enter_city` and ignore the minute — the plan orders Task 8 after this, so use `enter_city` here and keep it): `a_goon_sees_you_at_eight_not_nine`, `a_wall_blocks_sight` (player at (6,8), foe at (6,0)? the wall ring is the only wall: put the foe outside the ring at (18,18)? No tile there is walkable; instead test `line_clear` directly across `(5,8)`→`(5,0)`: blocked by the wall at y=0 only when `to` is beyond it; use `line_clear(city, Street, (5,8), (5,-1))` false because off-map; simpler: assert `line_clear((2,2),(38,2))` true and `line_clear((0,8),(40,8))` false (walls at both ends excluded, void beyond)) — write it so one assertion crosses a `Wall` tile strictly between the ends, e.g. `line_clear((−1,8),(2,8))` crosses `(0,8)` Wall → false; `look_marks_hunting_and_refreshes_seen`; `step_toward_walks_the_shortest_way_and_never_onto_you`; `in_sight_is_nearest_first`; `lines_read_the_spec_format`; `a_down_foe_is_not_standing_and_not_listed`.
- [ ] **Step 2: Run** → fail. **Step 3: Implement** `foes.rs`, the overlay, the bump, the console.
- [ ] **Step 4: Tests** in `explore.rs`: `a_standing_foe_blocks_a_step_and_a_down_one_does_not`, `the_goon_is_drawn_red_and_down_as_a_percent` (draw_screen and find the cell), `console_foe_and_foes`.
- [ ] **Step 5: Run** `cargo test -p strata-zero foes:: explore::` → PASS. **Step 6: Commit** `feat(app): foes: the goon, its sight, its lines, drawn on the street`

---

### Task 6: The engagement on any place; the foe's turn

**Files:**
- Modify: `crates/strata-zero/src/net.rs` (`Actor` 64, `Engagement` 79, `initiative` 1401, `pools_for`, `engage` 1438, `seat_standing`, `end_turn` 1570, `start_ice_turn` 1617, `ice_pace` 1631, `turn_readout` 1668, `hunted` 1966; new `hunting`, `pace`, `seat_foes`)
- Modify: `crates/strata-zero/src/foes.rs` (`step`, `pace_step`)
- Modify: every caller of `e.building` / `engage(st, b, ..)` / `ice_pace` / `any_awake(st, e.building)` (`explore.rs` ~1567, 1942; `console.rs` `engage`/`calm`; tests)
- Test: `net.rs` and `foes.rs` tests; `explore.rs` tests

**Interfaces:**
- Produces:
  ```rust
  pub enum Actor { You, Ice(usize), Foe(usize) }
  pub struct Engagement { pub place: Place, /* round, order, turn, moves, ap, phase, manual, acting_since as today */ }
  impl Engagement { pub fn construct(&self) -> Option<BuildingId> }
  pub fn engage(st: &mut AppState, place: Place, manual: bool);   // log: construct as today; street `ENGAGED · the goon has you` (name of the nearest hunting foe) when !manual, `engaged` when manual
  pub fn initiative(st: &AppState, place: Place) -> Vec<Actor>;  // construct as today; street: You by stat(Reflex), each foe of foes::in_memory by reflex; higher first, ties to you, then index
  pub fn hunting(st: &AppState) -> bool;   // the engaged place: construct → any_awake; stratum → foes::any_hunting
  pub fn pace(st: &mut AppState);          // Ice → the old ice_pace body; Foe → foes::pace_step
  pub fn seat_foes(st: &mut AppState);     // standing foes in memory not yet in the order go to the end (round end, as seat_standing)
  // foes.rs:
  pub fn step(st: &mut AppState, i: usize, can_move: bool, can_hit: bool) -> net::Step;  // spec §2: stunned → Nothing (and `stunned -= 1`, log "goon · stunned"); look; hunting & can_hit & in weapon range & line_clear → combat::fire_at_you (Task 7; until then a stub that logs) → Hit; can_move → step_toward(seen) → Moved; at `seen` without sight → lost+1, LOST_TURNS → Idle; else Nothing
  ```
- `end_turn` round end: construct → as today (`seat_standing`, `mask_wears`, `any_awake`); stratum → `seat_foes`, then `if !manual && !foes::any_hunting(st, place) { disengage }`.
- `start_turn` (renamed from `start_ice_turn`): `Actor::Foe(i)` → `foes::look(st, i)`; a down or stunned foe passes (`stunned` counts down here, log `goon · stunned`).
- `turn_readout`: a foe reads its `name`, with ` moves {n}` / ` fires` as ICE does.
- `hunted` stays the net's; `pub fn engaged_on_foot(st) -> bool` (engaged on a stratum).
- `e` in `explore.rs`: construct branch as today with `hunting`; stratum branch: `None` → `foes::in_memory(st)` non-empty ? `engage(st, place, true)` : `nothing to engage`; `Some` not your turn → `not your turn`; `Some` and `hunting` → `still hunted`; else `disengage`.
- `explore.rs::update`: in Walk mode on a stratum: if not engaged, `foes::look_all(st)` and when it returns true `engage(st, place, false)`; if engaged, `net::pace(st)`. Replace "out of the construct, out of the fight" with: engaged and `player.place != e.place` → `disengage`. Keep the construct beat as is.
- Console `engage`/`calm`: use the player's place and `hunting`.
- `net::jack_in_refusal`: engaged on a stratum → `not with a gun on you`. `interiors::enter`: engaged → `Err("not now")`.

- [ ] **Step 1: Tests** (`net.rs`): `initiative_on_the_street_is_by_reflex_with_ties_to_you` (you Reflex 4 vs goon 4 → you first; Reflex 3 → goon first), `pools_for_a_foe`, `a_hunting_foe_engages_the_street_with_the_magenta_line`, `a_round_with_no_hunter_calms_unless_manual`, `e_on_the_street_both_ways_and_still_hunted`, `jack_in_and_doors_refused_while_engaged`. (`foes.rs`): `a_foes_turn_on_the_pace_steps_toward_you_then_fires` (foe at 12 tiles, pistol range 8: three steps on PACE_TICKS each → at 9 tiles, out of range, its turn ends; next turn three more → 6 tiles, then `fires` with `GOON_AP`), `a_stunned_foe_passes_and_the_stun_counts_down`, `a_foe_with_no_path_ends_its_turn` (foe boxed in by other foes/wall; its turn ends after one pace with nothing), `a_foe_loses_you_after_three_turns`.
- [ ] **Step 2: Run** → fail. **Step 3: Implement**; fix every compile site (`e.building` → `e.construct()` / `e.place`). The existing net tests keep passing unchanged in meaning.
- [ ] **Step 4: Run** `cargo test -p strata-zero` → PASS. **Step 5: Commit** `feat(app): the engagement on any place; foes take turns on the pace`

---

### Task 7: `combat.rs`: fire, fire_at_you, strike, numbers in the net

**Files:**
- Create: `crates/strata-zero/src/combat.rs`
- Modify: `app.rs` (`#[cfg(test)] pub force_hit: Option<bool>` on `AppState`), `net.rs` (`hit` ~1702 and the attack program's successful turn ~575 push a `Number`), `foes.rs` (`step` calls `combat::fire_at_you`), `game/src/action.rs`? no — the cooldown goes through `game::apply(Fire)` from `combat::strike`.
- Test: `combat.rs` tests

**Interfaces:**
- Produces:
  ```rust
  pub const ROUND_GAP: u64 = 4;
  pub const DEATH_ON_THE_STREET: &str = "a goon on the street";   // "a {name} on the street"
  pub fn roll(st: &mut AppState) -> u8;   // 1..=100 from hash64(st.tick, st.fx.len() as u64 + counter); #[cfg(test)] force_hit: Some(true) → 1, Some(false) → 100
  pub fn miss_impact(city: &City, place: Place, from: Pos, target: Pos, range: i32) -> Pos;  // along the ray past target: first tile that blocks sight (the wall itself), else the last walkable tile within `range` of `from`, else `target`
  pub fn your_weapon(st: &AppState) -> Result<(ItemId, game::Weapon), String>;   // worn Primary with `weapon`, else `nothing to shoot with`
  pub fn your_chance(st: &AppState, foe: usize) -> Result<u8, String>;   // out of range / no line (line_clear and vis Visible) / the chance
  pub fn fire(st: &mut AppState, foe: usize) -> Result<(), String>;      // spend_ap once; rounds; log `you fire · goon 4 · 3 · miss`
  pub fn fire_at_you(st: &mut AppState, foe: usize);                      // the foe's weapon from content; log `goon fires · 5 · hp 23` / `goon fires · miss`
  pub fn strike(st: &mut AppState, id: &AbilityId, foe: usize) -> Result<(), String>;  // Strike only; `pick a target` when foe is not standing; ap check `e.ap >= ap` else `no AP left`; range/line as your_chance with Net+hacking; game::apply(Fire) for the cooldown; one round with the ability's damage; stun on hit (`goon stunned`)
  fn play_round(st, from: Pos, impact: Pos, hit: bool, damage: u8, fx: &[Fx], k: usize)   // pushes the instances with started = now + k·ROUND_GAP; impact shapes at + flight; Number's target shifted by (k, 0); label "-{damage}"
  ```
- Damage: a foe `hp = hp.saturating_sub(d)`, at 0 `down = true`, log `goon down`; you `character.hp = hp.saturating_sub(d)`, at 0 `net.death = Some(format!("a {} on the street", foe.name))` and `disengage`. (`net.death` stays where it is; the spec's "moves to AppState.death" is dropped — one field, two causes.)
- Numbers in the net: `net::hit` pushes `number(st, me, "-6", colour, now + ROLL_TICKS)` for the HP / focus / heat change (`+5` for heat), in the bolt's colour; the attack program's successful turn pushes `number(st, at, "-4", BOLT_ATTACK, now + ROLL_TICKS)`.

- [ ] **Step 1: Tests** (lot state, you at (6,8) with the Ratchet worn and `handguns`, goon at (14,8), engaged by hand, act half): `a_burst_rolls_three_rounds_and_pushes_the_fx_in_order` (force_hit true: goon hp 8, three Flash/Streak/Chips/Number sets, starts 0/4/8, numbers shifted a column each, no Spark), `a_miss_sparks_on_the_wall_behind` (force_hit false: hp 20, three Sparks at (40,8)? the wall ring at x=40 is a Wall → impact (40,8)), `a_miss_stops_at_range_on_open_ground` (goon at (10,8), range 10 → impact (16,8)), `fire_refuses_without_a_line` (foe out of sight: a `vis` Unseen → `no line`, no AP spent), `fire_refuses_out_of_range_and_without_a_gun` (goon at (18,8), 12 tiles, Ratchet range 10 → `out of range`; the shiv in Primary → `nothing to shoot with`), `the_goon_goes_down_at_zero` (hp 4 → down, logged), `the_foes_shot_takes_your_hp_and_can_kill` (hp 3 → death set, disengaged), `static_costs_two_ap_stuns_and_tears`, `an_ice_hit_and_an_attack_push_numbers` (in a construct, as the net tests do).
- [ ] **Step 2: Run** → fail. **Step 3: Implement.** **Step 4: Run** `cargo test -p strata-zero combat:: net::` → PASS. **Step 5: Commit** `feat(app): fire, the foe's shot, Static; damage numbers on both maps`

---

### Task 8: The lot start: `enter_city_at`, NEW GAME (COMBAT), save refusal

**Files:**
- Modify: `app.rs` (`enter_city` → `enter_city_at(city, minute)`; `enter_city` calls it with `START_MINUTE_OF_DAY`), `screens/title.rs` (menu `NEW GAME (COMBAT)` at index 2, `combat_game`), `game/src/character.rs` (`pub fn combat_runner(c) -> Character`), `screens/pause.rs` (SAVE refuses on a lot), `screens/explore.rs` (`place_label`: on a `Stratum` of a lot city → `the lot`)
- Test: `title.rs`, `character.rs`, `pause.rs`, `explore.rs` tests

**Interfaces:**
- `Character::combat_runner(c)`: `debug_runner` + `give_item(neural_port)` worn in `Slot::Neural` (through `wear`), `skills` gains `handguns`, `give(stim_patch, 3)`.
- `Title::combat_game(st)`: `st.character = combat_runner`; `st.enter_city_at(City::lot(seed), 18 * 60 + 15)`; `st.player = Some(Player::at(Place::Stratum(Street), Pos::new(6, 8)))` with `update_fov(&MapRef::stratum_at(city, Street, minute), bonus)`; `st.foes.push(Foe::goon(Place::Stratum(Street), Pos::new(18, 8)))`; `Switch { to: explore_screen(st), fx: Glitch }`.
- Pause SAVE on `city.is_lot()` → `st.warn("nothing to save on the lot")`, stays.

- [ ] **Step 1: Tests**: `combat_runner_wears_the_port_knows_handguns_and_carries_stims`; `the_combat_entry_opens_the_lot_at_dusk_with_a_goon_twelve_tiles_off` (menu index 2 → Switch; `st.city.is_lot()`, `minute_of_day() == 18*60+15`, player at (6,8), one foe at (18,8) idle, `st.world` has no people); `the_footer_reads_on_foot_the_lot`; `save_is_refused_on_the_lot`.
- [ ] **Step 2: Run** → fail. **Step 3: Implement.** **Step 4: Run** → PASS (the title's existing menu-text tests gain the new entry). **Step 5: Commit** `feat(app): NEW GAME (COMBAT): the lot at dusk, D'SHAWN and a goon`

---

### Task 9: The street's engaged keys: the target cursor, firing, moving, the prompt

**Files:**
- Modify: `screens/explore.rs` (`MapScreen.target: Option<Pos>`, `last_phase: Option<Phase>`; `handle_walk_inner` ~1527–1740; `turn_prompt` ~419; `update` ~1745–1980; the overlays ~2057–2160; `footer` ~311; the sidebar's `ice_lines` ~2324)
- Modify: `render/path.rs` (new `TargetOverlay`)
- Modify: `ui/hud.rs` (`Footer.engaged: bool` → `ENGAGED` magenta bold at the right of row 1, where `HUNTED` goes; foe lines in `(255,80,80)`: `Footer.ice` lines starting `▣ ` whose text does not contain `ICE`), `ui/sidebar.rs` (HERE takes the same lines)
- Modify: `actions.rs` (`perform`: `Action::Use` spends an AP while engaged through `spend_ap`, as `Fire` does; its refusals are `spend_ap`'s)
- Test: `explore.rs`, `hud.rs`, `actions.rs` tests

**Interfaces:**
- `TargetOverlay { line: &[Pos] /* between you and the target, ends excluded */, target: Pos, on_foe: bool, readout: Option<String> }`: line cells `·` `(90,90,90)` (glyph replaced, bg kept); the target cell inverted (fg black, bg CYAN) when `on_foe`, else the look cursor's block; the readout ` {name} · {weapon name} · {n} tiles · {chance}% ` (or `· out of range` / `· no line`) two rows above the target, cyan on `(0,20,26)`, the name bold, centred on the target and clamped into the map area.
- `MapScreen::open_target(&mut self, st)`: `target = foes::in_sight(st).first().map(pos)` (else `None`). Called when the act half opens on a stratum (the phase goes `Move → Act`, tracked by `last_phase` in `update`) and by `tab` when `target` is `None`.
- Keys on a stratum, your turn, act half, `target.is_some()` or not: `dir` → move `target` (default: your tile) one step within the map; `tab` → the next foe in sight after the one under the cursor, wrapping; `esc` → `target = None`; `space` → `combat::fire(st, foe)` when `foe_at(target)` else `pick a target`; `1`–`5` → `Strike` ability → `combat::strike(st, id, foe)` (`pick a target` when the cursor is off a foe); other abilities → `perform(Fire)` as today. On the move half on a stratum while engaged: a direct `hjkl` step spends a move through `spend_move` before `try_move`, and when `moves == 0` after it `stay(st)` opens the act half (as `walk_step` does); `g` opens the aim (`aim_path` on a stratum: `astar` over `city.walkable` tiles not held by a standing foe; `lit` as today); `enter` walks it through the existing `Walk` (`walk_step` on a stratum checks `city.walkable` and `foe_at` instead of the construct).
- `turn_prompt` on a stratum: act half → `("AIM", "hjkl target · tab next · space fire · 1-5 ability · . end turn")`; move half → the net's move prompt.
- `footer`: `engaged: net::engaged_on_foot(st)`; `ice` on a stratum → `foes::lines(st, seen)` with `seen` = vis not Unseen; the sidebar's `ice_lines` the same.
- `update`: the turn ends or leaves you → `target = None`; `last_phase` tracking.

- [ ] **Step 1: Tests** (`explore.rs`, lot state engaged by hand with the goon at (14,8)): `the_act_half_opens_the_cursor_on_the_nearest_foe` (space to stay → `s.target == Some((14,8))`, the drawn frame shows the readout `goon · Scrap nine · 8 tiles · 85%` (reflex 10, skilled, range 10: 50+40+10−15) and the inverted cell), `tab_cycles_foes_nearest_first`, `space_fires_and_spends_an_ap` (`AP ●○ 1/2`, three fx pushed), `space_off_a_foe_says_pick_a_target`, `esc_drops_the_cursor`, `a_direct_step_spends_a_move_and_the_last_opens_the_act_half`, `g_aims_a_path_on_the_street_and_enter_walks_it`, `street_keys_on_the_foes_turn` (`.` and `e` → `not your turn`), `number_keys_in_a_street_fight` (netscan spends one AP; static off a foe → `pick a target`, AP unchanged; static on a foe → AP 0, stun), `the_footer_reads_engaged_and_the_foe_line`, `a_stim_from_the_deck_costs_an_ap` (`actions.rs`: `perform(Use(stim_patch))` engaged on the act half → AP −1; on the move half → `move first · space to stay`).
- [ ] **Step 2: Run** → fail. **Step 3: Implement.** **Step 4: Run** `cargo test -p strata-zero` → PASS. **Step 5: Commit** `feat(app): the target cursor, firing from the street, the engaged prompt and footer`

---

### Task 10: Smoke, screenshots, docs

**Files:**
- Modify: `screens/shots.rs` (ignored `combat_aim` and `combat_burst` shots: the lot, engaged, the cursor on the goon; and four ticks into a burst), `screens/explore.rs` (one smoke test `a_whole_street_fight`), `README.md` (a short "Street combat" section: the COMBAT entry, `e`, the pools, aim/fire keys, Static), `docs/superpowers/specs/2026-10-19-ranged-combat-design.md` (status line: built)

- [ ] **Step 1: The smoke test**: COMBAT start → walk east four tiles (`l` ×4 through `update`) → engaged by sight (`ENGAGED · the goon has you`) → settle until your turn → `space` (stay) → `space` (fire, force_hit true) → `.` → settle → repeat until `goon down` → calm at the round's end; assert the log lines in order.
- [ ] **Step 2: Run** `cargo test -p strata-zero a_whole_street_fight` → PASS; `SHOT_DIR=/tmp/shots cargo test -p strata-zero shots::combat -- --ignored` writes two SVGs; look at them.
- [ ] **Step 3: Full check**: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` → clean.
- [ ] **Step 4: Build the smoke binary** `cargo build --release --target-dir target/smoke` and run `target/smoke/release/strata` by hand through one fight (NEW GAME (COMBAT) → walk → aim → fire → the goon's turn → down).
- [ ] **Step 5: Commit** `docs: street combat in the README; combat shots; the spec's build notes`
