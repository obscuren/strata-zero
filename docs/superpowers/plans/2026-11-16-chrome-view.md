# Chrome on GEAR & INVENTORY (#103) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** GEAR & INVENTORY shows the body as MEAT or as CHROME (`c` switches); CHROME is a tile per site and per socket, with everything the ATTRIBUTES ledger did, and ATTRIBUTES keeps only the attributes.

**Architecture:** The kit's body layout (`screens/kit/body.rs`) becomes generic over its tile key, so MEAT keys it by `BodyPart` and CHROME by a new `Stop` (a site, or a socket of what's in it). The ledger's rules, card, picker and LOAD move from `screens/sheet/ledger.rs` to a new `screens/kit/chrome.rs`; the sheet loses its ledger pane.

**Tech Stack:** Rust, ratatui.

**Spec:** `docs/superpowers/specs/2026-11-16-chrome-view-design.md` (mock: `docs/superpowers/specs/2026-11-16-chrome-view-mock/gen.py`, round 2 for the body, round 3 for the right pane)

## Global Constraints

- The five checks from CLAUDE.md pass before each commit, always `--target-dir target/smoke`.
- No change to fitting, fee, load or bay rules; the ledger's behaviour moves unchanged.
- No saved state changes: the kit's and sheet's state are UI (`deck_sheet`, kit state bound `_` in `runs::snapshot`).
- The full CHROME table (offsets from the graph's origin, every tile 11×5): EYES (12,0), NEURAL (24,0), EARS (36,0), JAW (48,0); L ARM's HAND (0,6), L ARM (12,6), ORGANS (24,6), R ARM (36,6), R ARM's HAND (48,6); L ARM's FOREARM (12,12), SPINE (24,12), R ARM's FOREARM (36,12); BACK's BAY (0,18), BACK (12,18), LEGS (24,18), SKIN (36,18). Joins: EYES→NEURAL, EARS→NEURAL, JAW→EARS, ORGANS→NEURAL, L ARM→ORGANS, R ARM→ORGANS, SPINE→ORGANS, BACK→SPINE, LEGS→SPINE, SKIN→SPINE, each HAND and FOREARM→its arm, BAY→BACK. Vertical joints at each tile's centre column (5).
- Tile labels: the site's name as `Site` prints it in the ledger (`NEURAL`, `L ARM`, ...); a socket's tile is its `SocketKind::label()` in capitals (`HAND`, `FOREARM`, `BAY`).
- Header row: on MEAT `▌MEAT▐ CHROME  c switch`; on CHROME `MEAT ▌CHROME▐` + the ledger's mark (`· CHROMING UP` amber in a doc's chair, ` FREE ` badge with dev free chrome, nothing otherwise) + `  c switch`.
- CHROME's right pane: row 0 `LOAD ` + gauge across the pane + ` 19/24` (the ledger's colours: amber for what a pick adds, magenta over budget); row 1 dim `<n> left · <k> pieces in you` (`over by <n>` when over budget; `k` = `Character::pieces`); row 2 the divider `╠═══…` joined to the vertical one; under it the card, or FITS. The vertical divider and its `╠` are amber in a doc's chair or with free chrome, as the ledger's rule is.
- Hint strings move from the ledger with `← attributes` replaced by `c meat/chrome` and `↑↓ select` by `←↑↓→ select`; the picker's own `↑↓ choose` stays.
- Copy rules: digits for counts; no new chair line.

## Review Focus

1. The cursor on a socket tile whose piece goes (pull the Piston arm with the cursor on its FOREARM, or the arm swapped for a plain one): the cursor lands on the arm's site, no panic, no stale picker. Test in Task 3.
2. `c` pressed while FITS is open or an `x` pull is pending: the picker closes, the pending pull is forgotten, the view switches. Test in Task 3.
3. Every site and socket is on screen and reachable by the arrows at 100×30 and at 120×40, with the fullest loadout (both chrome arms with hand and forearm fitted, a cradle with a drone). Test in Task 2.
4. The chair opened while the deck was left on MEAT, on the pack, or on ATTRIBUTES: GEAR & INVENTORY, CHROME, NEURAL. Test in Task 3.
5. MEAT draws and walks exactly as before the generic layout (the existing kit tests pass untouched). Covered by Task 1's run.

---

### Task 1: The body layout keyed by any tile

**Files:**
- Modify: `crates/strata-zero/src/screens/kit/body.rs`, `crates/strata-zero/src/screens/kit/mod.rs` (callers only)

**Interfaces:**
- Produces: `Place<K>`, `BodyLayout<K>` (fields as now, `tiles: Vec<(K, Rect)>`), `fn place<K: Copy + Eq>(area: Rect, full: &[Place<K>], compact: &[Place<K>], keys: &[K], full_w: u16) -> BodyLayout<K>` (today's `layout` body), `step<K>`, `joints<K>`; `layout(area, parts)` keeps its signature for MEAT and calls `place` with today's tables. `const fn at` and `Place::to` become generic.

- [ ] **Step 1:** Make the types and functions generic as above; no behaviour change.
- [ ] **Step 2:** Run `cargo test -p strata-zero --target-dir target/smoke -- kit` — every existing kit test passes unchanged.
- [ ] **Step 3:** The five checks; commit `kit: the body layout keyed by any tile`.

---

### Task 2: CHROME's stops and their layout

**Files:**
- Create: `crates/strata-zero/src/screens/kit/chrome.rs` (module declared in `kit/mod.rs`)

**Interfaces:**
- Consumes: Task 1's `Place<K>`, `place`, `step`, `BodyLayout<K>`.
- Produces: `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Stop { Site(Site), Socket(Site, usize) }` with `Default` = `Site(Site::Neural)`; `pub fn stops(st: &AppState) -> Vec<Stop>` (each site in `Site::ALL` order, then a `Socket(site, n)` for each socket of the piece installed there); `pub(crate) fn chrome_layout(area: Rect, st: &AppState) -> BodyLayout<Stop>`; `pub(crate) fn socket_kind(st: &AppState, site: Site, n: usize) -> Option<SocketKind>` (moved from the ledger, which then calls this one).

- [ ] **Step 1: Write the failing tests** (in `chrome.rs`)

```rust
#[test] fn the_full_chrome_body_sits_where_the_mock_puts_it() {
    // tab 118×27 (120×40's body), the attributes-shot loadout: each site's tile at the table's offset from the
    // graph's origin, 11×5; L ARM's Hand at (0,6) and Forearm at (12,12), BACK's Bay at (0,18).
}
#[test] fn a_socket_has_a_tile_only_while_its_piece_is_in() {
    // fresh debug runner: no Socket stops, no socket tiles; Piston arm at L ARM: Socket(LeftArm,0) and (LeftArm,1);
    // drone cradle at BACK: Socket(Back,0).
}
#[test] fn the_trunk_branches_under_the_spine() {
    // the glyphs from `joints`: '┬' on SPINE's bottom border at its joint, '┼' under it, '┴' on LEGS' top.
}
#[test] fn every_stop_shows_and_walks_at_both_sizes() {
    // both chrome arms (Piston arm L, Grip arm R) with a hand and a forearm piece each, a cradle with a drone:
    // at 98×22 (100×30's body) and 118×27, every stop has a tile inside the tab, no two tiles overlap, and from
    // NEURAL the arrows reach every stop (breadth-first over `step`).
}
```

- [ ] **Step 2:** Run `cargo test -p strata-zero --target-dir target/smoke -- chrome::` — they fail.
- [ ] **Step 3:** Implement: the full table from Global Constraints; a socket's place by its kind and host (Hand left of L ARM / right of R ARM, Forearm under its arm, Bay left of BACK). The compact table (below 113×27, in the left half) is yours: every stop stays a tile inside the tab (tiles may be 4 tall), drawn as at full size otherwise; keep the joins.
- [ ] **Step 4:** Tests pass; the five checks; commit `kit: CHROME's stops and their places`.

---

### Task 3: The CHROME view, with the ledger's rules

**Files:**
- Modify: `crates/strata-zero/src/screens/kit/chrome.rs`, `kit/mod.rs`, `kit/tests.rs`, `crates/strata-zero/src/screens/deck.rs` (`Deck::chair`, the hint row for the kit)
- Modify: `crates/strata-zero/src/screens/sheet/ledger.rs` (what moves leaves; the rest of the ledger goes in Task 4)
- Modify: `manual/05-the-deck.md` (GEAR & INVENTORY: the CHROME view), `manual/10-chrome-and-drones.md` (the chair opens GEAR & INVENTORY on CHROME)

**Interfaces:**
- Consumes: Task 2's `Stop`, `stops`, `chrome_layout`.
- Produces: `pub enum View { Meat, Chrome }` (default `Meat`) in `KitState`, with `stop: Stop` and `picker: Option<usize>`; `Kit` keeps `pulling: Option<Stop>` and the card's tick. The ledger's `fitting`, `Pick`, `install`, `pull`, `act`, `chair_free`, `pick_fee`, `load_after`, `piece_card`, `fit_card`, `chrome_mark` move here (renamed as you see fit); `Deck::chair` sets `Tab::Kit`, `View::Chrome`, `Stop::Site(Site::Neural)`.

- [ ] **Step 1: Write the failing tests** — move every ledger test in `sheet/ledger.rs` (from `the_mark_is_on_the_header_line` through `pull_and_reinstall_keeps_pieces`) to `kit/` with its asserts, driving GEAR on CHROME instead of the ledger: a `LedgerRow::Site(s)` becomes `Stop::Site(s)`, a socket row `Stop::Socket(s, n)`; hint strings change only as Global Constraints says; positions change to where CHROME draws them. Add:

```rust
#[test] fn c_switches_meat_and_chrome_from_anywhere_on_the_tab() {
    // on a MEAT tile, on the pack: `c` → View::Chrome on the remembered stop; `c` → View::Meat, the pane back on Body.
}
#[test] fn c_closes_fits_and_forgets_a_pending_pull() { /* Review Focus 2 */ }
#[test] fn a_socket_stop_whose_piece_goes_falls_back_to_its_site() {
    // cursor on Socket(LeftArm,1), the Piston arm pulled (free chrome): the stop is Site(LeftArm), picker None.
}
#[test] fn the_chair_opens_gear_on_chrome() {
    // deck left on MEAT's pack and, separately, on ATTRIBUTES: Deck::chair → Tab::Kit, View::Chrome, Site(Neural).
}
#[test] fn load_sits_over_the_divider() {
    // 120×40 on CHROME: right pane row 0 starts "LOAD" and ends "19/24"; row 1 "5 left · 8 pieces in you";
    // row 2 starts '╠' at the vertical divider; with a pick open, the added segments are amber.
}
#[test] fn the_tiles_say_what_each_piece_gives() {
    // L ARM "Piston" / "arm" / "BODY +1 ■□"; BACK's line "1 bay ■"; the Bay "docked"; an empty site '·'.
}
```

- [ ] **Step 2:** Run them — they fail.
- [ ] **Step 3: Implement.** The tiles as MEAT draws them (name on two lines; the gives line: first perk short via `perk_short`, else the first granted ability's name; an arm adds its sockets' pips `■`/`□` in the pips colour; a cradle `<n> bay` + pips; a docked drone `docked`; empty `·`); the header row and the right pane as Global Constraints; `c` on both views; on CHROME the arrows walk `step`, `space`/`x`/the picker as the ledger's keys. The card types out as the ledger's does. Manual 05 gets the CHROME view (`c`, a tile per site and per socket, LOAD, the card, changes only in a doc's chair, the bay anywhere); manual 10's "The deck opens on ATTRIBUTES, on the list of your body's sites" becomes GEAR & INVENTORY on CHROME, "the ledger" the CHROME view.
- [ ] **Step 4:** Tests pass; the five checks; commit `kit: CHROME on GEAR & INVENTORY, the chair opens it`.

---

### Task 4: ATTRIBUTES without the ledger

**Files:**
- Modify: `crates/strata-zero/src/screens/sheet/mod.rs`, delete what's left of `sheet/ledger.rs`, `crates/strata-zero/src/screens/deck.rs` (anything naming `SheetPane::Ledger` or `LedgerRow`)
- Modify: `manual/05-the-deck.md` (ATTRIBUTES), `manual/02-your-runner.md` (its tab table, if it puts chrome on ATTRIBUTES)

**Interfaces:**
- Consumes: Task 3 (nothing of the ledger is still used).
- Produces: `SheetState { tile }` only (no `pane`, `row`, `picker`); `SheetPane` and `LedgerRow` gone.

- [ ] **Step 1: Write the failing test** (sheet/mod.rs)

```rust
#[test] fn the_readout_fills_the_right_half() {
    // 120×40: the tiles left of the rule at RULE_X as now; the readout's first row ("BODY 4 · 8 in all" on BODY)
    // starts two columns past the rule, on the pane's first row; nothing reads "CHROME" on the tab; `→` does nothing.
}
```

- [ ] **Step 2:** Run it — it fails.
- [ ] **Step 3:** Implement: the readout draws in the right half (from `RULE_X + 2` to the tab's right edge, full height); the left keeps the three blocks of tiles. Hints lose `→`/chrome. Update the sheet's tests that read the readout under the tiles to read it in the right half. Manual 05's ATTRIBUTES section loses "`→` past the last tile crosses to the chrome...".
- [ ] **Step 4:** Tests pass; the five checks; commit `sheet: ATTRIBUTES keeps the attributes; the readout on the right`.

---

### Task 5: Shots

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs`

- [ ] **Step 1:** `attributes_sheet` keeps its left-pane shots (`attributes_left_emp`, `attributes_left_reflex`) and gets `attributes_sheet` at both sizes; its ledger and picker shots move to a new `gear_chrome` shot test with the same loadout and names `gear_chrome_limb`, `gear_chrome_cradle`, `gear_chrome_bay`, `gear_chrome_picker_bay`, `gear_chrome_picker_site`, `gear_chrome_picker_socket`, `gear_chrome_picker_swap` at 100×30 and 120×40; add `gear_chrome_meat_arms` (no arms, no cradle) and `gear_chrome_in_the_chair` (seated, the picker on the empty forearm).
- [ ] **Step 2:** Run `SHOT_DIR=<dir> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored gear_chrome attributes_sheet gear_and_inventory`; look at the PNGs against round 2 and 3 of the mock; fix what doesn't match.
- [ ] **Step 3:** The five checks; commit `shots: CHROME on GEAR, ATTRIBUTES alone (Closes #103)`.
