# TALENTS board Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The deck's TALENTS tab shows one tree at a time as a circuit board of pixel-icon chips, over a new four-column grid of sixty talents.

**Architecture:** `game` learns the grid (`Tree.tiers` of optional slots), the trace rules and the talents' art; `flavor/` carries the sixty talents and their exported icons; `strata-zero`'s `ui/talents.rs` keeps the cursor state and draws the board, and the deck hands it keys and its body.

**Tech Stack:** Rust (ratatui, crossterm, serde/ron), Python for the art export.

**Spec:** `docs/superpowers/specs/2026-10-31-talent-board-design.md`

## Global Constraints

- `TREE_COLS` = 4 slots a tier, `TREE_TIERS` = 5 tiers at most; `tier_gate` stays 3.
- Icons are 6×6 pixels (`TALENT_ART_W` = `TALENT_ART_H` = 6), exported by `docs/superpowers/specs/2026-10-31-talent-board-art/talentart.py export flavor/talent_art.ron`; never hand-edit the `.ron`.
- Board offsets from the body's top-left: tree slots columns 0-12, board 14-64, rail 15, chip `c` at column `21 + 11c`, tier `t` at row `2 + 4t`, readout from column 67.
- Colours: tree = `attribute_colour`; board ground = tree at 3.5% over `PANEL_BG`; dots tree at 14%; plate tree at 16%; dark trace/pin `#3a4250`; maxed LED amber `(255, 200, 80)`; learnable next LED green `(80, 230, 120)`; NEED ✓ green, ✗ red `(255, 77, 106)`.
- Hint line: `←→↑↓ move · [ ] trees · space burn a point · Q/E tabs · tab close`.
- Build into `--target-dir target/smoke`; check both front ends (CLAUDE.md's five commands).

## Review Focus

- A character from an old save holding ranks in a talent whose tier or prerequisite moved: it loads, its ranks count, nothing panics (game test: ranks in `bullet_time` with `long_shot` at 0 still count in `tree_points` and `can_learn` reports normally).
- A tree whose cursor spot points at an empty slot or a tier that no longer exists (old `TalentsState`, or content change): the cursor lands on the nearest talent instead of nothing (app test in Task 4).
- Zero talent points (the usual state): open chips stay in colour, no LED is green, the readout says `✗ no points left` and has no `burn a point`, space reports `no points left` as a warning (app test in Task 5).
- The deck at exactly 100×30: the whole board, five tiers, the rail's `12`, and the readout's `NEED` lines all draw (app test in Task 5).
- A capstone learned: its LED amber, `at its best` in the readout, the trace into it lit (app test in Task 5).

---

### Task 1: The grid and its trace rules in `game`

**Files:**
- Modify: `crates/game/src/model.rs` (Talent, Tree), `crates/game/src/content.rs` (validation, `tree_of`, tests), `crates/game/src/character.rs` (`tree_points`, tests), `crates/game/src/action.rs` (test at ~1140)
- Modify: `flavor/content.ron` (trees as grids; talents: `icon` removed, moved prerequisites, fifteen new talents, exactly as the spec's tables)

**Interfaces:**
- Produces: `pub const TREE_COLS: usize = 4; pub const TREE_TIERS: usize = 5;` in `model.rs`; `Tree.tiers: Vec<Vec<Option<TalentId>>>`; `impl Tree { pub fn slot(&self, tier: usize, col: usize) -> Option<&TalentId>; pub fn talents(&self) -> impl Iterator<Item = (usize, usize, &TalentId)> }` (tier-major, left to right); `pub fn trace_slots(from: (usize, usize), to: (usize, usize)) -> Vec<(usize, usize)>` in `model.rs` (the `(tier, col)` slots a trace from a prerequisite at `from` to a talent at `to` passes through, per the spec); `Content::tree_of` keeps its signature `(&Tree, tier, col)`. `Talent` loses `icon`.

- [ ] **Step 1: Write the failing tests** in `content.rs` tests:
  - `the_trees_are_grids`: five trees; every tier has `TREE_COLS` slots; `≤ TREE_TIERS` tiers; 60 talents, each in exactly one slot; `tree_of("counter_cut")` = EDGE, tier 2, col 2; `tree_of("the_half_second")`... use `half_second` = tier 4, col 1; `c.talent("light_step").requires` is `None`, `backstab.requires == Some("light_step")`.
  - `trace_slots_go_across_then_down`: `trace_slots((1,3),(2,2)) == [(1,2)]`; `trace_slots((2,1),(4,1)) == [(3,1)]`; `trace_slots((0,0),(1,0)) == []`; `trace_slots((0,3),(2,1)) == [(0,2),(0,1),(1,1)]`.
  - `the_grid_refuses_bad_trees` (each on a fresh `Content::load()` clone, `validate()` errs and the message names the problem): a tier of three slots; a sixth tier (`vec![None; 4]` plus a talent moved into it); an empty tier; a prerequisite on the same tier; a prerequisite in another tree; a trace down through a chip (Half second requiring Steady hand with Long shot between); a trace sideways through a chip (Razor wire at (3,2) requiring Fan fire's neighbour across an occupied slot); two traces through one slot; a two-rank talent in the last tier.
  - Replace `the_trees_load_and_validate`'s 2-a-tier assertions and the old tests that index `tiers[i][j]` with `slot`.
- [ ] **Step 2: Run** `cargo test -p game --target-dir target/smoke` — expected: compile errors / FAIL.
- [ ] **Step 3: Implement** the types and helpers; validation in `Content::validate` (tree section: slot count, tier count, empty tier, unknown talent; talent section: one slot, prerequisite same tree and lower tier, every `trace_slots` slot empty, no slot in two traces, last-tier talents one rank); `tree_of` and `tree_points` over `Tree::talents`. Rewrite `flavor/content.ron`'s `trees` as grids (`[Some("quick_draw"), Some("steady_hand"), None, Some("light_step")]`) and its talents per the spec (drop every `icon:`; new talents' boons, skills and descriptions from the spec's table).
- [ ] **Step 4: Run** `cargo test -p game --target-dir target/smoke` — expected: PASS.
- [ ] **Step 5: Commit** `game: talent trees are four-column grids; prerequisites may come from the side`.

### Task 2: The talents' icons

**Files:**
- Create: `flavor/talent_art.ron` (exported), `docs/superpowers/specs/2026-10-31-talent-board-art/talentart.py` (already drawn)
- Modify: `crates/game/src/model.rs`, `crates/game/src/content.rs`, `crates/game/src/lib.rs` (re-exports if needed)

**Interfaces:**
- Produces: `pub const TALENT_ART_W: usize = 6; pub const TALENT_ART_H: usize = 6; pub struct TalentArt { pub rows: Vec<String>, pub colours: BTreeMap<char, (u8, u8, u8)> }`; `Content.talent_art: BTreeMap<TalentId, TalentArt>` (`#[serde(skip)]`, loaded in `Content::parse` from `include_str!("../../../flavor/talent_art.ron")`); `Content::talent_art(&self, id: &TalentId) -> Option<&TalentArt>`.

- [ ] **Step 1: Write the failing test** `every_talent_has_its_icon`: every talent has art, 6 rows of 6 chars, every non-`.` char has a colour; validation errs ("art") when a talent's art is removed, when a row is 5 wide, and when art names `levitation`.
- [ ] **Step 2: Run** `cargo test -p game every_talent_has_its_icon --target-dir target/smoke` — FAIL.
- [ ] **Step 3: Export** `python3 -I docs/superpowers/specs/2026-10-31-talent-board-art/talentart.py export flavor/talent_art.ron`; implement the type, loader and validation (as `implant_art_ok`).
- [ ] **Step 4: Run** `cargo test -p game --target-dir target/smoke` — PASS.
- [ ] **Step 5: Commit** `game: every talent has a 6x6 icon from talent_art.ron`.

### Task 3: Sharpening reaches strikes and marks

**Files:** Modify `crates/game/src/character.rs` (`Character::ability`, tests).

- [ ] **Step 1: Failing test** `a_sharpened_strike_reaches_further`: with `arc_lash` at 1, `ability("jolt")` is a `Strike` with `range` 4 (content's 3 + 1); with `static_bloom` at 2, `ability("static")` range 10 and cooldown 180 (240 − 60); a `Mark` ability sharpened by a test-built talent reaches its range plus the radius; a radius that would go below 1 stops at 1.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement**: in `Character::ability`, `Active::Strike { range, .. } | Active::Mark { range, .. }` get `(*range + dr).max(1)` like the scan radii.
- [ ] **Step 4: Run** `cargo test -p game --target-dir target/smoke` — PASS.
- [ ] **Step 5: Commit** `game: sharpening adds to a strike's and a mark's range`.

### Task 4: The cursor: `TalentsState`, keys and memory

**Files:** Modify `crates/strata-zero/src/ui/talents.rs`, `crates/strata-zero/src/screens/deck.rs`, `crates/strata-zero/src/app.rs`.

**Interfaces:**
- Consumes: `Tree::slot`, `Tree::talents`.
- Produces: in `ui/talents.rs`: `#[derive(Clone, Debug, Default, PartialEq, Eq)] pub struct TalentsState { pub tree: usize, pub spots: Vec<(usize, usize)> }` with `pub fn cursor(&self, c: &Content) -> TreeCursor` (the tree's spot snapped to the nearest talent; a missing spot is the tree's first talent), `pub fn step(&mut self, c: &Content, dx: i32, dy: i32)`, `pub fn switch(&mut self, c: &Content, d: i32)`; `TreeCursor { tree, tier, col }` stays (drop `pack`/`unpack`); `talent_at(c, cur) -> Option<&TalentId>` via `slot`. `AppState.deck_talents: TalentsState` (reset with the rest of the deck state). `Deck.talents: TalentsState` replaces `Deck.cursor`.

- [ ] **Step 1: Failing tests** in `talents.rs`: `along_a_tier_skips_the_gaps_and_wraps` (EDGE tier 1: from Quick draw `→` Steady hand `→` Light step `→` Quick draw; `←` from Quick draw is Light step); `up_and_down_find_the_nearest_talent` (EDGE: from Light step (0,3) `↓` Backstab (1,3); from Backstab `↓` Counter cut (2,2); from Counter cut `↓` Razor wire (3,2); `↓` The half second (4,1); `↓` wraps to Steady hand… nearest to col 1 on tier 0 is Steady hand; a tie takes the left one); `trees_switch_and_remember` (`switch(+1)` from MEAT is EDGE at its first talent; move, `switch(-1)`, `switch(+1)` returns to the moved spot; `switch(-1)` from MEAT is ICE); `a_stale_spot_snaps_to_a_talent` (spot (2,3) in EDGE, an empty slot, gives Counter cut; spot (9,9) gives the nearest on the last tier).
- [ ] **Step 2: Run** `cargo test -p strata-zero talents --target-dir target/smoke` — FAIL.
- [ ] **Step 3: Implement** the state and moves (`←→` nearest talent in that direction along the tier, wrapping; `↑↓` the next tier in that direction with a talent, wrapping, landing on the smallest `|col − cur.col|`, left on a tie). In the deck: `[` `]` call `switch`, arrows / `h j k l` call `step`, both set `changed`; space learns at `cursor(c)`; `restore` and `update` read and write `st.deck_talents`; drop the Talents entry from `deck_rows` handling.
- [ ] **Step 4: Run** the talents tests and the deck tests — PASS (the old drawing tests are rewritten in Task 5; mark them `#[ignore]` only if they block, and delete them there).
- [ ] **Step 5: Commit** `deck: the talents cursor stays in one tree; [ ] change trees and remember the spot`.

### Task 5: The board

**Files:** Modify `crates/strata-zero/src/ui/talents.rs` (`TalentsView::draw` and helpers), `crates/strata-zero/src/screens/deck.rs` (call site, hint line, tests).

**Interfaces:**
- Consumes: `TalentsState::cursor`, `Content::talent_art`, `attribute_colour`, `ui::pixels::Pixels`/`Pen` (as `ui/implant_art.rs` uses them), `ui::art::grey_at`, `reveal`.
- Produces: `TalentsView { content, character, cursor: TreeCursor, tick, changed, flash }` with `draw(&self, area: Rect, buf: &mut Buffer)`; `pub fn readout(c: &Content, ch: &Character, id: &TalentId) -> Vec<Line<'static>>` (replaces `dossier`).

- [ ] **Step 1: Failing tests** (deck tests, 100×30 and 120×40 through `draw`): `talents_board_draws_one_tree` (`TREES`, `MEAT`…`ICE` in the slots, `▶MEAT`, `1 to burn`, the board title `MEAT · the body is the first machine`, the rail's ` 0` ` 3` ` 6` ` 9` `12`, `┣━`, pins `┤`/`├`, `> thick_skin.tal`, `○○○ 0/3 · tier 1`, `THICK SKIN`, `NEXT  +3 max HP`, `[space] burn a point`, the new hint line; no other tree's talent names); `talents_board_reads_the_locked_reasons` (EDGE Bullet time with Quick draw 3, Double tap 2, Light step 2, Steady hand 1: `✓ 6 points in EDGE`, `✗ Long shot 2/2`, no `burn a point`; with zero points: `✗ no points left`); `talents_board_learns_and_lights` (space on Thick skin: rank 1, LED `●` at the chip's bottom-right in MEAT's colour, the flash magenta on its pins, `0 to burn`; Long haul, open, keeps its colour; Second lung, closed, greys); `talents_board_lights_a_capstone` (Meat machine 1/1 with its line maxed: amber `●`, `at its best`, the trace `▼` into it in MEAT's colour); `talents_board_fits_the_smallest_deck` (100×30: tier 5's chip and `12` draw, `NEED` lines draw). Delete the old talents drawing tests.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement** the board as the spec lays it out (slots, ground and dots, rail and taps, traces from `trace_slots` geometry, chips with plate/pixels/grey, pins, LEDs, cursor corners, flash, readout with `reveal`), the deck's call site and hint line.
- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke` — PASS.
- [ ] **Step 5: Commit** `deck: TALENTS is a circuit board, one tree at a time`.

### Task 6: Shots, docs and both front ends

**Files:** Modify `crates/strata-zero/src/screens/shots.rs` (new ignored `talents_board` shot), `CLAUDE.md`/`docs` only if they mention the old tab, the spec's "My calls".

- [ ] **Step 1:** Add `talents_board`: the mock's runner (EDGE: Quick draw 3, Double tap 2, Light step 2, Steady hand 1; ICE: Ice pick 2; 2 points) at 100×30 and 120×40, cursor on Hair trigger, and one on Bullet time; run it with `SHOT_DIR` and look at the PNGs.
- [ ] **Step 2:** Run CLAUDE.md's five checks (fmt, both clippies, both test runs) into `target/smoke`; all green.
- [ ] **Step 3:** Commit `shots: the talents board`.
