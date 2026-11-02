# Doctor's in by ten Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The second slice of the morning after: Seren's word through the holo set and a mail
with a code, the package opening into a shard read by build, the clinic held shut for you,
Odile Vance's scene, her tray on Castellan's tab, and the hook to Coz.

**Architecture:** Generic story pieces in `crates/strata-zero/src/missions/` driven by
`flavor/missions.ron`: needs (conditions), readings (an item read at the rig, the bench or
by Look), a sim hold on a person, a held clinic opened by a journal code, an item that opens,
a tray of chrome on someone's tab. Content last, then shots.

**Tech Stack:** Rust workspace (citygen, sim, game, strata-zero), ratatui + crossterm, serde/RON.

**Spec:** `docs/superpowers/specs/2026-11-02-doctor-in-by-ten-design.md` (§ numbers below are
its sections). Mock pictures and the throwaway mock code in
`docs/superpowers/specs/2026-11-02-doctor-in-by-ten-mock/`.

## Global Constraints

- Build and test only with `--target-dir target/smoke`, never the default `target/`.
- Every task ends green on: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`; the same with `--features strata-zero/window`; `cargo test --workspace --target-dir target/smoke`. (Task 12 also runs the window test suite and the Linux cross-check.)
- No authored names in non-test Rust: no "Odile", "Coz", "Castellan", "Seren", "shard", "ledger", "package" in code outside `#[cfg(test)]`. Rust knows needs, readings, holds, trays and tabs; wording comes from data and item/role names.
- New optional RON fields use `#[serde(default)]` so existing data parses unchanged.
- Draw through ratatui only, exact `Color::Rgb` colours (reuse the missions/ui constants: MAGENTA, AMBER, CYAN, DIM, TEXT, FAINT and friends where they exist). `space` confirms, `esc` backs out; never `enter` as a window's confirm.
- Only box-drawing (U+2500–257F), block (U+2580–259F) and characters already drawn elsewhere in the game; `every_drawn_character_has_a_glyph` must stay green.
- `AppState::new()` is dev-locked; tests needing dev call `st.unlock_dev()`.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Match the surrounding code's comment density and naming (short doc comments, plain words).

## Review Focus

1. **A story restart** (`missions::begin` in a new city, or the same city's restart): `unsealed`, `read`, `tab`, `tray`, `notes`, the pending release and the run start ticks reset with `Missions::default`; the shard is taken back with the package. Test in Task 7.
2. **A different piece at the tab's chair** pays its fee as before and leaves the tab unspent; the tab piece at another clinic's chair pays. Test in Task 10.
3. **Esc on the tray** leaves it pending; talking to its `from` role reopens it; `Chromed` never fires before the tab is spent and you are out of the chair. Test in Task 10.
4. **`Late` across midnight**: the mission begins 07:20, you arrive at 01:00 the next day: late. Begins 07:20, arrive 09:59: not late. Test in Task 3.
5. **Nothing panics when a role can't be drawn**: a `Say` by a cast person who isn't on your floor goes to the corner as today; `Holo` away from the flat does nothing; a mail code for an uncast role leaves `{code}` blank and learns nothing. Tests in Tasks 5, 6, 9.

---

### Task 1: Holding a person at work (sim)

**Files:**
- Modify: `crates/sim/src/world.rs` (the `World` struct beside `satisfied`; the per-minute departure loop around `world.rs:778-811`)
- Test: `crates/sim/src/world.rs` tests module

**Interfaces:**
- Produces: `World::keep_at_work(&mut self, id: PersonId)`, `World::release(&mut self, id: PersonId)`, `World::is_held(&self, id: PersonId) -> bool`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_held_worker_stays_at_work_out_of_hours() {
    // A person whose work is a building (a ripperdoc: Day 18:00–04:00): at 10:00 keep them.
    // keep_at_work puts them inside their work building with goal Work at once; advance
    // six game hours: still inside it, still listed Working by `inside(b)`; is_held true.
}

#[test]
fn release_sends_them_where_the_schedule_says() {
    // Held at 10:00, release, advance two game hours: no longer inside the work building
    // (their schedule says not at work); is_held false.
}

#[test]
fn a_new_world_holds_nobody() { /* is_held false for every person of a fresh world */ }
```

- [ ] **Step 2: Run them to see them fail** — `cargo test -p sim --target-dir target/smoke held` → FAIL (no such methods).
- [ ] **Step 3: Implement.** `held: BTreeSet<PersonId>` on `World` (empty from every constructor). `keep_at_work`: set the person's goal to Work, whereabouts inside their work building (or at their work spot for a spot worker), clear any pending trip, insert into `held`. The departure loop `continue`s for held people. `release` removes from `held`. `advance` needs nothing extra.
- [ ] **Step 4: Run the tests** → PASS; run `cargo test -p sim --target-dir target/smoke` → all green.
- [ ] **Step 5: Commit** `sim: hold a person at work against their schedule`.

### Task 2: Items that open (game)

**Files:**
- Modify: `crates/game/src/model.rs` (`Item` near `on_use`, `model.rs:823`), `crates/game/src/action.rs` (`Action`, `apply`)
- Modify: `flavor/content.ron` (`taped_package` at line 482; a new `ledger_shard` beside it)
- Test: `crates/game/src/action.rs` tests

**Interfaces:**
- Produces: `Item.opens: Vec<(ItemId, u32)>` (`#[serde(default)]`); `Action::Open(ItemId)`; the outcome message `You open the <name>: <output names joined with ", ">.`; refusal `"<name> doesn't open"` for an item with empty `opens`, and the usual none-held refusal.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn opening_takes_one_and_gives_what_it_holds() {
    // A test content with item "box" opens [("bolt", 2), ("nut", 1)]: hold 1 box, apply
    // Action::Open("box"): box 0, bolt 2, nut 1, message == "You open the Box: 2× Bolt, Nut."
}

#[test]
fn an_item_that_does_not_open_is_refused() { /* Err contains "doesn't open"; pack unchanged */ }

#[test]
fn the_shipped_package_opens_into_the_shard() {
    // Content::load (the shipped content): taped_package.opens == [("ledger_shard", 1)];
    // ledger_shard exists, kind Component, value 0,
    // desc == "A ledger shard. Locked. Somebody will know whose."
}
```

- [ ] **Step 2: Run** `cargo test -p game --target-dir target/smoke open` → FAIL.
- [ ] **Step 3: Implement** the field, the action arm in `apply` (remove one with `remove_from_pack`, `give_item` each output), and the content: `taped_package` gains `opens: [("ledger_shard", 1)]`; `ledger_shard`: `Component`, weight 0.1, value 0, the desc above. Message format: `You open the <name>: <parts>.` where each part is `<name>` or `<n>× <name>` when n > 1.
- [ ] **Step 4: Run** the game tests → PASS.
- [ ] **Step 5: Commit** `game: items that open into others; the package holds a shard`.

### Task 3: Needs, the jacket the camera saw, conditional lines

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (new `Need`; `Beat::Say` gains `need`)
- Modify: `crates/strata-zero/src/missions/mod.rs` (`Missions`: `unsealed: BTreeSet<String>`, `read: BTreeSet<String>`; `Run` gains `began: Option<u64>` set when its first step fires; `Left` gains `worn`; `pub fn holds`)
- Modify: `crates/strata-zero/src/missions/news.rs` (`walk_away` records `worn`; `sought` names it when known)
- Modify: `crates/strata-zero/src/missions/scene.rs` (a `Say` whose need fails is skipped with no time passing)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq, Deserialize)]
  pub enum Need { Skill(String), Read(String), Carries(String), Unsealed(String),
                  Changed, Unchanged, Late(u16), Not(Box<Need>), All(Vec<Need>) }
  pub fn holds(st: &AppState, mission: Option<usize>, need: &Need) -> bool
  ```
  `mission` is the mission the need is asked for (for `Late`); `None` makes `Late` false.
  `Beat::Say { …, #[serde(default)] need: Option<Need> }`. `Left { at, heading, worn: Option<ItemId> }`.
  `pub fn jacket_seen(st: &AppState) -> Option<ItemId>`: the `worn` of the body of the news item in `Missions.news` (fresh or stale).
- Consumes: `Character::has_skill(&SkillId)`, `Character::count(&ItemId)`, `Character::equipped(Slot::Torso)`.

- [ ] **Step 1: Write the failing tests** (in `missions/mod.rs` tests unless noted)

```rust
#[test] fn skill_carries_unsealed_and_read_hold_as_named() { /* each true/false flip by setting the state */ }
#[test] fn not_and_all_combine() { /* All([]) true; Not(All([])) false; All([true,false]) false */ }
#[test] fn late_is_past_the_first_such_clock_after_the_mission_began() {
    // run.began at 07:20 on day 1: Late(600) false at 09:59, true at 10:01, true at 01:00 day 2.
    // run.began at 11:00: Late(600) false at 23:00, true at 10:01 day 2.
}
#[test] fn changed_and_unchanged_need_a_known_jacket() {
    // No news item: both false. A body with left.worn None: both false.
    // left.worn Some(salvage_jacket), wearing it: Unchanged true, Changed false;
    // wearing nothing or another jacket: Changed true, Unchanged false. Stale news: same answers.
}
// news.rs
#[test] fn walking_away_keeps_the_jacket_and_sought_names_it() {
    // wear salvage_jacket, walk away (the existing walk-away test setup), take it off:
    // left.worn == Some(salvage_jacket) and sought(...)[0] still says "salvage jacket".
}
// scene.rs
#[test] fn a_say_whose_need_fails_is_skipped_at_once() {
    // A test scene: Say(need: Some(Skill("nonesuch"))) then Say(no need): after one tick the
    // running beat is the second Say; the clock moved no more than one tick.
}
```

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke missions::` → the new tests FAIL.
- [ ] **Step 3: Implement.** `holds` per the spec §2 table; `Late(m)`: the deadline is the first tick at or after `run.began` whose minute-of-day is `m` (if `began` is already past `m` that day, the next day's); late when now > deadline. `Read(item)` is `st.missions.read.contains(item)`. `Missions::default` empties the new sets. Record `began` in `run_step` when `at == 0`. `walk_away` stores `equipped(Torso)`; `sought` uses `left.worn` when a `Left` exists. Scene: when advancing onto a `Say` with `need` that doesn't hold (asked with the scene's mission), move past it in the same tick.
- [ ] **Step 4: Run** → PASS; full `cargo test -p strata-zero --target-dir target/smoke` green.
- [ ] **Step 5: Commit** `missions: needs, conditional lines, the jacket the camera saw`.

### Task 4: Casting nearest first, names without people

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (`Pick::Nearest(String)`; `Book::needs_cast`)
- Modify: `crates/strata-zero/src/missions/cast.rs` (order; the new pick)
- Modify: `crates/strata-zero/src/missions/mod.rs` (`fill`: `{role.spaced}`; book-name fallback)

**Interfaces:**
- Produces: `Pick::Nearest(String)` (profession id; nearest work tile to the home door, spot or building; never dead or taken). `{role.spaced}`: each word of the name upper-cased with one space between its letters, words joined by three spaces (`"Coz"` → `"C O Z"`, `"Ab Cd"` → `"A B   C D"`).
  `needs_cast(&self, m: &Mission) -> BTreeSet<String>`: only roles needing a person (spec §9 list), scanning the mission's steps and the scenes its steps play (and their `next` chain).
- Consumes: `cast::work_tile`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn nearest_casts_the_nearest_of_its_profession() { /* Nearest("ripperdoc") picks the clinic doc whose work tile is nearest home_door */ }
#[test] fn nearest_picks_cast_before_workers() {
    // A book with roles "aaa": Worker(fixer, range 0..999) and "mezz": NearestFixer on the
    // story test seed: mezz is the nearest fixer (as with mezz alone), aaa someone else or uncast.
}
#[test] fn shipped_roles_cast_as_before() { /* on the seeds the story tests use, mezz and marlo are the same persons as on main (assert by name of their workplace or id captured from a book with only those two roles) */ }
#[test] fn a_name_slot_fills_from_the_book_when_nobody_plays_the_role() {
    // role "zed" uncast: fill("{zed} · {zed.spaced} · {zed.title}") == "Zed · Z E D · fixer"
}
#[test] fn needs_cast_skips_bare_names_and_counts_places() {
    // a mission whose Log names "{zed}" only: zed not needed; "{zed.place}": needed;
    // Talk("zed"): needed; a scene it plays with Say(who: "zed"): needed.
}
```

- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.** In `cast`, iterate roles in two passes: `NearestFixer`/`Nearest` first, then `Worker`, each in id order (visitors skipped). `fill` resolves `{r}`, `{r.spaced}`, `{r.title}` from the book when `missions.cast` lacks `r`; other slots of an uncast role stay as today.
- [ ] **Step 4: Run** → PASS; whole crate green (the existing story tests must pass unchanged).
- [ ] **Step 5: Commit** `missions: nearest picks first, spaced names, names without people`.

### Task 5: Beside someone indoors: `By`, the chair mark, anchors, tracking

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (`Where::By { role, within }`, `Mark::Chair(String)`)
- Modify: `crates/strata-zero/src/missions/mod.rs` (`at`), `missions/marks.rs` (`Chair`), `missions/scene.rs` (`spot(who)` for a cast person on your floor)
- Modify: `crates/strata-zero/src/interiors.rs` (an accessor for where a floor draws a person)
- Modify: `crates/strata-zero/src/track/mod.rs` (`resolve_story` for `At(By)`)

**Interfaces:**
- Produces: `pub fn drawn_at(st: &AppState, id: PersonId) -> Option<(Place, Pos)>` in `interiors.rs` — the street tile from `world.pos_of`, or, when the person is inside a building, the tile `Placement` puts them on for the floor they occupy (their station's floor). `Where::By` holds when `drawn_at` and the player share the `Place` and Manhattan ≤ `within`. `Mark::Chair(role)` → the `Furniture::Chair` tile on the role's work building (its floor). Track: `At(By{role,within})` → `Target::Spot { place, pos, radius: within }` from `drawn_at`, or `Target::Person` when on the street.
- Consumes: `interiors::Placement::new`, `cast` ids.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn by_holds_beside_a_doc_at_work_and_not_at_the_front_door() {
    // clinic::testutil::inside() at NIGHT, cast a role onto the doc: At(By(role, 4)) true at the
    // player's spot beside the chair; false standing on the clinic's entrance tile.
}
#[test] fn the_chair_mark_finds_the_doc_chair() { /* resolves to the tile with Furniture::Chair */ }
#[test] fn a_doc_line_anchors_beside_her() { /* scene Say(who: role) → the bubble's anchor is drawn_at's tile, not the corner */ }
#[test] fn a_line_by_someone_elsewhere_goes_to_the_corner() { /* role's person on another floor: no anchor, no panic */ }
#[test] fn tracking_by_leads_to_her_floor() { /* resolve_story → Spot on her floor at her tile, radius 4 */ }
```

- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** as above. `drawn_at` builds the placement for the person's floor from `st.interiors` (ensure the floor, as the explore draw does); keep it cheap enough to call each tick (cache nothing new unless a test shows it slow).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `missions: beside someone indoors, the chair mark, anchors and tracking`.

### Task 6: The held clinic, its street code, the mail that carries it

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (`Effect::Hold(String)`, `Effect::Release(String)`; `Effect::Mail` gains `#[serde(default)] code: Option<CodeOf>`; `enum CodeOf { Clinic(String) }`)
- Modify: `crates/strata-zero/src/missions/effects.rs`, `missions/mod.rs` (`Missions.releasing: BTreeSet<String>`, checked each tick)
- Modify: `crates/strata-zero/src/clinic.rs` (`is_open` false while the doc is held; `held(st, b) -> bool`)
- Modify: `crates/strata-zero/src/credentials.rs` (`pub const STREET_DOOR: u16`; what a code opens, for the journal's words). The code is learned by the `Mail` effect as it lands; the stored `Mail` struct is unchanged.
- Modify: `crates/strata-zero/src/interiors.rs` (`enter`: a shut clinic opens for its street code)
- Modify: `crates/strata-zero/src/interact.rs` (the shut door's line gains ` · code NNNN`)
- Modify: `crates/strata-zero/src/missions/visitors.rs` (where `Lights` draws: the held clinic's look)

**Interfaces:**
- Produces: `clinic::held(st, b) -> bool` (its doc `is_held`); `credentials::street_code(st, b) -> u16` (= `door_code(seed, b, 0, STREET_DOOR)`); `STREET_DOOR` = `u16::MAX`; journal line for a street code `door code NNNN · <building name>, street door · <source words>`; a back room code reads `<building name>, back room`. `{code}` in a mail body filled with the 4-digit code.
- Consumes: Task 1's `keep_at_work/release/is_held`; `mission` role → person via `missions.cast`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn hold_keeps_the_doc_in_and_the_clinic_shut() {
    // clinic::testutil::outside(DAY): cast a role on the clinic's doc, apply Hold(role):
    // doc_at_work Some; is_open false; interiors::enter → Err (closed_line) without the code.
}
#[test] fn the_street_code_lets_you_in_and_the_chair_works() {
    // learn the street code: enter → Ok("you key in NNNN"); seated() style sit → operating == Chair.
}
#[test] fn the_shut_door_shows_the_code_you_hold() { /* window line ends " · code NNNN" */ }
#[test] fn release_waits_until_you_are_out() {
    // held, inside: Release(role) → still held after a tick; step outside → next tick not held.
}
#[test] fn the_mail_learns_the_code_as_it_lands() {
    // Mail(code: Some(Clinic(role))) → journal.code for (b, 0, STREET_DOOR) present, Source::Mail;
    // the mail body's "{code}" replaced by the 4 digits; journal entry text contains "street door".
}
#[test] fn a_code_for_nobody_learns_nothing() { /* uncast role: body "{code}" → "", journal unchanged, no panic */ }
#[test] fn a_held_clinic_draws_dim_with_a_cold_pool() {
    // draw the map inside a held clinic: a cell 6 tiles from the chair is dimmer than unheld;
    // a cell beside the chair keeps its brightness with blue ≥ red.
}
```

- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.** Hold/Release map role → person; Release adds to `releasing`; each `missions::tick`, a role in `releasing` whose work building you are not inside is released and removed. `enter`: when the building is a shut clinic, `journal.code_for(b, 0, STREET_DOOR)` lets you in with the status `you key in NNNN`. Journal wording: the entry knows its building's kind and the room (flat number, back room, street door). The look (spec §5): factor 0.35 off-pool; within 3 tiles (Manhattan) of the chair full colour with red −20% and blue +20% (clamped).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `clinic: a held doc, the shutter's code, the mail that carries it`.

### Task 7: Unsealing and opening in the pack

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (`Effect::Unseal(String)`; `Book::gives` counts `opens` outputs of given items)
- Modify: `crates/strata-zero/src/missions/effects.rs`
- Modify: `crates/strata-zero/src/screens/deck.rs` (`item_popup`, `copy_popup`: an `Open` row), `crates/strata-zero/src/actions.rs` (`perform` refuses `Open` unless unsealed)

**Interfaces:**
- Produces: `missions::unsealed(st, id) -> bool`; popup row `Choice::new('o', "Open", Pick::Act(Action::Open(id)))` (pick a free letter if `o` is taken in that popup), shown only when `!item.opens.is_empty() && unsealed`; `perform(Action::Open)` → `Err("not yours to open")` while sealed.
- Consumes: Task 2's `Action::Open`, Task 3's `Missions.unsealed`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn the_package_offers_open_only_once_unsealed() { /* item_popup rows: no Open; Unseal; Open present */ }
#[test] fn opening_a_sealed_item_is_refused() { /* perform → Err("not yours to open"), pack unchanged */ }
#[test] fn opening_gives_the_shard() { /* after Unseal, perform Open → shard 1, package 0, message "You open the Taped package: Ledger shard." */ }
#[test] fn a_restart_takes_back_the_shard_too() {
    // gives() contains "ledger_shard"; begin on a fresh story with a shard in the pack → 0.
    // Missions::default(): unsealed, read empty.
}
```

- [ ] **Step 2–4:** run → FAIL, implement, run → PASS.
- [ ] **Step 5: Commit** `missions: the story unseals an item; the pack opens it`.

### Task 8: Readings and the HOLO card

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (`Book.readings: BTreeMap<String, Vec<Reading>>` `#[serde(default)]`; `Reading`, `ReadAt { Rig, Bench, Look }`, `Row`)
- Create: `crates/strata-zero/src/missions/reading.rs` (the card `Screen`)
- Modify: `crates/strata-zero/src/interact.rs` (rig rows), `screens/popup.rs` (a `Pick::Read { item: ItemId, at: ReadAt }`), `screens/deck.rs` (bench row; `item_card_of` uses `item_desc`), `look.rs` (map Look uses `item_desc`)

**Interfaces:**
- Produces:
  ```rust
  pub struct Reading { pub at: ReadAt, pub need: Need, #[serde(default)] pub verb: String,
      #[serde(default)] pub head: String, #[serde(default)] pub rows: Vec<Row>,
      #[serde(default)] pub close: String, #[serde(default)] pub tells: bool,
      #[serde(default)] pub text: String }
  pub enum Row { Field(String, String), Hot(String), Tag(String), Art(String), You(String), Ledger(u32, u32), Blank }
  pub fn reading(st: &AppState, item: &str, at: ReadAt) -> Option<&Reading>   // first whose need holds (mission None)
  pub fn item_desc(st: &AppState, id: &ItemId) -> String                     // Look reading's text, filled, else item.desc
  pub struct ReadCard { … } impl ReadCard { pub fn open(st: &mut AppState, item: &ItemId, at: ReadAt) -> Option<ReadCard> }
  ```
  `open` records `missions.read` when `tells`. Card layout per spec §3 and the mock `s2_read_rig`/`s2_read_bench`: bracket frame (`face::bracket_frame`), `HOLO ▸ <head>` (Rig) or `BENCH ▸ <head>` (Bench) at row 1 col 3, the signal `▂▄▆█` and `HH:MM` at the right as the feed draws them, rows from row 3 col 3, `space <close>` two rows from the bottom; width `min(66, area − 4)`, height rows + 7, centred two rows above middle; space and esc pop it. `Field` label padded to 8 columns. Backticked spans in `Field` text and `Art` are AMBER bold. `Ledger(shown, total)`: `shown` rows `MM.DD  ¤NN NNN  ▓▓▓▓▓▓▓  <place>` seeded by the city seed and the item id, every other row a place name from the city's districts or landmarks (others leave a FAINT bar), then `  …<total−shown> more`.
  Rig: `Choice::on('r', <verb>, <item name>, CYAN, Pick::Read{..})` per carried item with a Rig reading. Bench: `Choice::new(<free letter>, <verb>, Pick::Read{..})` in `item_popup` when `has_workbench` and a Bench reading holds.
- Consumes: Task 3 `holds`, `fill`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn the_first_reading_whose_need_holds_is_used() { /* two Look readings, needs flip */ }
#[test] fn item_desc_falls_back_to_the_description() {}
#[test] fn the_rig_offers_read_only_for_a_matching_reading() { /* hacking + shard carried: row "Read: Ledger shard"; no hacking: no row; no shard: no row */ }
#[test] fn the_bench_row_needs_a_bench_and_the_reading() {}
#[test] fn a_telling_card_records_the_read() { /* ReadCard::open → missions.read contains the item */ }
#[test] fn the_card_draws_its_rows() {
    // draw at 120x40 and 100x30: buffer has "HOLO", the head, "owner", the role's name,
    // "space", the close word; ledger rows count == shown; "…N more".
}
```

- [ ] **Step 2–4:** run → FAIL, implement, run → PASS (window clippy too: the card is a new Screen).
- [ ] **Step 5: Commit** `missions: readings of an item at the rig, the bench and by Look`.

### Task 9: Seren through the set; story notes

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (`Effect::Holo { from: String, text: String }`; `Effect::Note { at: Mark, text: String }`; `Beat::Note { at: Mark, text: String }`)
- Modify: `missions/effects.rs`, `missions/scene.rs`, `missions/mod.rs` (`Missions.notes: Vec<(Place, Pos, String)>`), `look.rs` (a note shows under its tile's Look card)

**Interfaces:**
- Produces: Holo label `format!("{}▚▞▚ {}", channel_without_last_three_chars, first_letter_of_from_name)`, colour `visitors::colour(&st.book, from)`, through `asides::say(Speaker::Tile(set_tile))` only while `at(Home)` and the set exists. Notes are filled at the moment they're set and appended as their own paragraph to the Look card of their tile.
- Consumes: `news::set_tile`, `news::channel`, `marks::resolve`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn holo_speaks_through_the_set_at_home() { /* one aside, speaker the set tile, label ends "▚▞▚ S" for a role named "Seren"-like test role, text filled */ }
#[test] fn holo_away_from_home_says_nothing() {}
#[test] fn a_note_reads_under_its_tile_look() { /* Note at Mark::Chair(role): look card text contains the note */ }
```

- [ ] **Step 2–4:** run → FAIL, implement, run → PASS.
- [ ] **Step 5: Commit** `missions: a role's line through the holo set; story notes`.

### Task 10: The tray, the tab, `Chromed`

**Files:**
- Modify: `crates/strata-zero/src/missions/content.rs` (`Effect::Tray { from, tab, title, line, sets: Vec<TraySet> }`, `TraySet { need: Need, items: Vec<String> }`, `Trigger::Chromed(String)`)
- Modify: `missions/effects.rs`, `missions/mod.rs` (`Missions.tray: Option<TrayOffer>`, `Missions.tab: Option<Tab>`; `Chromed` in `fires`)
- Modify: `screens/popup.rs` (`Pick::Tray(usize)`; Talk to the tray's `from` reopens it), `screens/explore.rs` (put up a pending tray once no scene plays, as feeds are popped)
- Modify: `screens/sheet/ledger.rs` (`pick_fee` and the fee line), `actions.rs` or wherever installs charge the fee (spend the tab)

**Interfaces:**
- Produces: `pub struct TrayOffer { pub mission: usize, pub step: usize, pub from: String, pub tab: String, pub title: String, pub line: String, pub items: Vec<ItemId>, pub shown: bool }`; `pub struct Tab { pub doc: PersonId, pub item: ItemId, pub payer: String, pub spent: bool }`. The window: `Popup::verbs("<from name> · <title>", rows).with_body(vec![line])`, rows `Choice::on('1'..'3', "Take", <name>, CYAN, Pick::Tray(i))` with `tail` ` · <site label lower-case> · grants <A>, <B>` (no grants: just the site). Taking: give the item, set `tab`, clear `tray`. Fee line in the doc's chair for the tab item: spans `fee ` DIM, `TAB · <PAYER NAME>` AMBER bold, ` · ¤<fee> on the tab` DIM (the mock's "on him" needs a pronoun the code doesn't have). Fee charged 0; the install marks `spent`. `Chromed(role)`: `tab.spent && tab.doc == cast(role) && st.chair.is_none()`.
- Consumes: Task 3 `holds` (set needs, mission Some), Task 5 drawing order.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn the_tray_offers_the_first_set_that_fits() { /* hacking → set 1's three items; nothing → the last set */ }
#[test] fn taking_a_piece_bags_it_and_opens_the_tab() {}
#[test] fn esc_leaves_the_tray_and_talking_reopens_it() {}
#[test] fn the_tab_piece_goes_in_free_once() {
    // seated at the tab doc: fee line has "TAB ·" and the payer name; install → credits unchanged, spent.
}
#[test] fn another_piece_pays_and_the_tab_waits() {}
#[test] fn the_tab_piece_pays_at_another_doc() {}
#[test] fn chromed_waits_for_spent_and_up() { /* spent but seated: false; after esc out of the chair: true */ }
```

- [ ] **Step 2–4:** run → FAIL, implement, run → PASS.
- [ ] **Step 5: Commit** `missions: a tray of chrome on someone's tab`.

### Task 11: The slice as content, played end to end

**Files:**
- Modify: `flavor/missions.ron` (cast `odile`, `coz`; missions `doctor`, `ledger`; scenes `odile`, `odile_out`; `readings`; header comment lines for the new effects/triggers/needs — exactly the spec §1, §3, §8 text)
- Test: `crates/strata-zero/src/missions/mod.rs` tests (story test), `missions/content.rs` (book test)

**Interfaces:**
- Consumes: everything above. The RON is the spec's, with the tray sets of §8 written as `TraySet(need: Skill("hacking"), items: ["neural_port", "feedback_fuse", "limbic_damper"])` … `TraySet(need: All([]), items: ["subdermal_mesh", "target_lens", "neural_port"])`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn the_shipped_book_loads_the_doctor() { /* missions doctor and ledger exist; scenes odile, odile_out; readings for ledger_shard and taped_package */ }
#[test] fn every_tray_piece_installs_on_bare_meat() {
    // for every set of the doctor's Tray: each item is Cyberware and its site accepts it on
    // Character::new (no chrome): install succeeds in a seated test state with free_chrome.
}
#[test] fn the_doctor_plays_end_to_end() {
    // Story test seed, morning_after done (as the morning news tests do): tick →
    // the doc held; package unsealed; mail from Seren with 4 digits in place of {code};
    // the journal holds the street code; objective names the clinic.
    // Open the package; with hacking, read at the rig → read recorded.
    // Walk into the clinic with the code, stand beside her → scene "odile" plays;
    // with Read done the Coz line is skipped; jacket lines per Changed/Unchanged.
    // Scene ends → tray up; take set item 1 → tab; sit, install → free; esc up → "out" scene;
    // its end → Release pending; step outside → released; mission doctor Done;
    // mission ledger Open with the Coz objective.
}
#[test] fn the_doctor_plays_with_one_fixer() { /* a city whose only fixer is Mezz: doctor Open/plays, ledger Uncast; no literal "{coz}" in any shown text */ }
```

- [ ] **Step 2–4:** run → FAIL, write the content, run → PASS.
- [ ] **Step 5: Commit** `story: doctor's in by ten`.

### Task 12: Shots and the full checks

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs` (an ignored `doctor_shots`)

- [ ] **Step 1:** `doctor_shots` at 120x40 and 100x30, named `doctor_<what>_<w>x<h>`: `seren_set` (the flat at 07:52, the set's glitched aside), `read_rig` (hacking character, card over the flat), `read_bench` (wrench character, card over the deck's GEAR & INVENTORY), `odile` (the held clinic at 10:00, the scene's first `Say` up beside her), `tray`, `chair_tab` (seated, the tab piece selected).
- [ ] **Step 2:** `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored doctor_shots` → PNGs written; look at each.
- [ ] **Step 3:** the five CLAUDE.md checks and `cargo check -p strata-zero --features window --target x86_64-unknown-linux-gnu --target-dir target/smoke` → all green.
- [ ] **Step 4: Commit** `shots: doctor's in by ten`.
