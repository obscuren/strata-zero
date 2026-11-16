# Equipment: a body of parts — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 7 fixed equipment slots with a body of parts (weapons in hands, the off hand joining in) and draw it on GEAR & INVENTORY as joined tiles.

**Architecture:** `game::Slot` goes; `game::BodyPart` (a Copy enum, so `Host` stays Copy) names the parts and `game::PartKind` says what an item fits. The body's shape (each part's kind, name, the part it joins, the main hand) is a `body:` list in `flavor/content.ron`. Saves keep keying `equipment`/`worn` by part, and old slot names read through serde aliases. GEAR & INVENTORY's left side draws the body from a layout table that mirrors the agreed mock.

**Tech Stack:** Rust workspace (game, strata-zero), ratatui cells, RON content, MessagePack saves.

**Spec:** `docs/superpowers/specs/2026-11-16-equipment-design.md`; the look is `docs/superpowers/specs/2026-11-16-equipment-mock/gen.py` (`POS`, `tile`, `hline`, `vline`, `graph`, `frame`).

## Global Constraints

- Every check from CLAUDE.md passes before each commit: `cargo fmt --all --check`, both clippies with `-D warnings`, both test runs, always `--target-dir target/smoke`.
- Saves: never regenerate `crates/strata-zero/src/runs/fixtures/*`; they must keep opening (`the_first_saved_run_still_opens`). Enums grow only at the end; a renamed variant keeps its old name as a serde alias.
- Exact colours (`Color::Rgb`), box-drawing for lines; no new glyph outside what the window font has.
- On-screen counts in digits. Player-facing copy in plain words; no quotes from chat in docs or commits.
- The manual chapter that describes a changed key, screen or rule changes in the same task.
- Part labels as content names them: `HEAD`, `FACE`, `BACK`, `TORSO`, `L ARM`, `L HAND`, `R ARM`, `R HAND`, `WAIST`, `LEGS`, `FEET`.
- The off hand's chance: `OFF_HAND_CHANCE: u8 = 25` (percent).
- Tiles: 11 wide × 5 tall; TORSO 12 × 7. The full body shows when the tab's body area is at least 113 columns wide and 27 rows tall; the pack and card never get under 50 columns.

## Review Focus

1. An old save holding `Secondary`, `Eyes`, `Head` or `Legs` keys (the fixtures only hold Torso, Primary, Deck) must open with those items in L HAND, FACE, HEAD, LEGS. Test in Task 1.
2. Wearing a gun from the pack with both hands full must swap out the main hand's item into the pack, never drop it or lose its copy (sockets, stickers). Test in Task 1.
3. Taking the gun off the main hand while the off hand holds one: you can still shoot, with the off hand's gun, and no follow-up rolls for it. Test in Task 2.
4. The main volley downs the target: the off hand doesn't fire at the body. Test in Task 2.
5. Window sizes between the two layouts (112 vs 113 columns, 26 vs 27 rows) and very large ones (200×60) draw without panics or overlaps, and the cursor never lands on a part the layout doesn't draw. Test in Task 3.

---

### Task 1: The body in the model, content and saves

The whole rename has to compile at once, so this task touches every caller. The GEAR rack keeps its old 3-wide grid for now, drawing the body's parts in body order (Task 3 replaces it).

**Files:**
- Modify: `crates/game/src/model.rs` (remove `Slot`; add `BodyPart`, `PartKind`, `BodyNode`; `Item.slot: Option<PartKind>`)
- Modify: `crates/game/src/content.rs` (`Content.body`, validation, tests)
- Modify: `crates/game/src/character.rs`, `crates/game/src/action.rs` (`Host::Worn(BodyPart)`, equip actions), `crates/game/src/surgery.rs`, `crates/game/src/run.rs` tests
- Modify: every `strata-zero` caller the survey lists (`combat.rs` keeps today's behaviour reading `BodyPart::RightHand`; `missions/mod.rs`, `missions/news.rs` read `BodyPart::Torso`; `screens/kit.rs`, `deck.rs`, `blueprint.rs`, `cyberdeck.rs`, `stickers.rs`, `workbench.rs`, `shots.rs`, `cloak/testutil.rs`, tests)
- Modify: `flavor/content.ron` (slots, `body:`, 2 new items, kits)
- Modify: `docs/saving.md`, `manual/08-the-net.md`, `manual/11-fixers-jobs-and-the-story.md`, `manual/02-your-runner.md` if it names a slot

**Interfaces:**
- Produces (game):
  - `pub enum BodyPart { Head, Face, Back, Torso, LeftArm, LeftHand, RightArm, RightHand, Waist, Legs, Feet, ThirdArm, ThirdHand }` — `Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize`; serde aliases `Face` ← `"Eyes"`, `RightHand` ← `"Primary"`, `LeftHand` ← `"Secondary"`, `Waist` ← `"Deck"`. `ThirdArm`/`ThirdHand` are in no default body: they let content (later, chrome) add a third arm without new code.
  - `pub enum PartKind { Head, Face, Back, Torso, Arm, Hand, Waist, Legs, Feet }` with `pub fn lower(self) -> &'static str` (`"hand"`, `"waist"`, ...).
  - `pub struct BodyNode { pub part: BodyPart, pub kind: PartKind, pub name: String, #[serde(default)] pub joins: Option<BodyPart>, #[serde(default)] pub main: bool }`.
  - `Content.body: Vec<BodyNode>`; `pub fn node(&self, p: BodyPart) -> Option<&BodyNode>`.
  - `Character.equipment: BTreeMap<BodyPart, ItemId>`, `Character.worn: BTreeMap<BodyPart, ItemCopy>`.
  - `Character::wear(&mut self, part: BodyPart, item: ItemId, copy: Option<ItemCopy>) -> Option<Stack>`, `take_off(&mut self, part: BodyPart) -> Option<Stack>`, `equipped(&self, part: BodyPart) -> Option<&ItemId>`, `worn_copy(&self, part: BodyPart) -> Option<&ItemCopy>`.
  - `Character::body<'a>(&self, c: &'a Content) -> Vec<&'a BodyNode>` — the content body today (chrome adds parts later).
  - `Character::main_hand(&self, c: &Content) -> Option<BodyPart>`; `hands(&self, c: &Content) -> Vec<BodyPart>` (main first, then body order).
  - `Character::part_for(&self, c: &Content, item: &Item) -> Option<BodyPart>` — the first empty part of the item's kind (hands: main first); with none empty, the main hand for a hand item, else the first part of that kind; `None` when the body has no part of that kind.
  - `Host::Worn(BodyPart)`; `Action::Equip(ItemId)` and `EquipCopy(CopyId)` wear at `part_for`; new `Action::EquipAt(ItemId, BodyPart)` and `EquipCopyAt(CopyId, BodyPart)`; `Action::Unequip(BodyPart)`.
  - `Character::deck()` reads `BodyPart::Waist`.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing tests** (in `content.rs`, `character.rs`, `action.rs`, `runs/ids.rs` tests)

```rust
#[test] fn the_body_is_eleven_parts_with_one_main_hand() {
    let c = Content::load().unwrap();
    assert_eq!(c.body.len(), 11);
    let mains: Vec<_> = c.body.iter().filter(|n| n.main).collect();
    assert_eq!(mains.len(), 1);
    assert_eq!((mains[0].part, mains[0].kind), (BodyPart::RightHand, PartKind::Hand));
    assert_eq!(c.node(BodyPart::LeftHand).unwrap().name, "L HAND");
    assert_eq!(c.node(BodyPart::LeftHand).unwrap().joins, Some(BodyPart::LeftArm));
}
#[test] fn a_body_with_two_main_hands_or_a_join_to_nowhere_is_refused() { /* edit a loaded Content's body; validate() errs */ }
#[test] fn every_part_kind_but_the_arm_has_an_item() { /* replaces the every-Slot test */ }
#[test] fn a_weapon_spec_needs_the_hand() { /* ratchet with slot Torso → validate() errs */ }
#[test] fn old_slot_names_read_as_parts() {
    // a msgpack map {"Primary": "ratchet_smg", "Secondary": "shiv_blade", "Eyes": "mirrorshades",
    //  "Deck": "pawnshop_deck", "Head": "hardhat", "Torso": "salvage_jacket", "Legs": "cargo_pants"}
    // decodes to BTreeMap<BodyPart, ItemId> with RightHand, LeftHand, Face, Waist, Head, Torso, Legs.
}
#[test] fn a_gun_from_the_pack_takes_the_free_hand_then_swaps_the_main() {
    // ratchet in RightHand, shiv in LeftHand taken off: Equip(scrap_nine) → LeftHand.
    // both hands full: Equip(whisper_smg) → RightHand; the ratchet is in the pack with its copy (fitted sockets kept).
}
#[test] fn equip_at_wears_in_the_named_hand_and_refuses_the_wrong_part() {
    // EquipAt(scrap_nine, LeftHand) → LeftHand; EquipAt(scrap_nine, Torso) → Err containing "doesn't go on the torso".
}
```

The fixture test `the_first_saved_run_still_opens` must keep passing and now also assert `ch.equipped(BodyPart::RightHand) == Some(&ItemId::new("ratchet_smg"))` and `ch.deck()` is the pawnshop deck.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p game --target-dir target/smoke -- body old_slot equip_at a_gun_from`
Expected: compile errors (no `BodyPart`), then failures.

- [ ] **Step 3: Implement**

- Content: `slot:` values become `Hand` (was Primary, Secondary), `Face` (Eyes), `Waist` (Deck), `Head`, `Torso`, `Legs`. The file's first comment says every kind but the arm appears. Add the `body:` list in this order (it is the order perks, grants and the old rack read): HEAD (head), FACE (face, joins HEAD), BACK (back, joins TORSO), TORSO (torso, joins HEAD), R ARM (arm, joins TORSO), R HAND (hand, joins R ARM, main), L ARM (arm, joins TORSO), L HAND (hand, joins L ARM), WAIST (waist, joins TORSO), LEGS (legs, joins WAIST), FEET (feet, joins LEGS).
- New items, kind Armor:
  - `tread_boots` "Tread boots", slot Feet, weight 1.2, value 140, `perks: [Armor(1)]`, desc "Steel-shank work boots with the treads half gone. They've walked through worse than this."
  - `courier_sling` "Courier sling", slot Back, weight 0.6, value 180, `perks: [Carry(3.0)]`, desc "A messenger's sling worn across the back, patched at the strap. Room for what won't fit in your pockets."
  - Kits: the solo profession's kit gains `tread_boots`, the fixer's gains `courier_sling`.
- Validation (content.rs): body not empty; parts unique; every `joins` names a part listed earlier; exactly one `main`, of kind Hand; every item's `slot` kind exists in the body; a `weapon:` needs kind Weapon and slot Hand (error text: "only a weapon in a hand shoots").
- Everything that iterated `Slot::ALL` iterates `character.body(c)` in order. Kits in `from_build` wear at `part_for`. `debug_runner` wears the rebuilt deck at `Waist`. Messages that said a slot name say the part's name in lower case (`node.name.to_lowercase()`).
- `EquipAt`/`EquipCopyAt` refuse a part whose kind isn't the item's: "{item} doesn't go on the {part name lower}". `Equip`/`EquipCopy` with `part_for == None` refuse: "nothing on you takes {item}".
- GEAR rack (interim): tiles are `character.body(c)` in order, labels from `node.name`; `KitState.tile` indexes that list.
- `docs/saving.md`: equipment and worn are keyed by body part; old slot names read as parts through serde aliases. Manual 08 and 11: "a deck in your DECK slot" becomes "a deck on your waist"; 02 if it names a slot.

- [ ] **Step 4: Run every check**

Run the five commands from CLAUDE.md.
Expected: all pass; the fixtures open.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "body: equipment by body part, weapons in hands, old slots read as parts"
```

---

### Task 2: Hands in fights

**Files:**
- Modify: `crates/strata-zero/src/combat.rs` (`your_weapon`, `your_chance`, `fire`, tests)
- Modify: `crates/strata-zero/src/screens/deck.rs` (the card's weapon lines)
- Modify: `manual/07-turns-and-fights.md` ("Your gun")

**Interfaces:**
- Consumes: `Character::hands`, `main_hand`, `equipped` (Task 1).
- Produces: `pub const OFF_HAND_CHANCE: u8 = 25;` `pub fn your_weapon(st: &AppState) -> Result<(BodyPart, ItemId, Weapon), String>` (the hand that fires: the main hand's gun, else the first other hand's in `hands` order); `pub fn chance_with(st: &AppState, foe: usize, w: &Weapon) -> Result<u8, String>` (what `your_chance` computes, for any weapon); `your_chance` keeps its signature.

- [ ] **Step 1: Write the failing tests** (combat.rs)

```rust
#[test] fn with_no_gun_in_the_main_hand_the_other_hands_gun_fires() {
    // RightHand empty, scrap_nine in LeftHand: your_weapon → (LeftHand, scrap_nine, ..); fire() never rolls a follow-up.
}
#[test] fn the_off_hand_joins_in_about_one_shot_in_four() {
    // ratchet in RightHand, scrap_nine in LeftHand, a standing foe in range of both; seed the roll so the
    // follow-up roll is under 25: the log has "off hand · Scrap nine"; AP spent is 1 (as one shot).
    // seed so it's 25 or over: no "off hand" line.
}
#[test] fn the_off_hand_stays_quiet_out_of_its_range_or_when_the_target_is_down() {
    // scrap_nine range 8, foe at 10 within the ratchet's 12: no follow-up even on a winning roll.
    // main volley downs the foe: no follow-up.
}
#[test] fn the_ambush_is_the_main_hands_alone() {
    // cloaked, ambush fires: main rounds all land ×2; the follow-up's rounds roll with chance_with and plain damage.
}
#[test] fn the_shiv_in_the_off_hand_never_fires() { /* shiv has no weapon spec: no follow-up, REFLEX +1 still counts */ }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- combat::`
Expected: compile errors, then failures.

- [ ] **Step 3: Implement**

In `fire`: after the main volley and its log line, for each hand in `hands` except the one that fired, holding an item with a `weapon:`: stop if the target is down; roll `roll(st) < OFF_HAND_CHANCE`; skip if `reach(st, foe, w.range)` refuses; else `volley` with that gun's `chance_with` and fx, no AP, and log `off hand · {item name} {outcomes}`. The cloak is already gone after the main volley, so the follow-up gets no ambush on its own.

The card (deck.rs `item_card_of`) for a worn gun adds `{damage} damage × {rounds} rounds · range {range}`, and for a gun in a hand that isn't the main hand, `off hand · 1 in 4 to follow up`. Its "where it goes" line names the part (`weapon · right hand · main · 3.9 kg · ¤ 2150`, `· main` only for the main hand).

Manual 07: "Your gun" says the right hand's gun fires (or another hand's when the right holds none), and a gun in the other hand fires too about 1 shot in 4, free, at the same target if it reaches; the ambush is the first gun's alone.

- [ ] **Step 4: Run every check**

Run the five commands from CLAUDE.md. Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "combat: the main hand fires and the off hand joins in, 1 in 4"
```

---

### Task 3: The body on GEAR & INVENTORY

**Files:**
- Create: `crates/strata-zero/src/screens/kit/body.rs` (layout table, tile drawing, lines, the cursor's moves) — `kit.rs` becomes `kit/mod.rs` if needed
- Modify: `crates/strata-zero/src/screens/kit.rs` (layout, keys, hint), `crates/strata-zero/src/screens/deck.rs` (`slot_popup` → part popup using `EquipAt`/`EquipCopyAt`, `Unequip(part)`)
- Modify: `crates/strata-zero/src/screens/shots.rs` (`gear_and_inventory`)
- Modify: `manual/05-the-deck.md`

**Interfaces:**
- Consumes: `Character::body`, `equipped`, `worn_copy`, `main_hand`, `Content.body`, `Action::EquipAt/EquipCopyAt/Unequip` (Task 1); the card's weapon lines (Task 2).
- Produces: `KitState.part: BodyPart` (replaces `tile: usize`); `pub(crate) fn layout(area: Rect, parts: &[BodyPart]) -> BodyLayout` where `BodyLayout { full: bool, tiles: Vec<(BodyPart, Rect)>, lines: Vec<Line> }`; `pub(crate) fn step(layout: &BodyLayout, from: BodyPart, dir: Dir) -> Option<BodyPart>`.

- [ ] **Step 1: Write the failing tests** (kit/body.rs, kit.rs, shots.rs)

```rust
#[test] fn the_full_body_matches_the_mock_at_120_by_40() {
    // draw the tab at 120x40 with debug_runner: the left side's rows equal gen.py's graph text for the same
    // items (head row "┌ HEAD ───┐ ┌ FACE ───┐", the torso row with L HAND ... R HAND, "┌ BACK ─┴─┐", ...),
    // compared line by line on the tile borders and joints (┬ ┴ ├ ┤ ┼ ┌ ┐).
}
#[test] fn a_tile_says_the_name_on_two_lines_and_what_it_gives() {
    // R HAND: "Ratchet" / "SMG" / "● 4×3 ■■"; WAIST: "Rebuilt" / "pawn deck" / "■■■■■ □□";
    // TORSO: "+5 HP"; LEGS: "+4 kg"; FACE "COOL +1"; an empty FEET shows "·"; arms show "·".
}
#[test] fn below_113_columns_or_27_rows_the_body_goes_compact() {
    // layout at 112x27 and 113x26: full == false, no LeftArm/RightArm tiles, every tile 4 tall;
    // at 113x27 full == true; at 100x30 the compact body fits 48 columns by 22 rows.
}
#[test] fn every_size_draws_without_overlap() {
    // for (w,h) in [(100,30),(112,30),(113,40),(120,40),(160,50),(200,60)]: tiles don't overlap, all inside the
    // left area, and step() from every drawn part in every direction lands on a drawn part or crosses to the pack.
}
#[test] fn the_arrows_walk_the_body_and_right_crosses_to_the_pack() {
    // from TORSO: Up → HEAD, Left → L ARM, Right → R ARM, Down → WAIST; from R HAND Right → the pack;
    // from the pack Left → back to the last part.
}
#[test] fn a_third_arm_lands_in_its_free_places() {
    // a Content whose body adds ThirdArm (arm, joins TORSO) and ThirdHand (hand, joins ThirdArm):
    // they draw under R ARM and R HAND, on a branch off the line under TORSO, as gen.py's third-arm frame.
}
#[test] fn space_on_a_hand_offers_what_fits_a_hand_and_wears_it_there() {
    // L HAND tile, space: the popup lists the pack's guns and the shiv; choosing one sends EquipAt(_, LeftHand).
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- kit::`
Expected: compile errors, then failures.

- [ ] **Step 3: Implement**

- Layout: the tab's body area splits into the body (left) and the pack and card (right, `║` between). Full (≥ 113 × 27): the body takes 62 columns (the graph's 60 + margin), the mock's positions (`gen.py` `POS`, origin 2 columns in, 2 rows down: the header row `▌MEAT▐` and a `─` rule under it), lines as `graph()` draws them. Compact: the left half; no arm tiles (hands join the torso directly), tiles 4 tall with the name on one line (cut with `…`), TORSO 6 tall, the header without its rule, the same shape otherwise; centre the body vertically in its rows in both.
- Tiles as `gen.py`'s `tile()`: part name in the top border; item name wrapped at spaces onto 2 lines (1 in compact), a word too long cut with `…`; the dim gives-line: a gun `{damage}×{rounds}` then a pip per socket (`■` fitted, `□` empty); a hand prefixes `● ` (main) or `○ ` (other); a deck its program slots as `■` and sockets as `□` (as today's DECK tile marks); else the item's first perk, short: `+N HP`, `+N kg`, `{ATTR} +N` (`COOL +1`), `+N armour`, `+N move`, `+N sight`, `+N focus`. Empty part: a dim `·` in the middle. The cursor's tile: cyan border, its label bold in the selection colour, its inside on the selection background, as today's selected rack tile.
- Right side: the pack as today (filter row, groups) in the top rows, the weight and value rule, then the card; the half-size drawing goes under the card's text when 8 rows are left, else `space inspect` alone.
- Keys: `step()` picks, among drawn parts, the one whose tile centre lies in that direction and is nearest (distance along the direction, ties broken by the smaller sideways distance); none in the direction and `Right` crosses to the pack. Hint: `←↑↓→ body, then over to the pack · [ ] filter · space act · Q/E tabs · tab close`.
- Space on a part: the part popup lists pack items whose kind is the part's kind (`EquipAt`/`EquipCopyAt` at that part), then take off and inspect as today; nothing fits: `nothing in the pack fits the {part name lower}`.
- Shots: `gear_and_inventory` at (100,30) and (120,40), cursor on R HAND, plus `gear_and_inventory_third_arm` at (120,40) from the test body above.
- Manual 05: the GEAR & INVENTORY section describes the body (the parts, what a tile shows, the hands' `●`/`○`, the keys) in place of the slot list and "Only your PRIMARY weapon fires".

- [ ] **Step 4: Run every check, and look at the shots**

Run the five commands from CLAUDE.md, then `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored gear_and_inventory`.
Expected: all pass; the 120×40 PNG matches the mock's layout.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "kit: the body as joined tiles on GEAR & INVENTORY"
```
