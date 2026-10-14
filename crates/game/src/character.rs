//! The player's character: stats, skills, talents, equipment, inventory and quest log.

use crate::build::Build;
use crate::content::Content;
use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};

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
    Category(CategoryId),
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
    pub level: u8,
    pub xp: u32,
    pub focus: u32,
    pub focus_max: u32,
    pub stats: Stats,
    pub origin: OriginId,
    pub profession: ProfessionId,
    pub categories: BTreeSet<CategoryId>,
    pub skills: BTreeSet<SkillId>,
    pub talents: Vec<TalentId>,
    /// Innate abilities; categories and equipment add more (see `abilities`).
    pub abilities: Vec<AbilityId>,
    pub equipment: BTreeMap<Slot, ItemId>,
    pub inventory: Vec<Stack>,
    pub quests: Vec<QuestLog>,
    /// The tick each active ability last fired, for cooldowns.
    pub fired: BTreeMap<AbilityId, u64>,
    /// Unspent creation points, spendable from the deck under the same rules.
    pub stat_points: u32,
    pub sp: u32,
}

impl Character {
    /// A playable character from a build. An invalid build still builds with whatever
    /// it holds; the wizard validates before it gets here.
    pub fn from_build(c: &Content, b: &Build) -> Character {
        let cr = &c.creation;
        let mut ch = Character {
            handle: b.handle.trim().to_string(),
            credits: 0,
            hp: cr.hp,
            hp_base: cr.hp,
            level: 1,
            xp: 0,
            focus: cr.focus,
            focus_max: cr.focus,
            stats: b.stats.clone(),
            origin: b.origin.clone().unwrap_or_else(|| OriginId::new("")),
            profession: b
                .profession
                .clone()
                .unwrap_or_else(|| ProfessionId::new("")),
            categories: b.categories.clone(),
            skills: b.skills.clone(),
            talents: b.talents.clone(),
            abilities: Vec::new(),
            equipment: BTreeMap::new(),
            inventory: Vec::new(),
            quests: cr
                .quests
                .iter()
                .map(|(q, d)| QuestLog {
                    quest: q.clone(),
                    done: d.clone(),
                })
                .collect(),
            fired: BTreeMap::new(),
            stat_points: b.points_left(c).max(0) as u32,
            sp: b.sp_left(c).max(0) as u32,
        };
        let mut kit: Vec<(ItemId, u32)> = Vec::new();
        if let Some(p) = b.profession.as_ref().and_then(|id| c.profession(id)) {
            ch.credits = p.credits;
            kit.extend(p.kit.iter().cloned());
        }
        kit.extend(b.kit.iter().cloned());
        // The first item that fits a free slot is worn; everything else is packed.
        for (id, qty) in kit {
            let Some(item) = c.item(&id) else { continue };
            let mut qty = qty;
            if let Some(slot) = item.slot
                && qty > 0
                && !ch.equipment.contains_key(&slot)
            {
                ch.equipment.insert(slot, id.clone());
                qty -= 1;
            }
            ch.give(id, qty);
        }
        ch.hp = ch.hp_max(c);
        ch
    }

    /// The character a template describes, or none for an unknown template.
    pub fn from_template(c: &Content, id: &TemplateId) -> Option<Character> {
        let t = c.template(id)?;
        Some(Self::from_build(c, &Build::from_template(c, t)))
    }

    pub fn has_skill(&self, id: &SkillId) -> bool {
        self.skills.contains(id)
    }

    pub fn has_category(&self, id: &CategoryId) -> bool {
        self.categories.contains(id)
    }

    /// Whether an owned category opens the workbench.
    pub fn has_workbench(&self, c: &Content) -> bool {
        self.categories
            .iter()
            .filter_map(|id| c.category(id))
            .any(|cat| cat.unlocks.contains(&Unlock::Workbench))
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

    /// Experience needed to leave `level`.
    pub fn xp_to_next(level: u8) -> u32 {
        200 * level as u32
    }

    /// Adds points and levels up while the threshold is crossed; returns the levels
    /// gained. Level 255 never gains another; the points still add.
    pub fn gain_xp(&mut self, points: u32) -> u8 {
        self.xp = self.xp.saturating_add(points);
        let mut gained = 0u8;
        while self.level < u8::MAX && self.xp >= Self::xp_to_next(self.level) {
            self.xp -= Self::xp_to_next(self.level);
            self.level += 1;
            gained += 1;
        }
        gained
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

    /// Innate abilities first, then those granted by owned categories in content order,
    /// then those granted by equipment in slot order, deduplicated.
    pub fn abilities(&self, c: &Content) -> Vec<(AbilityId, Source)> {
        let mut out: Vec<(AbilityId, Source)> = self
            .abilities
            .iter()
            .map(|id| (id.clone(), Source::Innate))
            .collect();
        for cat in c
            .categories
            .iter()
            .filter(|cat| self.categories.contains(&cat.id))
        {
            for id in &cat.grants {
                if !out.iter().any(|(have, _)| have == id) {
                    out.push((id.clone(), Source::Category(cat.id.clone())));
                }
            }
        }
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

    /// Whether the skill and every input are there. Does not craft.
    pub fn can_craft(&self, _c: &Content, r: &Recipe) -> bool {
        self.has_skill(&r.skill) && r.inputs.iter().all(|(id, n)| self.count(id) >= *n)
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
    use crate::build::Build;

    fn vex(c: &Content) -> Character {
        Character::from_template(c, &TemplateId::new("vex")).unwrap()
    }

    #[test]
    fn vex_from_template_matches_the_old_starter() {
        let c = Content::load().unwrap();
        let ch = vex(&c);
        assert_eq!(ch.handle, "VEX");
        assert_eq!(
            (ch.level, ch.xp, ch.focus, ch.focus_max, ch.credits),
            (1, 0, 30, 30, 320)
        );
        assert_eq!(
            (
                ch.stats.body,
                ch.stats.reflex,
                ch.stats.tech,
                ch.stats.cool,
                ch.stats.net
            ),
            (4, 6, 5, 5, 6)
        );
        assert_eq!(
            ch.equipped(Slot::Primary),
            Some(&ItemId::new("ratchet_smg"))
        );
        assert_eq!(
            ch.equipped(Slot::Torso),
            Some(&ItemId::new("salvage_jacket"))
        );
        assert_eq!(
            ch.equipped(Slot::Eyes),
            Some(&ItemId::new("nightglass_optics"))
        );
        assert_eq!(ch.equipped(Slot::Deck), Some(&ItemId::new("pawnshop_deck")));
        for (id, n) in [
            ("copper_coil", 3),
            ("scrap_servo", 2),
            ("burner_shard", 1),
            ("soy_noodle_cup", 2),
            ("stim_patch", 1),
            ("bottle_caps", 12),
            ("dead_phone", 1),
        ] {
            assert_eq!(ch.count(&ItemId::new(id)), n, "{id}");
        }
        assert!(
            ch.has_skill(&SkillId::new("hacking")) && ch.has_skill(&SkillId::new("ice_breaker"))
        );
        assert!(!ch.has_skill(&SkillId::new("handguns")));
        assert!(ch.has_category(&CategoryId::new("wrench")) && ch.has_workbench(&c));
        assert_eq!((ch.stat_points, ch.sp), (0, 0));
        assert_eq!(ch.origin, OriginId::new("creche"));
        assert_eq!(ch.quests.len(), 3);
        assert_eq!(ch.hp, ch.hp_max(&c));
        let abilities = ch.abilities(&c);
        assert!(abilities.iter().any(|(id, s)| {
            id == &AbilityId::new("street_sense")
                && *s == Source::Category(CategoryId::new("street"))
        }));
        assert!(
            abilities
                .iter()
                .any(|(id, _)| id == &AbilityId::new("signal_ghost"))
        );
        assert!(
            abilities.iter().any(|(id, s)| {
                id == &AbilityId::new("netscan") && matches!(s, Source::Item(_))
            })
        );
    }

    #[test]
    fn kit_equips_first_fit_and_packs_the_rest() {
        let c = Content::load().unwrap();
        let mut b = Build::from_template(&c, c.template(&TemplateId::new("kessler")).unwrap());
        b.kit.push((ItemId::new("salvage_jacket"), 1)); // torso is taken by the weave
        let ch = Character::from_build(&c, &b);
        assert_eq!(
            ch.equipped(Slot::Torso),
            Some(&ItemId::new("ballistic_weave"))
        );
        assert_eq!(
            ch.count(&ItemId::new("salvage_jacket")),
            1,
            "second torso item is packed"
        );
        assert!(!ch.has_workbench(&c));
        assert_eq!((ch.stat_points, ch.sp), (0, 0));
        let mut spare = Build::blank(&c);
        spare.handle = "Nix".into();
        spare.origin = Some(OriginId::new("vat_grown"));
        spare.profession = Some(ProfessionId::new("techie"));
        let ch = Character::from_build(&c, &spare);
        assert_eq!((ch.stat_points, ch.sp), (20, 6), "unspent pools ride along");
        assert_eq!(ch.credits, 300);
        assert_eq!(ch.handle, "Nix");
    }

    #[test]
    fn crafting_needs_the_skill_not_a_level() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let shiv = c.recipe(&RecipeId::new("wire_shiv")).unwrap();
        assert!(ch.can_craft(&c, shiv));
        ch.skills.remove(&SkillId::new("electronics"));
        assert!(!ch.can_craft(&c, shiv));
    }

    #[test]
    fn xp_levels_up_and_saturates() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        assert_eq!(Character::xp_to_next(1), 200);
        assert_eq!(ch.gain_xp(150), 0);
        assert_eq!((ch.level, ch.xp), (1, 150));
        assert_eq!(ch.gain_xp(100), 1);
        assert_eq!((ch.level, ch.xp), (2, 50));
        assert_eq!(ch.gain_xp(400 + 600), 2);
        assert_eq!((ch.level, ch.xp), (4, 50));
        ch.level = u8::MAX;
        ch.xp = 0;
        assert_eq!(ch.gain_xp(u32::MAX), 0);
        assert_eq!((ch.level, ch.xp), (u8::MAX, u32::MAX));
    }

    #[test]
    fn derived_values_follow_the_equipment() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        assert_eq!(ch.hp_max(&c), 45, "jacket +5");
        assert_eq!(ch.stat(&c, Stat::Net), 7, "deck +1 net");
        assert_eq!(ch.sight_bonus(&c), 4);
        ch.equipment.remove(&Slot::Eyes);
        assert_eq!(ch.sight_bonus(&c), 0);
        ch.equipment.remove(&Slot::Deck);
        assert_eq!(ch.stat(&c, Stat::Net), 6);
        assert!(ch.weight(&c) > 0.0 && ch.carry_cap(&c) > ch.weight(&c));
        assert_eq!(
            ch.abilities(&c)
                .iter()
                .filter(|(_, s)| matches!(s, Source::Item(_)))
                .count(),
            0,
            "nothing worn grants now"
        );
    }

    #[test]
    fn give_saturates_instead_of_overflowing() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let caps = ItemId::new("bottle_caps");
        ch.give(caps.clone(), u32::MAX);
        assert_eq!(ch.count(&caps), u32::MAX);
        ch.give(caps.clone(), 0);
        assert_eq!(ch.inventory.iter().filter(|s| s.item == caps).count(), 1);
    }
}
