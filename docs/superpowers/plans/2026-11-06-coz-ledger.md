# Coz's book Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** End act one: the story keeps the items it still needs (#5), Coz buys his ledger back in a bubbles scene, pays and offers work, and once the doctor is also done Seren's line tears through a holo ad and lands as a mail, leaving The three open where the story stands (#4).

**Architecture:** Everything is driven from `flavor/missions.ron` through the story engine in `crates/strata-zero/src/missions/`: new needs, effects and a trigger parse from the book and run in `missions::effects`, `missions::holds` and `Trigger::fires`. The keep rule sits in `actions::perform`, so every path meets it; the pack's popups only reflect it. The ad line is a small module beside the news (`missions/ad.rs`) ticked from the explore frame. `Stay` is a sim call beside `keep_at_work`.

**Tech Stack:** Rust (workspace crates `game`, `sim`, `citygen`, `strata-zero`), ratatui, RON content, serde saves.

**Spec:** `docs/superpowers/specs/2026-11-06-coz-ledger-design.md`

## Global Constraints

- Build and test into `--target-dir target/smoke` (relative to the worktree), never `target/`.
- Check both front ends (`CLAUDE.md` "Building and checking"): fmt, clippy with and without `--features strata-zero/window`, tests with and without it.
- Saved types: `#[serde(default)]` on every new field; no `HashSet`, `Rc`, `Cell`, `Instant`, `&'static str`; a new `Missions` field gets its line in `runs::ids::clear_lost`; `runs/fixtures/` keep opening and are never regenerated.
- Copy is exact: the refusal is `You're not leaving that lying around.`; scene, objective, ad and mail text exactly as spec §2–§5.
- The ad waits for 6 tiles walked (Chebyshev) or another place, and at most 30 game minutes; Coz pays 800; standing +1.
- Commit messages record decisions, no chat quotes; the commit that finishes #5 ends with `Closes #5`, the one that finishes #4 with `Closes #4`; every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- No merge, no push.

## Review Focus

- Coz caught off his corner by day, or inside a bar: the scene plays where he stands, his bubbles on him, and he is not moved (Task 4 test).
- Esc through Coz's scene: its end still pays, takes the shard and releases him (Task 4 test, `scene::skip`).
- The ad line put up while you are inside a building or jacked in: it waits, then shows after you come out by an ad, or times out at 30 minutes and the mail lands (Task 3 tests).
- A run saved while the ad waits, reopened: the wait, its start tile and deadline survive (Task 3 test).
- Talking to Coz before the shard is known or after the sale: the plain fixer rows and his line card, no scene (Task 4 test).

---

### Task 1: The story keeps what it still needs (#5)

**Files:**
- Modify: `crates/strata-zero/src/missions/mod.rs` (add `kept`), `crates/strata-zero/src/missions/content.rs` (add `Book::items_named`)
- Modify: `crates/strata-zero/src/actions.rs` (`perform`, a `KEPT` const)
- Modify: `crates/strata-zero/src/screens/deck.rs` (`item_popup`, `copy_popup`, a `keeps` helper beside `hands_off`)
- Test: the in-file test modules of those files

**Interfaces:**
- Produces: `impl Book { pub fn items_named(&self, m: &Mission) -> BTreeSet<String> }` — every item id a mission's steps name in a trigger's need or an effect (`Give`, `Take`, `Unseal`, `Told`, and the needs `Carries`, `Read`, `Unsealed` anywhere in `Holds`, `ObjectiveFit` fits, nested in `Not`/`All`). `pub fn kept(st: &AppState, item: &game::ItemId) -> bool` in `missions/mod.rs`: the item is in `st.book.gives(&st.content)` and some mission whose run is `Waiting` or `Open` names it. `pub const KEPT: &str = "You're not leaving that lying around.";` in `actions.rs`.

- [ ] **Step 1: Write the failing tests**

`actions.rs`: `a_story_item_is_not_dropped` — story run to the package opened (`missions::tests::the_word_with(&[])` then `open_the_package`): `perform(Action::Drop { item: ledger_shard, qty: 1 })` is `Err(KEPT)`, the pack still holds 1; the same with `qty: 2`. `a_kept_copy_is_not_dropped` — if any story item can be a copy in the content, `DropCopy` of it is `Err(KEPT)`; if none can, assert with a comment that no `Book::gives` item has a copy form (so the arm is still written but untestable). `other_items_still_drop` — `Drop` of `stim_patch` is `Ok`.

`missions/mod.rs`: `dropping_the_shard_before_reading_it_is_refused` — as above, then `tick`: shard in the pack, `ledger_objectives(&st).last()` still the "Find out whose shard this is" objective. `the_package_is_kept_until_opened` — after Seren's word, `Drop` of `taped_package` is refused. `a_story_item_no_live_mission_names_drops` — mark every run `Done { at: 0 }` and the shard drops `Ok`. `items_named_reads_needs_and_effects` (in `content.rs`) — `items_named` of the shipped `ledger` mission contains `ledger_shard`, of `the_drop` contains `taped_package`.

`deck.rs`: `the_pack_greys_drop_for_a_kept_item` — `item_popup` for the kept shard: its `d` choice has `enabled == false` and the body contains `KEPT`; for `stim_patch` the `d` choice is enabled and the body lacks it.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke not_dropped other_items_still_drop dropping_the_shard package_is_kept no_live_mission items_named greys_drop`
Expected: FAIL (no `kept`, drops succeed).

- [ ] **Step 3: Implement**

`items_named` and `kept` as in Interfaces. In `perform`, after the off-the-map check: resolve the dropped item (`Drop { item, .. }`, or `DropCopy(copy)` through `st.character.copy_in_pack(copy)`) and `if missions::kept(st, &item) { return Err(KEPT.to_string()); }`. In `deck.rs`, `fn keeps(st: &AppState, item: &ItemId, choices: &mut [Choice]) -> Option<String>` disables choices whose pick is `Pick::Act(Action::Drop { .. } | Action::DropCopy(_))` and returns `Some(KEPT)` when kept; both popups push it onto the body after `hands_off`.

- [ ] **Step 4: Run them to see them pass**

Same command. Expected: PASS. Then `cargo test -p strata-zero --target-dir target/smoke` — PASS.

- [ ] **Step 5: Commit** — subject "story: what the story still needs stays in your pack", body ends `Closes #5`.

### Task 2: New pieces: `Build`, `Done`, `Pay`, `Standing`, `Stay`, `Later`

**Files:**
- Modify: `crates/sim/src/world.rs` (`keep_where`)
- Modify: `crates/strata-zero/src/missions/content.rs` (`Need`, `Effect`, `Trigger`, `texts`, `named_roles`, `needs_cast`, `validate`/`check_need`, the content-loading id test)
- Modify: `crates/strata-zero/src/missions/mod.rs` (`holds`, `Trigger::fires`)
- Modify: `crates/strata-zero/src/missions/effects.rs` (`apply`)
- Modify: `crates/strata-zero/src/track/mod.rs` (`resolve_story`)
- Modify: `flavor/missions.ron` (header comment only)

**Interfaces:**
- Produces: `Need::Build(String)`, `Need::Done(String)`, `Effect::Pay(u32)`, `Effect::Standing { role: String, by: i32 }`, `Effect::Stay(String)`, `Trigger::Later`; `pub fn keep_where(&mut self, id: PersonId)` on `sim::World`: as `keep_at_work` but the person stays at their whereabouts now (on the move: stopped at `pos_of(id)`, loitering; inside: inside), held until `release`.

- [ ] **Step 1: Write the failing tests**

`sim/src/world.rs`: `keep_where_holds_a_walker_where_they_stand` — a person travelling at noon: `keep_where`, advance an hour, `pos_of` unchanged and in `held`; `release`, advance, the schedule moves them again. `keep_where_keeps_someone_inside_inside`.

`content.rs`: `done_names_a_mission` — a step `Holds(Done("nope"))` fails `validate`; `Done("the_drop")` passes. `every_build_need_names_a_build` — in the content-loading test that checks skill and item ids, every `Build` id in the shipped book is a profession in `content.ron`; a hand-built book with `Build("chef")` is caught by the same helper. `stay_and_standing_need_their_role` — `needs_cast` of a mission with `Stay("zed")` and `Standing(role: "kay", by: 1)` holds both. `later_parses`.

`mod.rs`: `build_holds_for_the_chosen_build` (true for the chosen `character.profession`, false otherwise); `done_holds_once_the_mission_is_done`; `later_never_fires` (a mission's second step on `Later` stays waiting through `tick` and a day of advancing).

`effects.rs`: `pay_adds_credits` (+800); `standing_raises_the_fixers_standing` (`st.jobs.standing(coz)` +1; a role nobody plays changes nothing); `stay_keeps_the_person_where_they_are` (Coz by day off his corner: `apply(Stay("coz"))` leaves `pos_of` where it was and he is held; `Release` then frees him through the existing release path).

`track/mod.rs`: `later_has_nothing_to_track`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --workspace --target-dir target/smoke keep_where done_ build_ stay_ standing_ later_ pay_adds`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`holds`: `Build(id)` compares `st.character.profession` with `id`; `Done(m)` is the named run's state `Done { .. }`. `fires`: `Later => false`. `apply`: `Pay(n)` saturating-adds credits; `Standing` → `person_of` then `st.jobs.fixer_mut(id).standing += by`; `Stay(role)` → `person_of` then `world.keep_where(id)`. `needs_cast` and `named_roles` take `Stay` and `Standing` as `Hold`; `texts` none. `validate` checks `Done` names a mission. `resolve_story`: `Later` → `None`. Document all six in the `missions.ron` header in its style.

- [ ] **Step 4: Run them to see them pass** — same command; then `cargo test --workspace --target-dir target/smoke`. PASS.

- [ ] **Step 5: Commit** — "missions: needs by build and by done mission, pay, standing, stay, and a step for later".

### Task 3: The ad line (`Effect::Ad`)

**Files:**
- Create: `crates/strata-zero/src/missions/ad.rs`
- Modify: `crates/strata-zero/src/missions/mod.rs` (`pub mod ad;`, `Missions.ad`, `Missions.ad_cut`)
- Modify: `crates/strata-zero/src/missions/content.rs` (`Effect::Ad`, `texts`, `named_roles`, `presents`)
- Modify: `crates/strata-zero/src/missions/effects.rs` (`apply`; factor the holo label into `fn holo_label(st: &AppState, from: &str) -> String`)
- Modify: `crates/strata-zero/src/missions/news.rs` (one `pub(super) fn ad_in_view(st: &AppState) -> Option<Pos>` holding the cut's own preconditions — on the street, asides free to show — and `ad_in_sight`; the news cut calls it too)
- Modify: `crates/strata-zero/src/runs/ids.rs` (`clear_lost` drops an `ad` whose step is lost; names `ad_cut` as never saved)
- Modify: `crates/strata-zero/src/screens/explore.rs` (tick beside the news tick; the magenta glow takes `ad::cut_in` too)
- Modify: `flavor/missions.ron` header comment; `docs/saving.md` (the new saved field)

**Interfaces:**
- Consumes: `asides::say`, `asides::lasts`, `missions::end`, `st.net.now(st)` (game minute), `st.clock_tick()`.
- Produces: `Effect::Ad { from: String, text: String }`. In `ad.rs`: `#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)] pub struct AdLine { pub from: String, pub text: String, pub story: StepRef, pub since: u64, pub at: (Place, Pos) }` (text filled; `since` the game minute; `at` where you stood); `pub const WALK_OFF: i32 = 6;`, `pub const WAIT_MINUTES: u64 = 30;`; `pub fn put_up(st: &mut AppState, from: &str, text: &str, story: StepRef)`; `pub fn tick(st: &mut AppState, walking: bool)`; `pub fn cut_in(st: &AppState) -> Option<Pos>`. `Missions.ad: Option<ad::AdLine>` (`#[serde(default)]`); `Missions.ad_cut: Option<(Pos, u64)>` (`#[serde(skip)]`).

- [ ] **Step 1: Write the failing tests**

In `ad.rs` (seed 8 street; an ad found as `news::tests::by_an_ad` finds one, made `pub(crate)` if needed):
- `the_line_waits_until_you_walk_off` — `put_up` standing 3 tiles from an ad in sight: `tick(st, true)` shows nothing; stand 6+ tiles from the start tile with an ad in sight: one aside whose label ends `▚▞▚ S`, text `He's gone quiet. Work for Coz. I'll find you. —S` (with the slot filled), colour Seren's tint; `cut_in` is that ad; `st.missions.ad` is `None`; the step is in its run's `ended`.
- `another_place_counts_as_walking_off` — put up on a floor, then on the street by an ad: it shows.
- `the_line_ends_unseen_after_thirty_minutes` — put up indoors; at 29 game minutes still waiting; at 30 cleared, no aside, step ended.
- `a_waiting_line_survives_a_reopen` — save and reopen through `runs::testutil::Dir`: `ad` equal to before.
- `a_lost_step_drops_the_line` — `clear_lost` with the line's step lost leaves `ad` `None`.

In `content.rs`: `ad_presents_and_names_its_speaker` — `Ended` may name a step holding only an `Ad`; an `Ad` whose `from` is not in the cast fails `validate` as a `Holo` would.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke the_line_ another_place a_waiting_line a_lost_step ad_presents`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`apply(Effect::Ad)` calls `ad::put_up` with the step's `StepRef`, as `News` passes `here`. `put_up` fills the text, records `since` and where you stand; a line already waiting has its step ended first. `tick`: none → return; `now >= since + WAIT_MINUTES` → clear, `end` its step; otherwise, when walking, walked off (`place != at.0` or Chebyshev ≥ `WALK_OFF` from `at.1`) and `news::ad_in_view` finds an ad → `asides::say(Speaker::Tile(street, ad), holo_label(st, from), visitors::colour(&st.book, from), text)`, `ad_cut = Some((ad, clock + asides::lasts(&text)))`, clear, `end` the step. `cut_in`: `ad_cut` while the clock is before its end. Explore: `crate::missions::ad::tick(st, self.mode == Mode::Walk)` right after the news tick; the glow uses `news::cut_in(st).or_else(|| crate::missions::ad::cut_in(st))`.

- [ ] **Step 4: Run them to see them pass** — same command; then `cargo test -p strata-zero --target-dir target/smoke runs::` and `missions::news`. PASS.

- [ ] **Step 5: Commit** — "missions: a line torn through the nearest holo ad".

### Task 4: Coz's scene, the pay, and The three (content, #4)

**Files:**
- Modify: `flavor/missions.ron` (ledger `know`/`sell`/`paid`, scene `coz`, mission `the_three`, the comment above the ledger)
- Modify: `crates/strata-zero/src/missions/mod.rs` tests (the placeholder assertion, `bring_it_to_coz`, new chain tests)

**Interfaces:**
- Consumes: Task 2's `Build`, `Done`, `Pay`, `Standing`, `Stay`, `Later`; Task 3's `Ad`; existing `Release`, `Take`, `Scene`, `Log`, `Mail`, `Objective`.

- [ ] **Step 1: Write the failing tests**

- The existing test asserting the log line `Coz · "The ledger? Not here. I'll find you."` now asserts that `Talked(coz)` starts scene `coz` and leaves the ledger `Open`.
- `bring_it_to_coz` returns `Bring it to Coz · {place} · {district} · {hours}` (`cast::hours_of`).
- `selling_the_ledger_ends_the_act` — netrunner, the doctor done first (through her scene and chair as the existing doctor tests do), shard read at the rig, `Talked(coz)`: scene `coz` runs, Coz held where he stood; `scene::skip`, `tick`: shard gone, credits +800, `st.jobs.standing(coz)` +1, Coz released, log has `Coz · ¤800 · for bringing it home`, ledger `Done`, `the_three` `Open` with objectives `["Ask Castellan where they are · Seren will find you"]`, `st.missions.ad` waiting with the spec's text. Walk off to an ad and `ad::tick`: Seren's mail `quiet` lands with the spec's three body lines filled; `the_three` waits on step `later`.
- `sold_before_the_doctor_the_act_waits_for_her` — sell first: ledger `Done`, `the_three` `Waiting`, no ad; then through Odile's chair: `the_three` `Open`, ad waiting.
- `coz_hears_who_you_are` — the lines said through the scene for a netrunner (the netrunner line, not the fixer line), a `fixer` build without hacking (the fixer line only), a `solo` without hacking (neither).
- `coz_stays_where_you_caught_him` — by day, Coz off his corner: `Talked(coz)` after the shard is known; through the scene he is not moved; his first bubble anchors on his drawn tile.
- `coz_is_only_a_fixer_off_the_story` — `Talked(coz)` before the shard is known starts no scene; after the sale starts no scene.
- `a_run_saved_at_the_three_reopens_there` — save after the mail (`runs::testutil::Dir`), reopen: `the_three` open on `later`, its objective kept.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke selling_the_ledger sold_before coz_hears coz_stays coz_is_only a_run_saved_at_the_three bring_it_to_coz`
Expected: FAIL.

- [ ] **Step 3: Write the content** — per spec §2–§5: `know`'s objective ends `· {coz.hours}`; `sell` = `Stay("coz")`, `Scene("coz")`; `paid` on `Ended("sell")` = `Release("coz")`, `Take(item: "ledger_shard", count: 1)`, `Pay(800)`, `Standing(role: "coz", by: 1)`, ``Log("`{coz}` · ¤800 · for bringing it home")``; scene `"coz"` (Bubbles, title "The ledger", `Pace(type: 45, walk: 0.3, hold: Some(2.5))`) with spec §4's beats verbatim; mission `the_three` (title "The three", from `coz`) after `ledger`: `ask` on `Holds(All([Done("ledger"), Done("doctor")]))`, `quiet` on `Ended("ask")`, `later` on `Later`, effects as spec §5.

- [ ] **Step 4: Run them to see them pass** — same command; then `cargo test -p strata-zero --target-dir target/smoke missions::` and `runs::`. PASS.

- [ ] **Step 5: Commit** — subject "story: Coz buys his book back and the act ends on Seren's word", body ends `Closes #4`.

### Task 5: Shots, the mock, docs, and both front ends

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs` (ignored `coz_shots`)
- Add: `docs/superpowers/specs/2026-11-06-coz-ledger-design.md`, `docs/superpowers/specs/2026-11-06-coz-ledger-mock/gen.py`, `.../coz_mock.diff`, this plan (`index.html` is generated, not committed)

- [ ] **Step 1: Write `coz_shots`** — as `doctor_shots`, at 120×40 and 100×30, a netrunner with the doctor done, 21:40: the corner with the objective and Coz tracked; the scene at its first beat and its last; after the sale, walked off by an ad, the ad line up through the real `ad::tick`; the JOBS tab on The three; the shard's verbs in GEAR & INVENTORY with Drop greyed (before the sale). Nothing faked.

- [ ] **Step 2: Render and look** — `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored coz_shots`; PASS, and the PNGs match the mock's frames.

- [ ] **Step 3: Every check** — all pass:
`cargo fmt --all --check`
`cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`
`cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings`
`cargo test --workspace --target-dir target/smoke`
`cargo test --workspace --features strata-zero/window --target-dir target/smoke`
`cargo check -p strata-zero --features window --target x86_64-unknown-linux-gnu --target-dir target/smoke`

- [ ] **Step 4: Commit** — "docs: Coz's book, its spec, plan and mock; shots".
