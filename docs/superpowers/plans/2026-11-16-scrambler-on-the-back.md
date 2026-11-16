# The scrambler on the back, and WREN — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The field scrambler is worn on the back (its cell, grant and recharge are the worn copy's), and a third starting runner, WREN, starts with one.

**Architecture:** `field_scrambler` gets `slot: Back`; the cloak's cell code (`crates/strata-zero/src/cloak/cell.rs`) reads the copy worn at a body part instead of pack copies; the "cell grants while carried" path in `Character::abilities` and the content rule go. WREN is a content template; `Character::from_build` wears the build's kit before the profession's.

**Tech Stack:** Rust (game, strata-zero), RON content.

**Spec:** `docs/superpowers/specs/2026-11-16-scrambler-on-the-back-design.md`

## Global Constraints

- The five checks from CLAUDE.md pass before each commit, always `--target-dir target/smoke`.
- Saves: never regenerate `crates/strata-zero/src/runs/fixtures/*`; they keep opening. New saved fields need `#[serde(default)]` (none expected).
- Copy: footer `◌ field scrambler  62%` as today; BACK tile gives line `◌ 62%`, `· burned` when burned out. Counts in digits.
- WREN: handle `WREN`, line `every camera on the block kept a smear, never a face`, origin `slab_rat`, profession `fixer`, attributes BODY 5, REFLEX 7, TECH 4, COOL 8, NET 3, categories `ghost`, `iron`, `street`, skills `stealth`, `lockpick`, `handguns`, `streetwise`, kit `field_scrambler`, `whisper_smg`.
- Rulings after this plan was written supersede it: WREN is REFLEX 8 (to spend exactly 20 points), and the Whisper is in the right hand and the Ratchet in the left (the build's kit is worn first), not REFLEX 7 and the Review Focus 5 hand order.
- The manual chapter that describes a changed rule changes in the same task.

## Review Focus

1. A save cloaked on a pack scrambler (the old way) opens without a panic, cloak off, the scrambler still in the pack with its charge. Test in Task 1.
2. Swapping the scrambler off the back for the Courier sling while it runs switches the cloak off and leaves the scrambler in the pack with the charge it had. Test in Task 1.
3. Two scramblers, one worn and one spare: Scramble runs on the worn one, only it recharges, sleep fills only it, the footer shows its charge. Test in Task 1.
4. VEX and KESSLER start wearing exactly what they wore before the kit order changed. Test in Task 2.
5. WREN starts with the scrambler worn on the back (not the sling), the Ratchet in the right hand and the Whisper in the left, Scramble on the ability bar. Test in Task 2.

---

### Task 1: The scrambler worn on the back

**Files:**
- Modify: `flavor/content.ron` (`field_scrambler`: `slot: Some(Back)`)
- Modify: `crates/game/src/content.rs` (the cell rule: a cell item needs a slot; drop the carried-grant exception; tests)
- Modify: `crates/game/src/character.rs` (`abilities`: no grant from carried cell items; tests such as `a_carried_scrambler_grants_scramble_with_a_full_cell` become worn)
- Modify: `crates/strata-zero/src/cloak/cell.rs`, `crates/strata-zero/src/cloak.rs` (switching on picks the worn copy; the running cloak ends when its copy isn't worn), `crates/strata-zero/src/cloak/hud.rs` (footer charge), `crates/strata-zero/src/cloak/testutil.rs` (`scrambler()` wears one)
- Modify: `crates/strata-zero/src/screens/kit/body.rs` (the BACK tile's gives line for a cell item)
- Modify: `manual/06-the-city-and-the-world.md`, `manual/07-turns-and-fights.md`

**Interfaces:**
- Produces: `cell::worn(st: &AppState, item: &ItemId) -> Option<(BodyPart, CopyId, Charge)>` (the worn copy of `item` with its part and charge; replaces `best`); `cell::charge(st, id)` and `drain` find the copy among worn copies; `recharge` and `fill_cells` touch worn cell copies only.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn only_a_worn_scrambler_grants_scramble() { /* in the pack: no "scramble" ability; worn on BACK: it's there */ }
#[test] fn scramble_runs_on_the_worn_copy_and_only_it_recharges() {
    // worn copy at 50%, spare in the pack at 20%: Scramble's Pool::Cell is the worn copy's id; after it ran and
    // stopped, recharge(st, n) raises the worn one only; fill_cells (sleep) fills the worn one only, the spare stays 20%.
}
#[test] fn taking_it_off_while_it_runs_switches_the_cloak_off() {
    // Unequip(Back) and, separately, EquipAt(courier_sling, Back) while scrambled: the next frame the cloak is off,
    // the scrambler is in the pack with the charge it had.
}
#[test] fn a_cloak_saved_on_a_pack_scrambler_opens_off() { /* st.cloak = Some(Pool::Cell(pack copy id)); a frame: off, no panic */ }
#[test] fn the_footer_and_the_back_tile_read_the_worn_charge() {
    // worn at 62%: footer_charge text "◌ field scrambler  62%"; BACK tile gives line "◌ 62%"; burned: "· burned";
    // only a spare in the pack: footer_charge empty.
}
#[test] fn a_cell_item_needs_a_slot() { /* content with field_scrambler.slot = None fails validate() */ }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test --workspace --target-dir target/smoke -- scrambler cell_item worn_charge`
Expected: failures.

- [ ] **Step 3: Implement**

As the Interfaces and the spec's "The scrambler on the back" say. The cloak's existing check that ends it when its copy left the pack becomes "when its copy isn't worn". Manual 06's field scrambler section and 07's cloak section say it's worn on the back, that only the worn one recharges, that taking it off switches the cloak off, and that the back holds it or the Courier sling.

- [ ] **Step 4: Run every check** — the five commands from CLAUDE.md; all pass, fixtures open.

- [ ] **Step 5: Commit** — `cloak: the field scrambler is worn on the back`

---

### Task 2: WREN, and the build's kit first

**Files:**
- Modify: `flavor/content.ron` (template `wren`)
- Modify: `crates/game/src/character.rs` (`from_build`: the build's kit before the profession's; tests)
- Modify: the new-game screen's tests or shots only if they count templates (grep `templates` in `crates/strata-zero/src/screens/`)
- Modify: `manual/02-your-runner.md` (WREN beside VEX and KESSLER)

**Interfaces:**
- Consumes: Task 1's worn scrambler (WREN's Scramble comes from wearing it).

- [ ] **Step 1: Write the failing tests** (character.rs)

```rust
#[test] fn wren_starts_scrambled_on_the_back_with_a_gun_in_each_hand() {
    // from the "wren" template: Back = field_scrambler (sling in the pack), RightHand = ratchet_smg,
    // LeftHand = whisper_smg, skills include stealth, lockpick, handguns, streetwise, abilities include scramble,
    // attributes BODY 5 REFLEX 7 TECH 4 COOL 8 NET 3.
}
#[test] fn vex_and_kessler_wear_what_they_wore() {
    // the full `equipment` map of each, built from its template, equals the map listed in the test
    // (record it from main before changing the order).
}
```

The existing `kit_equips_first_fit_and_packs_the_rest` pushes a jacket onto the build's kit and expects the profession's weave to keep the torso: with the build's kit first, the build's jacket is worn and the weave packed; update its expectation and its comment.

- [ ] **Step 2: Run them to see them fail** — `cargo test -p game --target-dir target/smoke -- wren vex_and_kessler kit_equips`

- [ ] **Step 3: Implement** — the template in content (values from Global Constraints); `from_build` extends `kit` with `b.kit` first, then the profession's. Manual 02: one line for WREN in the list, in the same voice as VEX's and KESSLER's.

- [ ] **Step 4: Run every check** — the five commands; all pass.

- [ ] **Step 5: Commit** — `runner: WREN, the ghost, with a scrambler on the back`
