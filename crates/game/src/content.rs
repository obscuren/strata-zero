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
    #[error("{where_}: unknown quest `{id}`")]
    UnknownQuest { where_: String, id: String },
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
    #[error("starter level must be at least 1")]
    StarterLevel,
    #[error("ability `{ability}` is passive but has fx")]
    FxOnPassive { ability: String },
    #[error("quest `{quest}`: starter flags do not match its objectives")]
    ObjectiveCount { quest: String },
    #[error("starter {what}: duplicate `{id}`")]
    DuplicateStarter { what: String, id: String },
    #[error("starter talent `{talent}` needs {skill} {need}; the starter has {have}")]
    TalentLevel {
        talent: String,
        skill: String,
        need: u8,
        have: u8,
    },
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
    #[error("creation quest `{quest}`: flags do not match its objectives")]
    CreationQuest { quest: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Content {
    pub items: Vec<Item>,
    pub skills: Vec<Skill>,
    pub talents: Vec<Talent>,
    pub quests: Vec<Quest>,
    pub recipes: Vec<Recipe>,
    #[serde(default)]
    pub abilities: Vec<Ability>,
    pub starter: Starter,
    pub creation: Creation,
    pub origins: Vec<Origin>,
    pub professions: Vec<Profession>,
    pub categories: Vec<Category>,
    pub templates: Vec<Template>,
    pub stat_blurbs: Vec<(Stat, String)>,
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
            for p in &i.perks {
                if let Perk::Skill(id, _) = p {
                    need_skill(format!("item {}", i.id), id)?;
                }
            }
            for id in &i.grants {
                need_ability(format!("item {}", i.id), id)?;
            }
        }
        for t in &self.talents {
            need_skill(format!("talent {}", t.id), &t.skill)?;
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
        }
        self.validate_creation(&need_item, &need_skill, &need_ability)?;
        let s = &self.starter;
        if s.level == 0 {
            return Err(ContentError::StarterLevel);
        }
        for (id, _) in &s.inventory {
            need_item("starter inventory".into(), id)?;
        }
        let dup = |what: &str, id: String| ContentError::DuplicateStarter {
            what: what.to_string(),
            id,
        };
        let mut slots: HashSet<Slot> = HashSet::new();
        for (slot, id) in &s.equipped {
            let item = need_item("starter equipment".into(), id)?;
            if item.slot != Some(*slot) {
                return Err(ContentError::SlotMismatch {
                    item: id.0.clone(),
                    slot: *slot,
                });
            }
            if !slots.insert(*slot) {
                return Err(dup("equipment", slot.label().to_string()));
            }
        }
        let mut skills: HashSet<&SkillId> = HashSet::new();
        for (id, _) in &s.skills {
            need_skill("starter skills".into(), id)?;
            if !skills.insert(id) {
                return Err(dup("skills", id.0.clone()));
            }
        }
        for id in &s.talents {
            let t = self.talent(id).ok_or_else(|| ContentError::UnknownTalent {
                where_: "starter talents".into(),
                id: id.0.clone(),
            })?;
            let have = s
                .skills
                .iter()
                .find(|(sk, _)| sk == &t.skill)
                .map(|(_, l)| *l)
                .unwrap_or(0);
            if have < t.level {
                return Err(ContentError::TalentLevel {
                    talent: id.0.clone(),
                    skill: t.skill.0.clone(),
                    need: t.level,
                    have,
                });
            }
        }
        let mut abilities: HashSet<&AbilityId> = HashSet::new();
        for id in &s.abilities {
            need_ability("starter abilities".into(), id)?;
            if !abilities.insert(id) {
                return Err(dup("abilities", id.0.clone()));
            }
        }
        let mut quests: HashSet<&QuestId> = HashSet::new();
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
            if !quests.insert(id) {
                return Err(dup("quests", id.0.clone()));
            }
        }
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
            for id in &t.talents {
                self.talent(id).ok_or_else(|| ContentError::UnknownTalent {
                    where_: where_.clone(),
                    id: id.0.clone(),
                })?;
            }
            for (id, _) in &t.kit {
                need_item(where_.clone(), id)?;
            }
        }
        for (id, flags) in &self.creation.quests {
            let q = self.quest(id).ok_or_else(|| ContentError::UnknownQuest {
                where_: "creation quests".into(),
                id: id.0.clone(),
            })?;
            if flags.len() != q.objectives.len() {
                return Err(ContentError::CreationQuest {
                    quest: id.0.clone(),
                });
            }
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
        self.talents.iter().filter(move |t| &t.skill == skill)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_content_parses_and_covers_the_kit() {
        let c = Content::load().unwrap();
        assert!(c.items.len() >= 14, "{} items", c.items.len());
        assert_eq!(c.skills.len(), 24);
        assert!(c.talents.len() >= 8);
        assert!(c.quests.len() >= 4);
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
        let s = &c.starter;
        assert_eq!(s.handle, "Vex");
        assert!((2..=4).contains(&s.equipped.len()));
        assert!(
            s.equipped.len() < Slot::ALL.len(),
            "at least one slot stays empty"
        );
        assert!(
            s.quests.iter().any(|(_, d)| d.iter().all(|x| *x)),
            "one quest is done"
        );
        assert!(
            s.quests.iter().any(|(_, d)| d.iter().any(|x| !*x)),
            "one quest is active"
        );
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
        c.starter.level = 0;
        assert!(matches!(c.validate(), Err(ContentError::StarterLevel)));

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
        c.starter.equipped[0].0 = Slot::Legs; // the first equipped item is not legwear
        let first = c.starter.equipped[0].1.clone();
        assert_ne!(c.item(&first).unwrap().slot, Some(Slot::Legs));
        assert!(matches!(
            c.validate(),
            Err(ContentError::SlotMismatch { .. })
        ));

        let mut c = good.clone();
        let dup = c.items[0].clone();
        c.items.push(dup);
        assert!(matches!(c.validate(), Err(ContentError::DuplicateId(_))));

        let mut c = good.clone();
        c.starter.quests[0].1.push(true);
        assert!(matches!(
            c.validate(),
            Err(ContentError::ObjectiveCount { .. })
        ));

        let mut c = good.clone();
        c.talents[0].skill = SkillId::new("origami");
        assert!(matches!(
            c.validate(),
            Err(ContentError::UnknownSkill { .. })
        ));

        let mut c = good.clone();
        c.starter.talents.push(TalentId::new("levitation"));
        assert!(matches!(
            c.validate(),
            Err(ContentError::UnknownTalent { .. })
        ));

        let mut c = good.clone();
        c.starter.quests[0].0 = QuestId::new("the_grail");
        assert!(matches!(
            c.validate(),
            Err(ContentError::UnknownQuest { .. })
        ));

        let mut c = good.clone();
        let dup = c.starter.equipped[0].clone();
        c.starter.equipped.push(dup);
        assert!(matches!(
            c.validate(),
            Err(ContentError::DuplicateStarter { .. })
        ));

        let mut c = good.clone();
        let dup = c.starter.skills[0].clone();
        c.starter.skills.push(dup);
        assert!(matches!(
            c.validate(),
            Err(ContentError::DuplicateStarter { .. })
        ));

        let mut c = good.clone();
        let dup = c.starter.quests[0].clone();
        c.starter.quests.push(dup);
        assert!(matches!(
            c.validate(),
            Err(ContentError::DuplicateStarter { .. })
        ));

        let mut c = good.clone();
        c.starter.talents.push(TalentId::new("deep_dive")); // hacking 4; the starter has 3
        assert!(matches!(
            c.validate(),
            Err(ContentError::TalentLevel { .. })
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
        c.items[optics]
            .perks
            .push(Perk::Skill(SkillId::new("origami"), 1));
        assert!(matches!(
            c.validate(),
            Err(ContentError::UnknownSkill { .. })
        ));

        let mut c = good.clone();
        c.starter.abilities.push(AbilityId::new("flight"));
        assert!(matches!(
            c.validate(),
            Err(ContentError::UnknownAbility { .. })
        ));

        let mut c = good.clone();
        c.starter.abilities.push(AbilityId::new("street_sense"));
        assert!(matches!(
            c.validate(),
            Err(ContentError::DuplicateStarter { .. })
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
        assert_eq!(c.starter.abilities, vec![AbilityId::new("street_sense")]);
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
        assert!(c.talents.iter().all(|t| c.skill(&t.skill).is_some()));
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
}
