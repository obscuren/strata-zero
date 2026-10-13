# Game Menus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the game its own interface over the live map: a pause menu on Esc, settings, the deck (quests, gear, inventory, workbench, map, skills) on Tab backed by a real content model and starter kit, and a glitch flash when jacking in or out.

**Architecture:** A new terminal-free `game` crate holds the content model (items, skills, talents, quests, recipes, starter kit) parsed from `flavor/content.ron` and the `Character` built from it. The app runner becomes a screen stack: menus `Push` over the map, which keeps drawing underneath through a scanline wash and stops updating; `Pop` returns; `Flash` runs a transition in place. Pause, Settings and Deck are ordinary `Screen`s.

**Tech Stack:** Rust 2024, ratatui 0.29, crossterm 0.28, serde + ron (content), thiserror. Existing `anim` (pulse, typewriter, glitch_cells, Tween), `ui::logo`, `ui::menu`, `ui::panel::boxed`, `walker::Player::vis`.

**Spec:** `docs/superpowers/specs/2026-10-13-game-menus-design.md`

## Global Constraints

- Every glyph is exactly one terminal cell wide; new glyphs (`▰ ▱ ☑ ☐ ✓ ▮ ◍ ┍ ┑ ┕ ┙`) join a width test.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` green after every task.
- The `game` crate depends only on `serde`, `ron`, `thiserror`; no terminal dependencies.
- Content names are original (no trademarked cyberpunk brands); the starter kit covers every `ItemKind` and every `Slot` at least once, 8 skills, 8 talents, 4 quests, 6 recipes with at least one craftable and one not.
- Keys on the map: Esc and `q` open the pause menu; Tab opens the deck; `c` is the dev camera (dev mode only); `s` pushes the save screen; Enter jacks in/out and flashes. Title Esc still quits; Configure/Files Esc go back (`Pop`).
- The character is not saved; `.city` stays format 2.
- Palette: cyan `(0, 229, 255)`, magenta `(255, 0, 200)`, dim `(120, 130, 145)`, text `(205, 212, 222)`, selected-row background `(0, 60, 70)`.

## Review Focus

1. Esc while the help overlay is open must close the overlay only, never open the pause menu on top of it. Test in Task 6 (`esc_closes_help_before_pausing`).
2. The deck at the game's minimum size (100x30) must still show the tab bar, both panes and the hint row without clipping into the frame. Test in Task 7 (`deck_fits_the_minimum_terminal`).
3. A `Pop` on a one-screen stack must be ignored, never leave the app without a screen. Test in Task 3 (`pop_on_last_screen_is_ignored_and_switch_clears`).
4. A `Flash` followed by a draw at a different terminal size must not index the stale snapshot out of bounds. Test in Task 3 (`flash_runs_a_transition_in_place`).
5. Opening the deck's Map tab before the player exists must show "no signal", not panic. Test in Task 8 (`map_tab_shows_memory_or_no_signal`).

---

## File structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | workspace member `crates/game` |
| `crates/game/Cargo.toml`, `src/lib.rs` | the game model crate |
| `crates/game/src/model.rs` | ids, enums, content structs, `Starter` |
| `crates/game/src/content.rs` | `Content`: parse, validate, lookups |
| `crates/game/src/character.rs` | `Character` from the starter kit, helpers |
| `flavor/content.ron` | the starter kit |
| `crates/cyberpunk/Cargo.toml` | depends on `game` |
| `crates/cyberpunk/src/app.rs` | `ScreenCmd::{Push, Pop, Flash}`, screen stack, `AppState` fields |
| `crates/cyberpunk/src/ui/dim.rs` | **new**: scanline wash |
| `crates/cyberpunk/src/ui/transition.rs` | `Transition::flash` |
| `crates/cyberpunk/src/screens/files.rs` | returns with `Pop`; no `back` hack |
| `crates/cyberpunk/src/screens/title.rs` | pushes the load screen |
| `crates/cyberpunk/src/screens/settings.rs` | **new**: dev / rain / fps |
| `crates/cyberpunk/src/screens/pause.rs` | **new**: pause menu |
| `crates/cyberpunk/src/screens/deck.rs` | **new**: the deck |
| `crates/cyberpunk/src/render/memmap.rs` | **new**: memory map widget |
| `crates/cyberpunk/src/screens/explore.rs` | keys, overlays by setting, fps option, flash on jack |
| `crates/cyberpunk/src/ui/panel.rs` | `status_bar(fps: Option<f32>)` |
| `crates/cyberpunk/src/screens/help.rs`, `README.md` | keys |

---

### Task 1: The `game` crate: model, content file, validation

**Files:**
- Modify: `Cargo.toml` (workspace members)
- Create: `crates/game/Cargo.toml`, `crates/game/src/lib.rs`, `crates/game/src/model.rs`, `crates/game/src/content.rs`, `flavor/content.ron`
- Test: `crates/game/src/content.rs` (tests module)

**Interfaces:**
- Produces: `game::{ItemId, SkillId, TalentId, QuestId, RecipeId}` (newtype `String`, `serde(transparent)`, `new(&str)`, `Display`), `Slot::{ALL, label}`, `ItemKind::{ALL, label}`, `Stat::label`, `Item`, `Skill`, `Talent`, `Quest`, `Recipe`, `Stats`, `Starter`, `Content::{load, parse, validate, item, skill, talent, quest, recipe, talents_for}`, `ContentError`.

- [ ] **Step 1: Write the failing tests**

Create `crates/game/src/content.rs` with only a tests module for now (the implementation comes in Step 3):

```rust
//! Content: everything the world is made of, parsed from `flavor/content.ron` and validated.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn embedded_content_parses_and_covers_the_kit() {
        let c = Content::load().unwrap();
        assert!(c.items.len() >= 14, "{} items", c.items.len());
        assert_eq!(c.skills.len(), 8);
        assert!(c.talents.len() >= 8);
        assert!(c.quests.len() >= 4);
        assert!(c.recipes.len() >= 6);
        for kind in ItemKind::ALL {
            assert!(c.items.iter().any(|i| i.kind == kind), "no {kind:?} item");
        }
        for slot in Slot::ALL {
            assert!(c.items.iter().any(|i| i.slot == Some(slot)), "no item for {slot:?}");
        }
        let s = &c.starter;
        assert_eq!(s.handle, "Vex");
        assert!((2..=4).contains(&s.equipped.len()));
        assert!(s.equipped.len() < Slot::ALL.len(), "at least one slot stays empty");
        assert!(s.quests.iter().any(|(_, d)| d.iter().all(|x| *x)), "one quest is done");
        assert!(s.quests.iter().any(|(_, d)| d.iter().any(|x| !*x)), "one quest is active");
        assert_eq!(c.item(&ItemId::new("copper_coil")).unwrap().kind, ItemKind::Component);
        assert_eq!(c.talents_for(&SkillId::new("hacking")).count(), 2);
    }

    #[test]
    fn validation_catches_broken_content() {
        let good = Content::load().unwrap();

        let mut c = good.clone();
        c.recipes[0].inputs[0].0 = ItemId::new("unobtainium");
        assert!(matches!(c.validate(), Err(ContentError::UnknownItem { .. })));

        let mut c = good.clone();
        c.starter.equipped[0].0 = Slot::Legs; // the first equipped item is not legwear
        let first = c.starter.equipped[0].1.clone();
        assert_ne!(c.item(&first).unwrap().slot, Some(Slot::Legs));
        assert!(matches!(c.validate(), Err(ContentError::SlotMismatch { .. })));

        let mut c = good.clone();
        let dup = c.items[0].clone();
        c.items.push(dup);
        assert!(matches!(c.validate(), Err(ContentError::DuplicateId(_))));

        let mut c = good.clone();
        c.starter.quests[0].1.push(true);
        assert!(matches!(c.validate(), Err(ContentError::ObjectiveCount { .. })));

        let mut c = good.clone();
        c.talents[0].skill = SkillId::new("origami");
        assert!(matches!(c.validate(), Err(ContentError::UnknownSkill { .. })));

        let mut c = good.clone();
        c.starter.talents.push(TalentId::new("levitation"));
        assert!(matches!(c.validate(), Err(ContentError::UnknownTalent { .. })));

        let mut c = good.clone();
        c.starter.quests[0].0 = QuestId::new("the_grail");
        assert!(matches!(c.validate(), Err(ContentError::UnknownQuest { .. })));

        assert!(matches!(Content::parse("not ron"), Err(ContentError::Parse(_))));
    }
}
```

Create `crates/game/Cargo.toml`:

```toml
[package]
name = "game"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
ron.workspace = true
thiserror.workspace = true
```

Create `crates/game/src/lib.rs`:

```rust
//! The game model: content (items, skills, talents, quests, recipes) and the character.

pub mod content;
pub mod model;

pub use content::*;
pub use model::*;
```

Add `"crates/game"` to `members` in the workspace `Cargo.toml`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p game`
Expected: compile error (`unresolved import crate::model` / `Content` not found).

- [ ] **Step 3: Implement the model**

Create `crates/game/src/model.rs`:

```rust
//! Ids, enums and the content structs.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(s: &str) -> Self {
                $name(s.to_string())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_type!(ItemId);
id_type!(SkillId);
id_type!(TalentId);
id_type!(QuestId);
id_type!(RecipeId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Slot {
    Head,
    Eyes,
    Neural,
    Arms,
    Torso,
    Legs,
    Primary,
    Secondary,
    Deck,
}

impl Slot {
    pub const ALL: [Slot; 9] = [
        Slot::Head,
        Slot::Eyes,
        Slot::Neural,
        Slot::Arms,
        Slot::Torso,
        Slot::Legs,
        Slot::Primary,
        Slot::Secondary,
        Slot::Deck,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Slot::Head => "head",
            Slot::Eyes => "eyes",
            Slot::Neural => "neural",
            Slot::Arms => "arms",
            Slot::Torso => "torso",
            Slot::Legs => "legs",
            Slot::Primary => "primary",
            Slot::Secondary => "secondary",
            Slot::Deck => "deck",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ItemKind {
    Weapon,
    Armor,
    Cyberware,
    Deck,
    Consumable,
    Component,
    Junk,
}

impl ItemKind {
    pub const ALL: [ItemKind; 7] = [
        ItemKind::Weapon,
        ItemKind::Armor,
        ItemKind::Cyberware,
        ItemKind::Deck,
        ItemKind::Consumable,
        ItemKind::Component,
        ItemKind::Junk,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Weapon => "weapon",
            ItemKind::Armor => "armor",
            ItemKind::Cyberware => "cyberware",
            ItemKind::Deck => "deck",
            ItemKind::Consumable => "consumable",
            ItemKind::Component => "component",
            ItemKind::Junk => "junk",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stat {
    Body,
    Reflex,
    Tech,
    Cool,
    Net,
}

impl Stat {
    pub fn label(self) -> &'static str {
        match self {
            Stat::Body => "body",
            Stat::Reflex => "reflex",
            Stat::Tech => "tech",
            Stat::Cool => "cool",
            Stat::Net => "net",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub name: String,
    pub kind: ItemKind,
    #[serde(default)]
    pub slot: Option<Slot>,
    pub weight: f32,
    pub value: u32,
    pub desc: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    pub id: SkillId,
    pub name: String,
    pub stat: Stat,
    pub desc: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Talent {
    pub id: TalentId,
    pub name: String,
    pub skill: SkillId,
    pub level: u8,
    pub desc: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quest {
    pub id: QuestId,
    pub title: String,
    pub giver: String,
    pub location: String,
    pub desc: String,
    pub objectives: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub id: RecipeId,
    pub name: String,
    pub skill: SkillId,
    pub level: u8,
    pub inputs: Vec<(ItemId, u32)>,
    pub output: (ItemId, u32),
    pub desc: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub body: u8,
    pub reflex: u8,
    pub tech: u8,
    pub cool: u8,
    pub net: u8,
}

/// What a new character starts with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Starter {
    pub handle: String,
    pub credits: u32,
    pub hp: u32,
    pub stats: Stats,
    pub inventory: Vec<(ItemId, u32)>,
    pub equipped: Vec<(Slot, ItemId)>,
    pub skills: Vec<(SkillId, u8)>,
    pub talents: Vec<TalentId>,
    pub quests: Vec<(QuestId, Vec<bool>)>,
}
```

Implementation above the tests in `crates/game/src/content.rs`:

```rust
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

const DEFAULT_CONTENT: &str = include_str!("../../../flavor/content.ron");

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ContentError {
    #[error("content parse error: {0}")]
    Parse(String),
    #[error("{where_}: unknown item `{id}`")]
    UnknownItem { where_: String, id: String },
    #[error("{where_}: unknown skill `{id}`")]
    UnknownSkill { where_: String, id: String },
    #[error("{where_}: unknown talent `{id}`")]
    UnknownTalent { where_: String, id: String },
    #[error("{where_}: unknown quest `{id}`")]
    UnknownQuest { where_: String, id: String },
    #[error("item `{item}` cannot sit in slot {slot:?}")]
    SlotMismatch { item: String, slot: Slot },
    #[error("duplicate id `{0}`")]
    DuplicateId(String),
    #[error("quest `{quest}`: starter flags do not match its objectives")]
    ObjectiveCount { quest: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Content {
    pub items: Vec<Item>,
    pub skills: Vec<Skill>,
    pub talents: Vec<Talent>,
    pub quests: Vec<Quest>,
    pub recipes: Vec<Recipe>,
    pub starter: Starter,
}

impl Content {
    /// Parses the embedded `flavor/content.ron` and validates it.
    pub fn load() -> Result<Content, ContentError> {
        Self::parse(DEFAULT_CONTENT)
    }

    pub fn parse(text: &str) -> Result<Content, ContentError> {
        let c: Content = ron::from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        c.validate()?;
        Ok(c)
    }

    pub fn validate(&self) -> Result<(), ContentError> {
        let mut seen: HashSet<&str> = HashSet::new();
        let ids = self
            .items
            .iter()
            .map(|i| i.id.0.as_str())
            .chain(self.skills.iter().map(|s| s.id.0.as_str()))
            .chain(self.talents.iter().map(|t| t.id.0.as_str()))
            .chain(self.quests.iter().map(|q| q.id.0.as_str()))
            .chain(self.recipes.iter().map(|r| r.id.0.as_str()));
        for id in ids {
            if !seen.insert(id) {
                return Err(ContentError::DuplicateId(id.to_string()));
            }
        }
        let need_item = |where_: String, id: &ItemId| -> Result<&Item, ContentError> {
            self.item(id).ok_or_else(|| ContentError::UnknownItem {
                where_,
                id: id.0.clone(),
            })
        };
        let need_skill = |where_: String, id: &SkillId| -> Result<&Skill, ContentError> {
            self.skill(id).ok_or_else(|| ContentError::UnknownSkill {
                where_,
                id: id.0.clone(),
            })
        };
        for t in &self.talents {
            need_skill(format!("talent {}", t.id), &t.skill)?;
        }
        for r in &self.recipes {
            need_skill(format!("recipe {}", r.id), &r.skill)?;
            for (id, _) in &r.inputs {
                need_item(format!("recipe {}", r.id), id)?;
            }
            need_item(format!("recipe {} output", r.id), &r.output.0)?;
        }
        let s = &self.starter;
        for (id, _) in &s.inventory {
            need_item("starter inventory".into(), id)?;
        }
        for (slot, id) in &s.equipped {
            let item = need_item("starter equipment".into(), id)?;
            if item.slot != Some(*slot) {
                return Err(ContentError::SlotMismatch {
                    item: id.0.clone(),
                    slot: *slot,
                });
            }
        }
        for (id, _) in &s.skills {
            need_skill("starter skills".into(), id)?;
        }
        for id in &s.talents {
            if self.talent(id).is_none() {
                return Err(ContentError::UnknownTalent {
                    where_: "starter talents".into(),
                    id: id.0.clone(),
                });
            }
        }
        for (id, flags) in &s.quests {
            let q = self.quest(id).ok_or_else(|| ContentError::UnknownQuest {
                where_: "starter quests".into(),
                id: id.0.clone(),
            })?;
            if flags.len() != q.objectives.len() {
                return Err(ContentError::ObjectiveCount {
                    quest: id.0.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn item(&self, id: &ItemId) -> Option<&Item> {
        self.items.iter().find(|i| &i.id == id)
    }

    pub fn skill(&self, id: &SkillId) -> Option<&Skill> {
        self.skills.iter().find(|s| &s.id == id)
    }

    pub fn talent(&self, id: &TalentId) -> Option<&Talent> {
        self.talents.iter().find(|t| &t.id == id)
    }

    pub fn quest(&self, id: &QuestId) -> Option<&Quest> {
        self.quests.iter().find(|q| &q.id == id)
    }

    pub fn recipe(&self, id: &RecipeId) -> Option<&Recipe> {
        self.recipes.iter().find(|r| &r.id == id)
    }

    /// Talents of one skill, in content order.
    pub fn talents_for<'a>(&'a self, skill: &'a SkillId) -> impl Iterator<Item = &'a Talent> + 'a {
        self.talents.iter().filter(move |t| &t.skill == skill)
    }
}
```

Create `flavor/content.ron`:

```ron
// Starter kit: original names in the shared cyberpunk register.
// Every ItemKind and every Slot appears at least once. Ids are snake_case strings.
Content(
    items: [
        Item(id: "ratchet_smg", name: "Ratchet SMG", kind: Weapon, slot: Some(Primary), weight: 3.2, value: 900,
             desc: "Stamped-receiver machine pistol with a filed-down selector. Jams on cheap ammo, sings on the good stuff."),
        Item(id: "shiv_blade", name: "Monowire shiv", kind: Weapon, slot: Some(Secondary), weight: 0.4, value: 350,
             desc: "A hand-wrapped grip around twenty centimetres of monofilament. Do not test the edge with a thumb."),
        Item(id: "hardhat", name: "Hardhat with visor", kind: Armor, slot: Some(Head), weight: 0.9, value: 60,
             desc: "Dock-issue hardhat. The visor still has the union sticker."),
        Item(id: "salvage_jacket", name: "Salvage jacket", kind: Armor, slot: Some(Torso), weight: 2.5, value: 420,
             desc: "Aramid panels sewn into a courier jacket. Stops a knife, argues with a pistol."),
        Item(id: "cargo_pants", name: "Cargo pants", kind: Armor, slot: Some(Legs), weight: 1.1, value: 90,
             desc: "Eleven pockets. You have lost count of what is in them."),
        Item(id: "nightglass_optics", name: "Nightglass optics", kind: Cyberware, slot: Some(Eyes), weight: 0.2, value: 2200,
             desc: "Low-light lenses pulled from a security contractor's surplus. The left one still shows a corporate boot logo on cold start."),
        Item(id: "neural_port", name: "Neural port", kind: Cyberware, slot: Some(Neural), weight: 0.1, value: 1500,
             desc: "Standard jack behind the right ear. The seal itches when it rains."),
        Item(id: "grip_arm", name: "Grip arm", kind: Cyberware, slot: Some(Arms), weight: 4.0, value: 3100,
             desc: "Hydraulic forearm with a clamp hand. Built for cargo, fine for doors."),
        Item(id: "pawnshop_deck", name: "Pawnshop deck", kind: Deck, slot: Some(Deck), weight: 1.8, value: 1200,
             desc: "Three previous owners, one of them dead. Boots in four seconds and runs hot."),
        Item(id: "soy_noodle_cup", name: "Soy-noodle cup", kind: Consumable, weight: 0.3, value: 8,
             desc: "Just add water. Water sold separately."),
        Item(id: "stim_patch", name: "Stim patch", kind: Consumable, weight: 0.05, value: 45,
             desc: "Twelve minutes of clarity, then the bill."),
        Item(id: "scrap_servo", name: "Scrap servo", kind: Component, weight: 0.6, value: 30,
             desc: "Pulled from a vending arm. Still answers to the right voltage."),
        Item(id: "copper_coil", name: "Copper coil", kind: Component, weight: 0.2, value: 12,
             desc: "A spool of salvaged wire. Every fix starts here."),
        Item(id: "burner_shard", name: "Burner shard", kind: Component, weight: 0.05, value: 25,
             desc: "A memory shard with one write cycle left."),
        Item(id: "bottle_caps", name: "Bottle caps", kind: Junk, weight: 0.1, value: 1,
             desc: "Someone in the Undercity collects these. You have not met them."),
        Item(id: "dead_phone", name: "Dead phone", kind: Junk, weight: 0.2, value: 5,
             desc: "Cracked screen, swollen battery, somebody's whole life inside."),
    ],
    skills: [
        Skill(id: "blades", name: "Blades", stat: Reflex, desc: "Knives, shivs and anything with an edge."),
        Skill(id: "handguns", name: "Handguns", stat: Reflex, desc: "Pistols and machine pistols at street range."),
        Skill(id: "stealth", name: "Stealth", stat: Cool, desc: "Not being where the cameras are looking."),
        Skill(id: "hacking", name: "Hacking", stat: Net, desc: "Getting into systems from the inside of the Net."),
        Skill(id: "electronics", name: "Electronics", stat: Tech, desc: "Building and fixing hardware at the bench."),
        Skill(id: "streetwise", name: "Streetwise", stat: Cool, desc: "Who sells what, who owes whom, which alley to skip."),
        Skill(id: "athletics", name: "Athletics", stat: Body, desc: "Running, climbing and taking a fall."),
        Skill(id: "medtech", name: "Medtech", stat: Tech, desc: "Patches, stims and keeping blood inside people."),
    ],
    talents: [
        Talent(id: "quick_draw", name: "Quick draw", skill: "handguns", level: 2, desc: "First shot comes out before the argument ends."),
        Talent(id: "double_tap", name: "Double tap", skill: "handguns", level: 4, desc: "Two rounds for the price of one trigger pull."),
        Talent(id: "backstab", name: "Backstab", skill: "blades", level: 3, desc: "From behind, the first cut is the only cut."),
        Talent(id: "ghost", name: "Ghost", skill: "stealth", level: 3, desc: "Cameras see a smear where you were."),
        Talent(id: "ice_pick", name: "Ice pick", skill: "hacking", level: 2, desc: "Gates open a little faster for you."),
        Talent(id: "deep_dive", name: "Deep dive", skill: "hacking", level: 4, desc: "Vaults give up their contents without the alarm."),
        Talent(id: "solder_hand", name: "Solder hand", skill: "electronics", level: 2, desc: "Recipes need one fewer component."),
        Talent(id: "field_medic", name: "Field medic", skill: "medtech", level: 2, desc: "Patches work on the move."),
    ],
    quests: [
        Quest(id: "the_drop", title: "The drop", giver: "Mama Sol", location: "Night market",
              desc: "A package needs to cross three districts without being scanned. Mama Sol says it is noodles. It is not noodles.",
              objectives: ["Pick up the package at the noodle stall", "Cross the corp core without a scan", "Hand it to the docks fixer"]),
        Quest(id: "noodle_debt", title: "Noodle debt", giver: "Mama Sol", location: "Night market",
              desc: "You owe for three weeks of dinners. She will take credits, or a favour.",
              objectives: ["Pay 120 credits, or run the drop"]),
        Quest(id: "first_jack", title: "First jack", giver: "Yourself", location: "Anywhere with a terminal",
              desc: "Find a public terminal and jack in. Come back out.",
              objectives: ["Jack in at a street terminal", "Jack out again"]),
        Quest(id: "the_vault", title: "The vault", giver: "Unknown", location: "Corp core",
              desc: "A data vault in the core has a buyer. Nobody knows what is in it.",
              objectives: ["Locate the vault in the Net", "Get past the ICE", "Copy the vault"]),
    ],
    recipes: [
        Recipe(id: "wire_shiv", name: "Wire shiv", skill: "electronics", level: 1,
               inputs: [("copper_coil", 1), ("scrap_servo", 1)], output: ("shiv_blade", 1),
               desc: "Wrap a servo housing around a coil and pull the wire taut."),
        Recipe(id: "hardhat_visor", name: "Hardhat visor", skill: "electronics", level: 1,
               inputs: [("dead_phone", 1), ("copper_coil", 1)], output: ("hardhat", 1),
               desc: "A phone screen makes a serviceable visor once you have scraped the life off it."),
        Recipe(id: "caps_fence", name: "Fence caps", skill: "streetwise", level: 1,
               inputs: [("bottle_caps", 10)], output: ("copper_coil", 1),
               desc: "The collector pays in wire."),
        Recipe(id: "stim_batch", name: "Stim batch", skill: "medtech", level: 2,
               inputs: [("copper_coil", 1), ("soy_noodle_cup", 1)], output: ("stim_patch", 2),
               desc: "Boil, filter, pray."),
        Recipe(id: "optics_tune", name: "Optics tune", skill: "electronics", level: 4,
               inputs: [("nightglass_optics", 1), ("copper_coil", 2)], output: ("nightglass_optics", 1),
               desc: "Re-seat the lenses and kill the boot logo."),
        Recipe(id: "servo_rebuild", name: "Servo rebuild", skill: "electronics", level: 2,
               inputs: [("scrap_servo", 4), ("copper_coil", 2)], output: ("grip_arm", 1),
               desc: "Four servos and a lot of wire make an arm that mostly obeys."),
    ],
    starter: Starter(
        handle: "Vex",
        credits: 320,
        hp: 40,
        stats: Stats(body: 4, reflex: 6, tech: 5, cool: 5, net: 6),
        inventory: [("copper_coil", 3), ("scrap_servo", 2), ("burner_shard", 1), ("soy_noodle_cup", 2),
                    ("stim_patch", 1), ("bottle_caps", 12), ("dead_phone", 1)],
        equipped: [(Primary, "ratchet_smg"), (Torso, "salvage_jacket"), (Eyes, "nightglass_optics"), (Deck, "pawnshop_deck")],
        skills: [("blades", 1), ("handguns", 2), ("stealth", 2), ("hacking", 3), ("electronics", 2),
                 ("streetwise", 3), ("athletics", 1), ("medtech", 1)],
        talents: ["quick_draw", "ice_pick", "solder_hand"],
        quests: [("the_drop", [true, false, false]), ("noodle_debt", [false]), ("first_jack", [true, true])],
    ),
)
```

(RON writes `Option` as `Some(Primary)`; a missing `slot` field defaults to `None` via `#[serde(default)]`.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p game`
Expected: PASS (2 tests). If RON rejects a field spelling, the error names it; fix the file, not the test.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/game flavor/content.ron
git commit -m "feat(game): content model, starter kit and validation"
```

---

### Task 2: The character

**Files:**
- Create: `crates/game/src/character.rs`
- Modify: `crates/game/src/lib.rs` (add `pub mod character; pub use character::*;`)
- Test: `crates/game/src/character.rs`

**Interfaces:**
- Consumes: `Content`, `Starter`, ids and structs from Task 1.
- Produces: `Stack { item: ItemId, qty: u32 }`, `QuestLog { quest: QuestId, done: Vec<bool> }`, `Character` with `from_starter(&Content)`, `skill_level(&SkillId) -> u8`, `count(&ItemId) -> u32`, `weight(&Content) -> f32`, `value(&Content) -> u32`, `can_craft(&Content, &Recipe) -> bool`, `has_talent(&TalentId) -> bool`, `equipped(Slot) -> Option<&ItemId>`, `is_equipped(&ItemId) -> bool`, `QuestLog::is_done(&self) -> bool`.

- [ ] **Step 1: Write the failing test**

```rust
//! The player's character: stats, skills, talents, equipment, inventory and quest log.

use crate::content::Content;
use crate::model::*;
use std::collections::BTreeMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_character_matches_the_content_file() {
        let c = Content::load().unwrap();
        let ch = Character::from_starter(&c);
        assert_eq!(ch.handle, "Vex");
        assert_eq!((ch.hp, ch.hp_max, ch.credits), (40, 40, 320));
        assert_eq!(ch.equipped(Slot::Eyes), Some(&ItemId::new("nightglass_optics")));
        assert_eq!(ch.equipped(Slot::Head), None);
        assert!(ch.is_equipped(&ItemId::new("ratchet_smg")));
        assert!(!ch.is_equipped(&ItemId::new("copper_coil")));
        assert_eq!(ch.count(&ItemId::new("copper_coil")), 3);
        assert_eq!(ch.count(&ItemId::new("grip_arm")), 0);
        assert_eq!(ch.skill_level(&SkillId::new("hacking")), 3);
        assert_eq!(ch.skill_level(&SkillId::new("origami")), 0);
        assert!(ch.has_talent(&TalentId::new("quick_draw")));
        assert!(!ch.has_talent(&TalentId::new("deep_dive")));
        assert_eq!(ch.quests.len(), 3);
        assert!(ch.quests.iter().any(|q| q.is_done()));
        assert!(ch.quests.iter().any(|q| !q.is_done()));

        let craftable = c.recipe(&RecipeId::new("wire_shiv")).unwrap();
        let level_locked = c.recipe(&RecipeId::new("optics_tune")).unwrap();
        let input_locked = c.recipe(&RecipeId::new("servo_rebuild")).unwrap();
        assert!(ch.can_craft(&c, craftable));
        assert!(!ch.can_craft(&c, level_locked));
        assert!(!ch.can_craft(&c, input_locked));

        let expected_weight: f32 = ch
            .inventory
            .iter()
            .map(|s| c.item(&s.item).unwrap().weight * s.qty as f32)
            .sum::<f32>()
            + ch.equipment
                .values()
                .map(|id| c.item(id).unwrap().weight)
                .sum::<f32>();
        assert!((ch.weight(&c) - expected_weight).abs() < 1e-4);
        assert!(ch.weight(&c) > 7.0 && ch.weight(&c) < 12.0, "{}", ch.weight(&c));
        assert!(ch.value(&c) > 4000);
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p game starter_character`
Expected: compile error, `Character` not found (after adding `pub mod character;` to `lib.rs`).

- [ ] **Step 3: Implement**

Above the tests in `character.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    pub item: ItemId,
    pub qty: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuestLog {
    pub quest: QuestId,
    pub done: Vec<bool>,
}

impl QuestLog {
    pub fn is_done(&self) -> bool {
        !self.done.is_empty() && self.done.iter().all(|d| *d)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    pub handle: String,
    pub credits: u32,
    pub hp: u32,
    pub hp_max: u32,
    pub stats: Stats,
    pub skills: Vec<(SkillId, u8)>,
    pub talents: Vec<TalentId>,
    pub equipment: BTreeMap<Slot, ItemId>,
    pub inventory: Vec<Stack>,
    pub quests: Vec<QuestLog>,
}

impl Character {
    /// A new character from the content's starter kit.
    pub fn from_starter(c: &Content) -> Character {
        let s = &c.starter;
        Character {
            handle: s.handle.clone(),
            credits: s.credits,
            hp: s.hp,
            hp_max: s.hp,
            stats: s.stats.clone(),
            skills: s.skills.clone(),
            talents: s.talents.clone(),
            equipment: s.equipped.iter().cloned().collect(),
            inventory: s
                .inventory
                .iter()
                .map(|(item, qty)| Stack {
                    item: item.clone(),
                    qty: *qty,
                })
                .collect(),
            quests: s
                .quests
                .iter()
                .map(|(quest, done)| QuestLog {
                    quest: quest.clone(),
                    done: done.clone(),
                })
                .collect(),
        }
    }

    pub fn skill_level(&self, id: &SkillId) -> u8 {
        self.skills
            .iter()
            .find(|(s, _)| s == id)
            .map(|(_, l)| *l)
            .unwrap_or(0)
    }

    /// How many of an item sit in the inventory (equipped items do not count).
    pub fn count(&self, id: &ItemId) -> u32 {
        self.inventory
            .iter()
            .filter(|s| &s.item == id)
            .map(|s| s.qty)
            .sum()
    }

    /// Carried weight: inventory plus everything equipped.
    pub fn weight(&self, c: &Content) -> f32 {
        let carried: f32 = self
            .inventory
            .iter()
            .filter_map(|s| c.item(&s.item).map(|i| i.weight * s.qty as f32))
            .sum();
        let worn: f32 = self
            .equipment
            .values()
            .filter_map(|id| c.item(id).map(|i| i.weight))
            .sum();
        carried + worn
    }

    pub fn value(&self, c: &Content) -> u32 {
        let carried: u32 = self
            .inventory
            .iter()
            .filter_map(|s| c.item(&s.item).map(|i| i.value * s.qty))
            .sum();
        let worn: u32 = self
            .equipment
            .values()
            .filter_map(|id| c.item(id).map(|i| i.value))
            .sum();
        carried + worn
    }

    /// Whether the skill level and every input are there. Does not craft.
    pub fn can_craft(&self, _c: &Content, r: &Recipe) -> bool {
        self.skill_level(&r.skill) >= r.level
            && r.inputs.iter().all(|(id, n)| self.count(id) >= *n)
    }

    pub fn has_talent(&self, id: &TalentId) -> bool {
        self.talents.iter().any(|t| t == id)
    }

    pub fn equipped(&self, slot: Slot) -> Option<&ItemId> {
        self.equipment.get(&slot)
    }

    pub fn is_equipped(&self, id: &ItemId) -> bool {
        self.equipment.values().any(|i| i == id)
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p game`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/game
git commit -m "feat(game): character built from the starter kit"
```

---

### Task 3: Screen stack, dim wash, flash, app state fields

**Files:**
- Modify: `crates/cyberpunk/Cargo.toml` (add `game = { path = "../game" }`)
- Modify: `crates/cyberpunk/src/app.rs`
- Create: `crates/cyberpunk/src/ui/dim.rs`; add `pub mod dim;` to `crates/cyberpunk/src/ui/mod.rs`
- Modify: `crates/cyberpunk/src/ui/transition.rs` (`Transition::flash`)
- Test: `crates/cyberpunk/src/app.rs`, `crates/cyberpunk/src/ui/dim.rs`, `crates/cyberpunk/src/ui/transition.rs`

**Interfaces:**
- Produces: `ScreenCmd::{Push { to, fx }, Pop { fx }, Flash(TransitionKind)}`; `Runner::depth(&self) -> usize` (test only); `ui::dim::wash(buf: &mut Buffer)`; `Transition::flash(kind, from, start) -> Transition` (10 ticks); `AppState { content: game::Content, character: game::Character, rain: bool, show_fps: bool, .. }`.

- [ ] **Step 1: Write the failing tests**

In `crates/cyberpunk/src/app.rs` tests module, replace `Says` and `switch_snapshots_the_previous_frame` with:

```rust
    use std::cell::Cell;
    use std::rc::Rc;

    /// A screen that draws its name on row 0 and reacts to single letters.
    struct Probe {
        name: &'static str,
        updates: Rc<Cell<u32>>,
    }

    impl Screen for Probe {
        fn update(&mut self, _st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
            self.updates.set(self.updates.get() + 1);
            let Some(k) = keys.first() else {
                return ScreenCmd::Stay;
            };
            match k.code {
                crossterm::event::KeyCode::Char('p') => ScreenCmd::Push {
                    to: Box::new(Probe {
                        name: "TOP",
                        updates: Rc::new(Cell::new(0)),
                    }),
                    fx: TransitionKind::None,
                },
                crossterm::event::KeyCode::Char('o') => ScreenCmd::Pop {
                    fx: TransitionKind::None,
                },
                crossterm::event::KeyCode::Char('f') => ScreenCmd::Flash(TransitionKind::Glitch),
                crossterm::event::KeyCode::Char('s') => ScreenCmd::Switch {
                    to: Box::new(Probe {
                        name: "WORLD",
                        updates: Rc::new(Cell::new(0)),
                    }),
                    fx: TransitionKind::SlideUp,
                },
                crossterm::event::KeyCode::Char('q') => ScreenCmd::Quit,
                _ => ScreenCmd::Stay,
            }
        }
        fn draw(&self, _st: &AppState, f: &mut Frame) {
            let row = if self.name == "TOP" { 1 } else { 0 };
            f.render_widget(
                ratatui::widgets::Paragraph::new(self.name)
                    .style(Style::default().fg(Color::Rgb(200, 200, 200))),
                Rect::new(0, row, 10, 1),
            );
        }
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(
            crossterm::event::KeyCode::Char(c),
            crossterm::event::KeyModifiers::NONE,
        )
    }

    fn fg_at(t: &Terminal<TestBackend>, x: u16, y: u16) -> Color {
        t.backend().buffer()[(x, y)].fg
    }

    #[test]
    fn stack_pushes_dims_updates_top_and_pops() {
        let updates = Rc::new(Cell::new(0));
        let base = Probe {
            name: "BASE",
            updates: updates.clone(),
        };
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut runner = Runner::new(Box::new(base), AppState::new());
        assert!(runner.frame(&mut t, &[]).unwrap());
        assert_eq!(runner.depth(), 1);
        assert_eq!(fg_at(&t, 0, 0), Color::Rgb(200, 200, 200));
        assert!(runner.frame(&mut t, &[key('p')]).unwrap());
        assert_eq!(runner.depth(), 2);
        assert_eq!(updates.get(), 2);
        let text = text(&t);
        assert!(text.contains("BASE") && text.contains("TOP"), "{text}");
        let Color::Rgb(r, _, _) = fg_at(&t, 0, 0) else {
            panic!("base row lost its colour");
        };
        assert!(r < 120, "base is not dimmed: {r}");
        // Only the top screen updates while it is up.
        assert!(runner.frame(&mut t, &[]).unwrap());
        assert_eq!(updates.get(), 2);
        assert!(runner.frame(&mut t, &[key('x')]).unwrap());
        assert_eq!(runner.depth(), 2, "TOP stays on an unknown key");
        assert!(runner.frame(&mut t, &[key('o')]).unwrap());
        assert_eq!(runner.depth(), 1);
        assert_eq!(fg_at(&t, 0, 0), Color::Rgb(200, 200, 200));
        assert_eq!(updates.get(), 2);
        assert!(runner.frame(&mut t, &[]).unwrap());
        assert_eq!(updates.get(), 3);
    }

    #[test]
    fn pop_on_last_screen_is_ignored_and_switch_clears() {
        let base = Probe {
            name: "BASE",
            updates: Rc::new(Cell::new(0)),
        };
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut runner = Runner::new(Box::new(base), AppState::new());
        assert!(runner.frame(&mut t, &[key('o')]).unwrap());
        assert_eq!(runner.depth(), 1);
        assert!(text(&t).contains("BASE"));
        assert!(runner.frame(&mut t, &[key('p')]).unwrap());
        assert!(runner.frame(&mut t, &[key('p')]).unwrap());
        assert_eq!(runner.depth(), 3);
        // Switch from the top replaces the whole stack and snapshots the frame.
        assert!(runner.frame(&mut t, &[key('s')]).unwrap());
        assert_eq!(runner.depth(), 1);
        let tr = runner.transition().expect("a transition is running");
        let snapshot: String = (0..4)
            .map(|x| tr.from[(x, 0)].symbol().to_string())
            .collect();
        assert_eq!(snapshot, "BASE");
        for _ in 0..12 {
            assert!(runner.frame(&mut t, &[]).unwrap());
        }
        assert!(text(&t).contains("WORLD"));
        assert!(!runner.frame(&mut t, &[key('q')]).unwrap());
    }

    #[test]
    fn flash_runs_a_transition_in_place() {
        let base = Probe {
            name: "BASE",
            updates: Rc::new(Cell::new(0)),
        };
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut runner = Runner::new(Box::new(base), AppState::new());
        assert!(runner.frame(&mut t, &[]).unwrap());
        assert!(runner.frame(&mut t, &[key('f')]).unwrap());
        assert_eq!(runner.depth(), 1);
        let tr = runner.transition().expect("flash started a transition");
        assert_eq!(tr.kind, TransitionKind::Glitch);
        assert_eq!(tr.dur, 10);
        // Drawing at another size while the flash runs must not touch the stale snapshot.
        let mut small = Terminal::new(TestBackend::new(100, 31)).unwrap();
        assert!(runner.frame(&mut small, &[]).unwrap());
        assert!(text(&small).contains("BASE"));
    }
```

Add to `crates/cyberpunk/src/ui/transition.rs` tests:

```rust
    #[test]
    fn flash_lasts_ten_ticks() {
        let from = Buffer::empty(Rect::new(0, 0, 20, 5));
        let tr = Transition::flash(TransitionKind::Glitch, from.clone(), 10);
        assert!(tr.active(19) && !tr.active(20));
        assert!(!Transition::flash(TransitionKind::None, from, 10).active(10));
    }
```

Create `crates/cyberpunk/src/ui/dim.rs` with its test:

```rust
//! A scanline wash that darkens a frame so a menu can sit over it.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

const DIM: f32 = 0.35;
const SCANLINE: f32 = 0.8;

fn scale(c: Color, f: f32) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * f).round() as u8,
            (g as f32 * f).round() as u8,
            (b as f32 * f).round() as u8,
        ),
        other => other,
    }
}

/// Darkens every cell; odd rows a little more, for scanlines.
pub fn wash(buf: &mut Buffer) {
    let area = buf.area;
    for y in area.y..area.bottom() {
        let f = if y % 2 == 1 { DIM * SCANLINE } else { DIM };
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            let fg = scale(cell.fg, f);
            let bg = scale(cell.bg, f);
            cell.set_fg(fg).set_bg(bg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn wash_darkens_rgb_and_leaves_reset_alone() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 2));
        buf[(0, 0)].set_fg(Color::Rgb(200, 100, 50)).set_bg(Color::Rgb(20, 20, 20));
        buf[(0, 1)].set_fg(Color::Rgb(200, 100, 50));
        buf[(1, 0)].set_fg(Color::Reset);
        wash(&mut buf);
        assert_eq!(buf[(0, 0)].fg, Color::Rgb(70, 35, 18));
        assert_eq!(buf[(0, 0)].bg, Color::Rgb(7, 7, 7));
        assert_eq!(buf[(0, 1)].fg, Color::Rgb(56, 28, 14), "odd rows are darker");
        assert_eq!(buf[(1, 0)].fg, Color::Reset);
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p cyberpunk app::` and `cargo test -p cyberpunk dim::`
Expected: compile errors (`Push` not a variant, `depth` missing, `wash` module missing).

- [ ] **Step 3: Implement**

`crates/cyberpunk/Cargo.toml` dependencies: `game = { path = "../game" }`. `crates/cyberpunk/src/ui/mod.rs`: `pub mod dim;`.

`transition.rs`:

```rust
    /// A transition that plays over the same screen (jack in / out): longer than a switch.
    pub fn flash(kind: TransitionKind, from: Buffer, start: u64) -> Self {
        let mut t = Self::new(kind, from, start);
        if kind != TransitionKind::None {
            t.dur = 10;
        }
        t
    }
```

`app.rs`: `AppState` gains

```rust
    pub content: game::Content,
    pub character: game::Character,
    /// Rain overlay on the map (Settings).
    pub rain: bool,
    /// FPS readout in the status bar (Settings).
    pub show_fps: bool,
```

initialised in `new()` with

```rust
        let content = game::Content::load().expect("embedded content pack is valid");
        let character = game::Character::from_starter(&content);
        AppState { params: GenParams::default(), city: None, player: None,
            style: StylePack::load().expect("embedded style pack is valid"),
            tick: 0, dev: DEV_DEFAULT, rain: true, show_fps: true, content, character, status: None }
```

`ScreenCmd`:

```rust
pub enum ScreenCmd {
    Stay,
    /// Replace the whole stack with one screen.
    Switch { to: Box<dyn Screen>, fx: TransitionKind },
    /// Put a screen on top; the ones below keep drawing, washed, and stop updating.
    Push { to: Box<dyn Screen>, fx: TransitionKind },
    /// Remove the top screen. Ignored on the last one.
    Pop { fx: TransitionKind },
    /// Run a transition in place without changing screens.
    Flash(TransitionKind),
    Quit,
}
```

`Runner`:

```rust
pub struct Runner {
    screens: Vec<Box<dyn Screen>>,
    pub st: AppState,
    transition: Option<Transition>,
    last_frame: Option<Buffer>,
}

impl Runner {
    pub fn new(screen: Box<dyn Screen>, st: AppState) -> Self {
        Runner { screens: vec![screen], st, transition: None, last_frame: None }
    }

    #[cfg(test)]
    pub fn transition(&self) -> Option<&Transition> { self.transition.as_ref() }

    #[cfg(test)]
    pub fn depth(&self) -> usize { self.screens.len() }

    fn snapshot(&self, area: Rect) -> Buffer {
        self.last_frame.clone().unwrap_or_else(|| Buffer::empty(area))
    }

    pub fn frame<B: Backend>(&mut self, term: &mut Terminal<B>, keys: &[KeyEvent]) -> Result<bool> {
        self.st.tick += 1;
        if self.transition.as_ref().is_some_and(|t| t.active(self.st.tick)) && !keys.is_empty() {
            self.transition = None;
        }
        let area = term.get_frame().area();
        let top = self.screens.last_mut().expect("the stack is never empty");
        match top.update(&mut self.st, keys) {
            ScreenCmd::Stay => {}
            ScreenCmd::Switch { to, fx } => {
                let snapshot = self.snapshot(area);
                self.screens = vec![to];
                self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
            }
            ScreenCmd::Push { to, fx } => {
                let snapshot = self.snapshot(area);
                self.screens.push(to);
                self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
            }
            ScreenCmd::Pop { fx } => {
                if self.screens.len() > 1 {
                    let snapshot = self.snapshot(area);
                    self.screens.pop();
                    self.transition = Some(Transition::new(fx, snapshot, self.st.tick));
                }
            }
            ScreenCmd::Flash(fx) => {
                let snapshot = self.snapshot(area);
                self.transition = Some(Transition::flash(fx, snapshot, self.st.tick));
            }
            ScreenCmd::Quit => return Ok(false),
        }
        let st = &self.st;
        let screens = &self.screens;
        let transition = &self.transition;
        let mut captured: Option<Buffer> = None;
        term.draw(|f| {
            if f.area().width < MIN_W || f.area().height < MIN_H {
                draw_too_small(f);
            } else {
                for (i, screen) in screens.iter().enumerate() {
                    if i > 0 {
                        crate::ui::dim::wash(f.buffer_mut());
                    }
                    screen.draw(st, f);
                }
                if let Some(t) = transition {
                    t.apply(f, st.tick);
                }
            }
            captured = Some(f.buffer_mut().clone());
        })?;
        self.last_frame = captured;
        Ok(true)
    }
}
```

(`Rect` import from `ratatui::layout` is already there for `draw_too_small`.)

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS. Any exhaustive `match` on `ScreenCmd` in tests elsewhere (grep `ScreenCmd::` across `crates/cyberpunk/src`) keeps compiling because they use `matches!`.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/Cargo.toml Cargo.lock crates/cyberpunk/src/app.rs crates/cyberpunk/src/ui/mod.rs crates/cyberpunk/src/ui/dim.rs crates/cyberpunk/src/ui/transition.rs
git commit -m "feat(app): screen stack with push, pop and flash; scanline wash; game state"
```

---

### Task 4: Files screen pops; the title pushes Load; the map pushes Save

**Files:**
- Modify: `crates/cyberpunk/src/screens/files.rs` (drop `back`, return `Pop`)
- Modify: `crates/cyberpunk/src/screens/title.rs:49-52` (Load → `Push`)
- Modify: `crates/cyberpunk/src/screens/explore.rs` (`s` → `Push`)
- Test: `files.rs`, `title.rs`, `explore.rs`

**Interfaces:**
- Consumes: `ScreenCmd::{Push, Pop}`.
- Produces: `Files::save(tick: u64, name: &str) -> Files`, `Files::save_in(dir, tick, name)`; Esc/cancel/done on Files → `Pop { fx: None }`; successful load → `Switch` (unchanged).

- [ ] **Step 1: Update the tests**

In `files.rs` tests: every `Files::save(...)`/`save_in(...)` call drops its last argument; assertions that a save or Esc returns `ScreenCmd::Switch { .. }` become `ScreenCmd::Pop { .. }`; the test that unpacks `let ScreenCmd::Switch { to, .. } = save.update(...)` to draw the returned map becomes `assert!(matches!(save.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Pop { .. }))`; Load Esc now asserts `Pop`. In `title.rs` tests, the Load item assertion becomes `matches!(cmd, ScreenCmd::Push { .. })`. In `explore.rs` `explore_and_walk_round_trip`, replace the save round-trip block with:

```rust
        // Saving pushes the save screen over the live map; the map keeps its state.
        s.stratum = StratumKind::Undercity;
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Char('s'))]),
            ScreenCmd::Push { .. }
        ));
        assert_eq!((s.mode, s.stratum), (Mode::Walk, StratumKind::Undercity));
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p cyberpunk files::`
Expected: compile errors on the changed signatures / failing `matches!`.

- [ ] **Step 3: Implement**

`files.rs`: remove the `back` field, the `back` parameters and `back_to_map`; add

```rust
    fn done() -> ScreenCmd {
        ScreenCmd::Pop {
            fx: TransitionKind::None,
        }
    }
```

and use `Self::done()` where `back_to_map` was called; the Load arm `KeyCode::Esc | KeyCode::Char('q') => return Self::done(),`. Remove the now-unused `Title` import. `title.rs`: `1 => ScreenCmd::Push { to: Box::new(Files::load(st.tick)), fx: TransitionKind::SlideUp }`. `explore.rs` `s`:

```rust
            KeyCode::Char('s') => {
                let name = st.city.as_ref().map(|c| c.name.clone())?;
                return Some(ScreenCmd::Push {
                    to: Box::new(Files::save(st.tick, &name)),
                    fx: TransitionKind::None,
                });
            }
```

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/screens/files.rs crates/cyberpunk/src/screens/title.rs crates/cyberpunk/src/screens/explore.rs
git commit -m "refactor(screens): file screens pop back instead of handing screens around"
```

---

### Task 5: Settings screen; rain and FPS wiring

**Files:**
- Create: `crates/cyberpunk/src/screens/settings.rs`; add `pub mod settings;` to `screens/mod.rs`
- Modify: `crates/cyberpunk/src/ui/panel.rs` (`status_bar` fps `Option<f32>`)
- Modify: `crates/cyberpunk/src/screens/explore.rs` (overlays by `st.rain`, fps by `st.show_fps`)
- Test: `settings.rs`, `explore.rs`, `panel.rs`

**Interfaces:**
- Produces: `Settings::new() -> Settings`; `status_bar(city, stratum, fps: Option<f32>, hints, status, dev)`.

- [ ] **Step 1: Write the failing tests**

`settings.rs`:

```rust
//! Settings: dev mode, rain and the FPS counter, flipped in place.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::ui::panel::boxed;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn settings_flip_state_and_pop() {
        let mut st = AppState::new();
        let mut s = Settings::new();
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("SETTINGS") && text.contains("Dev mode") && text.contains("[ ON ]"), "{text}");
        s.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Right)]);
        assert!(!st.rain);
        s.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Enter)]);
        assert!(!st.show_fps);
        s.update(&mut st, &[key(KeyCode::Up), key(KeyCode::Up), key(KeyCode::Char(' '))]);
        assert!(!st.dev);
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("[ OFF ]"));
        assert!(matches!(s.update(&mut st, &[key(KeyCode::Esc)]), ScreenCmd::Pop { .. }));
    }
}
```

`explore.rs` tests:

```rust
    #[test]
    fn rain_and_fps_follow_the_settings() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        st.tick = 100;
        s.update(&mut st, &[]);
        let rain: Vec<char> = st.style.glyphs.rain.clone();
        let count = |t: &Terminal<TestBackend>| {
            buffer_text(t.backend().buffer())
                .chars()
                .filter(|c| rain.contains(c))
                .count()
        };
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert!(count(&t) > 0, "no rain on the street");
        assert!(buffer_text(t.backend().buffer()).contains("fps"));
        st.rain = false;
        st.show_fps = false;
        t.draw(|f| s.draw(&st, f)).unwrap();
        assert_eq!(count(&t), 0);
        assert!(!buffer_text(t.backend().buffer()).contains("fps"));
    }
```

`panel.rs` tests: the two `status_bar` calls pass `Some(30.0)`; add one call with `None` and assert the bar's string does not contain `"fps"`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p cyberpunk settings` (compile error: no `Settings`), `cargo test -p cyberpunk rain_and_fps` (fails: rain still drawn).

- [ ] **Step 3: Implement**

`settings.rs` above the tests:

```rust
const ROWS: [&str; 3] = ["Dev mode", "Rain", "FPS counter"];
const ACCENT: Color = Color::Rgb(0, 229, 255);
const DIM: Color = Color::Rgb(120, 130, 145);
const TEXT: Color = Color::Rgb(205, 212, 222);

pub struct Settings {
    selected: usize,
}

impl Settings {
    pub fn new() -> Self {
        Settings { selected: 0 }
    }

    fn get(st: &AppState, i: usize) -> bool {
        match i {
            0 => st.dev,
            1 => st.rain,
            _ => st.show_fps,
        }
    }

    fn flip(st: &mut AppState, i: usize) {
        match i {
            0 => st.dev = !st.dev,
            1 => st.rain = !st.rain,
            _ => st.show_fps = !st.show_fps,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for Settings {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected = (self.selected + 1).min(ROWS.len() - 1)
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => {
                    Self::flip(st, self.selected)
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let (w, h) = (42u16.min(area.width), 9u16.min(area.height));
        let panel = Rect::new(
            area.x + (area.width - w) / 2,
            area.y + (area.height - h) / 2,
            w,
            h,
        );
        let block = boxed("SETTINGS");
        let inner = block.inner(panel);
        f.render_widget(block, panel);
        let mut lines: Vec<Line> = Vec::new();
        for (i, label) in ROWS.iter().enumerate() {
            let on = Self::get(st, i);
            let sel = i == self.selected;
            let style = if sel {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT)
            };
            lines.push(Line::from(vec![
                Span::styled(if sel { " ▸ " } else { "   " }, style),
                Span::styled(format!("{label:<14}"), style),
                Span::styled(
                    if on { "[ ON ]" } else { "[ OFF ]" },
                    Style::default().fg(if on { ACCENT } else { DIM }),
                ),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " ↑↓ select · ←→ flip · esc back",
            Style::default().fg(DIM),
        )));
        f.render_widget(Paragraph::new(lines), inner);
    }
}
```

`panel.rs` `status_bar`: parameter `fps: Option<f32>`; the fps span only `if let Some(fps) = fps`. `explore.rs`: overlays become a `Vec`:

```rust
        let mut overlays: Vec<&dyn Overlay> = Vec::new();
        if st.rain {
            overlays.push(&Weather);
        }
        overlays.push(&self.traffic);
        overlays.push(&digital);
        ... MapView { overlays: &overlays, .. }
```

and the status bar call passes `st.show_fps.then_some(self.fps.fps)`.

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/screens/settings.rs crates/cyberpunk/src/screens/mod.rs crates/cyberpunk/src/ui/panel.rs crates/cyberpunk/src/screens/explore.rs
git commit -m "feat(screens): settings screen; rain and fps readout follow it"
```

---

### Task 6: Pause menu on Esc and `q`

**Files:**
- Create: `crates/cyberpunk/src/screens/pause.rs`; add `pub mod pause;` to `screens/mod.rs`
- Modify: `crates/cyberpunk/src/screens/explore.rs` (Esc / `q`)
- Test: `pause.rs`, `explore.rs`

**Interfaces:**
- Consumes: `Menu`, `MenuView`, `Files::{save, load}`, `Settings::new`, `Title::new`.
- Produces: `Pause::new(tick: u64) -> Pause`.

- [ ] **Step 1: Write the failing tests**

`pause.rs`:

```rust
//! The pause menu: resume, save, load, settings, quit.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::screens::files::Files;
use crate::screens::settings::Settings;
use crate::screens::title::Title;
use crate::ui::menu::{Menu, MenuView};
use crate::ui::panel::boxed;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Paragraph, Widget};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn pause_menu_routes_every_item() {
        let mut st = AppState::new();
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut p = Pause::new(0);
        st.tick = 60;
        t.draw(|f| p.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("PAUSED") && text.contains("RESUME") && text.contains("QUIT GAME"), "{text}");
        assert!(matches!(p.update(&mut st, &[key(KeyCode::Esc)]), ScreenCmd::Pop { .. }));
        assert!(matches!(p.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Pop { .. }));
        let down = |p: &mut Pause, st: &mut AppState| {
            p.update(st, &[key(KeyCode::Down)]);
        };
        down(&mut p, &mut st);
        assert!(matches!(p.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Push { .. }), "save");
        down(&mut p, &mut st);
        assert!(matches!(p.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Push { .. }), "load");
        down(&mut p, &mut st);
        assert!(matches!(p.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Push { .. }), "settings");
        down(&mut p, &mut st);
        assert!(matches!(p.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Switch { .. }), "title");
        down(&mut p, &mut st);
        assert!(matches!(p.update(&mut st, &[key(KeyCode::Enter)]), ScreenCmd::Quit), "quit");
    }
}
```

`explore.rs` tests:

```rust
    #[test]
    fn esc_closes_help_before_pausing() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::Char('?'))]);
        assert!(s.help);
        assert!(matches!(s.update(&mut st, &[key(KeyCode::Esc)]), ScreenCmd::Stay));
        assert!(!s.help);
        assert!(matches!(s.update(&mut st, &[key(KeyCode::Esc)]), ScreenCmd::Push { .. }));
        assert!(matches!(s.update(&mut st, &[key(KeyCode::Char('q'))]), ScreenCmd::Push { .. }));
    }
```

Also change the existing assertions that `q` returns `Switch` (in `explore_and_walk_round_trip`) to `Push`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p cyberpunk pause` → compile error; `cargo test -p cyberpunk esc_closes_help` → fails (Esc returns `Switch`).

- [ ] **Step 3: Implement**

`pause.rs` above the tests:

```rust
const ITEMS: [&str; 6] = ["RESUME", "SAVE CITY", "LOAD CITY", "SETTINGS", "QUIT TO TITLE", "QUIT GAME"];

pub struct Pause {
    menu: Menu,
}

impl Pause {
    pub fn new(tick: u64) -> Self {
        Pause {
            menu: Menu::new(ITEMS.iter().map(|s| s.to_string()).collect(), tick),
        }
    }
}

impl Screen for Pause {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.menu.up(),
                KeyCode::Down | KeyCode::Char('j') => self.menu.down(),
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                KeyCode::Enter => {
                    return match self.menu.selected {
                        0 => ScreenCmd::Pop {
                            fx: TransitionKind::None,
                        },
                        1 => {
                            let name = st
                                .city
                                .as_ref()
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| "city".to_string());
                            ScreenCmd::Push {
                                to: Box::new(Files::save(st.tick, &name)),
                                fx: TransitionKind::None,
                            }
                        }
                        2 => ScreenCmd::Push {
                            to: Box::new(Files::load(st.tick)),
                            fx: TransitionKind::None,
                        },
                        3 => ScreenCmd::Push {
                            to: Box::new(Settings::new()),
                            fx: TransitionKind::None,
                        },
                        4 => ScreenCmd::Switch {
                            to: Box::new(Title::new(st.tick)),
                            fx: TransitionKind::SlideUp,
                        },
                        _ => ScreenCmd::Quit,
                    };
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let (w, h) = (36u16.min(area.width), 12u16.min(area.height));
        let panel = Rect::new(
            area.x + (area.width - w) / 2,
            area.y + (area.height - h) / 2,
            w,
            h,
        );
        let block = boxed("PAUSED");
        let inner = block.inner(panel);
        f.render_widget(block, panel);
        MenuView {
            menu: &self.menu,
            tick: st.tick,
            accent: Rgb(0, 255, 230),
        }
        .render(
            Rect::new(inner.x + 2, inner.y + 1, inner.width.saturating_sub(2), 6),
            f.buffer_mut(),
        );
        f.render_widget(
            Paragraph::new(Span::styled(
                " ↑↓ select · enter confirm · esc resume",
                Style::default().fg(Color::Rgb(120, 130, 145)),
            )),
            Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
        );
    }
}
```

`explore.rs` `handle_common`:

```rust
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.help {
                    self.help = false;
                } else {
                    return Some(ScreenCmd::Push {
                        to: Box::new(Pause::new(st.tick)),
                        fx: TransitionKind::None,
                    });
                }
            }
```

(import `crate::screens::pause::Pause`; the `Title` import goes if nothing else uses it.)

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/screens/pause.rs crates/cyberpunk/src/screens/mod.rs crates/cyberpunk/src/screens/explore.rs
git commit -m "feat(screens): pause menu on esc and q"
```

---

### Task 7: The deck: chrome, tabs, navigation, QUESTS / GEAR / INVENTORY

**Files:**
- Create: `crates/cyberpunk/src/screens/deck.rs`; add `pub mod deck;` to `screens/mod.rs`
- Test: `deck.rs`

**Interfaces:**
- Consumes: `game::{Content, Character, ItemId, ItemKind, Slot}`, `AppState.{content, character, player, city}`, `anim::effects::{pulse, typewriter, glitch_cells}`, `anim::{Tween, Easing}`, `ui::logo::{Logo, width, ROWS}`, `citygen::rng::hash64`.
- Produces: `Deck::new(tick) -> Deck`, `Tab::{ALL, label}`, `pub tab`, `pub row`; row/detail builders for Workbench and Skills are added in Task 8 (they return empty here); the MAP tab body is drawn in Task 8.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> AppState {
        let mut st = AppState::new();
        st.city = Some(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    fn draw(d: &Deck, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| d.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    #[test]
    fn deck_tabs_and_rows_render_from_content() {
        let mut st = state();
        let mut d = Deck::new(0);
        st.tick = 40;
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains("▌QUESTS▐"), "{text}");
        assert!(text.contains("VEX") && text.contains("¤ 320") && text.contains("hp 40/40"), "{text}");
        assert!(text.contains("The drop") && text.contains("✓ First jack"), "{text}");
        assert!(text.contains("LINK"), "{text}");

        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        assert_eq!(d.tab, Tab::Gear);
        st.tick += 30;
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains("▌GEAR▐") && text.contains("PRIMARY") && text.contains("Ratchet SMG"), "{text}");
        assert!(text.contains("— empty —"), "{text}");
        d.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(d.row, 1, "eyes");
        st.tick += 30;
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains("NIGHTGLASS OPTICS") && text.contains("cyberware · eyes"), "{text}");

        d.update(&mut st, &[key(KeyCode::Right)]);
        assert_eq!(d.tab, Tab::Inventory);
        st.tick += 30;
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains("── component ──") && text.contains("3× Copper coil"), "{text}");
        assert!(text.contains("kg") && text.contains("¤"), "{text}");
        // The first row is an item, never a header; headers are skipped when moving.
        let rows = d.rows(&st);
        assert!(rows[d.row].detail.is_some());
        let before = d.row;
        d.update(&mut st, &[key(KeyCode::Down)]);
        assert!(rows[d.row].detail.is_some() && d.row > before);

        d.update(&mut st, &[key(KeyCode::Left), key(KeyCode::Left), key(KeyCode::Left)]);
        assert_eq!(d.tab, Tab::Skills, "left wraps around");
        d.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(d.tab, Tab::Quests, "tab wraps forward");
        d.update(&mut st, &[key(KeyCode::BackTab)]);
        assert_eq!(d.tab, Tab::Skills);
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { fx: TransitionKind::Glitch }
        ));
    }

    #[test]
    fn deck_fits_the_minimum_terminal() {
        let mut st = state();
        st.tick = 40;
        let d = Deck::new(0);
        let text = draw(&d, &st, 100, 30);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with('┍') && lines[0].ends_with('┑'), "{:?}", lines[0]);
        assert!(lines[29].starts_with('┕') && lines[29].ends_with('┙'), "{:?}", lines[29]);
        assert!(text.contains("▌QUESTS▐") && text.contains("SKILLS"), "{text}");
        assert!(text.contains("esc close"), "{text}");
        assert!(text.contains("║"), "no pane divider: {text}");
        // Every row keeps its side borders.
        for (i, l) in lines.iter().enumerate().take(29).skip(1) {
            assert!(l.starts_with('│') && l.ends_with('│'), "row {i}: {l:?}");
        }
    }

    #[test]
    fn deck_glyphs_are_single_width() {
        use unicode_width::UnicodeWidthChar;
        for c in ['▰', '▱', '☑', '☐', '✓', '▮', '◍', '┍', '┑', '┕', '┙', '▌', '▐', '║', '◈', '⌗', '▣', '▪'] {
            assert_eq!(c.width(), Some(1), "{c:?}");
        }
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p cyberpunk deck::`
Expected: compile error, `Deck` not found.

- [ ] **Step 3: Implement**

```rust
//! The deck: the player's menu over the live map — quests, gear, inventory, workbench, map, skills.

use crate::anim::effects::{glitch_cells, pulse, typewriter};
use crate::anim::{Easing, Tween};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::ui::logo::{self, Logo};
use citygen::StratumKind;
use citygen::rng::hash64;
use crossterm::event::{KeyCode, KeyEvent};
use game::{ItemId, ItemKind, Slot};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

pub const CYAN: Color = Color::Rgb(0, 229, 255);
pub const MAGENTA: Color = Color::Rgb(255, 0, 200);
pub const DIM: Color = Color::Rgb(120, 130, 145);
pub const TEXT: Color = Color::Rgb(205, 212, 222);
const BAR_BG: Color = Color::Rgb(0, 60, 70);
const REVEAL_TICKS: u64 = 20;
const JITTER_PERIOD: u64 = 90;
const LOGO_MIN_ROWS: u16 = 36;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Quests,
    Gear,
    Inventory,
    Workbench,
    Map,
    Skills,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Quests,
        Tab::Gear,
        Tab::Inventory,
        Tab::Workbench,
        Tab::Map,
        Tab::Skills,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Quests => "QUESTS",
            Tab::Gear => "GEAR",
            Tab::Inventory => "INVENTORY",
            Tab::Workbench => "WORKBENCH",
            Tab::Map => "MAP",
            Tab::Skills => "SKILLS",
        }
    }

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Normal,
    Dim,
    Warn,
    Done,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Detail {
    Quest(usize),
    Slot(Slot),
    Item(ItemId),
    Recipe(usize),
    Skill(usize),
}

/// One list row and what the detail pane shows for it (`None` = a header).
pub(crate) struct Row {
    pub text: String,
    pub tone: Tone,
    pub detail: Option<Detail>,
}

pub struct Deck {
    pub tab: Tab,
    pub row: usize,
    /// Tick of the last row or tab change; drives the typewriter.
    changed: u64,
    tab_from: usize,
    tab_tween: Tween,
}

pub(crate) fn kind_icon(kind: ItemKind) -> char {
    match kind {
        ItemKind::Weapon => '/',
        ItemKind::Armor => '▣',
        ItemKind::Cyberware => '◈',
        ItemKind::Deck => '⌗',
        ItemKind::Consumable => '◍',
        ItemKind::Component => '▪',
        ItemKind::Junk => '·',
    }
}

fn tone_style(tone: Tone) -> Style {
    match tone {
        Tone::Normal => Style::default().fg(TEXT),
        Tone::Dim => Style::default().fg(DIM),
        Tone::Warn => Style::default().fg(MAGENTA),
        Tone::Done => Style::default().fg(Color::Rgb(90, 200, 140)),
    }
}

fn head(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into().to_uppercase(),
        Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
    ))
}

fn plain(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(TEXT)))
}

fn dim(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(DIM)))
}

fn warn(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), Style::default().fg(MAGENTA)))
}

impl Deck {
    pub fn new(tick: u64) -> Self {
        Deck {
            tab: Tab::Quests,
            row: 0,
            changed: tick,
            tab_from: 0,
            tab_tween: Tween::new(tick, 1, Easing::OutCubic),
        }
    }

    fn set_tab(&mut self, tab: Tab, tick: u64) {
        if tab == self.tab {
            return;
        }
        self.tab_from = self.tab.index();
        self.tab = tab;
        self.row = 0;
        self.changed = tick;
        self.tab_tween = Tween::new(tick, 6, Easing::OutCubic);
    }

    fn shift_tab(&mut self, delta: i32, tick: u64) {
        let n = Tab::ALL.len() as i32;
        let i = (self.tab.index() as i32 + delta).rem_euclid(n) as usize;
        self.set_tab(Tab::ALL[i], tick);
    }

    /// Moves the selection by `delta`, skipping header rows; clamps at the ends.
    fn move_row(&mut self, rows: &[Row], delta: i32, tick: u64) {
        if rows.is_empty() {
            self.row = 0;
            return;
        }
        let mut i = self.row as i32;
        loop {
            i += delta;
            if i < 0 || i >= rows.len() as i32 {
                return;
            }
            if rows[i as usize].detail.is_some() {
                self.row = i as usize;
                self.changed = tick;
                return;
            }
        }
    }

    fn first_selectable(rows: &[Row]) -> usize {
        rows.iter().position(|r| r.detail.is_some()).unwrap_or(0)
    }

    pub(crate) fn rows(&self, st: &AppState) -> Vec<Row> {
        match self.tab {
            Tab::Quests => Self::quest_rows(st),
            Tab::Gear => Self::gear_rows(st),
            Tab::Inventory => Self::inventory_rows(st),
            Tab::Workbench => Self::workbench_rows(st),
            Tab::Map => Vec::new(),
            Tab::Skills => Self::skill_rows(st),
        }
    }

    fn quest_rows(st: &AppState) -> Vec<Row> {
        let mut active = Vec::new();
        let mut done = Vec::new();
        for (i, log) in st.character.quests.iter().enumerate() {
            let Some(q) = st.content.quest(&log.quest) else { continue };
            let finished = log.is_done();
            let row = Row {
                text: format!("{} {}", if finished { "✓" } else { " " }, q.title),
                tone: if finished { Tone::Done } else { Tone::Normal },
                detail: Some(Detail::Quest(i)),
            };
            if finished { done.push(row) } else { active.push(row) }
        }
        active.extend(done);
        active
    }

    fn gear_rows(st: &AppState) -> Vec<Row> {
        Slot::ALL
            .iter()
            .map(|slot| {
                let name = st
                    .character
                    .equipped(*slot)
                    .and_then(|id| st.content.item(id))
                    .map(|i| i.name.clone());
                Row {
                    tone: if name.is_some() { Tone::Normal } else { Tone::Dim },
                    text: format!(
                        "{:<10} {}",
                        slot.label().to_uppercase(),
                        name.unwrap_or_else(|| "— empty —".to_string())
                    ),
                    detail: Some(Detail::Slot(*slot)),
                }
            })
            .collect()
    }

    fn inventory_rows(st: &AppState) -> Vec<Row> {
        let mut rows = Vec::new();
        for kind in ItemKind::ALL {
            let stacks: Vec<_> = st
                .character
                .inventory
                .iter()
                .filter(|s| st.content.item(&s.item).is_some_and(|i| i.kind == kind))
                .collect();
            if stacks.is_empty() {
                continue;
            }
            rows.push(Row {
                text: format!("── {} ──", kind.label()),
                tone: Tone::Dim,
                detail: None,
            });
            for s in stacks {
                let item = st.content.item(&s.item).expect("filtered above");
                rows.push(Row {
                    text: format!("{} {:>2}× {}", kind_icon(kind), s.qty, item.name),
                    tone: Tone::Normal,
                    detail: Some(Detail::Item(s.item.clone())),
                });
            }
        }
        rows
    }

    // Task 8 fills these in.
    fn workbench_rows(_st: &AppState) -> Vec<Row> {
        Vec::new()
    }

    fn skill_rows(_st: &AppState) -> Vec<Row> {
        Vec::new()
    }

    fn item_lines(st: &AppState, id: &ItemId) -> Vec<Line<'static>> {
        let Some(item) = st.content.item(id) else {
            return vec![warn(format!("unknown item {id}"))];
        };
        let mut out = vec![head(&item.name)];
        let slot = item
            .slot
            .map(|s| format!(" · {}", s.label()))
            .unwrap_or_default();
        out.push(dim(format!("{}{slot}", item.kind.label())));
        out.push(dim(format!("{:.1} kg · ¤ {}", item.weight, item.value)));
        if st.character.is_equipped(id) {
            out.push(Line::from(Span::styled("equipped", Style::default().fg(CYAN))));
        }
        out.push(plain(""));
        out.push(plain(item.desc.clone()));
        out
    }

    pub(crate) fn detail(&self, st: &AppState, rows: &[Row]) -> Vec<Line<'static>> {
        let Some(row) = rows.get(self.row) else {
            return vec![dim("nothing here yet")];
        };
        match &row.detail {
            None => Vec::new(),
            Some(Detail::Quest(i)) => {
                let log = &st.character.quests[*i];
                let Some(q) = st.content.quest(&log.quest) else { return Vec::new() };
                let mut out = vec![head(&q.title)];
                out.push(dim(format!("from {} · {}", q.giver, q.location)));
                out.push(plain(""));
                out.push(plain(q.desc.clone()));
                out.push(plain(""));
                for (j, text) in q.objectives.iter().enumerate() {
                    let done = log.done.get(j).copied().unwrap_or(false);
                    out.push(if done {
                        Line::from(Span::styled(format!("☑ {text}"), tone_style(Tone::Done)))
                    } else {
                        plain(format!("☐ {text}"))
                    });
                }
                out
            }
            Some(Detail::Slot(slot)) => match st.character.equipped(*slot) {
                Some(id) => Self::item_lines(st, id),
                None => vec![head("— empty —"), dim(format!("nothing in the {} slot", slot.label()))],
            },
            Some(Detail::Item(id)) => Self::item_lines(st, id),
            Some(Detail::Recipe(_)) | Some(Detail::Skill(_)) => Vec::new(), // Task 8
        }
    }

    fn link_line(st: &AppState) -> String {
        let (pos, stratum) = match &st.player {
            Some(p) => (Some(p.pos), p.stratum),
            None => (None, StratumKind::Street),
        };
        let district = pos
            .and_then(|p| st.city.as_ref().and_then(|c| c.tile(StratumKind::Street, p).and_then(|t| t.district).and_then(|d| c.district_opt(d)).map(|d| d.name.clone())))
            .unwrap_or_else(|| "no fix".to_string());
        format!("▍ LINK · {district} · {}", stratum.label())
    }
}
```

Frame and drawing helpers (same file):

```rust
/// Heavy top and bottom rules, light sides.
fn frame(buf: &mut Buffer, area: Rect) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let style = Style::default().fg(CYAN);
    for x in area.x..area.right() {
        buf[(x, area.y)].set_char('━').set_style(style);
        buf[(x, area.bottom() - 1)].set_char('━').set_style(style);
    }
    for y in area.y..area.bottom() {
        buf[(area.x, y)].set_char('│').set_style(style);
        buf[(area.right() - 1, y)].set_char('│').set_style(style);
    }
    buf[(area.x, area.y)].set_char('┍');
    buf[(area.right() - 1, area.y)].set_char('┑');
    buf[(area.x, area.bottom() - 1)].set_char('┕');
    buf[(area.right() - 1, area.bottom() - 1)].set_char('┙');
}

fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style, max_x: u16) {
    let mut cx = x;
    for ch in text.chars() {
        if cx >= max_x {
            break;
        }
        buf[(cx, y)].set_char(ch).set_style(style);
        cx += 1;
    }
}

/// Reveals `lines` progressively: `p` of all characters are shown, then a cursor.
fn reveal(lines: &[Line<'static>], p: f32, cursor_on: bool) -> Vec<Line<'static>> {
    let total: usize = lines.iter().map(|l| l.width()).sum();
    let mut budget = ((total as f32) * p.clamp(0.0, 1.0)).floor() as usize;
    let mut out = Vec::new();
    for line in lines {
        if budget == 0 && p < 1.0 {
            break;
        }
        let w = line.width();
        if w <= budget {
            out.push(line.clone());
            budget -= w;
        } else {
            let mut shown: Vec<Span<'static>> = Vec::new();
            let mut left = budget;
            for span in &line.spans {
                let text = span.content.to_string();
                let n = text.chars().count();
                if n <= left {
                    shown.push(span.clone());
                    left -= n;
                } else {
                    let p = left as f32 / n as f32;
                    shown.push(Span::styled(typewriter(&text, p).to_string(), span.style));
                    left = 0;
                    break;
                }
            }
            if cursor_on {
                shown.push(Span::styled("▮", Style::default().fg(MAGENTA)));
            }
            out.push(Line::from(shown));
            budget = 0;
            break;
        }
    }
    if p >= 1.0 {
        return lines.to_vec();
    }
    if out.len() == lines.len() && cursor_on {
        out.push(Line::from(Span::styled("▮", Style::default().fg(MAGENTA))));
    }
    out
}
```

`Screen` impl:

```rust
impl Screen for Deck {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            let rows = self.rows(st);
            if self.row >= rows.len() || rows.get(self.row).is_some_and(|r| r.detail.is_none()) {
                self.row = Self::first_selectable(&rows);
            }
            match k.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::Glitch,
                    };
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => self.shift_tab(-1, st.tick),
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.shift_tab(1, st.tick),
                KeyCode::Char(c @ '1'..='6') => {
                    let i = c as usize - '1' as usize;
                    self.set_tab(Tab::ALL[i], st.tick);
                }
                KeyCode::Up | KeyCode::Char('k') => self.move_row(&rows, -1, st.tick),
                KeyCode::Down | KeyCode::Char('j') => self.move_row(&rows, 1, st.tick),
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let tick = st.tick;
        let buf = f.buffer_mut();
        // Clear the panel body so the washed map does not bleed through the text.
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                buf[(x, y)].set_char(' ').set_bg(Color::Rgb(4, 6, 10)).set_fg(TEXT);
            }
        }
        frame(buf, area);
        let inner = Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), area.height.saturating_sub(2));
        let right = inner.right();
        let mut y = inner.y;

        // Title: block-font handle when tall enough, else a caption.
        let handle = st.character.handle.to_uppercase();
        let readout = format!(
            "¤ {} · hp {}/{} · {:.1} kg",
            st.character.credits,
            st.character.hp,
            st.character.hp_max,
            st.character.weight(&st.content)
        );
        if area.height >= LOGO_MIN_ROWS {
            let w = logo::width(&handle).min(inner.width);
            Logo { text: &handle, tick, accent: MAGENTA_RGB }.render(Rect::new(inner.x + 1, y, w, logo::ROWS), buf);
            y += logo::ROWS;
        } else {
            put(buf, inner.x + 1, y, &handle, Style::default().fg(CYAN).add_modifier(Modifier::BOLD), right);
        }
        let rx = right.saturating_sub(readout.chars().count() as u16 + 1);
        put(buf, rx, inner.y, &readout, Style::default().fg(DIM), right);
        y += 1;

        // Link line.
        let glow = 0.6 + 0.4 * pulse(tick, 40);
        put(buf, inner.x + 1, y, &Self::link_line(st), Style::default().fg(Rgb(0, 229, 255).scale(glow).to_color()), right);
        y += 1;

        // Tab bar.
        let mut x = inner.x + 1;
        let mut starts = [0u16; 6];
        for (i, tab) in Tab::ALL.iter().enumerate() {
            let active = *tab == self.tab;
            let label = if active { format!("▌{}▐", tab.label()) } else { format!(" {} ", tab.label()) };
            starts[i] = x;
            let style = if active {
                Style::default().fg(Rgb(0, 229, 255).scale(0.7 + 0.3 * pulse(tick, 30)).to_color()).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(DIM)
            };
            put(buf, x, y, &label, style, right);
            x += label.chars().count() as u16 + 2;
        }
        y += 1;
        // Rule with the sliding magenta segment.
        for cx in inner.x..right {
            buf[(cx, y)].set_char('─').set_fg(Color::Rgb(40, 60, 75));
        }
        let p = self.tab_tween.progress(tick);
        let from = starts[self.tab_from] as f32;
        let to = starts[self.tab.index()] as f32;
        let seg_x = (from + (to - from) * p).round() as u16;
        let seg_w = self.tab.label().chars().count() as u16 + 2;
        for cx in seg_x..(seg_x + seg_w).min(right) {
            buf[(cx, y)].set_char('━').set_fg(MAGENTA);
        }
        y += 1;

        // Body.
        let hint_y = inner.bottom().saturating_sub(1);
        let body = Rect::new(inner.x, y, inner.width, hint_y.saturating_sub(y));
        if self.tab == Tab::Map {
            self.draw_map(st, body, buf);
        } else {
            let list_w = (body.width * 2 / 5).max(28).min(body.width.saturating_sub(10));
            let list = Rect::new(body.x, body.y, list_w, body.height);
            let detail = Rect::new(body.x + list_w + 1, body.y, body.width.saturating_sub(list_w + 1), body.height);
            for cy in body.y..body.bottom() {
                buf[(body.x + list_w, cy)].set_char('║').set_fg(Color::Rgb(40, 60, 75));
            }
            let rows = self.rows(st);
            let first = self.row.saturating_sub(list.height.saturating_sub(1) as usize / 2);
            for (i, row) in rows.iter().enumerate().skip(first).take(list.height as usize) {
                let ry = list.y + (i - first) as u16;
                let selected = i == self.row && row.detail.is_some();
                let mut style = tone_style(row.tone);
                if selected {
                    style = style.bg(BAR_BG).fg(Rgb(0, 229, 255).scale(0.7 + 0.3 * pulse(tick, 30)).to_color()).add_modifier(Modifier::BOLD);
                    for cx in list.x..list.right() {
                        buf[(cx, ry)].set_bg(BAR_BG);
                    }
                }
                let text = format!("{}{}", if selected { "▸ " } else { "  " }, row.text);
                put(buf, list.x + 1, ry, &text, style, list.right());
            }
            if self.tab == Tab::Inventory {
                let footer = format!(
                    "{:.1} kg · ¤ {}",
                    st.character.weight(&st.content),
                    st.character.value(&st.content)
                );
                put(buf, list.x + 1, list.bottom().saturating_sub(1), &footer, Style::default().fg(DIM), list.right());
            }
            let lines = self.detail(st, &rows);
            let p = (tick.saturating_sub(self.changed)) as f32 / REVEAL_TICKS as f32;
            let shown = reveal(&lines, p, (tick / 4).is_multiple_of(2));
            Paragraph::new(shown)
                .wrap(Wrap { trim: false })
                .render(Rect::new(detail.x + 1, detail.y, detail.width.saturating_sub(2), detail.height), buf);
        }

        // Hints.
        put(buf, inner.x + 1, hint_y, "←→ tabs · ↑↓ select · 1-6 jump · esc close", Style::default().fg(DIM), right);

        // Signal jitter: a three-tick burst over one row every JITTER_PERIOD ticks.
        if tick % JITTER_PERIOD < 3 {
            let row = area.y + (hash64(tick / JITTER_PERIOD, 7) % area.height.max(1) as u64) as u16;
            let p = (tick % JITTER_PERIOD) as f32 / 3.0;
            for (gx, gy, ch) in glitch_cells(Rect::new(area.x, row, area.width, 1), tick, p) {
                let tint = if hash64(gx as u64, gy as u64).is_multiple_of(2) { CYAN } else { MAGENTA };
                buf[(gx, gy)].set_char(ch).set_fg(tint);
            }
        }
    }
}
```

with `const MAGENTA_RGB: Rgb = Rgb(255, 0, 200);` and, for this task, a placeholder-free Map body:

```rust
impl Deck {
    fn draw_map(&self, st: &AppState, body: Rect, buf: &mut Buffer) {
        // Task 8 replaces this with the memory map; until then the pane states the fact.
        let msg = match &st.player {
            Some(_) => "map link pending",
            None => "no signal — walk the city first",
        };
        put(buf, body.x + 1, body.y + 1, msg, Style::default().fg(DIM), body.right());
    }
}
```

Note `Line::width()` exists on ratatui 0.29 `Line`. The inventory footer overwrites the last list row only when the list is full; acceptable, the list rarely exceeds the pane.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p cyberpunk deck::`
Expected: PASS (3 tests). If `deck_fits_the_minimum_terminal` fails on a side border, some `put` ran past `right`; every `put` must pass `right`/`list.right()`.

- [ ] **Step 5: Run the suite and commit**

Run: `cargo test --workspace`
Expected: PASS.

```bash
git add crates/cyberpunk/src/screens/deck.rs crates/cyberpunk/src/screens/mod.rs
git commit -m "feat(screens): the deck — chrome, tabs, quests, gear, inventory"
```

---

### Task 8: Workbench, Skills, and the memory map

**Files:**
- Create: `crates/cyberpunk/src/render/memmap.rs`; add `pub mod memmap;` to `render/mod.rs`
- Modify: `crates/cyberpunk/src/screens/deck.rs` (`workbench_rows`, `skill_rows`, recipe/skill details, `draw_map`)
- Test: `deck.rs`, `memmap.rs`

**Interfaces:**
- Consumes: `Player::{vis, pos, stratum}`, `Character::{can_craft, skill_level, has_talent, count}`, `Content::talents_for`.
- Produces: `MemoryMap<'a> { city, stratum, vis: &Grid<Visibility>, player: Option<Pos>, style }: Widget`.

- [ ] **Step 1: Write the failing tests**

`deck.rs` tests:

```rust
    #[test]
    fn workbench_and_skills_show_what_you_can_and_cannot_do() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('4'))]);
        st.tick = 60;
        let rows = d.rows(&st);
        let shiv = rows.iter().find(|r| r.text.contains("Wire shiv")).unwrap();
        let optics = rows.iter().find(|r| r.text.contains("Optics tune")).unwrap();
        assert_eq!(shiv.tone, Tone::Normal);
        assert_eq!(optics.tone, Tone::Warn);
        let i = rows.iter().position(|r| r.text.contains("Optics tune")).unwrap();
        d.row = i;
        d.changed = 0;
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains("needs Electronics 4") && text.contains("you: 2"), "{text}");
        assert!(text.contains("1/1") || text.contains("0/1"), "have/need: {text}");

        d.update(&mut st, &[key(KeyCode::Char('6'))]);
        d.changed = 0;
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains("Hacking") && text.contains("▰▰▰▱▱ 3"), "{text}");
        let rows = d.rows(&st);
        d.row = rows.iter().position(|r| r.text.contains("Hacking")).unwrap();
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains("◈ Ice pick") && text.contains("Deep dive") && text.contains("needs 4"), "{text}");
    }

    #[test]
    fn map_tab_shows_memory_or_no_signal() {
        let mut st = state();
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('5'))]);
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains("no signal"), "{text}");
        let city = st.city.as_ref().unwrap();
        st.player = Some(crate::walker::Player::spawn(city, 1));
        let text = draw(&d, &st, 120, 40);
        assert!(text.contains('@'), "{text}");
        assert!(!text.contains("no signal"));
    }
```

`memmap.rs`:

```rust
//! The player's memory of a stratum: only what has been seen, scaled to fit.

use crate::render::style::StylePack;
use crate::render::{Rgb, Visibility};
use citygen::{City, Grid, Pos, Rect as TileRect, StratumKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;

pub struct MemoryMap<'a> {
    pub city: &'a City,
    pub stratum: StratumKind,
    pub vis: &'a Grid<Visibility>,
    pub player: Option<Pos>,
    pub style: &'a StylePack,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::Player;
    use citygen::{GenParams, Generator, SizePreset};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn memory_map_draws_only_what_was_seen() {
        let c = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let s = StylePack::load().unwrap();
        let p = Player::spawn(&c, 1);
        let mut t = Terminal::new(TestBackend::new(60, 30)).unwrap();
        t.draw(|f| {
            MemoryMap { city: &c, stratum: p.stratum, vis: p.vis(), player: Some(p.pos), style: &s }
                .render(f.area(), f.buffer_mut())
        })
        .unwrap();
        let buf = t.backend().buffer();
        let lit = (0..30u16)
            .flat_map(|y| (0..60u16).map(move |x| (x, y)))
            .filter(|(x, y)| matches!(buf[(*x, *y)].bg, Color::Rgb(r, g, b) if r + g + b > 30))
            .count();
        assert!(lit > 0 && lit < 60 * 30 / 4, "lit cells {lit}");
        assert!((0..30u16).any(|y| (0..60u16).any(|x| buf[(x, y)].symbol() == "@")));
        // Tiny areas never panic.
        let mut t = Terminal::new(TestBackend::new(1, 1)).unwrap();
        t.draw(|f| {
            MemoryMap { city: &c, stratum: p.stratum, vis: p.vis(), player: None, style: &s }
                .render(f.area(), f.buffer_mut())
        })
        .unwrap();
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p cyberpunk memmap::` (compile error) and `cargo test -p cyberpunk workbench_and_skills` (rows empty → `unwrap` on `None`).

- [ ] **Step 3: Implement**

`memmap.rs` above the tests:

```rust
impl Widget for MemoryMap<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let Some(st) = self.city.stratum(self.stratum) else { return };
        let size = self.city.size;
        let scale = (size.w as f32 / area.width as f32)
            .max(size.h as f32 / area.height as f32)
            .max(1.0)
            .ceil() as i32;
        let bg0 = self.style.background;
        for cy in 0..area.height {
            for cx in 0..area.width {
                let block = TileRect::new(cx as i32 * scale, cy as i32 * scale, scale, scale);
                let (mut total, mut seen, mut visible, mut walk) = (0usize, 0usize, 0usize, 0usize);
                let mut theme = None;
                for p in block.iter() {
                    let Some(t) = st.tiles.get(p) else { continue };
                    total += 1;
                    match self.vis.get(p).copied().unwrap_or(Visibility::Unseen) {
                        Visibility::Unseen => continue,
                        Visibility::Remembered => seen += 1,
                        Visibility::Visible => {
                            seen += 1;
                            visible += 1;
                        }
                    }
                    if t.is_walkable() {
                        walk += 1;
                    }
                    if theme.is_none() {
                        theme = t.district.and_then(|d| self.city.district_opt(d)).map(|d| d.theme);
                    }
                }
                let cell = &mut buf[(area.x + cx, area.y + cy)];
                if total == 0 || seen == 0 {
                    cell.set_char(' ').set_bg(bg0.to_color());
                    continue;
                }
                let accent = self.style.theme_or_neutral(theme).accent;
                let strength = if visible > 0 { 0.7 } else { 0.35 };
                let f = strength * (0.3 + 0.7 * walk as f32 / seen as f32);
                cell.set_char(' ').set_bg(accent.scale(f).to_color());
            }
        }
        if let Some(p) = self.player {
            let (cx, cy) = (p.x / scale, p.y / scale);
            if cx >= 0 && cy >= 0 && cx < area.width as i32 && cy < area.height as i32 {
                buf[(area.x + cx as u16, area.y + cy as u16)]
                    .set_char('@')
                    .set_fg(Color::Rgb(255, 255, 255));
            }
        }
    }
}
```

(`Rgb::scale` and `to_color` come from `crate::render::Rgb`; keep the import of `Rgb` only if used directly, otherwise drop it to satisfy clippy.)

`deck.rs`:

```rust
    fn workbench_rows(st: &AppState) -> Vec<Row> {
        st.content
            .recipes
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let ok = st.character.can_craft(&st.content, r);
                Row {
                    text: format!("{} {}", if ok { "▪" } else { "▫" }, r.name),
                    tone: if ok { Tone::Normal } else { Tone::Warn },
                    detail: Some(Detail::Recipe(i)),
                }
            })
            .collect()
    }

    fn skill_rows(st: &AppState) -> Vec<Row> {
        st.content
            .skills
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let lvl = st.character.skill_level(&s.id).min(5) as usize;
                let bar = format!("{}{}", "▰".repeat(lvl), "▱".repeat(5 - lvl));
                Row {
                    text: format!("{:<12} {bar} {lvl}", s.name),
                    tone: Tone::Normal,
                    detail: Some(Detail::Skill(i)),
                }
            })
            .collect()
    }
```

and in `detail`:

```rust
            Some(Detail::Recipe(i)) => {
                let r = &st.content.recipes[*i];
                let skill = st.content.skill(&r.skill).map(|s| s.name.clone()).unwrap_or_default();
                let have = st.character.skill_level(&r.skill);
                let mut out = vec![head(&r.name)];
                let need = format!("needs {skill} {}  (you: {have})", r.level);
                out.push(if have >= r.level { dim(need) } else { warn(need) });
                out.push(plain(""));
                for (id, n) in &r.inputs {
                    let name = st.content.item(id).map(|i| i.name.clone()).unwrap_or_else(|| id.to_string());
                    let got = st.character.count(id);
                    let line = format!("{got}/{n}  {name}");
                    out.push(if got >= *n { plain(line) } else { warn(line) });
                }
                let out_name = st.content.item(&r.output.0).map(|i| i.name.clone()).unwrap_or_default();
                out.push(Line::from(Span::styled(format!("→ {}× {out_name}", r.output.1), Style::default().fg(CYAN))));
                out.push(plain(""));
                out.push(plain(r.desc.clone()));
                out
            }
            Some(Detail::Skill(i)) => {
                let s = &st.content.skills[*i];
                let mut out = vec![head(&s.name)];
                out.push(dim(format!("stat · {}", s.stat.label())));
                out.push(plain(""));
                out.push(plain(s.desc.clone()));
                out.push(plain(""));
                out.push(dim("talents"));
                for t in st.content.talents_for(&s.id) {
                    if st.character.has_talent(&t.id) {
                        out.push(Line::from(Span::styled(format!("◈ {}", t.name), Style::default().fg(CYAN))));
                        out.push(dim(format!("   {}", t.desc)));
                    } else {
                        out.push(dim(format!("  {}  (needs {})", t.name, t.level)));
                    }
                }
                out
            }
```

`draw_map`:

```rust
    fn draw_map(&self, st: &AppState, body: Rect, buf: &mut Buffer) {
        match (&st.player, &st.city) {
            (Some(p), Some(city)) => {
                let inner = Rect::new(body.x + 1, body.y, body.width.saturating_sub(2), body.height);
                MemoryMap { city, stratum: p.stratum, vis: p.vis(), player: Some(p.pos), style: &st.style }
                    .render(inner, buf);
                put(buf, body.x + 1, body.y, &format!(" {} · what you remember ", p.stratum.label()), Style::default().fg(DIM).bg(Color::Rgb(4, 6, 10)), body.right());
            }
            _ => put(buf, body.x + 1, body.y + 1, "no signal — walk the city first", Style::default().fg(DIM), body.right()),
        }
    }
```

(import `crate::render::memmap::MemoryMap`.)

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/render/memmap.rs crates/cyberpunk/src/render/mod.rs crates/cyberpunk/src/screens/deck.rs
git commit -m "feat(screens): workbench, skills and the memory map in the deck"
```

---

### Task 9: Map keys: Tab opens the deck, `c` is the dev camera, jacking flashes; help and README

**Files:**
- Modify: `crates/cyberpunk/src/screens/explore.rs`, `crates/cyberpunk/src/screens/help.rs`, `README.md`
- Test: `explore.rs`

**Interfaces:**
- Consumes: `Deck::new`, `ScreenCmd::{Push, Flash}`.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn tab_opens_the_deck_and_c_is_the_dev_camera() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Tab)]),
            ScreenCmd::Push { fx: TransitionKind::Glitch, .. }
        ));
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        assert_eq!(s.mode, Mode::Walk);
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Explore, "c returns to the camera in dev mode");
        s.update(&mut st, &[key(KeyCode::Char('w'))]);
        st.dev = false;
        s.update(&mut st, &[key(KeyCode::Char('c'))]);
        assert_eq!(s.mode, Mode::Walk, "no camera without dev mode");
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Tab)]),
            ScreenCmd::Push { .. }
        ), "the deck opens in play too");
        st.tick += 500;
        let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
        t.draw(|f| s.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("tab deck") && !text.contains("c camera"), "{text}");
    }
```

And in `enter_jacks_in_and_the_net_is_the_fifth_stratum`, make the two `Enter` presses assert the flash:

```rust
        assert!(matches!(
            s.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Flash(TransitionKind::Glitch)
        ));
```

(both the jack-in and the jack-out press.)

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p cyberpunk tab_opens_the_deck` → fails (Tab still toggles the camera / returns `Stay`).

- [ ] **Step 3: Implement**

`explore.rs` `handle_common` gains, before `_ => return None`:

```rust
            KeyCode::Tab => {
                return Some(ScreenCmd::Push {
                    to: Box::new(Deck::new(st.tick)),
                    fx: TransitionKind::Glitch,
                });
            }
```

`handle_walk`: the `KeyCode::Tab if dev` arm becomes `KeyCode::Char('c') if dev`. The jack arm's two success branches end with `return Some(ScreenCmd::Flash(TransitionKind::Glitch));` after setting the status (the status message is set before the return). Hints:

```rust
        let cam = if st.dev { " · c camera" } else { "" };
        let hints = match (self.mode, jacked) {
            (Mode::Explore, _) => "hjkl pan · [] level · w walk · tab deck · d overlay · e legend · ? keys".to_string(),
            (Mode::Walk, true) => format!("⚡ jacked in · hjkl move · enter jack out · tab deck{cam} · ? keys"),
            (Mode::Walk, false) => format!("hjkl move · <> stairs · enter jack in · tab deck{cam} · ? keys"),
        };
```

`help.rs`: EXPLORE rows gain `("tab", "open the deck")` and `("esc q", "pause menu")` replaces `("q esc", "back to title")`; WALK rows: `("tab", "open the deck")`, `("c", "back to free camera (dev mode)")`, `("esc q", "pause menu")`. `README.md` Keys: Explore and Walk lines mention `tab` deck, `esc` pause, `c` camera (dev); a short "Deck" paragraph lists the tabs and that content comes from `flavor/content.ron`.

- [ ] **Step 4: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/screens/explore.rs crates/cyberpunk/src/screens/help.rs README.md
git commit -m "feat(screens): tab opens the deck, c is the dev camera, jacking flashes"
```

---

### Task 10: Lint, sweep, pty smoke

- [ ] **Step 1: Full checks**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo test -p citygen --test seeds -- --include-ignored`
Expected: all green.

- [ ] **Step 2: Pty smoke**

Drive the release binary (scratchpad script, not committed): title → new city → configure → generate → Esc shows `PAUSED` → ↓↓↓ Enter shows `SETTINGS` → ↓ → flips Rain to `[ OFF ]` → Esc → `PAUSED` → Esc back to the map → Tab shows `▌QUESTS▐` → `2`..`6` show `▌GEAR▐` … `▌SKILLS▐` → Esc → `w` → Tab → `5` shows `@` → Esc → Enter beside nothing shows "no terminal within reach" → Esc → ↓×5 Enter quits with exit code 0 and no panic.

- [ ] **Step 3: Commit anything the checks changed**

```bash
git add -A crates README.md
git commit -m "chore: lint and docs after the game menus"
```
(skip if nothing changed.)
