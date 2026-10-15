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
    Talent(TalentId),
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
    /// Max focus before talents; see `focus_max`.
    pub focus_base: u32,
    pub stats: Stats,
    pub origin: OriginId,
    pub profession: ProfessionId,
    pub categories: BTreeSet<CategoryId>,
    pub skills: BTreeSet<SkillId>,
    /// Ranks held per talent; absent means none.
    pub talents: BTreeMap<TalentId, u8>,
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
    /// Unspent talent points: one at creation, one per level.
    pub talent_points: u32,
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
            focus_base: cr.focus,
            stats: b.stats.clone(),
            origin: b.origin.clone().unwrap_or_else(|| OriginId::new("")),
            profession: b
                .profession
                .clone()
                .unwrap_or_else(|| ProfessionId::new("")),
            categories: b.categories.clone(),
            skills: b.skills.clone(),
            talents: BTreeMap::new(),
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
            talent_points: cr.talent_points,
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

    /// Perks of everything equipped, in slot order, then those of every talent rank.
    pub fn perks<'a>(&'a self, c: &'a Content) -> impl Iterator<Item = &'a Perk> + 'a {
        Slot::ALL
            .iter()
            .filter_map(move |s| self.equipment.get(s))
            .filter_map(move |id| c.item(id))
            .flat_map(|i| i.perks.iter())
            .chain(self.talent_effects(c).filter_map(|b| match b {
                Boon::Perk(p) => Some(p),
                _ => None,
            }))
    }

    /// Max focus: the base plus talent perks, never below one.
    pub fn focus_max(&self, c: &Content) -> u32 {
        let bonus: i32 = self
            .perks(c)
            .filter_map(|p| match p {
                Perk::Focus(n) => Some(*n),
                _ => None,
            })
            .sum();
        (self.focus_base as i32 + bonus).max(1) as u32
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
        self.talent_points += gained as u32;
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
    /// then those granted by equipment in slot order, then by talents; deduplicated,
    /// the first source winning.
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
        for t in &c.talents {
            let rank = self.talent_rank(&t.id) as usize;
            for b in t.effects.iter().take(rank).flatten() {
                if let Boon::Grant(id) = b
                    && !out.iter().any(|(have, _)| have == id)
                {
                    out.push((id.clone(), Source::Talent(t.id.clone())));
                }
            }
        }
        out
    }

    /// The content's ability with every sharpening the talents give it: reach added
    /// (never below one), cooldown added (never below zero).
    pub fn ability(&self, c: &Content, id: &AbilityId) -> Option<Ability> {
        let mut a = c.ability(id)?.clone();
        let (mut dr, mut dc) = (0i32, 0i64);
        for b in self.talent_effects(c) {
            if let Boon::Sharpen {
                ability,
                radius,
                cooldown,
            } = b
                && ability == id
            {
                dr += radius;
                dc += cooldown;
            }
        }
        if let AbilityKind::Active(
            Active::Scan { radius } | Active::Ping { radius } | Active::Trace { radius },
        ) = &mut a.kind
        {
            *radius = (*radius + dr).max(1);
        }
        a.cooldown = (a.cooldown as i64 + dc).max(0) as u64;
        Some(a)
    }

    /// The rank held in a talent; zero when unlearned.
    pub fn talent_rank(&self, id: &TalentId) -> u8 {
        self.talents.get(id).copied().unwrap_or(0)
    }

    /// Ranks held across one tree.
    pub fn tree_points(&self, c: &Content, stat: Stat) -> u32 {
        c.tree(stat)
            .map(|t| {
                t.tiers
                    .iter()
                    .flatten()
                    .map(|id| self.talent_rank(id) as u32)
                    .sum()
            })
            .unwrap_or(0)
    }

    /// Whether a tier (from zero) of a tree has opened: `tier_gate` points per tier
    /// below it.
    pub fn tier_open(&self, c: &Content, stat: Stat, tier: usize) -> bool {
        self.tree_points(c, stat) >= c.creation.tier_gate * tier as u32
    }

    /// Why a talent cannot take a point now, in the order the sheet reports it:
    /// no point left, the tier closed, the prerequisite short, already at its best.
    pub fn can_learn(&self, c: &Content, id: &TalentId) -> Result<(), String> {
        let t = c.talent(id).ok_or_else(|| format!("unknown talent {id}"))?;
        let (tree, tier, _) = c
            .tree_of(id)
            .ok_or_else(|| format!("{} hangs in no tree", t.name))?;
        if self.talent_points == 0 {
            return Err("no points left".into());
        }
        let need = c.creation.tier_gate * tier as u32;
        let have = self.tree_points(c, tree.stat);
        if have < need {
            let more = need - have;
            let s = if more == 1 { "" } else { "s" };
            return Err(format!("{more} more point{s} in {}", tree.name));
        }
        if let Some(req) = t.requires.as_ref().and_then(|r| c.talent(r))
            && self.talent_rank(&req.id) < req.ranks
        {
            return Err(format!("needs {} {}/{}", req.name, req.ranks, req.ranks));
        }
        if self.talent_rank(id) >= t.ranks {
            return Err("already at its best".into());
        }
        Ok(())
    }

    /// Spends a point on a talent under `can_learn`; returns the new rank.
    pub fn learn(&mut self, c: &Content, id: &TalentId) -> Result<u8, String> {
        self.can_learn(c, id)?;
        self.talent_points -= 1;
        let rank = self.talents.entry(id.clone()).or_insert(0);
        *rank += 1;
        Ok(*rank)
    }

    /// Every boon of every rank held, in content order.
    pub fn talent_effects<'a>(&'a self, c: &'a Content) -> impl Iterator<Item = &'a Boon> + 'a {
        c.talents.iter().flat_map(move |t| {
            let rank = self.talent_rank(&t.id) as usize;
            t.effects.iter().take(rank).flatten()
        })
    }

    /// The named promises the talents hold.
    pub fn flags(&self, c: &Content) -> BTreeSet<String> {
        self.talent_effects(c)
            .filter_map(|b| match b {
                Boon::Flag(f) => Some(f.clone()),
                _ => None,
            })
            .collect()
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
        self.talent_rank(id) >= 1
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
            (ch.level, ch.xp, ch.focus, ch.focus_base, ch.credits),
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

    #[test]
    fn learning_follows_points_tiers_prerequisites_and_ranks() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        assert_eq!(ch.talent_points, 1);
        assert!(ch.talents.is_empty());
        let edge = c.tree(Stat::Reflex).unwrap();
        let first = edge.tiers[0][0].clone();
        let second = edge.tiers[1][0].clone();
        assert_eq!(
            ch.can_learn(&c, &second).unwrap_err(),
            "3 more points in EDGE"
        );
        assert_eq!(ch.learn(&c, &first).unwrap(), 1);
        assert!(ch.has_talent(&first));
        assert_eq!(ch.talent_points, 0);
        assert_eq!(ch.can_learn(&c, &first).unwrap_err(), "no points left");
        ch.talent_points = 10;
        let ranks = c.talent(&first).unwrap().ranks;
        for r in 2..=ranks {
            assert_eq!(ch.learn(&c, &first).unwrap(), r);
        }
        assert_eq!(ch.can_learn(&c, &first).unwrap_err(), "already at its best");
        assert_eq!(ch.tree_points(&c, Stat::Reflex), ranks as u32);
        let other = edge.tiers[0][1].clone();
        while ch.tree_points(&c, Stat::Reflex) < 3 {
            assert!(!ch.tier_open(&c, Stat::Reflex, 1));
            ch.learn(&c, &other).unwrap();
        }
        assert!(ch.tier_open(&c, Stat::Reflex, 1));
        // A prerequisite must be maxed before its dependant opens.
        let needy = c.talents.iter().find(|t| t.requires.is_some()).unwrap();
        let (tree, tier, _) = c.tree_of(&needy.id).unwrap();
        let stat = tree.stat;
        let req = c.talent(needy.requires.as_ref().unwrap()).unwrap();
        let mut ch2 = vex(&c);
        ch2.talent_points = 30;
        for lower in 0..tier {
            for id in &c.tree(stat).unwrap().tiers[lower] {
                if id == &req.id {
                    continue;
                }
                while ch2.can_learn(&c, id).is_ok() {
                    ch2.learn(&c, id).unwrap();
                }
            }
        }
        while !ch2.tier_open(&c, stat, tier) {
            ch2.talents
                .insert(req.id.clone(), ch2.talent_rank(&req.id) + 1);
            if ch2.talent_rank(&req.id) >= req.ranks {
                break;
            }
        }
        if ch2.talent_rank(&req.id) < req.ranks {
            assert_eq!(
                ch2.can_learn(&c, &needy.id).unwrap_err(),
                format!("needs {} {}/{}", req.name, req.ranks, req.ranks)
            );
        }
        ch2.talents.insert(req.id.clone(), req.ranks);
        assert!(ch2.can_learn(&c, &needy.id).is_ok());
    }

    #[test]
    fn ranks_carry_perks_grants_sharpening_and_flags() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let with = |kind: fn(&Boon) -> bool| {
            c.talents
                .iter()
                .find(|t| t.effects.iter().flatten().any(kind))
                .unwrap()
                .id
                .clone()
        };
        let stat_t = with(|e| matches!(e, Boon::Perk(Perk::Stat(..))));
        let focus_t = with(|e| matches!(e, Boon::Perk(Perk::Focus(_))));
        let grant_t = with(|e| matches!(e, Boon::Grant(_)));
        let sharp_t = with(|e| matches!(e, Boon::Sharpen { .. }));
        let flag_t = with(|e| matches!(e, Boon::Flag(_)));
        let force = |ch: &mut Character, id: &TalentId| {
            let t = c.talent(id).unwrap();
            ch.talents.insert(id.clone(), t.ranks);
        };
        let before = ch.focus_max(&c);
        force(&mut ch, &focus_t);
        assert!(ch.focus_max(&c) > before);
        let first_stat = c
            .talent(&stat_t)
            .unwrap()
            .effects
            .iter()
            .flatten()
            .find_map(|e| match e {
                Boon::Perk(Perk::Stat(s, n)) => Some((*s, *n)),
                _ => None,
            })
            .unwrap();
        let before = ch.stat(&c, first_stat.0);
        force(&mut ch, &stat_t);
        assert!(ch.stat(&c, first_stat.0) as i32 >= before as i32 + first_stat.1 as i32);
        force(&mut ch, &grant_t);
        let granted = c
            .talent(&grant_t)
            .unwrap()
            .effects
            .iter()
            .flatten()
            .find_map(|e| match e {
                Boon::Grant(a) => Some(a.clone()),
                _ => None,
            })
            .unwrap();
        assert!(ch.abilities(&c).iter().any(|(a, _)| a == &granted));
        force(&mut ch, &sharp_t);
        let (ability, radius, cooldown) = c
            .talent(&sharp_t)
            .unwrap()
            .effects
            .iter()
            .flatten()
            .find_map(|e| match e {
                Boon::Sharpen {
                    ability,
                    radius,
                    cooldown,
                } => Some((ability.clone(), *radius, *cooldown)),
                _ => None,
            })
            .unwrap();
        let base = c.ability(&ability).unwrap();
        let eff = ch.ability(&c, &ability).unwrap();
        let reach = |a: &Ability| match a.kind {
            AbilityKind::Active(
                Active::Scan { radius } | Active::Ping { radius } | Active::Trace { radius },
            ) => radius,
            AbilityKind::Passive => 0,
        };
        if radius != 0 {
            assert_ne!(reach(&eff), reach(base));
        }
        if cooldown != 0 {
            assert_ne!(eff.cooldown, base.cooldown);
        }
        force(&mut ch, &flag_t);
        let flag = c
            .talent(&flag_t)
            .unwrap()
            .effects
            .iter()
            .flatten()
            .find_map(|e| match e {
                Boon::Flag(f) => Some(f.clone()),
                _ => None,
            })
            .unwrap();
        assert!(ch.flags(&c).contains(&flag));
        // Sharpening never takes a cooldown below zero or a reach below one.
        let mut sharp = c.clone();
        sharp
            .talents
            .iter_mut()
            .find(|t| t.id == sharp_t)
            .unwrap()
            .effects[0] = vec![Boon::Sharpen {
            ability: ability.clone(),
            radius: -99,
            cooldown: -99_999,
        }];
        let eff = ch.ability(&sharp, &ability).unwrap();
        assert_eq!(eff.cooldown, 0);
        assert_eq!(reach(&eff), 1);
        // An ability granted by a talent and an item lists once.
        let item = c
            .items
            .iter()
            .find(|i| i.grants.contains(&granted))
            .cloned();
        if let Some(item) = item {
            ch.give(item.id.clone(), 1);
            ch.equipment.insert(item.slot.unwrap(), item.id.clone());
        }
        let listed = ch.abilities(&c);
        for (a, _) in &listed {
            assert_eq!(listed.iter().filter(|(b, _)| b == a).count(), 1, "{a} once");
        }
    }

    #[test]
    fn a_level_brings_a_talent_point() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        ch.talent_points = 0;
        assert_eq!(ch.gain_xp(Character::xp_to_next(1)), 1);
        assert_eq!(ch.talent_points, 1);
        assert_eq!(
            ch.gain_xp(Character::xp_to_next(2) + Character::xp_to_next(3)),
            2
        );
        assert_eq!(ch.talent_points, 3);
    }

    #[test]
    fn boons_read_in_words() {
        let c = Content::load().unwrap();
        assert_eq!(
            Boon::Perk(Perk::Stat(Stat::Reflex, 1)).words(&c),
            "+1 Reflex"
        );
        assert_eq!(Boon::Perk(Perk::HpMax(3)).words(&c), "+3 max HP");
        assert_eq!(Boon::Perk(Perk::Focus(2)).words(&c), "+2 focus");
        assert_eq!(Boon::Perk(Perk::Sight(2)).words(&c), "sight +2");
        assert_eq!(Boon::Perk(Perk::Carry(4.0)).words(&c), "carry +4 kg");
        assert_eq!(Boon::Grant(AbilityId::new("ping")).words(&c), "grants Ping");
        assert_eq!(
            Boon::Sharpen {
                ability: AbilityId::new("ping"),
                radius: 4,
                cooldown: 0
            }
            .words(&c),
            "ping reaches 4 further"
        );
        assert_eq!(
            Boon::Sharpen {
                ability: AbilityId::new("ping"),
                radius: 0,
                cooldown: -60
            }
            .words(&c),
            "ping cools 60 ticks sooner"
        );
        assert_eq!(
            Boon::Flag("quick_draw".into()).words(&c),
            "later: quick_draw"
        );
    }
}
