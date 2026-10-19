//! Content: everything the world is made of, parsed from `flavor/content.ron` and validated.

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
    #[error("{where_}: unknown ability `{id}`")]
    UnknownAbility { where_: String, id: String },
    #[error("item `{item}` has a use but is not a consumable")]
    UseOnNonConsumable { item: String },
    #[error("item `{item}` has perks or grants but no slot")]
    PerkWithoutSlot { item: String },
    #[error("item `{item}` cannot sit in slot {slot:?}")]
    SlotMismatch { item: String, slot: Slot },
    #[error("duplicate id `{0}`")]
    DuplicateId(String),
    #[error("ability `{ability}` is passive but has fx")]
    FxOnPassive { ability: String },
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
    #[error("{what} `{id}`: emblem must be three rows of seven glyphs")]
    EmblemShape { what: String, id: String },
    #[error("template `{id}`: {why}")]
    TemplateInvalid { id: String, why: String },
    #[error("talent `{id}`: {why}")]
    TalentInvalid { id: String, why: String },
    #[error("recipe `{id}`: {why}")]
    RecipeInvalid { id: String, why: String },
    #[error("program `{id}`: {why}")]
    ProgramInvalid { id: String, why: String },
    #[error("deck `{id}`: {why}")]
    DeckInvalid { id: String, why: String },
    #[error("item `{id}`: {why}")]
    SocketsInvalid { id: String, why: String },
    #[error("hardware `{id}`: {why}")]
    HardwareInvalid { id: String, why: String },
    #[error("item `{id}`: {why}")]
    LookInvalid { id: String, why: String },
    #[error("sticker `{id}`: {why}")]
    StickerInvalid { id: String, why: String },
    #[error("tree {name}: {why}")]
    TreeInvalid { name: String, why: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Content {
    pub items: Vec<Item>,
    pub skills: Vec<Skill>,
    pub talents: Vec<Talent>,
    #[serde(default)]
    pub trees: Vec<Tree>,
    pub quests: Vec<Quest>,
    pub recipes: Vec<Recipe>,
    #[serde(default)]
    pub abilities: Vec<Ability>,
    pub creation: Creation,
    pub origins: Vec<Origin>,
    pub professions: Vec<Profession>,
    pub categories: Vec<Category>,
    pub templates: Vec<Template>,
    pub stat_blurbs: Vec<(Stat, String)>,
    /// Files that land on a host's disk when its theme and building kind match.
    #[serde(default)]
    pub lore: Vec<Lore>,
}

/// A file written on matching hosts. `{resident}` in the path is the first resident's user.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lore {
    pub path: String,
    /// The device theme name; `None` matches any.
    pub theme: Option<String>,
    /// The building kind label; `None` matches any.
    pub kind: Option<String>,
    pub text: String,
}

/// Lettering a sticker may carry, in cells, and a picture's size in pixels.
const LETTERED_MAX: usize = 18;
const PICTURE_MAX: (usize, usize) = (14, 12);

/// A sticker is art and nothing else, and its art draws; anything else has no art.
fn sticker_ok(i: &Item) -> Result<(), String> {
    let art = match (i.kind, &i.sticker) {
        (ItemKind::Sticker, Some(art)) => art,
        (ItemKind::Sticker, None) => return Err("no art".into()),
        (_, Some(_)) => return Err("sticker art on something that is not a sticker".into()),
        (_, None) => return Ok(()),
    };
    let more = i.slot.is_some()
        || !i.sockets.is_empty()
        || i.hardware.is_some()
        || i.deck.is_some()
        || i.program.is_some()
        || i.look.is_some()
        || !i.stickers.is_empty();
    if more {
        return Err("a sticker is its art and nothing else".into());
    }
    match art {
        StickerArt::Lettered { text, .. } => {
            let n = text.chars().count();
            if n == 0 || n > LETTERED_MAX {
                return Err(format!("lettering runs 1 to {LETTERED_MAX} cells"));
            }
        }
        StickerArt::Picture { rows, colours } => {
            let w = rows.first().map_or(0, |r| r.chars().count());
            let (most_w, most_h) = PICTURE_MAX;
            if w == 0 || w > most_w || rows.is_empty() || rows.len() > most_h {
                return Err(format!(
                    "a picture is 1 to {most_w} pixels wide and 1 to {most_h} tall"
                ));
            }
            if rows.iter().any(|r| r.chars().count() != w) {
                return Err("its rows differ in length".into());
            }
            let pixels: Vec<char> = rows
                .iter()
                .flat_map(|r| r.chars())
                .filter(|&c| c != '.')
                .collect();
            if let Some(c) = pixels.iter().find(|c| !colours.contains_key(c)) {
                return Err(format!("pixel `{c}` has no colour"));
            }
            if pixels.is_empty() {
                return Err("no pixel is set".into());
            }
        }
    }
    Ok(())
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
            .chain(self.recipes.iter().map(|r| r.id.0.as_str()))
            .chain(self.abilities.iter().map(|a| a.id.0.as_str()));
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
        let need_ability = |where_: String, id: &AbilityId| -> Result<&Ability, ContentError> {
            self.ability(id)
                .ok_or_else(|| ContentError::UnknownAbility {
                    where_,
                    id: id.0.clone(),
                })
        };
        for i in &self.items {
            match (i.kind, i.program) {
                (ItemKind::Program, None) => {
                    return Err(ContentError::ProgramInvalid {
                        id: i.id.0.clone(),
                        why: "no program numbers".into(),
                    });
                }
                (ItemKind::Program, Some(p)) => {
                    if i.slot.is_some() {
                        return Err(ContentError::ProgramInvalid {
                            id: i.id.0.clone(),
                            why: "a program has no slot; it is loaded".into(),
                        });
                    }
                    if p.rating == 0 || p.size == 0 || p.work == 0 {
                        return Err(ContentError::ProgramInvalid {
                            id: i.id.0.clone(),
                            why: "rating, size and work are at least one".into(),
                        });
                    }
                }
                (_, Some(_)) => {
                    return Err(ContentError::ProgramInvalid {
                        id: i.id.0.clone(),
                        why: "program numbers on something that is not a program".into(),
                    });
                }
                (_, None) => {}
            }
            match (i.kind, i.deck) {
                (ItemKind::Deck, None) => {
                    return Err(ContentError::DeckInvalid {
                        id: i.id.0.clone(),
                        why: "no slots".into(),
                    });
                }
                (ItemKind::Deck, Some(d)) if d.slots == 0 => {
                    return Err(ContentError::DeckInvalid {
                        id: i.id.0.clone(),
                        why: "a deck has at least one slot".into(),
                    });
                }
                (ItemKind::Deck, Some(d)) if d.slots > d.look.max_slots() => {
                    return Err(ContentError::DeckInvalid {
                        id: i.id.0.clone(),
                        why: format!(
                            "a {} deck has room for {} slots",
                            d.look.name(),
                            d.look.max_slots()
                        ),
                    });
                }
                (ItemKind::Deck, Some(d)) if i.grants.len() > d.look.max_grants() => {
                    return Err(ContentError::DeckInvalid {
                        id: i.id.0.clone(),
                        why: format!(
                            "a {} deck has room for {} ROM abilities",
                            d.look.name(),
                            d.look.max_grants()
                        ),
                    });
                }
                (k, Some(_)) if k != ItemKind::Deck => {
                    return Err(ContentError::DeckInvalid {
                        id: i.id.0.clone(),
                        why: "deck slots on something that is not a deck".into(),
                    });
                }
                _ => {}
            }
            if !i.sockets.is_empty() {
                // Only a drawing can place sockets, and each kind must sit where it is drawn.
                let drawn = i.drawn_sockets();
                for (n, kind) in i.sockets.iter().enumerate() {
                    let why = match drawn.get(n) {
                        None => format!("its drawing places no socket {}", n + 1),
                        Some(&(_, want)) if want != *kind => {
                            format!("socket {} is a {} on its drawing", n + 1, want.fits())
                        }
                        _ => continue,
                    };
                    return Err(ContentError::SocketsInvalid {
                        id: i.id.0.clone(),
                        why,
                    });
                }
            }
            if i.look.is_some() && (i.deck.is_some() || i.hardware.is_some()) {
                return Err(ContentError::LookInvalid {
                    id: i.id.0.clone(),
                    why: "a deck or hardware wears its own look".into(),
                });
            }
            if let Some(h) = &i.hardware
                && h.look.kind() != h.fits
            {
                return Err(ContentError::HardwareInvalid {
                    id: i.id.0.clone(),
                    why: format!("its look is drawn for a {}", h.look.kind().fits()),
                });
            }
            match (i.kind, &i.hardware) {
                (ItemKind::Hardware, None) => {
                    return Err(ContentError::HardwareInvalid {
                        id: i.id.0.clone(),
                        why: "no hardware numbers".into(),
                    });
                }
                (ItemKind::Hardware, Some(h)) if h.mods.is_empty() => {
                    return Err(ContentError::HardwareInvalid {
                        id: i.id.0.clone(),
                        why: "hardware does at least one thing".into(),
                    });
                }
                (k, Some(_)) if k != ItemKind::Hardware => {
                    return Err(ContentError::HardwareInvalid {
                        id: i.id.0.clone(),
                        why: "hardware numbers on something that is not hardware".into(),
                    });
                }
                _ => {}
            }
            if let Err(why) = sticker_ok(i) {
                return Err(ContentError::StickerInvalid {
                    id: i.id.0.clone(),
                    why,
                });
            }
            if !i.stickers.is_empty() && !i.takes_stickers() {
                return Err(ContentError::StickerInvalid {
                    id: i.id.0.clone(),
                    why: "stickers on something that takes none".into(),
                });
            }
            for s in &i.stickers {
                let on = need_item(format!("item {} stickers", i.id), &s.sticker)?;
                if on.kind != ItemKind::Sticker {
                    return Err(ContentError::StickerInvalid {
                        id: i.id.0.clone(),
                        why: format!("`{}` is not a sticker", on.id),
                    });
                }
            }
            if i.on_use.is_some() && i.kind != ItemKind::Consumable {
                return Err(ContentError::UseOnNonConsumable {
                    item: i.id.0.clone(),
                });
            }
            if (!i.perks.is_empty() || !i.grants.is_empty()) && i.slot.is_none() {
                return Err(ContentError::PerkWithoutSlot {
                    item: i.id.0.clone(),
                });
            }
            for id in &i.grants {
                need_ability(format!("item {}", i.id), id)?;
            }
        }
        for tree in &self.trees {
            let bad = |why: String| ContentError::TreeInvalid {
                name: tree.name.clone(),
                why,
            };
            if tree.tiers.is_empty() {
                return Err(bad("no tiers".into()));
            }
            for (i, tier) in tree.tiers.iter().enumerate() {
                if tier.is_empty() {
                    return Err(bad(format!("tier {} is empty", i + 1)));
                }
                for id in tier {
                    self.talent(id).ok_or_else(|| ContentError::UnknownTalent {
                        where_: format!("tree {}", tree.name),
                        id: id.0.clone(),
                    })?;
                }
            }
            if self.trees.iter().filter(|o| o.stat == tree.stat).count() > 1 {
                return Err(bad(format!("two trees for {}", tree.stat.name())));
            }
        }
        for t in &self.talents {
            if let Some(skill) = &t.skill {
                need_skill(format!("talent {}", t.id), skill)?;
            }
            let bad = |why: String| ContentError::TalentInvalid {
                id: t.id.0.clone(),
                why,
            };
            if !(1..=3).contains(&t.ranks) {
                return Err(bad(format!("{} ranks; one to three", t.ranks)));
            }
            if t.effects.len() != t.ranks as usize {
                return Err(bad(format!(
                    "{} ranks but {} effect lists",
                    t.ranks,
                    t.effects.len()
                )));
            }
            for b in t.effects.iter().flatten() {
                if let Boon::Grant(a) | Boon::Sharpen { ability: a, .. } = b {
                    need_ability(format!("talent {}", t.id), a)?;
                }
            }
            let places = self
                .trees
                .iter()
                .flat_map(|tree| {
                    tree.tiers.iter().enumerate().flat_map(move |(i, tier)| {
                        tier.iter()
                            .enumerate()
                            .filter(|(_, id)| *id == &t.id)
                            .map(move |(j, _)| (tree, i, j))
                    })
                })
                .collect::<Vec<_>>();
            let (tree, tier, col) = match places.as_slice() {
                [] => return Err(bad("in no tree".into())),
                [one] => *one,
                _ => return Err(bad("in two trees".into())),
            };
            if let Some(req) = &t.requires {
                let Some((rtree, rtier, rcol)) = self.tree_of(req) else {
                    return Err(bad(format!("prerequisite {req} is in no tree")));
                };
                if rtree.stat != tree.stat || rtier >= tier || rcol != col {
                    return Err(bad(format!(
                        "prerequisite {req} must sit in the same column of a lower tier of {}",
                        tree.name
                    )));
                }
            }
            if tier + 1 == tree.tiers.len() && t.ranks != 1 {
                return Err(bad("a capstone has one rank".into()));
            }
        }
        for a in &self.abilities {
            if a.kind == AbilityKind::Passive && !a.fx.is_empty() {
                return Err(ContentError::FxOnPassive {
                    ability: a.id.0.clone(),
                });
            }
        }
        for r in &self.recipes {
            need_skill(format!("recipe {}", r.id), &r.skill)?;
            for (id, _) in &r.inputs {
                need_item(format!("recipe {}", r.id), id)?;
            }
            need_item(format!("recipe {} output", r.id), &r.output.0)?;
            let bad = |why: String| ContentError::RecipeInvalid {
                id: r.id.0.clone(),
                why,
            };
            if r.steps.len() > 4 {
                return Err(bad("at most four steps".into()));
            }
            if let Some(s) = r.steps.iter().find(|s| s.chars().count() > 16) {
                return Err(bad(format!("step `{s}` is longer than 16 characters")));
            }
            for (id, _) in &r.inputs {
                if self.item(id).is_some_and(Item::per_copy) {
                    return Err(bad(format!("input `{id}` is held per copy")));
                }
            }
        }
        self.validate_creation(&need_item, &need_skill, &need_ability)?;
        Ok(())
    }

    /// Origins, professions, categories and templates: ids unique within their kind,
    /// every reference resolving, every skill in exactly one category with its first
    /// skill free, emblems three rows of seven glyphs.
    fn validate_creation<'a>(
        &'a self,
        need_item: &dyn Fn(String, &ItemId) -> Result<&'a Item, ContentError>,
        need_skill: &dyn Fn(String, &SkillId) -> Result<&'a Skill, ContentError>,
        need_ability: &dyn Fn(String, &AbilityId) -> Result<&'a Ability, ContentError>,
    ) -> Result<(), ContentError> {
        fn unique<'b>(ids: impl Iterator<Item = &'b str>) -> Result<(), ContentError> {
            let mut seen: HashSet<&str> = HashSet::new();
            for id in ids {
                if !seen.insert(id) {
                    return Err(ContentError::DuplicateId(id.to_string()));
                }
            }
            Ok(())
        }
        unique(self.origins.iter().map(|o| o.id.0.as_str()))?;
        unique(self.professions.iter().map(|p| p.id.0.as_str()))?;
        unique(self.categories.iter().map(|c| c.id.0.as_str()))?;
        unique(self.templates.iter().map(|t| t.id.0.as_str()))?;
        let emblem_ok =
            |rows: &[String]| rows.len() == 3 && rows.iter().all(|r| r.chars().count() == 7);
        for o in &self.origins {
            if !emblem_ok(&o.emblem) {
                return Err(ContentError::EmblemShape {
                    what: "origin".into(),
                    id: o.id.0.clone(),
                });
            }
        }
        for p in &self.professions {
            if !emblem_ok(&p.emblem) {
                return Err(ContentError::EmblemShape {
                    what: "profession".into(),
                    id: p.id.0.clone(),
                });
            }
            for (id, _) in &p.kit {
                need_item(format!("profession {}", p.id), id)?;
            }
        }
        let mut owner: std::collections::HashMap<&SkillId, &CategoryId> =
            std::collections::HashMap::new();
        for cat in &self.categories {
            for (i, s) in cat.skills.iter().enumerate() {
                need_skill(format!("category {}", cat.id), &s.id)?;
                if i == 0 && s.cost != 0 {
                    return Err(ContentError::FirstSkillCosts {
                        category: cat.id.0.clone(),
                        skill: s.id.0.clone(),
                    });
                }
                if let Some(other) = owner.insert(&s.id, &cat.id) {
                    return Err(ContentError::SkillInTwoCategories {
                        id: s.id.0.clone(),
                        a: other.0.clone(),
                        b: cat.id.0.clone(),
                    });
                }
            }
            for id in &cat.grants {
                need_ability(format!("category {}", cat.id), id)?;
            }
        }
        for s in &self.skills {
            if !owner.contains_key(&s.id) {
                return Err(ContentError::SkillInNoCategory { id: s.id.0.clone() });
            }
        }
        for t in &self.templates {
            let where_ = format!("template {}", t.id);
            self.origin(&t.origin)
                .ok_or_else(|| ContentError::UnknownOrigin {
                    where_: where_.clone(),
                    id: t.origin.0.clone(),
                })?;
            self.profession(&t.profession)
                .ok_or_else(|| ContentError::UnknownProfession {
                    where_: where_.clone(),
                    id: t.profession.0.clone(),
                })?;
            for id in &t.categories {
                self.category(id)
                    .ok_or_else(|| ContentError::UnknownCategory {
                        where_: where_.clone(),
                        id: id.0.clone(),
                    })?;
            }
            for id in &t.skills {
                need_skill(where_.clone(), id)?;
            }
            for (id, _) in &t.kit {
                need_item(where_.clone(), id)?;
            }
        }
        for t in &self.templates {
            crate::build::Build::from_template(self, t)
                .validate(self)
                .map_err(|e| ContentError::TemplateInvalid {
                    id: t.id.0.clone(),
                    why: e.to_string(),
                })?;
        }
        Ok(())
    }

    pub fn item(&self, id: &ItemId) -> Option<&Item> {
        self.items.iter().find(|i| &i.id == id)
    }
    pub fn origin(&self, id: &OriginId) -> Option<&Origin> {
        self.origins.iter().find(|o| &o.id == id)
    }
    pub fn profession(&self, id: &ProfessionId) -> Option<&Profession> {
        self.professions.iter().find(|p| &p.id == id)
    }
    pub fn category(&self, id: &CategoryId) -> Option<&Category> {
        self.categories.iter().find(|c| &c.id == id)
    }
    pub fn template(&self, id: &TemplateId) -> Option<&Template> {
        self.templates.iter().find(|t| &t.id == id)
    }
    /// The category a skill sits in.
    pub fn category_of(&self, skill: &SkillId) -> Option<&Category> {
        self.categories
            .iter()
            .find(|c| c.skills.iter().any(|s| &s.id == skill))
    }
    /// What a skill costs in SP, by its category's listing.
    pub fn skill_cost(&self, skill: &SkillId) -> Option<u32> {
        self.category_of(skill)?
            .skills
            .iter()
            .find(|s| &s.id == skill)
            .map(|s| s.cost)
    }
    pub fn stat_blurb(&self, stat: Stat) -> &str {
        self.stat_blurbs
            .iter()
            .find(|(s, _)| *s == stat)
            .map(|(_, b)| b.as_str())
            .unwrap_or("")
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

    pub fn ability(&self, id: &AbilityId) -> Option<&Ability> {
        self.abilities.iter().find(|a| &a.id == id)
    }

    /// Talents of one skill, in content order.
    pub fn talents_for<'a>(&'a self, skill: &'a SkillId) -> impl Iterator<Item = &'a Talent> + 'a {
        self.talents
            .iter()
            .filter(move |t| t.skill.as_ref() == Some(skill))
    }

    /// The tree under a stat.
    pub fn tree(&self, stat: Stat) -> Option<&Tree> {
        self.trees.iter().find(|t| t.stat == stat)
    }

    /// Where a talent hangs: its tree, tier and column.
    pub fn tree_of(&self, id: &TalentId) -> Option<(&Tree, usize, usize)> {
        self.trees.iter().find_map(|tree| {
            tree.tiers
                .iter()
                .enumerate()
                .find_map(|(i, tier)| tier.iter().position(|t| t == id).map(|j| (tree, i, j)))
        })
    }
}

/// What a talent flag does today, for the talents tab; a flag nothing reads yet
/// has no words and shows as a promise.
pub fn flag_words(flag: &str) -> Option<&'static str> {
    Some(match flag {
        "ice_pick" => "+2 power to crack",
        "deep_dive" => "+2 power to copy · a vault copy makes no heat",
        "black_ice" => "+1 attack damage",
        "null_route" => "the first dump in a construct routes away",
        "deep_end" => {
            "black ICE hits halved · the failsafe waits for an eighth · ten minutes cooling"
        }
        _ => return None,
    })
}

impl Boon {
    /// The boon in words for the sheet: `+1 Reflex`, `ping reaches 4 further`.
    pub fn words(&self, c: &Content) -> String {
        let ability_name = |id: &AbilityId| {
            c.ability(id)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| id.to_string())
        };
        match self {
            Boon::Perk(Perk::Stat(s, n)) => {
                let mut name = s.name().to_ascii_lowercase();
                if let Some(first) = name.get_mut(0..1) {
                    first.make_ascii_uppercase();
                }
                format!("{n:+} {name}")
            }
            Boon::Perk(Perk::HpMax(n)) => format!("{n:+} max HP"),
            Boon::Perk(Perk::Focus(n)) => format!("{n:+} focus"),
            Boon::Perk(Perk::Sight(n)) => format!("sight {n:+}"),
            Boon::Perk(Perk::Carry(kg)) => format!("carry {kg:+} kg"),
            Boon::Grant(a) => format!("grants {}", ability_name(a)),
            Boon::Sharpen {
                ability,
                radius,
                cooldown,
            } => {
                let mut parts = Vec::new();
                match radius.signum() {
                    1 => parts.push(format!("reaches {radius} further")),
                    -1 => parts.push(format!("reaches {} less", -radius)),
                    _ => {}
                }
                match cooldown.signum() {
                    -1 => parts.push(format!("cools {} ticks sooner", -cooldown)),
                    1 => parts.push(format!("cools {cooldown} ticks later")),
                    _ => {}
                }
                if parts.is_empty() {
                    parts.push("unchanged".into());
                }
                format!(
                    "{} {}",
                    ability_name(ability).to_lowercase(),
                    parts.join(", ")
                )
            }
            Boon::Flag(f) => flag_words(f)
                .map(str::to_string)
                .unwrap_or_else(|| format!("later: {f}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ratchet_builds_from_five_parts_and_older_recipes_have_steps() {
        let c = Content::load().unwrap();
        let r = c.recipe(&RecipeId::new("ratchet_build")).unwrap();
        assert_eq!(r.skill, SkillId::new("scrapcraft"));
        let parts = [
            "ratchet_receiver",
            "ratchet_stock",
            "ratchet_barrel",
            "ratchet_mag",
            "ratchet_trigger",
        ];
        assert_eq!(r.inputs, parts.map(|p| (ItemId::new(p), 1)).to_vec());
        assert_eq!(r.output, (ItemId::new("ratchet_smg"), 1));
        assert!(r.steps.is_empty());
        for r in c.recipes.iter().filter(|r| r.id.0 != "ratchet_build") {
            assert!((2..=4).contains(&r.steps.len()), "{}", r.id);
        }
    }

    #[test]
    fn validate_refuses_bad_recipes() {
        let good = Content::load().unwrap();
        let mut c = good.clone();
        c.recipes[0].steps = vec!["a".into(); 5];
        assert!(matches!(
            c.validate(),
            Err(ContentError::RecipeInvalid { .. })
        ));
        let mut c = good.clone();
        c.recipes[0].steps = vec!["x".repeat(17)];
        assert!(matches!(
            c.validate(),
            Err(ContentError::RecipeInvalid { .. })
        ));
        let mut c = good.clone();
        c.recipes[0].inputs[0].0 = ItemId::new("ratchet_smg");
        assert!(matches!(
            c.validate(),
            Err(ContentError::RecipeInvalid { .. })
        ));
    }

    #[test]
    fn embedded_content_parses_and_covers_the_kit() {
        let c = Content::load().unwrap();
        assert!(c.items.len() >= 14, "{} items", c.items.len());
        assert_eq!(c.skills.len(), 24);
        assert!(c.talents.len() >= 8);
        assert!(c.quests.is_empty(), "authored quests come with spec 2");
        assert!(c.recipes.len() >= 6);
        for kind in ItemKind::ALL {
            assert!(c.items.iter().any(|i| i.kind == kind), "no {kind:?} item");
        }
        for slot in Slot::ALL {
            assert!(
                c.items.iter().any(|i| i.slot == Some(slot)),
                "no item for {slot:?}"
            );
        }
        let vex = c.template(&TemplateId::new("vex")).unwrap();
        assert_eq!(vex.handle, "VEX");
        assert_eq!(
            c.item(&ItemId::new("copper_coil")).unwrap().kind,
            ItemKind::Component
        );
        assert_eq!(c.talents_for(&SkillId::new("hacking")).count(), 2);
    }

    #[test]
    fn validation_catches_broken_content() {
        let good = Content::load().unwrap();

        let mut c = good.clone();
        c.abilities
            .iter_mut()
            .find(|a| a.id == AbilityId::new("lowlight"))
            .unwrap()
            .fx
            .push(Fx::Kick(1));
        assert!(matches!(
            c.validate(),
            Err(ContentError::FxOnPassive { .. })
        ));

        let mut c = good.clone();
        c.recipes[0].inputs[0].0 = ItemId::new("unobtainium");
        assert!(matches!(
            c.validate(),
            Err(ContentError::UnknownItem { .. })
        ));

        let mut c = good.clone();
        let dup = c.items[0].clone();
        c.items.push(dup);
        assert!(matches!(c.validate(), Err(ContentError::DuplicateId(_))));

        let mut c = good.clone();
        c.talents[0].skill = Some(SkillId::new("origami"));
        assert!(matches!(
            c.validate(),
            Err(ContentError::UnknownSkill { .. })
        ));

        let mut c = good.clone();
        c.trees[0].tiers[0].push(TalentId::new("levitation"));
        assert!(matches!(
            c.validate(),
            Err(ContentError::UnknownTalent { .. })
        ));

        let mut c = good.clone();
        c.templates[0].stats.body = 8;
        c.templates[0].stats.cool = 8;
        assert!(matches!(
            c.validate(),
            Err(ContentError::TemplateInvalid { .. })
        ));

        let mut c = good.clone();
        let coil = c
            .items
            .iter()
            .position(|i| i.id.0 == "copper_coil")
            .unwrap();
        c.items[coil].on_use = Some(Use::Heal(1));
        assert!(matches!(
            c.validate(),
            Err(ContentError::UseOnNonConsumable { .. })
        ));

        let mut c = good.clone();
        c.items[coil].perks.push(Perk::Sight(1));
        assert!(matches!(
            c.validate(),
            Err(ContentError::PerkWithoutSlot { .. })
        ));

        let mut c = good.clone();
        c.items[coil].grants.push(AbilityId::new("lowlight"));
        assert!(matches!(
            c.validate(),
            Err(ContentError::PerkWithoutSlot { .. })
        ));

        let mut c = good.clone();
        let optics = c
            .items
            .iter()
            .position(|i| i.id.0 == "nightglass_optics")
            .unwrap();
        c.items[optics].grants.push(AbilityId::new("flight"));
        assert!(matches!(
            c.validate(),
            Err(ContentError::UnknownAbility { .. })
        ));

        let mut c = good.clone();
        let dup = c.abilities[0].clone();
        c.abilities.push(dup);
        assert!(matches!(c.validate(), Err(ContentError::DuplicateId(_))));

        assert!(matches!(
            Content::parse("not ron"),
            Err(ContentError::Parse(_))
        ));
    }

    #[test]
    fn items_carry_perks_grants_and_uses() {
        let c = Content::load().unwrap();
        let optics = c.item(&ItemId::new("nightglass_optics")).unwrap();
        assert!(optics.perks.contains(&Perk::Sight(4)), "{:?}", optics.perks);
        assert_eq!(optics.grants, vec![AbilityId::new("lowlight")]);
        let stim = c.item(&ItemId::new("stim_patch")).unwrap();
        assert_eq!(stim.on_use, Some(Use::Heal(15)));
        let coil = c.item(&ItemId::new("copper_coil")).unwrap();
        assert!(coil.perks.is_empty() && coil.grants.is_empty() && coil.on_use.is_none());
        assert_eq!(
            c.item(&ItemId::new("mirrorshades")).unwrap().slot,
            Some(Slot::Eyes)
        );
        assert!(c.abilities.len() >= 4);
        let scan = c.ability(&AbilityId::new("netscan")).unwrap();
        assert!(matches!(
            scan.kind,
            AbilityKind::Active(Active::Scan { radius: 12 })
        ));
        assert_eq!(scan.cooldown, 300);
        assert!(matches!(
            c.ability(&AbilityId::new("ping")).unwrap().kind,
            AbilityKind::Active(Active::Ping { radius: 24 })
        ));
        assert_eq!(
            scan.fx,
            vec![Fx::Wave {
                colour: (0, 229, 255),
                ticks: 15
            }]
        );
        assert_eq!(
            c.ability(&AbilityId::new("ping")).unwrap().fx,
            vec![
                Fx::Wave {
                    colour: (255, 0, 200),
                    ticks: 20
                },
                Fx::Glow {
                    colour: (255, 0, 200),
                    ticks: 180
                }
            ]
        );
        assert!(
            c.ability(&AbilityId::new("lowlight"))
                .unwrap()
                .fx
                .is_empty()
        );
        assert!(matches!(
            c.ability(&AbilityId::new("lowlight")).unwrap().kind,
            AbilityKind::Passive
        ));
        let trace = c.ability(&AbilityId::new("arc_trace")).unwrap();
        assert!(matches!(
            trace.kind,
            AbilityKind::Active(Active::Trace { radius: 20 })
        ));
        assert_eq!(
            trace.fx,
            vec![
                Fx::Bolt {
                    colour: (255, 240, 150),
                    ticks: 36
                },
                Fx::Mark {
                    colour: (255, 240, 150),
                    ticks: 600
                }
            ]
        );
        assert_eq!(trace.cooldown, 450);
        assert_eq!(
            c.item(&ItemId::new("pawnshop_deck")).unwrap().grants,
            vec![AbilityId::new("netscan"), AbilityId::new("arc_trace")]
        );
    }

    #[test]
    fn creation_content_loads_and_resolves() {
        let c = Content::load().unwrap();
        assert_eq!(
            (
                c.creation.stat_base,
                c.creation.stat_cap,
                c.creation.stat_points
            ),
            (3, 8, 20)
        );
        assert_eq!(
            (c.creation.point_cost, c.creation.sp, c.creation.name_max),
            (2, 6, 16)
        );
        assert_eq!(c.origins.len(), 4);
        assert_eq!(c.professions.len(), 4);
        assert_eq!(c.categories.len(), 8);
        assert_eq!(c.templates.len(), 2);
        assert_eq!(c.skills.len(), 24);
        for o in &c.origins {
            assert_eq!(
                (o.cheap.len(), o.dear.len(), o.emblem.len()),
                (2, 2, 3),
                "{}",
                o.id
            );
        }
        for p in &c.professions {
            assert_eq!(
                (p.cheap.len(), p.dear.len(), p.emblem.len()),
                (1, 1, 3),
                "{}",
                p.id
            );
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
        assert!(
            c.talents
                .iter()
                .all(|t| t.skill.as_ref().is_none_or(|s| c.skill(s).is_some()))
        );
        assert!(c.recipes.iter().all(|r| c.skill(&r.skill).is_some()));
    }

    #[test]
    fn creation_content_is_validated() {
        let c = Content::load().unwrap();
        let mut two = c.clone();
        two.categories[1].skills.push(CategorySkill {
            id: SkillId::new("hacking"),
            cost: 1,
        });
        assert!(matches!(
            two.validate(),
            Err(ContentError::SkillInTwoCategories { .. })
        ));
        let mut none = c.clone();
        none.skills.push(Skill {
            id: SkillId::new("origami"),
            name: "Origami".into(),
            stat: Stat::Tech,
            desc: "paper".into(),
        });
        assert!(matches!(
            none.validate(),
            Err(ContentError::SkillInNoCategory { .. })
        ));
        let mut costs = c.clone();
        costs.categories[0].skills[0].cost = 1;
        assert!(matches!(
            costs.validate(),
            Err(ContentError::FirstSkillCosts { .. })
        ));
        let mut kit = c.clone();
        kit.professions[0].kit.push((ItemId::new("ghost_item"), 1));
        assert!(matches!(
            kit.validate(),
            Err(ContentError::UnknownItem { .. })
        ));
        let mut t = c.clone();
        t.templates[0].origin = OriginId::new("nowhere");
        assert!(matches!(
            t.validate(),
            Err(ContentError::UnknownOrigin { .. })
        ));
        let mut e = c.clone();
        e.origins[0].emblem.pop();
        assert!(matches!(
            e.validate(),
            Err(ContentError::EmblemShape { .. })
        ));
    }

    #[test]
    fn the_trees_load_and_validate() {
        let c = Content::load().unwrap();
        assert_eq!(c.trees.len(), 5);
        for t in &c.trees {
            assert_eq!(t.tiers.len(), 5, "{}", t.name);
            assert!(
                t.tiers[..4].iter().all(|tier| tier.len() == 2),
                "{}",
                t.name
            );
            assert_eq!(t.tiers[4].len(), 1, "{} capstone", t.name);
        }
        assert!(c.talents.len() >= 40);
        for id in [
            "quick_draw",
            "double_tap",
            "backstab",
            "ghost",
            "ice_pick",
            "deep_dive",
            "solder_hand",
            "field_medic",
        ] {
            assert!(c.tree_of(&TalentId::new(id)).is_some(), "{id} in a tree");
        }
        assert_eq!(c.creation.talent_points, 1);
        assert_eq!(c.creation.tier_gate, 3);
        for stat in [Stat::Body, Stat::Reflex, Stat::Tech, Stat::Cool, Stat::Net] {
            assert_eq!(c.tree(stat).unwrap().stat, stat);
        }
    }

    #[test]
    fn validation_refuses_a_misplaced_prerequisite_and_bad_ranks() {
        let c = Content::load().unwrap();
        let dt = TalentId::new("double_tap");
        let (stat, tier, col) = c.tree_of(&dt).map(|(t, i, j)| (t.stat, i, j)).unwrap();
        assert!(tier >= 1);
        let other = c.tree(stat).unwrap().tiers[tier - 1][1 - col.min(1)].clone();
        let mut bad = c.clone();
        bad.talents
            .iter_mut()
            .find(|t| t.id == dt)
            .unwrap()
            .requires = Some(other);
        let err = bad.validate().unwrap_err().to_string();
        assert!(
            err.contains("double_tap") && err.contains("column"),
            "{err}"
        );
        let mut bad = c.clone();
        bad.talents[0].ranks = 2;
        bad.talents[0].effects = vec![vec![]];
        let err = bad.validate().unwrap_err().to_string();
        assert!(
            err.contains(&c.talents[0].id.0) && err.contains("rank"),
            "{err}"
        );
        let mut bad = c.clone();
        bad.trees[0].tiers[1].clear();
        assert!(bad.validate().unwrap_err().to_string().contains("tier"));
        let mut bad = c.clone();
        bad.talents[0].effects[0].push(Boon::Grant(AbilityId::new("nope")));
        assert!(bad.validate().unwrap_err().to_string().contains("nope"));
        let mut bad = c.clone();
        let dup = bad.trees[0].tiers[0][0].clone();
        bad.trees[1].tiers[0].push(dup);
        assert!(bad.validate().is_err());
    }

    #[test]
    fn no_rank_is_only_a_grant_a_category_already_makes() {
        let c = Content::load().unwrap();
        for t in &c.talents {
            for (r, boons) in t.effects.iter().enumerate() {
                let only_category_grants = boons.iter().all(|b| match b {
                    Boon::Grant(a) => c.categories.iter().any(|cat| cat.grants.contains(a)),
                    _ => false,
                });
                assert!(
                    !only_category_grants,
                    "{} rank {} does nothing for an owner of the category that grants it",
                    t.id,
                    r + 1
                );
            }
        }
    }

    #[test]
    fn programs_and_decks_validate() {
        let c = Content::load().unwrap();
        for id in [
            "crack", "spoof", "blind", "copy", "backdoor", "crack_2", "copy_2",
        ] {
            let i = c.item(&ItemId::new(id)).unwrap();
            assert_eq!(i.kind, ItemKind::Program, "{id}");
            assert!(i.program.is_some() && i.slot.is_none(), "{id}");
        }
        assert_eq!(
            c.item(&ItemId::new("pawnshop_deck")).unwrap().deck,
            Some(DeckSpec {
                slots: 2,
                rating: 1,
                look: DeckLook::Pawn
            })
        );
        let crack = c.item(&ItemId::new("crack")).unwrap().program.unwrap();
        assert_eq!(
            (crack.op, crack.rating, crack.size, crack.noise, crack.work),
            (Op::Crack, 1, 1, 4, 3)
        );
        let at = |c: &Content, id: &str| c.items.iter().position(|i| i.id.0 == id).unwrap();
        let mut bad = c.clone();
        let i = at(&bad, "crack");
        bad.items[i].slot = Some(Slot::Deck);
        let err = bad.validate().unwrap_err().to_string();
        assert!(err.contains("crack") && err.contains("slot"), "{err}");
        let mut bad = c.clone();
        let i = at(&bad, "crack");
        bad.items[i].program = None;
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        let i = at(&bad, "pawnshop_deck");
        bad.items[i].deck = None;
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        let i = at(&bad, "crack");
        bad.items[i].program.as_mut().unwrap().size = 0;
        assert!(bad.validate().is_err());
        let mut bad = c.clone();
        let i = at(&bad, "hardhat");
        bad.items[i].program = Some(crack);
        assert!(bad.validate().is_err(), "a hardhat is no program");
    }

    #[test]
    fn deck_look_defaults_to_pawn() {
        let text = DEFAULT_CONTENT.replace(", look: Pawn", "");
        assert!(!text.contains("look: Pawn"));
        let c = Content::parse(&text).unwrap();
        let d = c.item(&ItemId::new("pawnshop_deck")).unwrap().deck.unwrap();
        assert_eq!(d.look, DeckLook::Pawn);
    }

    #[test]
    fn pawn_decks_have_room_for_five_slots_and_three_grants() {
        let c = Content::load().unwrap();
        let i = c
            .items
            .iter()
            .position(|i| i.id.0 == "pawnshop_deck")
            .unwrap();
        let mut bad = c.clone();
        bad.items[i].deck.as_mut().unwrap().slots = 6;
        assert!(matches!(
            bad.validate(),
            Err(ContentError::DeckInvalid { .. })
        ));
        let mut bad = c.clone();
        bad.items[i].grants = ["netscan", "arc_trace", "netscan", "arc_trace"]
            .map(AbilityId::new)
            .to_vec();
        assert!(matches!(
            bad.validate(),
            Err(ContentError::DeckInvalid { .. })
        ));
        let mut ok = c.clone();
        ok.items[i].deck.as_mut().unwrap().slots = 5;
        ok.items[i].grants = ["netscan", "arc_trace", "netscan"]
            .map(AbilityId::new)
            .to_vec();
        assert!(ok.validate().is_ok());
    }
    #[test]
    fn hardware_and_sockets_load() {
        let c = Content::load().unwrap();
        let get = |id: &str| c.item(&ItemId::new(id)).unwrap();
        assert_eq!(
            get("rebuilt_deck").sockets,
            vec![SocketKind::Deck, SocketKind::Deck]
        );
        assert!(get("pawnshop_deck").sockets.is_empty());
        for (id, name, weight, value, m, look) in [
            (
                "coldfin_1",
                "Coldfin vI",
                0.4,
                800,
                Mod::TraceCut(10),
                HardwareLook::ColdfinI,
            ),
            (
                "coldfin_2",
                "Coldfin vII",
                0.6,
                1600,
                Mod::TraceCut(20),
                HardwareLook::ColdfinII,
            ),
            (
                "sidecar",
                "Sidecar co-processor",
                0.2,
                900,
                Mod::Power(1),
                HardwareLook::Sidecar,
            ),
        ] {
            let i = get(id);
            assert_eq!(i.kind, ItemKind::Hardware, "{id}");
            assert_eq!(i.name, name);
            assert_eq!(i.weight, weight);
            assert_eq!(i.value, value);
            assert_eq!(
                i.hardware,
                Some(Hardware {
                    fits: SocketKind::Deck,
                    mods: vec![m],
                    look
                }),
                "{id}"
            );
        }
        assert!(get("rebuilt_deck").per_copy());
        assert!(get("coldfin_1").per_copy());
        // It carries stickers, so it is held per copy too.
        assert!(get("pawnshop_deck").per_copy());
        assert!(!get("crack").per_copy());
    }

    #[test]
    fn weapon_sockets_and_hardware_load() {
        let c = Content::load().unwrap();
        let get = |id: &str| c.item(&ItemId::new(id)).unwrap();
        let smg = get("ratchet_smg");
        assert_eq!(smg.look, Some(ItemLook::Ratchet));
        assert_eq!(smg.sockets, vec![SocketKind::Muzzle, SocketKind::Rail]);
        assert!(smg.per_copy());
        assert_eq!(smg.drawn_sockets()[0], ("MUZZLE", SocketKind::Muzzle));
        assert_eq!(
            get("rebuilt_deck").drawn_sockets()[1],
            ("R", SocketKind::Deck)
        );
        assert!(get("salvage_jacket").drawn_sockets().is_empty());
        for (id, name, weight, value, m, fits, look) in [
            (
                "hushcan",
                "Hushcan",
                0.5,
                700,
                Mod::Stat(Stat::Cool, 1),
                SocketKind::Muzzle,
                HardwareLook::Hushcan,
            ),
            (
                "redeye",
                "Redeye sight",
                0.2,
                550,
                Mod::Stat(Stat::Reflex, 1),
                SocketKind::Rail,
                HardwareLook::Redeye,
            ),
        ] {
            let i = get(id);
            assert_eq!(
                (i.kind, i.name.as_str(), i.weight, i.value),
                (ItemKind::Hardware, name, weight, value)
            );
            assert_eq!(
                i.hardware,
                Some(Hardware {
                    fits,
                    mods: vec![m],
                    look
                }),
                "{id}"
            );
            assert_eq!(look.kind(), fits);
        }
    }

    #[test]
    fn stickers_load() {
        let c = Content::load().unwrap();
        let get = |id: &str| c.item(&ItemId::new(id)).unwrap();
        let stickers: Vec<_> = c
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Sticker)
            .collect();
        assert_eq!(stickers.len(), 29);
        for s in &stickers {
            assert!(s.sticker.is_some() && s.slot.is_none(), "{}", s.id);
            assert_eq!((s.weight, s.value), (0.0, 5), "{}", s.id);
            assert!(!s.per_copy() && !s.takes_stickers(), "{}", s.id);
        }
        assert_eq!(get("sticker_no_future").name, "No Future");
        assert_eq!(
            get("sticker_no_future").sticker,
            Some(StickerArt::Lettered {
                text: " NO FUTURE ".into(),
                ink: (0xe8, 0xe8, 0xec),
                ground: (0xff, 0x00, 0xc8),
                bold: true,
            })
        );
        let Some(StickerArt::Picture { rows, colours }) = &get("sticker_chrome_skull").sticker
        else {
            panic!("a picture");
        };
        assert_eq!(rows[0], "..wwwwww..");
        assert_eq!(colours[&'r'], (0xff, 0x2a, 0x3c));
        let factory = vec![
            Stuck {
                sticker: ItemId::new("sticker_no_future"),
                x: 50,
                y: 10,
                turn: Turn::Level,
            },
            Stuck {
                sticker: ItemId::new("sticker_dead_face"),
                x: 45,
                y: 13,
                turn: Turn::Level,
            },
        ];
        for deck in ["pawnshop_deck", "rebuilt_deck"] {
            assert_eq!(get(deck).stickers, factory, "{deck}");
            assert!(get(deck).takes_stickers() && get(deck).per_copy(), "{deck}");
        }
        assert!(get("ratchet_smg").takes_stickers());
        for id in ["coldfin_1", "crack", "salvage_jacket", "stim_patch"] {
            assert!(!get(id).takes_stickers(), "{id}");
        }
    }

    #[test]
    fn turns_go_clockwise() {
        assert_eq!(Turn::Level.cw(), Turn::Quarter);
        assert_eq!(Turn::Quarter.cw(), Turn::Half);
        assert_eq!(Turn::Half.cw(), Turn::ThreeQuarter);
        assert_eq!(Turn::ThreeQuarter.cw(), Turn::Level);
        assert_eq!(Turn::ALL.map(Turn::degrees), [0, 90, 180, 270]);
    }

    #[test]
    fn validation_refuses_bad_stickers() {
        let c = Content::load().unwrap();
        let at = |c: &Content, id: &str| c.items.iter().position(|i| i.id.0 == id).unwrap();
        let broken = |id: &str, f: &dyn Fn(&mut Item)| {
            let mut bad = c.clone();
            let i = at(&bad, id);
            f(&mut bad.items[i]);
            bad.validate()
        };
        let invalid =
            |r: Result<(), ContentError>| matches!(r, Err(ContentError::StickerInvalid { .. }));
        let lettered = |text: &str| StickerArt::Lettered {
            text: text.into(),
            ink: (1, 2, 3),
            ground: (4, 5, 6),
            bold: true,
        };
        let picture = |rows: &[&str]| StickerArt::Picture {
            rows: rows.iter().map(|r| r.to_string()).collect(),
            colours: [('a', (1, 2, 3))].into_iter().collect(),
        };
        assert!(invalid(
            broken("sticker_neko", &|i| i.slot = Some(Slot::Deck))
        ));
        assert!(invalid(broken("sticker_neko", &|i| i.sticker = None)));
        assert!(invalid(
            broken("crack", &|i| i.sticker = Some(lettered(" X ")))
        ));
        assert!(invalid(broken("sticker_flatline", &|i| {
            i.sticker = Some(lettered(&"x".repeat(19)))
        })));
        assert!(
            broken("sticker_flatline", &|i| i.sticker =
                Some(lettered(&"x".repeat(18))))
            .is_ok()
        );
        assert!(invalid(
            broken("sticker_flatline", &|i| i.sticker = Some(lettered("")))
        ));
        assert!(invalid(broken("sticker_neko", &|i| {
            i.sticker = Some(picture(&["aa", "a"]))
        })));
        assert!(invalid(broken("sticker_neko", &|i| {
            i.sticker = Some(picture(&["ab"]))
        })));
        assert!(invalid(broken("sticker_neko", &|i| {
            i.sticker = Some(picture(&[&"a".repeat(15)]))
        })));
        assert!(
            broken("sticker_neko", &|i| i.sticker =
                Some(picture(&[&"a".repeat(14)])))
            .is_ok()
        );
        let tall: Vec<&str> = vec!["a"; 13];
        assert!(invalid(
            broken("sticker_neko", &|i| i.sticker = Some(picture(&tall)))
        ));
        assert!(broken("sticker_neko", &|i| i.sticker = Some(picture(&tall[..12]))).is_ok());
        assert!(invalid(broken("sticker_neko", &|i| {
            i.sticker = Some(picture(&["..", ".."]))
        })));
        let factory = c
            .item(&ItemId::new("pawnshop_deck"))
            .unwrap()
            .stickers
            .clone();
        assert!(invalid(
            broken("salvage_jacket", &|i| i.stickers = factory.clone())
        ));
        assert!(invalid(broken("pawnshop_deck", &|i| {
            i.stickers[0].sticker = ItemId::new("crack")
        })));
        assert!(matches!(
            broken("pawnshop_deck", &|i| i.stickers[0].sticker =
                ItemId::new("nope")),
            Err(ContentError::UnknownItem { .. })
        ));
    }

    #[test]
    fn mod_lines_read_as_the_card_does() {
        assert_eq!(Mod::TraceCut(10).line(), "trace per cast \u{2212}10%");
        assert_eq!(Mod::Power(1).line(), "program power +1");
        assert_eq!(Mod::TraceCut(20).sticker(), "TRACE \u{2212}20%");
        assert_eq!(Mod::Power(1).sticker(), "POWER +1");
        assert_eq!(Mod::Power(-1).line(), "program power \u{2212}1");
        assert_eq!(Mod::Power(-1).sticker(), "POWER \u{2212}1");
        assert_eq!(SocketKind::Deck.takes(), "deck hardware");
        assert_eq!(SocketKind::Deck.fits(), "deck socket");
        assert_eq!(Mod::Stat(Stat::Cool, 1).line(), "cool +1");
        assert_eq!(Mod::Stat(Stat::Cool, 1).sticker(), "COOL +1");
        assert_eq!(Mod::Stat(Stat::Reflex, -1).line(), "reflex \u{2212}1");
        assert_eq!(Mod::Stat(Stat::Reflex, -1).sticker(), "REFLEX \u{2212}1");
        assert_eq!(SocketKind::Muzzle.takes(), "muzzle hardware");
        assert_eq!(SocketKind::Rail.fits(), "rail socket");
    }

    #[test]
    fn validation_refuses_bad_hardware_and_sockets() {
        let c = Content::load().unwrap();
        let at = |c: &Content, id: &str| c.items.iter().position(|i| i.id.0 == id).unwrap();
        let hardware = c.item(&ItemId::new("coldfin_1")).unwrap().hardware.clone();

        let mut bad = c.clone();
        let i = at(&bad, "coldfin_1");
        bad.items[i].hardware = None;
        assert!(matches!(
            bad.validate(),
            Err(ContentError::HardwareInvalid { .. })
        ));

        let mut bad = c.clone();
        let i = at(&bad, "crack");
        bad.items[i].hardware = hardware;
        assert!(matches!(
            bad.validate(),
            Err(ContentError::HardwareInvalid { .. })
        ));

        let mut bad = c.clone();
        let i = at(&bad, "coldfin_1");
        bad.items[i].hardware.as_mut().unwrap().mods.clear();
        assert!(matches!(
            bad.validate(),
            Err(ContentError::HardwareInvalid { .. })
        ));

        let mut bad = c.clone();
        let i = at(&bad, "rebuilt_deck");
        bad.items[i].sockets = vec![SocketKind::Deck; 3];
        assert!(matches!(
            bad.validate(),
            Err(ContentError::SocketsInvalid { .. })
        ));

        let mut bad = c.clone();
        let i = at(&bad, "salvage_jacket");
        bad.items[i].sockets = vec![SocketKind::Deck];
        assert!(matches!(
            bad.validate(),
            Err(ContentError::SocketsInvalid { .. })
        ));

        let smg_sockets = |sockets: Vec<SocketKind>| {
            let mut bad = c.clone();
            let i = at(&bad, "ratchet_smg");
            bad.items[i].sockets = sockets;
            bad.validate()
        };
        use SocketKind::{Muzzle, Rail};
        assert!(matches!(
            smg_sockets(vec![Rail, Muzzle]),
            Err(ContentError::SocketsInvalid { .. })
        ));
        assert!(matches!(
            smg_sockets(vec![Muzzle, Rail, Rail]),
            Err(ContentError::SocketsInvalid { .. })
        ));
        assert!(smg_sockets(vec![Muzzle]).is_ok());

        let mut bad = c.clone();
        let i = at(&bad, "ratchet_smg");
        bad.items[i].look = None;
        assert!(matches!(
            bad.validate(),
            Err(ContentError::SocketsInvalid { .. })
        ));

        let mut bad = c.clone();
        let i = at(&bad, "hushcan");
        bad.items[i].hardware.as_mut().unwrap().fits = Rail;
        assert!(matches!(
            bad.validate(),
            Err(ContentError::HardwareInvalid { .. })
        ));

        for id in ["rebuilt_deck", "coldfin_1"] {
            let mut bad = c.clone();
            let i = at(&bad, id);
            bad.items[i].look = Some(ItemLook::Ratchet);
            assert!(
                matches!(bad.validate(), Err(ContentError::LookInvalid { .. })),
                "{id}"
            );
        }
    }
}
