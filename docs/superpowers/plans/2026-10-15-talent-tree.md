# Talent Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Five stat trees of tiered, ranked talents with real effects, one point at creation and one per level, spent from a TALENTS tab in the deck.

**Architecture:** The `game` crate gets the tree and talent model, validation, the character's ranks and points, the learning rules and the effects (perks, grants, sharpening, flags) read everywhere the sheet is read. The app fires abilities through the character's sharpened copy, adds the deck tab with its own full-pane drawing and cursor, two console commands, the summary line and the README.

**Tech Stack:** Rust 2024, ratatui 0.29, ron.

**Spec:** docs/superpowers/specs/2026-10-15-talent-tree-design.md

## Global Constraints

- Every commit passes `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace`; `cargo fmt --all` first.
- `creation.talent_points = 1`, `creation.tier_gate = 3`, ranks 1..=3, two talents per tier, one on the fifth.
- Content ids snake_case; names and descriptions in the game's voice.
- Commit attribution lines as in the session's system reminder.

## Review Focus

1. A talent whose prerequisite sits in another column or a higher tier: content validation refuses it with the talent's id in the message (Task 1 test).
2. Spending the last point and then levelling: the point count goes to zero, the box reads out of reach with "no points left", a level-up restores one and the box turns affordable (Task 1 and Task 3 tests).
3. A sharpened cooldown that would go below zero clamps at zero, and a sharpened radius never goes below one (Task 1 test).
4. The deck at its minimum size (100×30) still draws the TALENTS tab without panicking and keeps the dossier (Task 3 test).
5. An ability granted both by a talent and by an item lists once, with the first source (Task 1 test).

## File Structure

- `crates/game/src/model.rs` — `Tree`, `Talent` (new shape), `Effect`, `Perk::Focus`, `Creation` fields.
- `crates/game/src/content.rs` — `trees`, validation, `tree(stat)`, `tree_of(talent)`.
- `crates/game/src/character.rs` — ranks, points, learning, effects, `ability`, `flags`, xp.
- `crates/game/src/action.rs` — `Action::Learn`.
- `crates/game/src/build.rs` — `talents` removed.
- `flavor/content.ron` — the trees and sixty talents.
- `crates/strata-zero/src/actions.rs` — fire through `Character::ability`.
- `crates/strata-zero/src/screens/console.rs` — `talent`, `tpts`.
- `crates/strata-zero/src/screens/create/summary.rs` — the talent point line.
- `crates/strata-zero/src/ui/talents.rs` (new) — the tree widget and the dossier.
- `crates/strata-zero/src/screens/deck.rs` — `Tab::Talents`, cursor, keys, drawing.
- `README.md`.

---

### Task 1: The model, the rules and the trees

**Files:**
- Modify: `crates/game/src/model.rs`, `crates/game/src/content.rs`, `crates/game/src/character.rs`, `crates/game/src/action.rs`, `crates/game/src/build.rs`, `flavor/content.ron`
- Modify (app compiles): `crates/strata-zero/src/actions.rs`, any site that reads `Character.talents` or `Build.talents`.

**Interfaces:**
- Produces:
```rust
pub struct Tree { pub stat: Stat, pub name: String, pub line: String, pub tiers: Vec<Vec<TalentId>> }
pub struct Talent { pub id: TalentId, pub name: String, pub icon: char, pub ranks: u8, pub requires: Option<TalentId>, pub effects: Vec<Vec<Effect>>, pub skill: Option<SkillId>, pub desc: String }
pub enum Effect { Perk(Perk), Grant(AbilityId), Sharpen { ability: AbilityId, radius: i32, cooldown: i64 }, Flag(String) }
impl Effect { pub fn words(&self, c: &Content) -> String }   // "+1 Reflex", "ping reaches 4 further", "later: quick_draw"
impl Perk { Focus(i32) }  // new variant
impl Creation { pub talent_points: u32, pub tier_gate: u32 }
impl Content { pub fn tree(&self, stat: Stat) -> Option<&Tree>; pub fn tree_of(&self, id: &TalentId) -> Option<(&Tree, usize /*tier*/, usize /*column*/)>; }
impl Character {
    pub talents: BTreeMap<TalentId, u8>; pub talent_points: u32;
    pub fn talent_rank(&self, id: &TalentId) -> u8;
    pub fn tree_points(&self, c: &Content, stat: Stat) -> u32;
    pub fn tier_open(&self, c: &Content, stat: Stat, tier: usize) -> bool;
    pub fn can_learn(&self, c: &Content, id: &TalentId) -> Result<(), String>;
    pub fn learn(&mut self, c: &Content, id: &TalentId) -> Result<u8, String>;
    pub fn talent_effects<'a>(&'a self, c: &'a Content) -> impl Iterator<Item = &'a Effect> + 'a;
    pub fn ability(&self, c: &Content, id: &AbilityId) -> Option<Ability>;
    pub fn flags(&self, c: &Content) -> BTreeSet<String>;
}
pub enum Source { Innate, Category(CategoryId), Item(ItemId), Talent(TalentId) }
pub enum Action { ..., Learn(TalentId) }
```
  `has_talent(id)` stays and means rank ≥ 1. `Build.talents` and `Template.talents` are removed.

- [ ] **Step 1: Failing tests in `crates/game/src/character.rs` and `content.rs`**

```rust
    #[test]
    fn the_trees_load_and_validate() {
        let c = Content::load().unwrap();
        assert_eq!(c.trees.len(), 5);
        for t in &c.trees {
            assert_eq!(t.tiers.len(), 5, "{}", t.name);
            assert!(t.tiers[..4].iter().all(|tier| tier.len() == 2), "{}", t.name);
            assert_eq!(t.tiers[4].len(), 1, "{} capstone", t.name);
        }
        assert!(c.talents.len() >= 40);
        for id in ["quick_draw", "double_tap", "backstab", "ghost", "ice_pick", "deep_dive", "solder_hand", "field_medic"] {
            let (tree, _, _) = c.tree_of(&TalentId::new(id)).unwrap_or_else(|| panic!("{id} in a tree"));
            let _ = tree;
        }
        assert_eq!(c.creation.talent_points, 1);
        assert_eq!(c.creation.tier_gate, 3);
    }

    #[test]
    fn validation_refuses_a_misplaced_prerequisite_and_bad_ranks() {
        let mut c = Content::load().unwrap();
        // Prerequisite in another column.
        let (tree, tier, col) = c.tree_of(&TalentId::new("double_tap")).map(|(t, i, j)| (t.stat, i, j)).unwrap();
        let other = c.tree(tree).unwrap().tiers[tier - 1][1 - col.min(1)].clone();
        c.talents.iter_mut().find(|t| t.id == TalentId::new("double_tap")).unwrap().requires = Some(other);
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains("double_tap") && err.contains("column"), "{err}");
        // A rank count that does not match the effects.
        let mut c = Content::load().unwrap();
        c.talents[0].ranks = 2;
        c.talents[0].effects = vec![vec![]];
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains(&c.talents[0].id.0) && err.contains("rank"), "{err}");
        // An empty tier.
        let mut c = Content::load().unwrap();
        c.trees[0].tiers[1].clear();
        assert!(c.validate().unwrap_err().to_string().contains("tier"));
    }

    #[test]
    fn learning_follows_points_tiers_prerequisites_and_ranks() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);   // the existing test helper that builds VEX, or Character::from_build on the template
        assert_eq!(ch.talent_points, 1);
        let edge = c.tree(Stat::Reflex).unwrap();
        let first = edge.tiers[0][0].clone();
        let second = edge.tiers[1][0].clone();
        assert_eq!(ch.can_learn(&c, &second).unwrap_err(), "3 more points in EDGE");
        assert_eq!(ch.learn(&c, &first).unwrap(), 1);
        assert_eq!(ch.talent_points, 0);
        assert_eq!(ch.can_learn(&c, &first).unwrap_err(), "no points left");
        ch.talent_points = 10;
        let ranks = c.talent(&first).unwrap().ranks;
        for r in 2..=ranks { assert_eq!(ch.learn(&c, &first).unwrap(), r); }
        assert_eq!(ch.can_learn(&c, &first).unwrap_err(), "already at its best");
        assert_eq!(ch.tree_points(&c, Stat::Reflex), ranks as u32);
        // The second tier wants three points in the tree, whatever the talent.
        if (ranks as u32) < 3 {
            assert!(ch.can_learn(&c, &second).unwrap_err().contains("more points in EDGE"));
            let other = edge.tiers[0][1].clone();
            while ch.tree_points(&c, Stat::Reflex) < 3 { ch.learn(&c, &other).unwrap(); }
        }
        // A prerequisite must be maxed.
        let needy = c.talents.iter().find(|t| t.requires.is_some()).unwrap();
        let (tree, tier, _) = c.tree_of(&needy.id).unwrap();
        let mut ch2 = vex(&c);
        ch2.talent_points = 30;
        let stat = tree.stat;
        // Fill lower tiers without touching the prerequisite.
        for lower in 0..tier {
            for id in &c.tree(stat).unwrap().tiers[lower] {
                if Some(id) == needy.requires.as_ref() { continue; }
                while ch2.can_learn(&c, id).is_ok() { ch2.learn(&c, id).unwrap(); }
            }
        }
        if ch2.tier_open(&c, stat, tier) {
            let req = c.talent(needy.requires.as_ref().unwrap()).unwrap();
            assert_eq!(ch2.can_learn(&c, &needy.id).unwrap_err(), format!("needs {} {}/{}", req.name, req.ranks, req.ranks));
        }
    }

    #[test]
    fn ranks_carry_perks_grants_sharpening_and_flags() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        ch.talent_points = 40;
        // Find talents by effect kind and learn them fully.
        let with = |kind: fn(&Effect) -> bool| c.talents.iter().find(|t| t.effects.iter().flatten().any(kind)).unwrap().id.clone();
        let perk = with(|e| matches!(e, Effect::Perk(Perk::Stat(..))));
        let focus = with(|e| matches!(e, Effect::Perk(Perk::Focus(_))));
        let grant = with(|e| matches!(e, Effect::Grant(_)));
        let sharpen = with(|e| matches!(e, Effect::Sharpen { .. }));
        let flag = with(|e| matches!(e, Effect::Flag(_)));
        let force = |ch: &mut Character, id: &TalentId| { let t = c.talent(id).unwrap(); ch.talents.insert(id.clone(), t.ranks); };
        let before_focus = ch.focus_max(&c);
        force(&mut ch, &focus);
        assert!(ch.focus_max(&c) > before_focus);
        let (stat, n) = match c.talent(&perk).unwrap().effects[0][0] { Effect::Perk(Perk::Stat(s, n)) => (s, n), _ => unreachable!() };
        let before = ch.stat(&c, stat);
        force(&mut ch, &perk);
        assert!(ch.stat(&c, stat) as i32 >= before as i32 + n as i32 || ch.stat(&c, stat) == 10);
        force(&mut ch, &grant);
        let Effect::Grant(ability) = &c.talent(&grant).unwrap().effects[0][0] else { unreachable!() };
        assert!(ch.abilities(&c).iter().any(|(a, s)| a == ability && matches!(s, Source::Talent(_)) || a == ability));
        force(&mut ch, &sharpen);
        let Effect::Sharpen { ability, radius, cooldown } = &c.talent(&sharpen).unwrap().effects[0][0] else { unreachable!() };
        let base = c.ability(ability).unwrap();
        let eff = ch.ability(&c, ability).unwrap();
        let base_r = match base.kind { AbilityKind::Active(Active::Scan { radius } | Active::Ping { radius } | Active::Trace { radius }) => radius, _ => 0 };
        let eff_r = match eff.kind { AbilityKind::Active(Active::Scan { radius } | Active::Ping { radius } | Active::Trace { radius }) => radius, _ => 0 };
        assert!(eff_r >= base_r + radius.min(&0) && eff_r >= 1);
        assert_eq!(eff.cooldown, (base.cooldown as i64 + cooldown * c.talent(&sharpen).unwrap().ranks as i64).max(0) as u64);
        force(&mut ch, &flag);
        let Effect::Flag(f) = &c.talent(&flag).unwrap().effects[0][0] else { unreachable!() };
        assert!(ch.flags(&c).contains(f));
        // A cooldown can never go below zero.
        let mut sharp = c.clone();
        sharp.talents.iter_mut().find(|t| t.id == sharpen).unwrap().effects[0] = vec![Effect::Sharpen { ability: ability.clone(), radius: -99, cooldown: -99_999 }];
        let eff = ch.ability(&sharp, ability).unwrap();
        assert_eq!(eff.cooldown, 0);
        // A grant that an item also makes lists once.
        let item_grant = c.items.iter().find(|i| !i.grants.is_empty()).unwrap().clone();
        ch.give(item_grant.id.clone(), 1);
        ch.equipment.insert(item_grant.slot.unwrap(), item_grant.id.clone());
        let listed = ch.abilities(&c);
        for (a, _) in &listed { assert_eq!(listed.iter().filter(|(b, _)| b == a).count(), 1, "{a} once"); }
    }

    #[test]
    fn a_level_brings_a_talent_point_and_the_action_learns() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        ch.talent_points = 0;
        assert_eq!(ch.gain_xp(Character::xp_to_next(1)), 1);
        assert_eq!(ch.talent_points, 1);
        let first = c.tree(Stat::Net).unwrap().tiers[0][0].clone();
        let mut g = Ground::default();
        let at = At { place: Place::Stratum(StratumKind::Street), pos: Pos::new(0, 0) };
        let out = apply(&mut ch, &mut g, &c, at, 0, Action::Learn(first.clone())).unwrap();
        assert!(out.message.contains("1/"), "{}", out.message);
        assert_eq!(ch.talent_rank(&first), 1);
        assert_eq!(apply(&mut ch, &mut g, &c, at, 0, Action::Learn(first)).unwrap_err(), "no points left");
    }
```

- [ ] **Step 2: Run** `cargo test -p game` — compile errors on the new types.

- [ ] **Step 3: Implement** the model (`Tree`, new `Talent`, `Effect`, `Perk::Focus`, `Creation` fields with serde defaults of 1 and 3), `Content.trees` + `tree`/`tree_of` + validation (messages: `talent {id}: prerequisite {req} must sit in the same column of a lower tier of {tree}`, `talent {id}: {ranks} ranks but {n} effect lists`, `tree {name}: tier {i} is empty`, `talent {id} in no tree` / `in two trees`), the character (map and points; `from_build` sets `talent_points = c.creation.talent_points`; `can_learn` reasons in the spec's order; `learn`; `talent_effects`; `perks` chains talent perks; `focus_max` reads `Perk::Focus` (add `focus_max(&self, c)` and use it where `focus_max` the field was read: the field stays as the base); `abilities` adds grants with `Source::Talent`; `ability(c, id)` applies sharpening; `flags`; `gain_xp` adds points), `Action::Learn`, `Build` and `Template` without `talents`, `Effect::words`. Content: the five trees and about sixty talents, in voice, icons from a small set (`▲ ◆ ◉ ⚡ ✚ ◈ ☍ ⌁ ⚙ ✦ ⌖ ⟡ ✪`), with the eight old ones folded in. App: `actions.rs` fires through `st.character.ability(&st.content, &id)`; any `talents` field reader updated.

- [ ] **Step 4: Run** `cargo test --workspace` — green.
- [ ] **Step 5: Commit** `feat(game): talent trees, ranks, points and their effects`

---

### Task 2: Console, firing, summary, level-up line

**Files:** `crates/strata-zero/src/screens/console.rs`, `crates/strata-zero/src/screens/create/summary.rs`, `crates/strata-zero/src/actions.rs`, `crates/strata-zero/src/screens/deck.rs` (SKILLS tab shows `Source::Talent` as "talent").

- [ ] **Step 1: Failing tests**: console `talent <id>` learns and reports `"{name} 1/{ranks}"`, refuses with the rule's reason, `talent` alone says `talent <id>`; `tpts 3` → `"3 talent points"` and the count; the SUMMARY step text contains `1 talent point`; `xp` crossing a level logs `level 2 · a talent point`; firing a sharpened ping uses the sharpened radius (fx instance radius).
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement**; `COMMANDS` gains `talent` and `tpts`; help text updated.
- [ ] **Step 4: Run** the workspace — green.
- [ ] **Step 5: Commit** `feat(app): talent points in the console, the summary and the log`

---

### Task 3: The TALENTS tab

**Files:**
- Create: `crates/strata-zero/src/ui/talents.rs`
- Modify: `crates/strata-zero/src/screens/deck.rs`, `crates/strata-zero/src/ui/mod.rs`

**Interfaces:**
```rust
pub struct TreeCursor { pub tree: usize, pub tier: usize, pub col: usize }
pub struct TalentsView<'a> { pub content: &'a Content, pub character: &'a Character, pub cursor: TreeCursor, pub tick: u64, pub flash: Option<(TalentId, u64)> }
impl TalentsView<'_> { pub fn draw(&self, area: Rect, buf: &mut Buffer); }
pub fn talent_at(c: &Content, cur: TreeCursor) -> Option<&TalentId>;
pub fn step(c: &Content, cur: TreeCursor, dx: i32, dy: i32) -> TreeCursor;   // along a tier and across trees for dx, between tiers for dy, nearest column
pub fn dossier(c: &Content, ch: &Character, id: &TalentId) -> Vec<Line<'static>>;  // four lines
```
Layout: header row; five columns of 23 starting at `area.x`; each column: name line (stat colour) + line (dim); tiers from the next row, each tier two rows of boxes (11 cells, gap 1) plus one gap row with connectors `│` under a box whose dependent sits below, and the caption `── 3 more ──` centred on a closed tier; the dossier takes the last four rows of the area, separated by a light rule. When the area is shorter than needed, tiers that do not fit are left out from the bottom and the dossier stays.

- [ ] **Step 1: Failing tests** in `deck.rs`: `0` opens TALENTS and draws five tree names, the header with `1 points` (or `1 point`), the cursor on the first tree's first talent inverted; `l` ×2 moves to the second tree; `j` moves down a tier; `space` on an affordable talent fills one meter cell, the header reads `0 points`, the log/strip reads `Quick draw 1/3` (whichever is first); a closed tier draws `3 more`; the dossier reads the reason for an out-of-reach talent; the deck at 100×30 draws the tab and the dossier without panic; `tab` still changes tabs from TALENTS and `l` does not.
- [ ] **Step 2: Run** — failures.
- [ ] **Step 3: Implement** `Tab::Talents` (label `TALENTS`, key `0` maps to index 9; `ALL` length 10), `Deck.cursor: TreeCursor`, the key handling in the Talents tab before the generic `←→` tab shift, `Action::Learn` through `perform` with the deck's flash, the drawing via `TalentsView` in `draw` where `Tab::Map` has its own branch.
- [ ] **Step 4: Run** the workspace — green.
- [ ] **Step 5: Commit** `feat(app): the TALENTS tab`

---

### Task 4: Docs, smoke, screenshot

- [ ] **Step 1: README**: the deck line (tab `0`, its keys), a paragraph on talents under the deck paragraph.
- [ ] **Step 2: Smoke** `drive13.py`: VEX in, `tab`, `0`, expect `TALENTS` and the five names, `space`, expect `0 point` in the header and a `1/` meter in the dossier; `:tpts 5`, `0`... expect `5 points`; quit.
- [ ] **Step 3: Screenshot** of the tab with a few points spent (via `tpts` and `talent`), sent.
- [ ] **Step 4: Commit** `docs: the talent tree`.

---

## Self-review

Spec coverage: content and validation (1), rules and effects (1), firing through the sharpened ability (1, 2), console/summary/log (2), the tab (3), docs (4). Review Focus lines have tests in 1, 1+3, 1, 3, 1. Names match across tasks: `tree_of`, `can_learn`, `learn`, `ability`, `Action::Learn`, `TreeCursor`, `TalentsView`.
