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
    #[error("item `{item}` cannot sit in slot {slot:?}")]
    SlotMismatch { item: String, slot: Slot },
    #[error("duplicate id `{0}`")]
    DuplicateId(String),
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

#[cfg(test)]
mod tests {
    use super::*;

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

        assert!(matches!(
            Content::parse("not ron"),
            Err(ContentError::Parse(_))
        ));
    }
}
