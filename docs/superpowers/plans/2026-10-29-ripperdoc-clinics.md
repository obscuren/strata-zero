# Ripperdoc Clinics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Chrome goes in and out only in a ripperdoc's chair, for a fee, in clinics that
hide behind five hand-laid front businesses; `ripper` frees it in dev.

**Architecture:** `game` owns the fee rule (rates in content). `citygen` owns the fronts,
their layouts (the clinic's floor *is* its layout, turned to face the main door) and the
chair furniture. `strata-zero` gets a `clinic` module (open/closed by the doc's presence,
the chair's state, the one `operating` question) that `actions::perform`, `interact`,
`explore`, the deck and the ledger ask.

**Tech Stack:** Rust 2024 workspace, ratatui, serde/RON content, postcard saves.

**Spec:** `docs/superpowers/specs/2026-10-29-ripperdoc-clinics-design.md` (its "My calls" record the builder check's rulings: night hours 18:00–04:00, the storefront door, back doors into the front room, a locked room per layout, Look tells)

## Global Constraints

- The mark reads `· CHROMING UP` (amber, bold) on the existing CHROME header line; never a second line.
- Free chrome mark: ` FREE ` black on amber (the DEV badge's colours).
- Fees: put in 12%, take out 4% of `value`, nearest ¤10, at least ¤10: `max(10, (value * pct + 500) / 1000 * 10)`.
- Rates in `flavor/content.ron` as `surgery: (install: 12, pull: 4)`; serde default the same.
- Copy, verbatim: `chrome goes in and out in a ripperdoc's chair`; `Doc Lou has gone for the night`;
  `the doc's fee is ¤370 · you have ¤320`; `Hot Pot Lou is closed · opens at 18:00`;
  `CLOSED · opens 18:00 · closes 04:00`; `you get out of the chair`; `ripper on · chrome swaps anywhere, free`; `ripper off`.
- `Furniture::Chair` and every new enum variant that reaches a saved type is appended last. `FORMAT_VERSION` stays 4.
- Kits, runners and `Character::install` stay unlocked and free.
- Jacked in, `JACKED_IN` wins over every new refusal.
- Never rebuild `target/release/strata`; builds and smoke runs use `--target-dir target/smoke`.
- Run the tests per crate as each task says; the workspace run at the end is `cargo test --workspace --target-dir target/smoke`.

## Review Focus

- A clinic whose main door is on the west, north or east edge: the turned layout's `E` must be that door's exit and its doorstep inside. (Task 4 tests every turn of every layout, and every clinic in six seeds.)
- A clinic building with two street doors, or a cellar clinic with a tunnel door: every portal must get its own exit, or entering through it fails with "no way in here". (Task 4.)
- The doc leaving at 04:00 while the player sits in the deck: the next chrome key must refuse with the farewell and the mark must go. (Task 6, Task 10.)
- Pulling with too little money in the chair, and a swap whose in-plus-out fee is short: refused before anything moves. (Task 7.)
- `x` armed, then the cursor moves or Q/E changes tab: the arm must drop, so a later `x` on another piece doesn't pull it unasked. (Task 10.)

---

### Task 1: The doc's fees (`game`)

**Files:**
- Create: `crates/game/src/surgery.rs`
- Modify: `crates/game/src/content.rs` (the `Content` struct), `crates/game/src/lib.rs` (module and re-exports), `flavor/content.ron` (top-level `surgery:` entry)

**Interfaces:**
- Produces:
  - `pub struct Surgery { pub install: u32, pub pull: u32 }` (percent), `Default` = 12 / 4, `#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]`.
  - `Content::surgery: Surgery` with `#[serde(default)]`.
  - `pub fn put_in_fee(c: &Content, item: &ItemId) -> u32` and `pub fn take_out_fee(c: &Content, item: &ItemId) -> u32` (unknown item: 0).
  - `pub fn surgery_fee(ch: &Character, c: &Content, action: &Action) -> Option<u32>`: `Some` for `Install`, `InstallCopy`, `Pull`, and `Fit`/`Unfit` whose host is `Host::Site(_)`; `None` for everything else. Install and Fit add the take-out fee of what the place holds now. What is fitted inside a moved piece is never charged.
  - Re-exported from `game`: `Surgery`, `put_in_fee`, `take_out_fee`, `surgery_fee`.

- [ ] **Step 1: Write the failing tests** in `surgery.rs`'s `mod tests`, on `Content::load()` (or the crate's usual test content loader):
  - `fees_follow_the_table`: for each `(id, put, take)` in `[("smart_ink",80,30),("neural_port",180,60),("target_lens",260,90),("arc_hand",310,100),("piston_arm",340,110),("grip_arm",370,120),("wrecker_arm",590,200),("redline_spine",620,210),("ceramic_plating",820,270)]` assert `put_in_fee == put` and `take_out_fee == take`.
  - `the_floor_is_ten`: a `Content` whose item has `value: 50` charges 10 both ways.
  - `a_swap_is_in_plus_out`: a character with `piston_arm` installed at `Site::LeftArm` and `grip_arm` in the pack: `surgery_fee(&ch, &c, &Action::Install { item: grip_arm, site: LeftArm }) == Some(370 + 110)`.
  - `fitted_pieces_ride_free`: `piston_arm` at LeftArm with `arc_hand` fitted in socket 0: `Pull(LeftArm)` costs `Some(110)`; `Unfit { host: Host::Site(LeftArm), socket: 0 }` costs `Some(100)`; `Fit` of a pack `arc_hand` copy into an empty socket costs `Some(310)`.
  - `other_actions_have_no_fee`: `Action::Equip(..)`, `Action::Use(..)`, and `Fit { host: Host::Worn(Slot::Deck), .. }` give `None`.
  - `rates_come_from_content`: with `c.surgery = Surgery { install: 10, pull: 5 }`, `put_in_fee(grip_arm) == 310` and `take_out_fee(grip_arm) == 160`.
- [ ] **Step 2: Run and see them fail:** `cargo test -p game --target-dir target/smoke surgery` → compile errors (no module).
- [ ] **Step 3: Implement** `surgery.rs`, the `Content` field, the RON entry `surgery: (install: 12, pull: 4),`, and the re-exports. The held piece of a socket is the site's copy's `fitted[socket]` (`Character::chrome_copy`); of a site, `Character::chrome_at`.
- [ ] **Step 4: Run:** `cargo test -p game --target-dir target/smoke` → all pass.
- [ ] **Step 5: Commit:** `git commit -m "feat(game): the doc's fees, 12% in and 4% out, rates in content"`

### Task 2: The operating chair (furniture)

**Files:**
- Modify: `crates/citygen/src/tile.rs` (`Furniture`, `label`), `crates/strata-zero/src/render/tileset.rs` (glyph, colour, legend, its tests)

**Interfaces:**
- Produces: `Furniture::Chair` (last variant), label `"operating chair"`; glyph `'╦'`; colour `Rgb(0, 229, 255)`; legend `('╦', "operating chair")` beside the other furniture.

- [ ] **Step 1: Failing test:** extend `home_furniture_and_railings_have_glyphs` with `(Furniture::Chair, '╦')`, and add `the_chair_is_cyan`: `feature_color(Feature::Furniture(Furniture::Chair), ..)` is `Rgb(0, 229, 255)`; `legend_entries(StratumKind::Street)` contains `('╦', "operating chair")`.
- [ ] **Step 2: Run:** `cargo test -p strata-zero --target-dir target/smoke tileset` → fails to compile.
- [ ] **Step 3: Implement** the variant (last, with a doc comment "Last, so old saves keep their indices"), its label, glyph, colour arm before the `Furniture(_)` catch-all, and the legend entry.
- [ ] **Step 4: Run:** `cargo test -p citygen --target-dir target/smoke && cargo test -p strata-zero --target-dir target/smoke tileset` → pass.
- [ ] **Step 5: Commit:** `git commit -m "feat(citygen): the operating chair"`

### Task 3: Fronts and their names (`citygen`)

**Files:**
- Create: `crates/citygen/src/clinic.rs`
- Modify: `crates/citygen/src/lib.rs` (`pub mod clinic; pub use clinic::{Front, clinic_front, kind_label};`), `crates/citygen/src/flavor.rs`, `crates/citygen/src/generate/naming.rs`, `flavor/names.ron`

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)] pub enum Front { NoodleBar, Laundromat, Dentist, Tattoo, Butcher }`, `Front::ALL: [Front; 5]`, `Front::label(self) -> &'static str` ("noodle bar", "laundromat", "dentist", "tattoo parlour", "butcher"), `Front::floors(self) -> usize` (2 for `Laundromat`, else 1).
  - `pub fn clinic_front(city: &City, id: BuildingId) -> Option<Front>`: `None` unless the building is a `Clinic`; `Laundromat` when `span.lo == StratumKind::Undercity`; else `Front::ALL[hash64(city.seed, 0xC11C ^ id.index() as u64) % 5]`.
  - `pub fn kind_label(city: &City, id: BuildingId) -> &'static str`: the front's label for a clinic, else `kind.label()`.
  - `FlavorPack::fronts: Vec<(Front, Vec<String>)>` (`#[serde(default)]`), `FlavorPack::front_name(&self, front: Front, rng: &mut impl Rng) -> Option<String>`: the same calls as `shop_name` (one `pick`, one `syllable_word`).

- [ ] **Step 1: Failing tests** in `clinic.rs` on `small()`/Medium seeds 1, 2, 8, 11:
  - `every_clinic_has_a_front_and_nothing_else_does`.
  - `a_basement_clinic_is_a_laundromat`.
  - `the_front_is_stable`: two calls agree, and agree across two cities generated from the same params.
  - `fronts_cover_all_five`: over the ids `0..400` of a city (calling the hash directly through a `#[cfg(test)]` helper `front_for(seed, id, basement)`), every front appears.
  - `kind_label_names_the_front`: for each clinic, `kind_label == front.label()`; for a non-clinic, `== kind.label()`.
  - In `naming.rs`: `clinics_are_named_for_their_front`: every clinic's name matches one of its front's templates with `{syl}` as a capitalised word (check prefix/suffix around `{syl}`).
- [ ] **Step 2: Run:** `cargo test -p citygen --target-dir target/smoke clinic` → fails to compile.
- [ ] **Step 3: Implement** `clinic.rs`; in `names.ron` remove the `(Clinic, …)` line from `shops:` and add after it:
  ```
  fronts: [
      (NoodleBar, ["Hot Pot {syl}", "{syl} Noodles", "Ramen {syl}", "{syl} Bowl"]),
      (Laundromat, ["{syl} Wash", "Suds {syl}", "{syl} Coin-Op", "Laundromat {syl}"]),
      (Dentist, ["Saint {syl} Dental", "{syl} Dental", "Dr {syl}, Dentist"]),
      (Tattoo, ["{syl} Ink", "Needle {syl}", "{syl} Tattoo"]),
      (Butcher, ["{syl} Meats", "{syl} & Son, Butchers", "Cold Cuts {syl}"]),
  ],
  ```
  In `NamingStage::step`, a building with `clinic_front(&ctx.city, id)` takes `ctx.flavor.front_name(front, rng)`; the rest as before.
- [ ] **Step 4: Run:** `cargo test -p citygen --target-dir target/smoke` → pass (fix any seed-name snapshot test only if it named a clinic).
- [ ] **Step 5: Commit:** `git commit -m "feat(citygen): clinics hide behind fronts, named for them"`

### Task 4: The hand-laid clinic floors (`citygen`)

**Files:**
- Create: `crates/citygen/src/interior/layouts/clinics.rs`
- Modify: `crates/citygen/src/interior/plan.rs` (`FloorPlan`, `FloorPlan::new`, `floor_type`), `crates/citygen/src/interior/layouts.rs` (`mod clinics;`, `exit_tile`, `exits`, `generate`, remove `fn clinic`), `crates/citygen/src/interior/mod.rs` (tests that named the generic clinic)

**Interfaces:**
- Consumes: `Front`, `clinic_front` (Task 3); `Furniture::Chair` (Task 2).
- Produces:
  - `pub struct Turn { pub quarters: u8, pub mirror: bool }` (`Clone, Copy, Debug, PartialEq, Eq`), in `clinics.rs`, re-exported from `interior`.
  - `pub struct ClinicPlan { pub front: Front, pub turn: Turn, pub entrance: Pos }` and `FloorPlan::clinic: Option<ClinicPlan>`; `entrance` is the turned `E`.
  - `Floor::chair(&self) -> Option<Pos>`: the floor's `Furniture::Chair` tile.
  - Layout grids as `const` arrays, exactly the spec's five (laundromat: two grids, street then cellar), with the spec's legend.
  - `fn turn_size(w: i32, h: i32, t: Turn) -> (i32, i32)` and `fn turn_pos(p: Pos, w: i32, h: i32, t: Turn) -> Pos` (mirror first, then `quarters` clockwise quarter turns), pure and tested.
  - The turn that puts the layout's bottom edge on the main door's footprint edge (`edge(fp, door)`: 0 west, 1 east, 2 north, 3 south): south 0, west 1, north 2, east 3 quarters clockwise; `mirror` from bit 0 of `hash64(city.seed, 0xC11D ^ id)`.

- [ ] **Step 1: Failing tests** in `clinics.rs`:
  - `layouts_are_sound`: for every front and every floor grid: equal widths, the frame only `#`/`w`/`E` with `#` corners, exactly one `E` on the bottom row of the street grid and none elsewhere, one `X` per front with a `d` 4-adjacent, `v` on the street grid and `^` on the cellar grid at the same column and row, and every walkable mark (`. d E g + v ^ *`) reachable from `E` (cellar: from `^`).
  - `turns_round_trip`: for all 8 turns, `turn_pos` maps the four corners of a `w×h` grid onto the four corners of `turn_size`, and the bottom-row centre lands on the target edge (south/west/north/east as above).
  - In `interior/mod.rs` tests, `every_clinic_is_its_front` over Medium seeds 1, 2, 3, 8, 11, 21: for each clinic, `plan.clinic` is `Some`, `plan.size` is the turned layout size, every level validates with `validate_floor`, the street floor (laundromat: B1) has exactly one `Furniture::Chair` with a `Role::Surgery` station 4-adjacent, the main street door's exit is `entrance` with a walkable doorstep, every portal of the building on that floor's stratum has an exit with its own `at`, and a laundromat has `lowest == -1` with stairs at `plan.stair` on both floors.
  - Update `bars_clubs_clinics_temples_halls_factories_and_garages_have_their_rooms` (and the test at the other `BuildingKind::Clinic` list in `interior/mod.rs`) so a clinic is checked for a `Surgery` and a `Waiting` room, not the old stool rows and bed.
- [ ] **Step 2: Run:** `cargo test -p citygen --target-dir target/smoke clinic` → fails.
- [ ] **Step 3: Implement.**
  - `FloorPlan::new`: for a clinic, `size` is the turned layout size; `lowest` is −1 when `front.floors() == 2`; `stair` is the turned `v`; `lift` is `None`; `clinic` set. Everything else as today.
  - `floor_type`: a clinic's every level is `FloorType::Clinic` (checked before the `level < 0` basement rule).
  - `exit_tile(plan, door)`: the main street door (the first Street portal) of a clinic maps to `entrance`.
  - `exits()`: when a mapped tile is already used, the exit takes the nearest non-corner frame tile not used and not a window (walk the border outward from the mapped tile), so no portal goes without an exit.
  - `Builder::clinic_floor(&mut self, plan: &ClinicPlan)`: stamps the level's grid through `turn_pos`: `#` wall (frame tiles included), `w` window, `E` and `+` doors, `v`/`^` left to `shafts()`, `*` `Feature::Lamp`, `g` `Terrain::Grate`, `d` a `Role::Surgery` station, the letters their furniture, `X` `Furniture::Chair`; pushes rooms from the walled regions (the one holding `X` `Surgery`, the one holding `E` `Waiting`, the rest `BackRoom`, none private, rects their bounding boxes clamped to `frame.inset(1)`, `Surgery` first in `rooms`). Exits' doors and doorsteps stay as `exits()` set them.
  - `generate()`: `FloorType::Clinic => b.clinic_floor(..)`; skip `b.lamps()` for clinic floors (the layout has its lamps); `connect()` still runs.
  - Delete the generic `fn clinic`.
- [ ] **Step 4: Run:** `cargo test -p citygen --target-dir target/smoke` → pass; then `cargo test -p sim --target-dir target/smoke` (sim builds floors) → pass.
- [ ] **Step 5: Commit:** `git commit -m "feat(citygen): five hand-laid clinics, the floor turned to face the door"`

### Task 5: One doc per clinic; fronts named everywhere

**Files:**
- Modify: `flavor/people.ron` (ripperdoc `staff: (1, 1)`), `crates/sim/src/world.rs` (card), and the kind-label call sites: `crates/strata-zero/src/look.rs:604-611`, `ui/hud.rs:690,698`, `ui/panel.rs:91,113`, `access.rs:87,139`, `interiors.rs:206`, `net.rs:462`, `constructs.rs:47`

**Interfaces:**
- Consumes: `citygen::kind_label` (Task 3).

- [ ] **Step 1: Failing tests:**
  - sim `world.rs` tests: `a_doc_at_work_is_in_the_back_room`: the card of a ripperdoc inside their clinic at 12:00 reads `doing == "in the back room"`; `one_doc_per_clinic`: at 12:00 on seed 8, each clinic has exactly one working ripperdoc inside.
  - strata-zero `look.rs` tests: `a_clinic_looks_like_its_front`: the Look card of a clinic's footprint tile has `second == "{front label} · {district}"`.
- [ ] **Step 2: Run** the two crates' tests → fail.
- [ ] **Step 3: Implement:** staff `(1, 1)`; the card's `BuildingKind::Clinic => "in the back room"`; every listed site calls `citygen::kind_label(city, id)` where it called `kind.label()` for a building.
- [ ] **Step 4: Run:** `cargo test -p sim --target-dir target/smoke && cargo test -p strata-zero --target-dir target/smoke` → pass.
- [ ] **Step 5: Commit:** `git commit -m "feat: one doc per clinic, and clinics named for their front"`

### Task 6: The clinic module (`strata-zero`)

**Files:**
- Create: `crates/strata-zero/src/clinic.rs` (`mod clinic;` in `main.rs`)
- Modify: `crates/strata-zero/src/app.rs` (`AppState::chair`, `AppState::free_chrome`, both in `new()`)

**Interfaces:**
- Produces:
  - `pub struct Chair { pub building: BuildingId, pub level: i32, pub doc: sim::PersonId }` (`Clone, Copy, Debug, PartialEq`); `AppState::chair: Option<Chair>` (None); `AppState::free_chrome: bool` (false).
  - `pub enum Operating { Chair { doc: String }, Free, Locked, Gone { doc: String } }`.
  - `pub fn operating(st: &AppState) -> Operating`: `Free` when `free_chrome`; with a chair: `Gone` when the player is not on its floor or its doc is not working inside the building, else `Chair`; without: `Locked`.
  - `pub fn doc_at_work(st: &AppState, b: BuildingId) -> Option<sim::PersonId>`: the ripperdoc in `world.inside(b)` with `OccupantRole::Working`.
  - `pub fn is_open(st: &AppState, b: BuildingId) -> bool`: not a clinic, or a doc at work.
  - `pub fn doc_name(st: &AppState, id: sim::PersonId) -> String`: `"Doc "` + the first word of the person's name.
  - `pub fn hours(st: &AppState) -> (u16, u16)`: the ripperdoc profession's `Hours::Day` (fallback `(600, 1320)`); `pub fn hhmm(minute: u16) -> String` (`"10:00"`).
  - `pub fn closed_line(st: &AppState, b: BuildingId) -> String` (`"Hot Pot Lou is closed · opens at 10:00"`) and `pub fn hours_line(st: &AppState) -> String` (`"CLOSED · opens 10:00 · closes 22:00"`).
  - `pub const OUTSIDE: &str = "chrome goes in and out in a ripperdoc's chair";`
  - `pub fn gone_line(doc: &str) -> String` (`"Doc Lou has gone for the night"`).

- [ ] **Step 1: Failing tests** in `clinic.rs`, on the shots' seed-8 Medium city (cached), seeded with `enter_city_at` at 22:00 (open) or 12:00 (shut); `clinic::testutil::{outside, inside, seated, NIGHT, DAY}`:
  - `a_clinic_is_open_while_its_doc_is_in`: at 12:00 `is_open` and `doc_at_work` is `Some`; at 23:30 `is_open` is false; a non-clinic is always open.
  - `operating_follows_the_chair`: `Locked` with no chair; `Chair { doc }` with a chair on the player's floor and the doc in; `Gone` when the player stands elsewhere; `Free` whenever `free_chrome`.
  - `lines_read_as_the_spec`: `hours_line == "CLOSED · opens 10:00 · closes 22:00"`, `closed_line` for a building named "Hot Pot Lou" (set its name) `== "Hot Pot Lou is closed · opens at 10:00"`, `gone_line("Doc Lou") == "Doc Lou has gone for the night"`, `hhmm(600) == "10:00"`.
- [ ] **Step 2: Run:** `cargo test -p strata-zero --target-dir target/smoke clinic::` → fails.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same → pass.
- [ ] **Step 5: Commit:** `git commit -m "feat: the clinic module: doors by the doc, the chair, operating"`

### Task 7: The lock and the fee in `actions::perform`

**Files:**
- Modify: `crates/strata-zero/src/actions.rs`

**Interfaces:**
- Consumes: `game::surgery_fee` (Task 1); `clinic::{operating, Operating, OUTSIDE, gone_line}` (Task 6).
- Produces: `pub fn fee_line(fee: u32, credits: u32) -> String` (`"the doc's fee is ¤370 · you have ¤320"`), used by the ledger too.

- [ ] **Step 1: Failing tests** in `actions.rs` tests (keep `chrome_is_locked_jacked_in` as is):
  - `chrome_is_locked_outside_a_chair`: on the street, `Install`, `InstallCopy`, `Pull`, `Fit`/`Unfit` on `Host::Site` each give `Err(OUTSIDE)` and change nothing; `Fit` on `Host::Worn(Slot::Deck)` is not refused by it.
  - `the_chair_charges`: with a chair (doc in, player on the floor) and ¤1000, installing `grip_arm` at an empty RightArm succeeds, credits become 630, and the message ends `" · ¤370 to Doc {name}"`.
  - `short_is_refused_first`: with ¤320 the same install gives `Err("the doc's fee is ¤370 · you have ¤320")` and the arm stays in the pack.
  - `a_short_swap_is_refused`: piston arm installed, ¤400: swapping in `grip_arm` (fee 480) is refused, nothing moves.
  - `the_doc_gone_ends_the_chair`: with the player moved off the floor, `Pull` gives `Err(gone_line(..))` and `st.chair` is `None` afterwards.
  - `free_chrome_is_free`: `free_chrome = true` on the street: the install succeeds and credits don't move.
  - `jacked_in_still_wins`: jacked in with a chair: `Err(JACKED_IN)`.
- [ ] **Step 2: Run:** `cargo test -p strata-zero --target-dir target/smoke actions::` → fail.
- [ ] **Step 3: Implement** in `perform`, after the jacked-in check, for actions `surgery_fee` prices: the spec's order (Locked → `OUTSIDE`; Gone → clear `st.chair`, gone line; Chair and short → `fee_line`); after a successful `game::apply`, take the fee from `st.character.credits` (not with `Free`) and append ` · ¤{fee} to {doc}` to the message (not with `Free`, nor when the fee is 0).
- [ ] **Step 4: Run** the same, then `cargo test -p strata-zero --target-dir target/smoke sheet::` (the ledger's existing tests now run outside a chair: give their helpers `st.free_chrome = true` where they install or pull, so they keep testing the ledger and not the lock).
- [ ] **Step 5: Commit:** `git commit -m "feat: chrome changes only in the chair, for the doc's fee"`

### Task 8: The shut door

**Files:**
- Modify: `crates/strata-zero/src/screens/explore.rs:1747` (the door step), `crates/strata-zero/src/interact.rs` (`gather`), `crates/strata-zero/src/screens/shots.rs` (a shot)

**Interfaces:**
- Consumes: `clinic::{is_open, closed_line, hours_line}`.

- [ ] **Step 1: Failing tests:**
  - explore tests: `a_shut_clinic_turns_you_away`: at 23:30, stepping onto a clinic's street door leaves the player on the street and the log's last line is `closed_line`; at 12:00 the same step enters.
  - interact tests: `a_shut_door_says_when`: at 23:30, standing beside a clinic's door, `gather` has a subject on the door tile whose header name is the building's name with glyph `('▯', AMBER)`, subtitle `hours_line`, and no choices besides Look; at 12:00 there is no such subject.
- [ ] **Step 2: Run** → fail.
- [ ] **Step 3: Implement:** in the door step, before `interiors::enter`, refuse with `st.warn(closed_line)` when `!clinic::is_open`; in `gather`, for each 4-neighbour door tile of a closed building whose portals include it, `add` a subject with that header and subtitle and no rows (Look comes from `window`). Add the ignored shot `the_shut_door` (`tmp_pop_closed`'s setup from the mock, named for real).
- [ ] **Step 4: Run** the explore and interact tests → pass.
- [ ] **Step 5: Commit:** `git commit -m "feat: a clinic's door is shut while its doc is out"`

### Task 9: Sitting in the chair

**Files:**
- Modify: `crates/strata-zero/src/interact.rs` (the chair's subject), `crates/strata-zero/src/screens/popup.rs` (`Pick::Chair`), `crates/strata-zero/src/screens/deck.rs` (`Deck::chair`, close ends the chair), `crates/strata-zero/src/screens/shots.rs`

**Interfaces:**
- Consumes: `Floor::chair`, `clinic::{doc_at_work, doc_name, Chair}`, `clinic::testutil::{inside, seated, NIGHT, DAY}`.
- Produces: `Pick::Chair { building: BuildingId, level: i32 }`; `Deck::chair(st: &mut AppState) -> Deck` (open on `Tab::Attributes`, `sheet.state.pane = SheetPane::Ledger`, the ledger's row on its first chrome row, the card typing).

- [ ] **Step 1: Failing tests:**
  - interact: `the_chair_has_a_window`: beside a clinic's chair at 12:00 the chair's subject has header `("Operating chair", glyph ('╦', cyan))`, subtitle starting `"Doc "` and containing `“`, and a row `i` "Interact" / "Sit in the chair" that is enabled; with the doc gone the row is disabled with tail `" · the doc is out"`.
  - popup: firing `Pick::Chair` sets `st.chair` to that building, level and doc, logs `Doc {name}: “…”`, and returns `ScreenCmd::Replace` whose screen is a `Deck` on ATTRIBUTES with the ledger focused.
  - deck: `closing_gets_you_up`: a chair deck, esc → `ScreenCmd::Pop`, `st.chair == None`, last log `you get out of the chair`; a deck opened without a chair closes without that line.
- [ ] **Step 2: Run** → fail.
- [ ] **Step 3: Implement.** The subtitle's line is one of the profession's lines picked as Talk picks (`world.card(id, city).line`). Add the ignored shot `the_chair_window`.
- [ ] **Step 4: Run** → pass.
- [ ] **Step 5: Commit:** `git commit -m "feat: sit in the doc's chair and the deck opens on the ledger"`

### Task 10: The ledger in the chair

**Files:**
- Modify: `crates/strata-zero/src/screens/sheet/ledger.rs` (`chrome_header`, `ledger_key`, `act`, `pull`, `install`, `ledger_hints`, `fit_card`), `crates/strata-zero/src/screens/sheet/mod.rs` (`Sheet::pulling`, the pane rule's colour), `crates/strata-zero/src/screens/shots.rs`

**Interfaces:**
- Consumes: `clinic::{operating, Operating, OUTSIDE}`; `game::{put_in_fee, take_out_fee}`; `actions::fee_line`.
- Produces: `Sheet::pulling: Option<LedgerRow>` (transient, not in `SheetState`).

- [ ] **Step 1: Failing tests** in `ledger.rs` tests (a helper `in_chair(&mut st)` that seats the player in a seed-8 clinic at 12:00; the existing `loadout` stays):
  - `the_mark_is_on_the_header_line`: in the chair, row 5 at 100×30 reads `CHROME · CHROMING UP ` then `─`, then `LOAD`; `· CHROMING UP` cells are AMBER and bold; the pane rule `║` is AMBER; no other row contains `CHROMING`. With `free_chrome`, the row reads `CHROME  FREE ` with ` FREE ` black on the DEV amber. Outside, no mark and the rule as today.
  - `hints_in_the_chair`: on the Piston arm, `hints == "↑↓ select · space what fits · x pull it out ¤110 · ← attributes · esc get out of the chair"` with `¤110` AMBER.
  - `hints_anywhere_else`: `hints == "↑↓ select · ← attributes · Q/E tabs · tab close   chrome goes in and out in a ripperdoc's chair"`, the last part FAINTER; space and `x` warn `OUTSIDE` and open nothing.
  - `the_fee_line`: in FITS on Grip arm at an empty R ARM with ¤1200, the card's line under `load 19 → 22/24` is `fee ¤370 · leaves ¤830` (`¤370` AMBER bold); on L ARM (a swap) `fee ¤480 · in ¤370 · out ¤110`; with ¤320 `fee ¤370 · you have ¤320` all MAGENTA, the hint's `space install` FAINTER, and space leaves the picker open with `fee_line(370, 320)` in the strip.
  - `over_budget_goes_magenta`: COOL lowered so the budget is 22, Wrecker arm picked at R ARM: the header's added segment and `23` are MAGENTA; Grip arm at the same budget keeps AMBER.
  - `x_asks_first`: `x` on the Piston arm changes nothing and the hint reads `x again to pull Piston arm for ¤110 · it comes out with its Arc hand · esc keep it`; a second `x` pulls and takes ¤110; `x`, then esc: nothing moves, the deck stays open; `x`, then ↓, then `x`: the second `x` arms the new row, it does not pull; `x`, then `e` (next tab): the arm is gone.
  - `free_x_pulls_at_once`: with `free_chrome`, one `x` pulls.
- [ ] **Step 2: Run:** `cargo test -p strata-zero --target-dir target/smoke sheet::` → fail.
- [ ] **Step 3: Implement.** `chrome_header` takes the mark (`Operating`) and, for a pick over budget, MAGENTA for the added segments and total; `header` gets the mark as spans after the title (amber `· CHROMING UP`, or the FREE badge) and starts the rule after them. The FITS card's fee line goes straight under the load line, pushing the rest down. `pulling` arms on `x` when the fee is above 0, and clears on any other key (esc swallowed while armed, so it does not close the deck). Add the ignored shots `ledger_in_the_chair` (on a piece, FITS paying, FITS short, a swap, `x` asking, free).
- [ ] **Step 4: Run** → pass.
- [ ] **Step 5: Commit:** `git commit -m "feat(sheet): CHROMING UP, the doc's fees, and x asks first"`

### Task 11: `ripper`

**Files:**
- Modify: `crates/strata-zero/src/screens/console.rs` (`COMMANDS`, `run`, help, tests)

- [ ] **Step 1: Failing test:** `ripper_toggles_free_chrome`: `run("ripper") == Ok("ripper on · chrome swaps anywhere, free")`, `st.free_chrome`; again `Ok("ripper off")`; `complete(st, "", "rip") == ["ripper"]`; help contains `ripper`.
- [ ] **Step 2: Run** → fail.
- [ ] **Step 3: Implement** beside `eyes`/`light`; `COMMANDS` grows to 40.
- [ ] **Step 4: Run** → pass.
- [ ] **Step 5: Commit:** `git commit -m "feat(console): ripper frees chrome"`

### Task 12: Docs, shots and the whole run

**Files:**
- Modify: `README.md` (where it describes managing chrome), `docs/superpowers/specs/2026-10-29-ripperdoc-clinics-design.md` (only if a ruling changed during the build)

- [ ] **Step 1:** Update the README's chrome lines: chrome goes in and out at a ripperdoc's clinic, for a fee; `ripper` in the dev console.
- [ ] **Step 2:** `cargo fmt --all && cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings` → clean.
- [ ] **Step 3:** `cargo test --workspace --target-dir target/smoke` → all pass.
- [ ] **Step 4:** Shots: `SHOT_DIR=<scratch>/shots cargo test -p strata-zero --target-dir target/smoke shots:: -- --ignored`, render the new ones and look at them against the approved mocks.
- [ ] **Step 5:** Smoke: `cargo build --release --target-dir target/smoke`, run `target/smoke/release/strata` in a tmux pane, walk to a clinic by day (console `clock 12:00`, `tp`), sit, swap a piece, pull one; at night see the shut door.
- [ ] **Step 6: Commit:** `git commit -m "docs: clinics in the README"`
