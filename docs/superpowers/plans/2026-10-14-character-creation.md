# Character Creation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** NEW GAME opens a chooser (VEX, KESSLER or custom) and a six-step wizard that builds the character from origins, professions, stat affinity and SP-bought skill categories, and the deck gains STATS and a folding SKILLS board.

**Architecture:** The `game` crate gets the content types (origins, professions, categories, templates, creation numbers), a `build` module with the pure rules (costs, pools, toggles, validation) and `Character::from_build`. The app crate gets a `screens/create` module (one frame, one file per step, a chooser), a shared `ui/board` row builder used by both the wizard and the deck, a `ui/meter` for the pools, and the deck's new tabs. The `Build` lives in `AppState`.

**Tech Stack:** Rust 2024, ratatui 0.29, crossterm 0.28, ron, serde. Tests through `TestBackend`.

**Spec:** `docs/superpowers/specs/2026-10-14-character-creation-design.md`

## Global Constraints

- Every line of copy in `flavor/content.ron` and on the wizard is in voice: second person, short, street (spec: Voice). Step and category names in caps, labels lower case, the confirm is `JACK IN`.
- Numbers, verbatim from the spec: `stat_base: 3, stat_cap: 8, stat_points: 20, point_cost: 2, sp: 6, name_max: 16, hp: 40, focus: 30`. Cost clamp `1..=3`. Category costs: STREET 1, all others 2. Skill costs: first 0, second 1, third 2.
- Vex and Kessler both cost exactly 20 points and 6 SP; Vex's gear and pack equal today's starter.
- Saves hold only the city; no migration.
- Every commit ends with the two attribution trailers used on this branch; `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before each commit; the whole workspace suite green.
- Minimum terminal 100×30; block-font captions only at 36 rows or more.

## Review Focus

1. A template whose origin is changed after loading goes over budget: STATS and SUMMARY say *over by N* and JACK IN refuses (Task 2 rules test, Task 5 screen test).
2. Refunding a category that holds owned skills drops the skills and their SP (Task 2 test).
3. Kit equip order when two kit items share a slot: the first equips, the second packs; never a panic, never a lost item (Task 2 test).
4. A name of only spaces, or with a control character, is refused with the in-voice reason (Task 2 test, Task 4 screen test).
5. The deck's SKILLS tab with zero SP left still folds and unfolds and never buys; the WORKBENCH tab without WRENCH shows the locked line and `space` does nothing (Task 3 test).

## File Structure

- `crates/game/src/model.rs` — new ids and structs; `Talent`/`Recipe` lose `level`; `Perk::Skill` and `Starter` removed.
- `crates/game/src/content.rs` — new fields, lookups, validation.
- `crates/game/src/build.rs` — `Build`, `Step`, `Affinity`, `BuildError`, rules.
- `crates/game/src/character.rs` — new fields, `from_build`, `from_template`, `has_*`, `Source::Category`.
- `flavor/content.ron` — the content.
- `crates/strata-zero/src/ui/board.rs` — `board_rows` and glyph rules.
- `crates/strata-zero/src/ui/meter.rs` — pool meter.
- `crates/strata-zero/src/screens/create/{mod,chooser,name,origin,profession,stats,skills,summary}.rs` — the wizard.
- `crates/strata-zero/src/screens/{deck,title,console,help}.rs`, `app.rs`, `README.md` — integration.

---

### Task 1: Content types and the content file

**Files:**
- Modify: `crates/game/src/model.rs`
- Modify: `crates/game/src/content.rs`
- Modify: `flavor/content.ron`
- Test: `crates/game/src/content.rs` tests

**Interfaces:**
- Produces: `OriginId, ProfessionId, CategoryId, TemplateId`; `Origin, Profession, Category, CategorySkill, Unlock, Template, Creation`; `Stat::ALL`, `Stat::name()`; `Content::{creation, origins, professions, categories, templates, stat_blurbs}` and lookups `origin(&OriginId)`, `profession(&ProfessionId)`, `category(&CategoryId)`, `template(&TemplateId)`, `category_of(&SkillId) -> Option<&Category>`, `skill_cost(&SkillId) -> Option<u32>`, `stat_blurb(Stat) -> &str`; `ContentError::{SkillInNoCategory, SkillInTwoCategories, FirstSkillCosts, UnknownOrigin, UnknownProfession, UnknownCategory, UnknownTemplate, EmblemShape}`.
- The `starter` block stays in this task so the app keeps compiling; Task 2 removes it.

- [ ] **Step 1: Write the failing content tests**

Append to the `tests` module in `crates/game/src/content.rs`:

```rust
    #[test]
    fn creation_content_loads_and_resolves() {
        let c = Content::load().unwrap();
        assert_eq!((c.creation.stat_base, c.creation.stat_cap, c.creation.stat_points), (3, 8, 20));
        assert_eq!((c.creation.point_cost, c.creation.sp, c.creation.name_max), (2, 6, 16));
        assert_eq!(c.origins.len(), 4);
        assert_eq!(c.professions.len(), 4);
        assert_eq!(c.categories.len(), 8);
        assert_eq!(c.templates.len(), 2);
        assert_eq!(c.skills.len(), 24);
        for o in &c.origins {
            assert_eq!((o.cheap.len(), o.dear.len(), o.emblem.len()), (2, 2, 3), "{}", o.id);
        }
        for p in &c.professions {
            assert_eq!((p.cheap.len(), p.dear.len(), p.emblem.len()), (1, 1, 3), "{}", p.id);
        }
        let net = c.category(&CategoryId::new("netrunner")).unwrap();
        assert_eq!(net.cost, 2);
        assert_eq!(net.skills[0].cost, 0);
        assert_eq!(net.grants, vec![AbilityId::new("signal_ghost")]);
        let wrench = c.category(&CategoryId::new("wrench")).unwrap();
        assert_eq!(wrench.unlocks, vec![Unlock::Workbench]);
        assert_eq!(c.category_of(&SkillId::new("hacking")).unwrap().id, net.id);
        assert_eq!(c.skill_cost(&SkillId::new("ice_breaker")), Some(1));
        assert_eq!(c.skill_cost(&SkillId::new("vault_run")), Some(2));
        assert_eq!(c.skill_cost(&SkillId::new("origami")), None);
        assert!(c.stat_blurb(Stat::Body).contains("meat"));
        assert_eq!(c.template(&TemplateId::new("vex")).unwrap().handle, "VEX");
        assert!(c.item(&ItemId::new("ballistic_weave")).is_some());
        assert!(c.ability(&AbilityId::new("signal_ghost")).is_some());
        assert!(c.talents.iter().all(|t| c.skill(&t.skill).is_some()));
        assert!(c.recipes.iter().all(|r| c.skill(&r.skill).is_some()));
    }

    #[test]
    fn creation_content_is_validated() {
        let c = Content::load().unwrap();
        let mut two = c.clone();
        two.categories[1].skills.push(CategorySkill { id: SkillId::new("hacking"), cost: 1 });
        assert!(matches!(two.validate(), Err(ContentError::SkillInTwoCategories { .. })));
        let mut none = c.clone();
        none.skills.push(Skill { id: SkillId::new("origami"), name: "Origami".into(), stat: Stat::Tech, desc: "paper".into() });
        assert!(matches!(none.validate(), Err(ContentError::SkillInNoCategory { .. })));
        let mut costs = c.clone();
        costs.categories[0].skills[0].cost = 1;
        assert!(matches!(costs.validate(), Err(ContentError::FirstSkillCosts { .. })));
        let mut kit = c.clone();
        kit.professions[0].kit.push((ItemId::new("ghost_item"), 1));
        assert!(matches!(kit.validate(), Err(ContentError::UnknownItem { .. })));
        let mut t = c.clone();
        t.templates[0].origin = OriginId::new("nowhere");
        assert!(matches!(t.validate(), Err(ContentError::UnknownOrigin { .. })));
        let mut e = c.clone();
        e.origins[0].emblem.pop();
        assert!(matches!(e.validate(), Err(ContentError::EmblemShape { .. })));
    }
```

- [ ] **Step 2: Run them to watch them fail**

Run: `cargo test -p game -- creation_content 2>&1 | grep -E "^error|^test " | head`
Expected: compile errors, `creation`, `origins`, `CategoryId` not found.

- [ ] **Step 3: Add the types to `model.rs`**

After the `id_type!` list: `id_type!(OriginId); id_type!(ProfessionId); id_type!(CategoryId); id_type!(TemplateId);`

In `impl Stat` add:

```rust
    pub const ALL: [Stat; 5] = [Stat::Body, Stat::Reflex, Stat::Tech, Stat::Cool, Stat::Net];
    /// The label in caps, as the sheet sets it.
    pub fn name(self) -> &'static str {
        match self {
            Stat::Body => "BODY",
            Stat::Reflex => "REFLEX",
            Stat::Tech => "TECH",
            Stat::Cool => "COOL",
            Stat::Net => "NET",
        }
    }
```

Before `Stats`:

```rust
/// Where you were made. Cheap stats cost one less per point, dear ones one more.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Origin {
    pub id: OriginId,
    pub name: String,
    pub line: String,
    pub desc: String,
    pub cheap: Vec<Stat>,
    pub dear: Vec<Stat>,
    /// Three rows of seven glyphs for the dossier.
    pub emblem: Vec<String>,
}

/// What you did before this: affinity, a kit and credits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profession {
    pub id: ProfessionId,
    pub name: String,
    pub line: String,
    pub desc: String,
    pub cheap: Vec<Stat>,
    pub dear: Vec<Stat>,
    pub kit: Vec<(ItemId, u32)>,
    pub credits: u32,
    pub emblem: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategorySkill {
    pub id: SkillId,
    pub cost: u32,
}

/// What owning a category opens besides its skills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unlock {
    Workbench,
}

/// A skill category: costs SP to own, then its skills cost SP each.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub line: String,
    pub cost: u32,
    pub skills: Vec<CategorySkill>,
    #[serde(default)]
    pub grants: Vec<AbilityId>,
    #[serde(default)]
    pub unlocks: Vec<Unlock>,
}

/// A complete build the chooser offers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub id: TemplateId,
    pub handle: String,
    pub line: String,
    pub origin: OriginId,
    pub profession: ProfessionId,
    pub stats: Stats,
    pub categories: Vec<CategoryId>,
    pub skills: Vec<SkillId>,
    #[serde(default)]
    pub talents: Vec<TalentId>,
    /// Extra kit on top of the profession's.
    #[serde(default)]
    pub kit: Vec<(ItemId, u32)>,
}

/// The numbers every new character is built under.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creation {
    pub stat_base: u8,
    pub stat_cap: u8,
    pub stat_points: u32,
    pub point_cost: u32,
    pub sp: u32,
    pub name_max: usize,
    pub hp: u32,
    pub focus: u32,
    pub quests: Vec<(QuestId, Vec<bool>)>,
}
```

Leave `Starter`, `Talent.level`, `Recipe.level` and `Perk::Skill` in place for this task.

- [ ] **Step 4: Content fields, lookups and validation**

In `Content` add after `abilities`:

```rust
    pub creation: Creation,
    pub origins: Vec<Origin>,
    pub professions: Vec<Profession>,
    pub categories: Vec<Category>,
    pub templates: Vec<Template>,
    pub stat_blurbs: Vec<(Stat, String)>,
```

Errors to add to `ContentError`:

```rust
    #[error("skill `{id}` belongs to no category")]
    SkillInNoCategory { id: String },
    #[error("skill `{id}` belongs to categories `{a}` and `{b}`")]
    SkillInTwoCategories { id: String, a: String, b: String },
    #[error("category `{category}`: its first skill `{skill}` must be free")]
    FirstSkillCosts { category: String, skill: String },
    #[error("{where_}: unknown origin `{id}`")]
    UnknownOrigin { where_: String, id: String },
    #[error("{where_}: unknown profession `{id}`")]
    UnknownProfession { where_: String, id: String },
    #[error("{where_}: unknown category `{id}`")]
    UnknownCategory { where_: String, id: String },
    #[error("{where_}: unknown template `{id}`")]
    UnknownTemplate { where_: String, id: String },
    #[error("{what} `{id}`: emblem must be three rows of seven glyphs")]
    EmblemShape { what: String, id: String },
```

Lookups, next to `recipe`:

```rust
    pub fn origin(&self, id: &OriginId) -> Option<&Origin> { self.origins.iter().find(|o| &o.id == id) }
    pub fn profession(&self, id: &ProfessionId) -> Option<&Profession> { self.professions.iter().find(|p| &p.id == id) }
    pub fn category(&self, id: &CategoryId) -> Option<&Category> { self.categories.iter().find(|c| &c.id == id) }
    pub fn template(&self, id: &TemplateId) -> Option<&Template> { self.templates.iter().find(|t| &t.id == id) }
    /// The category a skill sits in.
    pub fn category_of(&self, skill: &SkillId) -> Option<&Category> {
        self.categories.iter().find(|c| c.skills.iter().any(|s| &s.id == skill))
    }
    /// What a skill costs in SP, by its category's listing.
    pub fn skill_cost(&self, skill: &SkillId) -> Option<u32> {
        self.category_of(skill)?.skills.iter().find(|s| &s.id == skill).map(|s| s.cost)
    }
    pub fn stat_blurb(&self, stat: Stat) -> &str {
        self.stat_blurbs.iter().find(|(s, _)| *s == stat).map(|(_, b)| b.as_str()).unwrap_or("")
    }
```

Validation, before `Ok(())`, covering: ids of origins, professions, categories, templates join the duplicate scan; every category skill resolves (`need_skill`); every skill in exactly one category; each category's first skill costs 0; grants resolve; profession kit items resolve; template origin, profession, categories, skills, talents, kit resolve; creation quests resolve with matching flag counts; emblems are three rows of seven chars (`chars().count() == 7`). The template's budget check comes in Task 2 with `Build`.

- [ ] **Step 5: Write the content**

Rewrite `flavor/content.ron`: keep every item, add the three new items; replace the eight `skills` with 24 (the eight existing ids keep their names and stats; new ones follow), add `signal_ghost` to `abilities`, remove `level` from nothing yet (Task 2), add `creation`, `origins`, `professions`, `categories`, `templates`, `stat_blurbs`. The `starter` block stays until Task 2.

New skills (id · name · stat · desc), all in voice:

```
ice_breaker   ICE breaker     Net   "You find the seam in the ICE before it finds you."
vault_run     Vault run       Net   "You go past the vault door and come back with the vault."
scrapcraft    Scrapcraft      Tech  "Two broken things make one that works, mostly."
overclock     Overclock       Tech  "Everything you touch runs hot and fast and short."
longarm       Longarm         Reflex "Rifles and the patience to use them."
snapshot      Snapshot        Reflex "The first round leaves before the thought does."
close_work    Close work      Reflex "Inside the swing, under the guard, done."
riposte       Riposte         Reflex "They cut, you cut back, they stop."
lockpick      Lockpick        Cool  "Doors are a suggestion."
vanish        Vanish          Cool  "You were never here, and the footage agrees."
haggle        Haggle          Cool  "Every price has a second price."
fixers_ear    Fixer's ear     Cool  "You hear the job before it is posted."
field_patch   Field patch     Tech  "Tape, gel and a steady hand, on the run."
stim_cocktail Stim cocktail   Tech  "Mix it right and the night lasts twice as long."
freerun       Freerun         Body  "Walls are a slower kind of floor."
second_wind   Second wind     Body  "The point where others stop is where you start."
```

Origins (desc in voice, three-line emblem each), professions (kit and credits per the spec), categories (lines per the spec), the two templates verbatim from the spec, `stat_blurbs` with the five lines from the spec, and:

```ron
    creation: Creation(
        stat_base: 3, stat_cap: 8, stat_points: 20, point_cost: 2,
        sp: 6, name_max: 16, hp: 40, focus: 30,
        quests: [("the_drop", [true, false, false]), ("noodle_debt", [false]), ("first_jack", [true, true])],
    ),
```

Emblems: SLAB RAT `░▒▓█▓▒░ / ▒█ ◉ █▒ / ░▒▓█▓▒░`, CRÈCHE `┌─────┐ / │ ▣▣▣ │ / └─────┘`, STOWAWAY `▁▂▃▅▃▂▁ / ◢█▇▆▇█◣ / ▔▔▔▔▔▔▔`, VAT-GROWN `╭─────╮ / │ ◌ ◌ │ / ╰─~~~─╯`; NETRUNNER `╔═╦═╦═╗ / ║◆║◇║◆║ / ╚═╩═╩═╝`, FIXER `· ¤ ¤ · / ¤ ◉ ◉ ¤ / · ¤ ¤ ·`, SOLO `▌▌▌▌▌▌▌ / ▌◢██◣ ▌ / ▌▌▌▌▌▌▌`, TECHIE `⚙ ─ ⚙ ─ / ─ ⚙ ─ ⚙ / ⚙ ─ ⚙ ─`. Each row must be exactly seven characters; pad with spaces.

- [ ] **Step 6: Run the content tests**

Run: `cargo test -p game 2>&1 | grep -E "^test result|FAILED|panicked at"`
Expected: all green, including the existing `embedded_content_parses_and_covers_the_kit` (24 skills now; adjust its count assertion if it pins 8).

- [ ] **Step 7: Workspace suite, lint, commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace 2>&1 | grep -E "^test result|FAILED"`
Expected: green. Commit `feat(content): origins, professions, categories, templates`.

---

### Task 2: Build rules and the character swap

**Files:**
- Create: `crates/game/src/build.rs`
- Modify: `crates/game/src/lib.rs` (add `pub mod build; pub use build::*;`)
- Modify: `crates/game/src/model.rs` (remove `Starter`, `Perk::Skill`, `Talent.level`, `Recipe.level`)
- Modify: `crates/game/src/content.rs` (remove `starter` and its validation; add template budget validation via `Build`)
- Modify: `crates/game/src/character.rs`
- Modify: `crates/game/src/action.rs` tests (use `from_template`)
- Modify: `flavor/content.ron` (drop `starter`, drop `level`s, perk swaps)
- Modify: `crates/strata-zero/src/app.rs` (`AppState::new` and `build` field), `crates/strata-zero/src/screens/deck.rs` (compile-only adaptation), `crates/strata-zero/src/render/items.rs` and `ui/panel.rs` tests if they used the starter.
- Test: `crates/game/src/build.rs` tests, `crates/game/src/character.rs` tests

**Interfaces:**
- Consumes: Task 1 types and lookups.
- Produces: `game::{Build, Step, Affinity, BuildError}`; `Character::{from_build, from_template, has_skill, has_category, has_workbench, stat_points, sp, origin, profession, categories, skills: BTreeSet}`; `Source::Category(CategoryId)`; `AppState.build: Option<Build>`.

- [ ] **Step 1: Write the failing build tests**

`crates/game/src/build.rs` starts as the tests module alone:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Content;
    use crate::model::*;

    fn vex(c: &Content) -> Build {
        Build::from_template(c, c.template(&TemplateId::new("vex")).unwrap())
    }

    #[test]
    fn costs_follow_origin_and_profession_and_clamp() {
        let c = Content::load().unwrap();
        let b = vex(&c);
        assert_eq!(b.stat_cost(&c, Stat::Body), 3, "dear twice clamps to 3");
        assert_eq!(b.stat_cost(&c, Stat::Net), 1, "cheap twice clamps to 1");
        assert_eq!(b.stat_cost(&c, Stat::Reflex), 2);
        assert_eq!(b.affinity(&c, Stat::Tech), Affinity::Cheap);
        for o in &c.origins {
            for p in &c.professions {
                let mut x = Build::blank(&c);
                x.origin = Some(o.id.clone());
                x.profession = Some(p.id.clone());
                for s in Stat::ALL {
                    assert!((1..=3).contains(&x.stat_cost(&c, s)), "{} {} {s:?}", o.id, p.id);
                }
            }
        }
        let blank = Build::blank(&c);
        assert_eq!(blank.stat_cost(&c, Stat::Body), 2, "no picks: base cost");
    }

    #[test]
    fn templates_spend_exactly_their_pools() {
        let c = Content::load().unwrap();
        for t in &c.templates {
            let b = Build::from_template(&c, t);
            assert_eq!((b.points_spent(&c), b.sp_spent(&c)), (20, 6), "{}", t.id);
            assert_eq!(b.validate(&c), Ok(()), "{}", t.id);
        }
    }

    #[test]
    fn buying_and_refunding_stats() {
        let c = Content::load().unwrap();
        let mut b = Build::blank(&c);
        b.origin = Some(OriginId::new("slab_rat"));
        b.profession = Some(ProfessionId::new("solo"));
        assert_eq!(b.points_left(&c), 20);
        b.buy_stat(&c, Stat::Body).unwrap();
        assert_eq!((b.stats.body, b.points_left(&c)), (4, 19), "body is cheap twice: 1");
        assert_eq!(b.refund_stat(&c, Stat::Tech), Err(BuildError::AtBase(Stat::Tech)));
        for _ in 0..4 {
            b.buy_stat(&c, Stat::Body).unwrap();
        }
        assert_eq!(b.buy_stat(&c, Stat::Body), Err(BuildError::AtCap(Stat::Body)));
        b.stats.tech = 8;
        assert!(b.points_left(&c) < 0);
        assert_eq!(b.buy_stat(&c, Stat::Reflex), Err(BuildError::NoPoints));
        assert!(matches!(b.ready(&c, Step::Stats), Err(BuildError::OverPoints(_))));
    }

    #[test]
    fn changing_origin_can_overspend_a_template() {
        let c = Content::load().unwrap();
        let mut b = vex(&c);
        b.origin = Some(OriginId::new("stowaway"));
        assert!(b.points_left(&c) < 0);
        assert!(matches!(b.validate(&c), Err(BuildError::OverPoints(_))));
        assert!(b.validate(&c).unwrap_err().to_string().starts_with("over by "));
    }

    #[test]
    fn categories_gate_skills_and_refund_them() {
        let c = Content::load().unwrap();
        let mut b = Build::blank(&c);
        let net = CategoryId::new("netrunner");
        let ice = SkillId::new("ice_breaker");
        assert_eq!(b.toggle_skill(&c, &ice), Err(BuildError::CategoryLocked(ice.clone())));
        assert_eq!(b.toggle_category(&c, &net), Ok(true));
        assert!(b.skills.contains(&SkillId::new("hacking")), "the free first skill comes with it");
        assert_eq!(b.sp_left(&c), 4);
        assert_eq!(b.toggle_skill(&c, &ice), Ok(true));
        assert_eq!(b.sp_left(&c), 3);
        assert_eq!(b.skill_cost(&c, &SkillId::new("vault_run")), Some(2));
        assert_eq!(b.toggle_category(&c, &net), Ok(false));
        assert!(b.skills.is_empty() && b.sp_left(&c) == 6, "refunding the category drops its skills");
        b.toggle_category(&c, &CategoryId::new("wrench")).unwrap();
        b.toggle_category(&c, &CategoryId::new("iron")).unwrap();
        b.toggle_category(&c, &CategoryId::new("edge")).unwrap();
        assert_eq!(b.toggle_category(&c, &CategoryId::new("ghost")), Err(BuildError::NoSp));
        assert_eq!(b.toggle_skill(&c, &SkillId::new("scrapcraft")), Err(BuildError::NoSp), "0 left");
    }

    #[test]
    fn readiness_and_names() {
        let c = Content::load().unwrap();
        let mut b = Build::blank(&c);
        assert_eq!(b.ready(&c, Step::Name), Err(BuildError::NoName));
        b.handle = "   ".into();
        assert_eq!(b.ready(&c, Step::Name), Err(BuildError::NoName));
        b.handle = "a".repeat(17);
        assert_eq!(b.ready(&c, Step::Name), Err(BuildError::NameTooLong(16)));
        b.handle = "Vex\u{7}".into();
        assert_eq!(b.ready(&c, Step::Name), Err(BuildError::BadName('\u{7}')));
        b.handle = " Mara-7 ".into();
        assert_eq!(b.ready(&c, Step::Name), Ok(()));
        assert_eq!(b.ready(&c, Step::Origin), Err(BuildError::NoOrigin));
        assert_eq!(b.validate(&c), Err(BuildError::NoOrigin));
        b.origin = Some(OriginId::new("creche"));
        assert_eq!(b.ready(&c, Step::Profession), Err(BuildError::NoProfession));
        b.profession = Some(ProfessionId::new("fixer"));
        assert_eq!(b.ready(&c, Step::Skills), Ok(()), "unspent is fine");
        assert_eq!(b.validate(&c), Ok(()));
        assert_eq!(BuildError::NoName.to_string(), "no name on the sheet");
        assert_eq!(BuildError::NoOrigin.to_string(), "pick where you were made");
        assert_eq!(BuildError::NoProfession.to_string(), "pick what you did");
        assert_eq!(BuildError::OverSp(1).to_string(), "over by 1 SP");
    }
}
```

- [ ] **Step 2: Run to watch them fail**

Run: `cargo test -p game -- build::tests 2>&1 | grep -E "^error" | head -3`
Expected: `Build` not found.

- [ ] **Step 3: Implement `build.rs`**

```rust
//! A character in progress: the wizard's choices and the rules that price them.

use crate::content::Content;
use crate::model::*;
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step { Name, Origin, Profession, Stats, Skills, Summary }

impl Step {
    pub const ALL: [Step; 6] = [Step::Name, Step::Origin, Step::Profession, Step::Stats, Step::Skills, Step::Summary];
    pub fn label(self) -> &'static str { match self { Step::Name => "NAME", Step::Origin => "ORIGIN", Step::Profession => "PROFESSION", Step::Stats => "STATS", Step::Skills => "SKILLS", Step::Summary => "SUMMARY" } }
    pub fn tagline(self) -> &'static str { match self {
        Step::Name => "what they will call you",
        Step::Origin => "where you were made before the city got its hands on you",
        Step::Profession => "what you did before this",
        Step::Stats => "what the meat can do",
        Step::Skills => "what you know",
        Step::Summary => "the sheet",
    } }
    pub fn index(self) -> usize { Step::ALL.iter().position(|s| *s == self).unwrap_or(0) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Affinity { Cheap, Normal, Dear }

impl Affinity {
    pub fn label(self) -> &'static str { match self { Affinity::Cheap => "cheap", Affinity::Normal => "normal", Affinity::Dear => "dear" } }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    NoName, NameTooLong(usize), BadName(char), NoOrigin, NoProfession,
    AtCap(Stat), AtBase(Stat), OverPoints(u32), NoPoints, OverSp(u32), NoSp,
    CategoryLocked(SkillId), Unknown(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::NoName => write!(f, "no name on the sheet"),
            BuildError::NameTooLong(n) => write!(f, "a name is {n} characters at most"),
            BuildError::BadName(c) => write!(f, "the sheet will not take {c:?}"),
            BuildError::NoOrigin => write!(f, "pick where you were made"),
            BuildError::NoProfession => write!(f, "pick what you did"),
            BuildError::AtCap(s) => write!(f, "{} is maxed", s.name()),
            BuildError::AtBase(s) => write!(f, "nothing to refund on {}", s.name()),
            BuildError::OverPoints(n) => write!(f, "over by {n} point{}", if *n == 1 { "" } else { "s" }),
            BuildError::NoPoints => write!(f, "no points left"),
            BuildError::OverSp(n) => write!(f, "over by {n} SP"),
            BuildError::NoSp => write!(f, "no SP left"),
            BuildError::CategoryLocked(s) => write!(f, "unlock the category that holds {} first", s),
            BuildError::Unknown(id) => write!(f, "the content does not know {id}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Build {
    pub handle: String,
    pub origin: Option<OriginId>,
    pub profession: Option<ProfessionId>,
    pub stats: Stats,
    pub categories: BTreeSet<CategoryId>,
    pub skills: BTreeSet<SkillId>,
    pub talents: Vec<TalentId>,
    pub kit: Vec<(ItemId, u32)>,
    pub template: Option<TemplateId>,
}

const NAME_MARKS: &str = "-_.' ";

impl Build {
    pub fn blank(c: &Content) -> Build {
        let b = c.creation.stat_base;
        Build { handle: String::new(), origin: None, profession: None,
            stats: Stats { body: b, reflex: b, tech: b, cool: b, net: b },
            categories: BTreeSet::new(), skills: BTreeSet::new(), talents: Vec::new(), kit: Vec::new(), template: None }
    }

    pub fn from_template(c: &Content, t: &Template) -> Build {
        let mut b = Build::blank(c);
        b.handle = t.handle.clone();
        b.origin = Some(t.origin.clone());
        b.profession = Some(t.profession.clone());
        b.stats = t.stats.clone();
        b.categories = t.categories.iter().cloned().collect();
        b.skills = t.skills.iter().cloned().collect();
        b.talents = t.talents.clone();
        b.kit = t.kit.clone();
        b.template = Some(t.id.clone());
        b
    }

    fn shift(cheap: &[Stat], dear: &[Stat], s: Stat) -> i32 {
        if cheap.contains(&s) { -1 } else if dear.contains(&s) { 1 } else { 0 }
    }

    pub fn stat_cost(&self, c: &Content, s: Stat) -> u32 {
        let o = self.origin.as_ref().and_then(|id| c.origin(id)).map_or(0, |o| Self::shift(&o.cheap, &o.dear, s));
        let p = self.profession.as_ref().and_then(|id| c.profession(id)).map_or(0, |p| Self::shift(&p.cheap, &p.dear, s));
        (c.creation.point_cost as i32 + o + p).clamp(1, 3) as u32
    }

    pub fn affinity(&self, c: &Content, s: Stat) -> Affinity {
        match self.stat_cost(c, s) { 1 => Affinity::Cheap, 3 => Affinity::Dear, _ => Affinity::Normal }
    }

    pub fn points_spent(&self, c: &Content) -> u32 {
        Stat::ALL.iter().map(|&s| {
            let raised = self.stats.get(s).saturating_sub(c.creation.stat_base) as u32;
            raised * self.stat_cost(c, s)
        }).sum()
    }

    pub fn points_left(&self, c: &Content) -> i64 { c.creation.stat_points as i64 - self.points_spent(c) as i64 }

    fn stat_mut(&mut self, s: Stat) -> &mut u8 {
        match s { Stat::Body => &mut self.stats.body, Stat::Reflex => &mut self.stats.reflex, Stat::Tech => &mut self.stats.tech, Stat::Cool => &mut self.stats.cool, Stat::Net => &mut self.stats.net }
    }

    pub fn buy_stat(&mut self, c: &Content, s: Stat) -> Result<(), BuildError> {
        if self.stats.get(s) >= c.creation.stat_cap { return Err(BuildError::AtCap(s)); }
        if self.points_left(c) < self.stat_cost(c, s) as i64 { return Err(BuildError::NoPoints); }
        *self.stat_mut(s) += 1;
        Ok(())
    }

    pub fn refund_stat(&mut self, c: &Content, s: Stat) -> Result<(), BuildError> {
        if self.stats.get(s) <= c.creation.stat_base { return Err(BuildError::AtBase(s)); }
        *self.stat_mut(s) -= 1;
        Ok(())
    }

    pub fn sp_spent(&self, c: &Content) -> u32 {
        let cats: u32 = self.categories.iter().filter_map(|id| c.category(id)).map(|cat| cat.cost).sum();
        let skills: u32 = self.skills.iter().filter_map(|id| c.skill_cost(id)).sum();
        cats + skills
    }

    pub fn sp_left(&self, c: &Content) -> i64 { c.creation.sp as i64 - self.sp_spent(c) as i64 }

    /// The SP a skill costs, or `None` while its category is not owned.
    pub fn skill_cost(&self, c: &Content, id: &SkillId) -> Option<u32> {
        let cat = c.category_of(id)?;
        self.categories.contains(&cat.id).then(|| c.skill_cost(id))?
    }

    /// Buys an unowned category (its free skills come along) or refunds an owned one
    /// (its skills go with it). Returns whether it is owned afterwards.
    pub fn toggle_category(&mut self, c: &Content, id: &CategoryId) -> Result<bool, BuildError> {
        let cat = c.category(id).ok_or_else(|| BuildError::Unknown(id.to_string()))?;
        if self.categories.remove(id) {
            for s in &cat.skills { self.skills.remove(&s.id); }
            return Ok(false);
        }
        if self.sp_left(c) < cat.cost as i64 { return Err(BuildError::NoSp); }
        self.categories.insert(id.clone());
        for s in cat.skills.iter().filter(|s| s.cost == 0) { self.skills.insert(s.id.clone()); }
        Ok(true)
    }

    pub fn toggle_skill(&mut self, c: &Content, id: &SkillId) -> Result<bool, BuildError> {
        let cat = c.category_of(id).ok_or_else(|| BuildError::Unknown(id.to_string()))?;
        if !self.categories.contains(&cat.id) { return Err(BuildError::CategoryLocked(id.clone())); }
        if self.skills.remove(id) { return Ok(false); }
        let cost = c.skill_cost(id).unwrap_or(0);
        if self.sp_left(c) < cost as i64 { return Err(BuildError::NoSp); }
        self.skills.insert(id.clone());
        Ok(true)
    }

    fn check_name(&self, c: &Content) -> Result<(), BuildError> {
        let name = self.handle.trim();
        if name.is_empty() { return Err(BuildError::NoName); }
        if name.chars().count() > c.creation.name_max { return Err(BuildError::NameTooLong(c.creation.name_max)); }
        if let Some(bad) = name.chars().find(|ch| !(ch.is_alphanumeric() || NAME_MARKS.contains(*ch))) { return Err(BuildError::BadName(bad)); }
        Ok(())
    }

    fn check_pools(&self, c: &Content) -> Result<(), BuildError> {
        let left = self.points_left(c);
        if left < 0 { return Err(BuildError::OverPoints((-left) as u32)); }
        let sp = self.sp_left(c);
        if sp < 0 { return Err(BuildError::OverSp((-sp) as u32)); }
        Ok(())
    }

    pub fn ready(&self, c: &Content, step: Step) -> Result<(), BuildError> {
        match step {
            Step::Name => self.check_name(c),
            Step::Origin => self.origin.as_ref().map(|_| ()).ok_or(BuildError::NoOrigin),
            Step::Profession => self.profession.as_ref().map(|_| ()).ok_or(BuildError::NoProfession),
            Step::Stats | Step::Skills => self.check_pools(c),
            Step::Summary => self.validate(c),
        }
    }

    pub fn validate(&self, c: &Content) -> Result<(), BuildError> {
        self.check_name(c)?;
        let o = self.origin.as_ref().ok_or(BuildError::NoOrigin)?;
        c.origin(o).ok_or_else(|| BuildError::Unknown(o.to_string()))?;
        let p = self.profession.as_ref().ok_or(BuildError::NoProfession)?;
        c.profession(p).ok_or_else(|| BuildError::Unknown(p.to_string()))?;
        for s in Stat::ALL {
            if self.stats.get(s) > c.creation.stat_cap { return Err(BuildError::AtCap(s)); }
            if self.stats.get(s) < c.creation.stat_base { return Err(BuildError::AtBase(s)); }
        }
        for id in &self.categories { c.category(id).ok_or_else(|| BuildError::Unknown(id.to_string()))?; }
        for id in &self.skills {
            let cat = c.category_of(id).ok_or_else(|| BuildError::Unknown(id.to_string()))?;
            if !self.categories.contains(&cat.id) { return Err(BuildError::CategoryLocked(id.clone())); }
        }
        self.check_pools(c)
    }
}
```

The `validate` order matters for the tests: name, origin, profession, stats, categories, skills, pools. Keep `check_pools` reporting points before SP.

- [ ] **Step 4: Run the build tests**

Run: `cargo test -p game -- build::tests 2>&1 | grep -E "^test |panicked" | head -12`
Expected: six passes. If `templates_spend_exactly_their_pools` fails, the content's stats or costs are off; fix the content, not the rule.

- [ ] **Step 5: Write the failing character tests**

Replace the starter tests in `character.rs` and add:

```rust
    #[test]
    fn vex_from_template_matches_the_old_starter() {
        let c = Content::load().unwrap();
        let ch = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        assert_eq!(ch.handle, "VEX");
        assert_eq!((ch.level, ch.xp, ch.focus, ch.focus_max, ch.credits), (1, 0, 30, 30, 320));
        assert_eq!((ch.stats.body, ch.stats.reflex, ch.stats.tech, ch.stats.cool, ch.stats.net), (4, 6, 5, 5, 6));
        assert_eq!(ch.equipped(Slot::Primary), Some(&ItemId::new("ratchet_smg")));
        assert_eq!(ch.equipped(Slot::Torso), Some(&ItemId::new("salvage_jacket")));
        assert_eq!(ch.equipped(Slot::Eyes), Some(&ItemId::new("nightglass_optics")));
        assert_eq!(ch.equipped(Slot::Deck), Some(&ItemId::new("pawnshop_deck")));
        for (id, n) in [("copper_coil", 3), ("scrap_servo", 2), ("burner_shard", 1), ("soy_noodle_cup", 2), ("stim_patch", 1), ("bottle_caps", 12), ("dead_phone", 1)] {
            assert_eq!(ch.count(&ItemId::new(id)), n, "{id}");
        }
        assert!(ch.has_skill(&SkillId::new("hacking")) && ch.has_skill(&SkillId::new("ice_breaker")));
        assert!(!ch.has_skill(&SkillId::new("handguns")));
        assert!(ch.has_category(&CategoryId::new("wrench")) && ch.has_workbench(&c));
        assert_eq!((ch.stat_points, ch.sp), (0, 0));
        assert_eq!(ch.origin, OriginId::new("creche"));
        assert_eq!(ch.quests.len(), 3);
        assert_eq!(ch.hp, ch.hp_max(&c));
        let abilities = ch.abilities(&c);
        assert!(abilities.iter().any(|(id, s)| id == &AbilityId::new("street_sense") && *s == Source::Category(CategoryId::new("street"))));
        assert!(abilities.iter().any(|(id, _)| id == &AbilityId::new("signal_ghost")));
        assert!(abilities.iter().any(|(id, s)| id == &AbilityId::new("netscan") && matches!(s, Source::Item(_))));
    }

    #[test]
    fn kit_equips_first_fit_and_packs_the_rest() {
        let c = Content::load().unwrap();
        let mut b = Build::from_template(&c, c.template(&TemplateId::new("kessler")).unwrap());
        b.kit.push((ItemId::new("salvage_jacket"), 1)); // torso is taken by the weave
        let ch = Character::from_build(&c, &b);
        assert_eq!(ch.equipped(Slot::Torso), Some(&ItemId::new("ballistic_weave")));
        assert_eq!(ch.count(&ItemId::new("salvage_jacket")), 1, "second torso item is packed");
        assert!(!ch.has_workbench(&c));
        assert_eq!((ch.stat_points, ch.sp), (0, 0));
        let mut spare = Build::blank(&c);
        spare.handle = "Nix".into();
        spare.origin = Some(OriginId::new("vat_grown"));
        spare.profession = Some(ProfessionId::new("techie"));
        let ch = Character::from_build(&c, &spare);
        assert_eq!((ch.stat_points, ch.sp), (20, 6), "unspent pools ride along");
        assert_eq!(ch.credits, 300);
    }

    #[test]
    fn crafting_needs_the_skill_not_a_level() {
        let c = Content::load().unwrap();
        let mut ch = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        let shiv = c.recipe(&RecipeId::new("wire_shiv")).unwrap();
        assert!(ch.can_craft(&c, shiv));
        ch.skills.remove(&SkillId::new("electronics"));
        assert!(!ch.can_craft(&c, shiv));
    }
```

Existing character tests that call `from_starter` switch to `from_template(&c, &TemplateId::new("vex")).unwrap()`; tests asserting `skill_level` numbers are replaced by `has_skill` checks; the `action.rs` tests the same.

- [ ] **Step 6: Run them to watch them fail**

Run: `cargo test -p game -- character::tests 2>&1 | grep -E "^error" | head -3`
Expected: `from_template`, `has_skill` not found.

- [ ] **Step 7: The model swap**

`model.rs`: delete `Starter` and `fn one`; delete `Perk::Skill`; delete `level` from `Talent` and `Recipe`.

`content.rs`: delete the `starter` field and the whole starter validation block, `ContentError::{StarterLevel, ObjectiveCount, DuplicateStarter, TalentLevel}` (keep `ObjectiveCount` if reused for `creation.quests`; rename its message to *creation quest*). Remove the `Perk::Skill` arm in item validation. Add after the structural checks:

```rust
        for t in &self.templates {
            let b = crate::build::Build::from_template(self, t);
            b.validate(self).map_err(|e| ContentError::TemplateInvalid { id: t.id.0.clone(), why: e.to_string() })?;
        }
```

with `#[error("template `{id}`: {why}")] TemplateInvalid { id: String, why: String }`.

`character.rs`:

```rust
pub enum Source { Innate, Category(CategoryId), Item(ItemId) }

pub struct Character {
    pub handle: String, pub credits: u32, pub hp: u32, pub hp_base: u32, pub level: u8, pub xp: u32,
    pub focus: u32, pub focus_max: u32, pub stats: Stats,
    pub origin: OriginId, pub profession: ProfessionId,
    pub categories: BTreeSet<CategoryId>, pub skills: BTreeSet<SkillId>,
    pub talents: Vec<TalentId>, pub abilities: Vec<AbilityId>,
    pub equipment: BTreeMap<Slot, ItemId>, pub inventory: Vec<Stack>, pub quests: Vec<QuestLog>,
    pub fired: BTreeMap<AbilityId, u64>,
    /// Unspent creation points, spendable from the deck under the same rules.
    pub stat_points: u32, pub sp: u32,
}

impl Character {
    /// A playable character from a valid build; an invalid one still builds, with whatever it holds.
    pub fn from_build(c: &Content, b: &Build) -> Character {
        let cr = &c.creation;
        let mut ch = Character {
            handle: b.handle.trim().to_string(), credits: 0, hp: cr.hp, hp_base: cr.hp, level: 1, xp: 0,
            focus: cr.focus, focus_max: cr.focus, stats: b.stats.clone(),
            origin: b.origin.clone().unwrap_or_else(|| OriginId::new("")),
            profession: b.profession.clone().unwrap_or_else(|| ProfessionId::new("")),
            categories: b.categories.clone(), skills: b.skills.clone(), talents: b.talents.clone(),
            abilities: Vec::new(), equipment: BTreeMap::new(), inventory: Vec::new(),
            quests: cr.quests.iter().map(|(q, d)| QuestLog { quest: q.clone(), done: d.clone() }).collect(),
            fired: BTreeMap::new(),
            stat_points: b.points_left(c).max(0) as u32, sp: b.sp_left(c).max(0) as u32,
        };
        let mut kit: Vec<(ItemId, u32)> = Vec::new();
        if let Some(p) = b.profession.as_ref().and_then(|id| c.profession(id)) {
            ch.credits = p.credits;
            kit.extend(p.kit.iter().cloned());
        }
        kit.extend(b.kit.iter().cloned());
        for (id, qty) in kit {
            let Some(item) = c.item(&id) else { continue };
            let mut qty = qty;
            if let Some(slot) = item.slot && qty > 0 && !ch.equipment.contains_key(&slot) {
                ch.equipment.insert(slot, id.clone());
                qty -= 1;
            }
            ch.give(id, qty);
        }
        ch.hp = ch.hp_max(c);
        ch
    }

    pub fn from_template(c: &Content, id: &TemplateId) -> Option<Character> {
        Some(Self::from_build(c, &Build::from_template(c, c.template(id)?)))
    }

    pub fn has_skill(&self, id: &SkillId) -> bool { self.skills.contains(id) }
    pub fn has_category(&self, id: &CategoryId) -> bool { self.categories.contains(id) }
    pub fn has_workbench(&self, c: &Content) -> bool {
        self.categories.iter().filter_map(|id| c.category(id)).any(|cat| cat.unlocks.contains(&Unlock::Workbench))
    }
```

`abilities()`: innate, then for each owned category in content order its `grants` with `Source::Category`, then items. `can_craft`: `self.has_skill(&r.skill) && inputs`. Delete `skill_base`, `skill_level`.

`flavor/content.ron`: delete `starter`, delete every `level:` in talents and recipes, `shiv_blade` perk → `Stat(Reflex, 1)`, `pawnshop_deck` perk → `Stat(Net, 1)`, remove `street_sense` from nowhere (it was only in the starter).

- [ ] **Step 8: Adapt the app crate to compile**

- `app.rs`: `let character = game::Character::from_template(&content, &game::TemplateId::new("vex")).expect("vex template");` and add `pub build: Option<game::Build>` (init `None`).
- `deck.rs`: `Detail::Skill(usize)` → `Detail::Stat(Stat)`; `skill_rows` becomes `stat_rows` (one row per `Stat::ALL`: `format!("{:<8} {:>2}  {bar}", s.name(), v)` with a ten-cell bar; detail: blurb, the skills that use it); `ability_rows` stays for now as the SKILLS tab (Task 3 replaces it with the board); the recipe detail says `needs {skill}` and whether you hold it; drop the `Perk::Skill` arm in `perk_lines`; the talent lines under a stat go away (talents are a later spec). Keep the `Tab` labels; rename `Tab::Attributes` to `Tab::Stats` with label `STATS`.
- `render/items.rs`, `ui/panel.rs`, `screens/*.rs` tests: `from_starter` → `from_template`.

- [ ] **Step 9: Suite, lint, commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace 2>&1 | grep -E "^test result|FAILED"`
Expected: green. Fix any deck test that pinned `▌ATTRIBUTES▐` or skill bars by asserting the STATS tab instead. Commit `feat(game): builds, templates and the character swap`.

---

### Task 3: Board, meter, and the deck's new tabs

**Files:**
- Create: `crates/strata-zero/src/ui/board.rs`, `crates/strata-zero/src/ui/meter.rs`
- Modify: `crates/strata-zero/src/ui/mod.rs`, `crates/strata-zero/src/screens/deck.rs`, `crates/strata-zero/src/screens/console.rs`
- Test: each file's tests

**Interfaces:**
- Produces: `ui::board::{BoardRow, RowKind, board_rows}`; `ui::meter::meter(label: &str, used: u32, total: u32, colour: Rgb, width: u16) -> Line`; deck `Detail::{Category(CategoryId), Skill(SkillId)}`; `Deck.open: BTreeSet<CategoryId>`; console `sp`, `pts`.

```rust
pub enum RowKind { Category(CategoryId), Skill(SkillId), Note }
pub struct BoardRow { pub line: Line<'static>, pub kind: RowKind, pub selectable: bool }
/// One row per category; an open category is followed by its skills with connectors
/// and a note row for what it grants or unlocks.
pub fn board_rows(c: &Content, owned_cats: &BTreeSet<CategoryId>, owned_skills: &BTreeSet<SkillId>,
    sp_left: i64, open: &BTreeSet<CategoryId>, tick: u64, flash: Option<(usize, u64)>) -> Vec<BoardRow>
```

Glyph rules: owned `◆` cyan; affordable `◇` text; unaffordable or locked `◇` dim; category cost text `owned` / `{n} SP` / `first skill free` note; skill cost `free` / `{n} SP`; connectors `├─` and `└─`; notes `⚡ {ability name}` and `⚙ unlocks the workbench`; a row whose index is in `flash` and `tick < until` draws inverted.

- [ ] **Step 1: Failing board and meter tests** — rows for a blank build (eight folded rows, every `◇`), Vex's board (owned `◆`, open categories unfolded with `├─`/`└─`, the `⚡ Signal ghost` note under NETRUNNER), `sp_left = 0` makes every unowned row dim, and `meter("SP", 4, 6, ..)` renders `SP  ▮▮▮▮▯▯  2 left`.
- [ ] **Step 2: Watch them fail.** Run `cargo test -p strata-zero -- ui::board ui::meter`.
- [ ] **Step 3: Implement `board.rs` and `meter.rs`**, register in `ui/mod.rs`.
- [ ] **Step 4: Deck SKILLS tab** — `rows()` for `Tab::Skills` maps `board_rows` to `Row`s with `Detail::Category`/`Detail::Skill` and `detail: None` for notes; `Deck` gains `open: BTreeSet<CategoryId>` initialised to the owned categories; `→` opens, `←` folds the selected category (or the one holding the selected skill); `space` on a category or skill runs the same rules as the wizard against the character's pools: build a `Build` from the character (`Build::from_character(c, &ch)` added in `game::build`, pools set from the character), toggle, and write back `categories`, `skills`, `sp` (refunds allowed only for things bought this session? No: refunds are not allowed from the deck; `space` on an owned row does nothing and says *that is yours now*). Detail pane: category line, cost, skills with owned marks, grants; skill desc, cost, category.
- [ ] **Step 5: Deck STATS tab** — rows per stat with a cost tail `next {n} pts` when `stat_points > 0`; header rows `points {n} · sp {n}` dim; `space` buys when affordable (same `Build::from_character` path, writes `stats` and `stat_points` back), says *no points left* otherwise.
- [ ] **Step 6: WORKBENCH lock** — when `!has_workbench`, `workbench_rows` returns `[dim("no bench. WRENCH unlocks it.")]` and `interact` ignores it.
- [ ] **Step 7: Console** — `sp <n>` and `pts <n>` add to the pools; help text lists them.
- [ ] **Step 8: Deck tests** — STATS tab renders five bars and spends a point when `pts 3` was given; SKILLS tab folds and unfolds, buys WRENCH with `sp 2`, refuses with none; WORKBENCH locked line for a character without WRENCH and unlocked rows for Vex; Review Focus 5.
- [ ] **Step 9: Suite, lint, commit** `feat(deck): stats tab and the folding skills board`.

---

### Task 4: The wizard: frame, chooser, NAME, ORIGIN, PROFESSION, SUMMARY

**Files:**
- Create: `crates/strata-zero/src/screens/create/{mod,chooser,name,origin,profession,summary}.rs` (STATS and SKILLS steps are stubs that pass through until Task 5)
- Modify: `crates/strata-zero/src/screens/mod.rs`, `screens/title.rs`, `app.rs`
- Test: `screens/create/mod.rs` tests

**Interfaces:**
- Produces: `screens::create::{Create, Chooser}`; `Create::new(st, step, tick)`; frame helpers `frame::draw(f, st, step, build, tick) -> Panes { left, right, footer }`.

Design of `mod.rs`:

```rust
pub struct Create {
    step: Step,
    opened: u64,
    cursor: [usize; 6],
    name: Form,            // one Field::Text, max = creation.name_max
    open: BTreeSet<CategoryId>,
    flash: Option<(usize, u64)>,
    notice: Option<(String, u64)>,   // footer reason, shown until the tick
}
```

`update`: `esc` → previous step, or on NAME `ScreenCmd::Pop` back to the chooser; `enter` → `build.ready(step)`; on `Err(e)` set `notice = (e.to_string(), tick + 90)` and stay; on SUMMARY `enter` validates, builds the character, clears `st.build`, and returns `Switch` to `Generating::new(Title::fresh_params(st, st.tick), st.tick)`. Other keys go to the step module's `handle(&mut self, st, key)`.

`draw`: strip, caption (block font when `area.height >= 36`, bold line otherwise), tagline, heavy rules, then the step's `draw_left`/`draw_right`, footer with keys and `step N of 6` and the notice in magenta when live.

Chooser: a `Menu` with `VEX`, `KESSLER`, `CUSTOM BUILD` and their lines; `enter` sets `st.build` (`Build::from_template` or `Build::blank`) and `Switch`es to `Create` at SUMMARY or NAME with the glitch; `esc` pops.

Title: NEW GAME → `Push { Chooser, SlideUp }`.

- [ ] **Step 1: Failing screen tests** — chooser lists three rows; `enter` on VEX leaves `st.build` filled and the next screen on SUMMARY; `esc` on the chooser pops; at 120×40 the ORIGIN step shows the strip with `▌2 ORIGIN▐`, a block-font row (contains `▀` or `▄`), the tagline, four origins and the DOSSIER box with `AFFINITY`; at 100×30 no block glyphs and still the strip; `enter` on an empty NAME shows *no name on the sheet* in the footer and stays; typing `Mara` then `enter` reaches ORIGIN; `esc` on NAME pops; SUMMARY for Vex contains `JACK IN`, `VEX`, `CRÈCHE`, `NETRUNNER`; `enter` there yields a `Switch` and `st.character.handle == "VEX"` with `st.build == None`; Review Focus 4 (a name of spaces is refused on screen).
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement the frame, chooser, NAME, ORIGIN, PROFESSION and SUMMARY**, with STATS and SKILLS as pass-through stubs showing *coming in the next task* in the left pane (removed in Task 5).
- [ ] **Step 4: Title wiring**, help text for the chooser and wizard keys.
- [ ] **Step 5: Suite, lint, commit** `feat(create): the chooser and the wizard frame with name, origin, profession and summary`.

---

### Task 5: STATS and SKILLS steps

**Files:**
- Create: `screens/create/stats.rs`, `screens/create/skills.rs` (replacing the stubs)
- Test: `screens/create/mod.rs` tests

- [ ] **Step 1: Failing tests** — STATS at 120×40 shows `POINTS`, five stat rows with `next`, `cheap` on a cheap row for Vex; `→` on REFLEX raises it and the meter; `←` refunds; at cap `→` shows *REFLEX is maxed*; changing origin on a loaded Vex then visiting STATS shows `over by` in the meter and SUMMARY `enter` refuses with *over by N points* (Review Focus 1); SKILLS shows the board with `SP`, `→` unfolds NETRUNNER, `space` buys WRENCH and the meter drops, `space` on an owned category refunds it and its skills.
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement** both steps on `Build`'s rules, the pulse on the selected bar, the six-tick flash on a bought segment or row, and the dossier text for stats (blurb, skills it feeds, origin and profession shifts, resulting cost) and for categories and skills.
- [ ] **Step 4: Suite, lint, commit** `feat(create): stats and skills steps`.

---

### Task 6: Docs, smoke and screenshots

- [ ] **Step 1: README** — Title (chooser), Create section with keys, deck tabs list, dev console commands. Help screen entries.
- [ ] **Step 2: Smoke `drive10.py`** — title → NEW GAME → chooser → CUSTOM → type a name → origin → profession → buy two stat points → unlock WRENCH → summary → JACK IN → generating → walk; then a second run picking VEX and jacking in from SUMMARY. Expect strings: `NAME`, `DOSSIER`, `POINTS`, `SP`, `JACK IN`, `GENERATING`, `on foot`, and the deck's `▌STATS▐` and `▌SKILLS▐`.
- [ ] **Step 3: Screenshots** of ORIGIN, STATS, SKILLS and SUMMARY at 120×40 via the capture pipeline; send them.
- [ ] **Step 4: Commit** `docs: character creation keys and smoke`.
