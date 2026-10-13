//! The player's character: stats, skills, talents, equipment, inventory and quest log.

use crate::content::Content;
use crate::model::*;
use std::collections::BTreeMap;

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
        self.skill_level(&r.skill) >= r.level && r.inputs.iter().all(|(id, n)| self.count(id) >= *n)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_character_matches_the_content_file() {
        let c = Content::load().unwrap();
        let ch = Character::from_starter(&c);
        assert_eq!(ch.handle, "Vex");
        assert_eq!((ch.hp, ch.hp_max, ch.credits), (40, 40, 320));
        assert_eq!(
            ch.equipped(Slot::Eyes),
            Some(&ItemId::new("nightglass_optics"))
        );
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
        assert!(
            ch.weight(&c) > 7.0 && ch.weight(&c) < 12.0,
            "{}",
            ch.weight(&c)
        );
        assert!(ch.value(&c) > 4000);
    }
}
