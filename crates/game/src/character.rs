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

/// Where an ability comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Innate,
    Item(ItemId),
}

/// The character. Nothing derived is cached: every value that depends on equipment
/// is computed on read from what is equipped right now.
#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    pub handle: String,
    pub credits: u32,
    pub hp: u32,
    /// Max HP before equipment; see `hp_max`.
    pub hp_base: u32,
    pub stats: Stats,
    pub skills: Vec<(SkillId, u8)>,
    pub talents: Vec<TalentId>,
    /// Innate abilities; equipment adds more (see `abilities`).
    pub abilities: Vec<AbilityId>,
    pub equipment: BTreeMap<Slot, ItemId>,
    pub inventory: Vec<Stack>,
    pub quests: Vec<QuestLog>,
    /// The tick each active ability last fired, for cooldowns.
    pub fired: BTreeMap<AbilityId, u64>,
}

impl Character {
    /// A new character from the content's starter kit.
    pub fn from_starter(c: &Content) -> Character {
        let s = &c.starter;
        let mut ch = Character {
            handle: s.handle.clone(),
            credits: s.credits,
            hp: s.hp,
            hp_base: s.hp,
            stats: s.stats.clone(),
            skills: s.skills.clone(),
            talents: s.talents.clone(),
            abilities: s.abilities.clone(),
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
            fired: BTreeMap::new(),
        };
        ch.hp = ch.hp_max(c);
        ch
    }

    /// Perks of everything equipped, in slot order.
    pub fn perks<'a>(&'a self, c: &'a Content) -> impl Iterator<Item = &'a Perk> + 'a {
        Slot::ALL
            .iter()
            .filter_map(move |s| self.equipment.get(s))
            .filter_map(move |id| c.item(id))
            .flat_map(|i| i.perks.iter())
    }

    pub fn hp_max(&self, c: &Content) -> u32 {
        let bonus: i32 = self
            .perks(c)
            .filter_map(|p| match p {
                Perk::HpMax(n) => Some(*n),
                _ => None,
            })
            .sum();
        (self.hp_base as i32 + bonus).max(1) as u32
    }

    pub fn stat(&self, c: &Content, stat: Stat) -> u8 {
        let bonus: i32 = self
            .perks(c)
            .filter_map(|p| match p {
                Perk::Stat(s, n) if *s == stat => Some(*n as i32),
                _ => None,
            })
            .sum();
        (self.stats.get(stat) as i32 + bonus).clamp(0, 10) as u8
    }

    /// The skill level before equipment.
    pub fn skill_base(&self, id: &SkillId) -> u8 {
        self.skills
            .iter()
            .find(|(s, _)| s == id)
            .map(|(_, l)| *l)
            .unwrap_or(0)
    }

    /// The skill level with equipment.
    pub fn skill_level(&self, c: &Content, id: &SkillId) -> u8 {
        let bonus: i32 = self
            .perks(c)
            .filter_map(|p| match p {
                Perk::Skill(s, n) if s == id => Some(*n as i32),
                _ => None,
            })
            .sum();
        (self.skill_base(id) as i32 + bonus).max(0) as u8
    }

    /// Extra field-of-view radius from equipment.
    pub fn sight_bonus(&self, c: &Content) -> i32 {
        self.perks(c)
            .filter_map(|p| match p {
                Perk::Sight(n) => Some(*n as i32),
                _ => None,
            })
            .sum()
    }

    /// Carry capacity in kilograms: body plus equipment.
    pub fn carry_cap(&self, c: &Content) -> f32 {
        let bonus: f32 = self
            .perks(c)
            .filter_map(|p| match p {
                Perk::Carry(kg) => Some(*kg),
                _ => None,
            })
            .sum();
        8.0 + 2.0 * self.stats.body as f32 + bonus
    }

    pub fn overburdened(&self, c: &Content) -> bool {
        self.weight(c) > self.carry_cap(c)
    }

    /// Innate abilities first, then those granted by equipment in slot order, deduplicated.
    pub fn abilities(&self, c: &Content) -> Vec<(AbilityId, Source)> {
        let mut out: Vec<(AbilityId, Source)> = self
            .abilities
            .iter()
            .map(|id| (id.clone(), Source::Innate))
            .collect();
        for slot in Slot::ALL {
            let Some(item) = self.equipment.get(&slot).and_then(|id| c.item(id)) else {
                continue;
            };
            for id in &item.grants {
                if !out.iter().any(|(have, _)| have == id) {
                    out.push((id.clone(), Source::Item(item.id.clone())));
                }
            }
        }
        out
    }

    /// Ticks until an ability is ready again; `None` when ready (or unknown).
    pub fn cooling(&self, c: &Content, id: &AbilityId, tick: u64) -> Option<u64> {
        let fired = *self.fired.get(id)?;
        let ready = fired + c.ability(id)?.cooldown;
        (ready > tick).then(|| ready - tick)
    }

    /// Puts `qty` of an item in the pack, merging into a stack of the same item.
    pub fn give(&mut self, item: ItemId, qty: u32) {
        if qty == 0 {
            return;
        }
        match self.inventory.iter_mut().find(|s| s.item == item) {
            Some(s) => s.qty = s.qty.saturating_add(qty),
            None => self.inventory.push(Stack { item, qty }),
        }
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
    pub fn can_craft(&self, c: &Content, r: &Recipe) -> bool {
        self.skill_level(c, &r.skill) >= r.level
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_character_matches_the_content_file() {
        let c = Content::load().unwrap();
        let ch = Character::from_starter(&c);
        assert_eq!(ch.handle, "Vex");
        assert_eq!((ch.hp, ch.hp_max(&c), ch.credits), (45, 45, 320));
        assert_eq!(
            ch.equipped(Slot::Eyes),
            Some(&ItemId::new("nightglass_optics"))
        );
        assert_eq!(ch.equipped(Slot::Head), None);
        assert!(ch.is_equipped(&ItemId::new("ratchet_smg")));
        assert!(!ch.is_equipped(&ItemId::new("copper_coil")));
        assert_eq!(ch.count(&ItemId::new("copper_coil")), 3);
        assert_eq!(ch.count(&ItemId::new("grip_arm")), 0);
        assert_eq!(ch.skill_level(&c, &SkillId::new("hacking")), 4);
        assert_eq!(ch.skill_level(&c, &SkillId::new("origami")), 0);
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

    #[test]
    fn derived_values_follow_the_equipment() {
        let c = Content::load().unwrap();
        let mut ch = Character::from_starter(&c);
        assert_eq!(ch.hp_max(&c), 45, "jacket +5");
        assert_eq!(ch.hp, 45, "starts full");
        assert_eq!(ch.sight_bonus(&c), 4, "optics");
        assert_eq!(ch.skill_base(&SkillId::new("hacking")), 3);
        assert_eq!(
            ch.skill_level(&c, &SkillId::new("hacking")),
            4,
            "pawnshop deck"
        );
        assert_eq!(ch.stat(&c, Stat::Body), 4);
        assert!(
            (ch.carry_cap(&c) - 16.0).abs() < 1e-4,
            "{}",
            ch.carry_cap(&c)
        );
        assert!(!ch.overburdened(&c));
        let abilities = ch.abilities(&c);
        assert_eq!(
            abilities[0],
            (AbilityId::new("street_sense"), Source::Innate)
        );
        assert!(abilities.contains(&(
            AbilityId::new("lowlight"),
            Source::Item(ItemId::new("nightglass_optics"))
        )));
        assert!(abilities.contains(&(
            AbilityId::new("netscan"),
            Source::Item(ItemId::new("pawnshop_deck"))
        )));
        assert!(
            !abilities.iter().any(|(id, _)| id.0 == "ping"),
            "neural port is not equipped"
        );
        ch.equipment.clear();
        assert_eq!(ch.hp_max(&c), 40);
        assert_eq!(ch.sight_bonus(&c), 0);
        assert_eq!(ch.skill_level(&c, &SkillId::new("hacking")), 3);
        assert_eq!(ch.abilities(&c).len(), 1);
        ch.equipment.insert(Slot::Arms, ItemId::new("grip_arm"));
        assert_eq!(ch.stat(&c, Stat::Body), 5);
        assert!(
            (ch.carry_cap(&c) - 28.0).abs() < 1e-4,
            "{}",
            ch.carry_cap(&c)
        );
        ch.inventory.push(Stack {
            item: ItemId::new("scrap_servo"),
            qty: 40,
        });
        assert!(ch.overburdened(&c));
        let scan = AbilityId::new("netscan");
        assert_eq!(ch.cooling(&c, &scan, 1000), None);
        ch.fired.insert(scan.clone(), 1000);
        assert_eq!(ch.cooling(&c, &scan, 1100), Some(200));
        assert_eq!(ch.cooling(&c, &scan, 1300), None);
    }

    #[test]
    fn give_saturates_instead_of_overflowing() {
        let c = Content::load().unwrap();
        let mut ch = Character::from_starter(&c);
        ch.give(ItemId::new("copper_coil"), u32::MAX);
        ch.give(ItemId::new("copper_coil"), 1);
        assert_eq!(ch.count(&ItemId::new("copper_coil")), u32::MAX);
    }
}
