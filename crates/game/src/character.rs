//! The player's character: stats, skills, talents, equipment, inventory and quest log.

use crate::action::Host;
use crate::build::Build;
use crate::content::Content;
use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};

/// A copy's id, unique for the character that made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CopyId(pub u32);

/// One copy of an item that carries state of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemCopy {
    pub id: CopyId,
    /// One entry per socket; a fitted piece is a stack of one with its own copy.
    pub fitted: Vec<Option<Stack>>,
}

impl ItemCopy {
    /// What its fitted pieces weigh.
    fn fitted_weight(&self, c: &Content) -> f32 {
        self.fitted.iter().flatten().map(|s| s.weight(c)).sum()
    }

    fn fitted_value(&self, c: &Content) -> u32 {
        self.fitted.iter().flatten().map(|s| s.value(c)).sum()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    pub item: ItemId,
    pub qty: u32,
    /// The copy, for an item held per copy: such a stack holds one and never merges.
    pub copy: Option<ItemCopy>,
}

impl Stack {
    /// A stack of an item with no state of its own.
    pub fn new(item: ItemId, qty: u32) -> Stack {
        Stack {
            item,
            qty,
            copy: None,
        }
    }

    pub fn copy_id(&self) -> Option<CopyId> {
        self.copy.as_ref().map(|k| k.id)
    }

    /// The stack's weight, fitted pieces included.
    pub fn weight(&self, c: &Content) -> f32 {
        let own = c
            .item(&self.item)
            .map_or(0.0, |i| i.weight * self.qty as f32);
        own + self.copy.as_ref().map_or(0.0, |k| k.fitted_weight(c))
    }

    /// The stack's value, fitted pieces included.
    pub fn value(&self, c: &Content) -> u32 {
        let own = c.item(&self.item).map_or(0, |i| i.value * self.qty);
        own + self.copy.as_ref().map_or(0, |k| k.fitted_value(c))
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

/// Where a program lands among the loaded ones, and what it sends back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotPlan {
    /// The loaded order after the slot.
    pub loaded: Vec<ItemId>,
    /// The slotted program's index in `loaded`.
    pub at: usize,
    /// What goes back to the pack, in removal order.
    pub displaced: Vec<ItemId>,
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
    /// The copy of each worn item held per copy; a stateless worn item has none.
    pub worn: BTreeMap<Slot, ItemCopy>,
    pub inventory: Vec<Stack>,
    /// The tick each active ability last fired, for cooldowns.
    pub fired: BTreeMap<AbilityId, u64>,
    /// Unspent creation points, spendable from the deck under the same rules.
    pub stat_points: u32,
    pub sp: u32,
    /// Unspent talent points: one at creation, one per level.
    pub talent_points: u32,
    /// Programs loaded into the jacked deck, in loading order; each is also in the pack.
    pub loaded: Vec<ItemId>,
    /// The id the next copy takes.
    pub next_copy: u32,
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
            worn: BTreeMap::new(),
            inventory: Vec::new(),
            fired: BTreeMap::new(),
            stat_points: b.points_left(c).max(0) as u32,
            sp: b.sp_left(c).max(0) as u32,
            talent_points: cr.talent_points,
            loaded: Vec::new(),
            next_copy: 0,
        };
        let mut kit: Vec<(ItemId, u32)> = Vec::new();
        if let Some(p) = b.profession.as_ref().and_then(|id| c.profession(id)) {
            ch.credits = p.credits;
            kit.extend(p.kit.iter().cloned());
        }
        kit.extend(b.kit.iter().cloned());
        // The first item that fits a free slot is worn, as a new copy when it is held
        // per copy; everything else is packed.
        for (id, qty) in kit {
            let Some(item) = c.item(&id) else { continue };
            let mut qty = qty;
            if let Some(slot) = item.slot
                && qty > 0
                && !ch.equipment.contains_key(&slot)
            {
                let copy = item.per_copy().then(|| ch.new_copy(c, &id));
                ch.wear(slot, id.clone(), copy);
                qty -= 1;
            }
            ch.give_item(c, id, qty);
        }
        ch.hp = ch.hp_max(c);
        ch.load_all(c);
        ch
    }

    /// Loads the pack's programs in pack order while they fit.
    fn load_all(&mut self, c: &Content) {
        let programs: Vec<ItemId> = self
            .inventory
            .iter()
            .map(|s| s.item.clone())
            .filter(|id| c.item(id).is_some_and(|i| i.kind == ItemKind::Program))
            .collect();
        for id in programs {
            let _ = self.load(c, &id);
        }
    }

    /// The jacked deck and its slots.
    pub fn deck<'a>(&self, c: &'a Content) -> Option<(&'a Item, DeckSpec)> {
        let item = c.item(self.equipment.get(&Slot::Deck)?)?;
        Some((item, item.deck?))
    }

    /// Slots the loaded programs take.
    pub fn loaded_size(&self, c: &Content) -> u8 {
        self.loaded_programs(c)
            .filter_map(|i| i.program)
            .map(|p| p.size)
            .sum()
    }

    /// The loaded programs, in loading order.
    pub fn loaded_programs<'a>(&'a self, c: &'a Content) -> impl Iterator<Item = &'a Item> + 'a {
        self.loaded.iter().filter_map(move |id| c.item(id))
    }

    /// Why a program cannot load now, in the order the deck reports it.
    pub fn can_load(&self, c: &Content, id: &ItemId) -> Result<(), String> {
        let item = c.item(id).ok_or_else(|| format!("unknown item {id}"))?;
        let Some(program) = item.program.filter(|_| item.kind == ItemKind::Program) else {
            return Err("that is not a program".into());
        };
        let (_, deck) = self.deck(c).ok_or_else(|| "no deck jacked".to_string())?;
        if self.count(id) == 0 {
            return Err("not in your pack".into());
        }
        if self.loaded.contains(id) {
            return Err("already loaded".into());
        }
        let used = self.loaded_size(c);
        if used + program.size > deck.slots {
            return Err(format!("no room: {used} of {} slots", deck.slots));
        }
        Ok(())
    }

    /// Loads a program under `can_load`.
    pub fn load(&mut self, c: &Content, id: &ItemId) -> Result<String, String> {
        self.can_load(c, id)?;
        self.loaded.push(id.clone());
        let name = c.item(id).map(|i| i.name.as_str()).unwrap_or("?");
        let slots = self.deck(c).map(|(_, d)| d.slots).unwrap_or(0);
        Ok(format!(
            "`{name}` loaded · {}/{slots} slots",
            self.loaded_size(c)
        ))
    }

    pub fn unload(&mut self, c: &Content, id: &ItemId) -> Result<String, String> {
        let at = self
            .loaded
            .iter()
            .position(|l| l == id)
            .ok_or_else(|| "not loaded".to_string())?;
        self.loaded.remove(at);
        let name = c.item(id).map(|i| i.name.as_str()).unwrap_or("?");
        Ok(format!("`{name}` unloaded"))
    }

    /// Where a program would land at `at` in the loaded order, and what it sends
    /// back to the pack. Changes nothing.
    pub fn plan_slot(&self, c: &Content, id: &ItemId, at: usize) -> Result<SlotPlan, String> {
        let item = c.item(id).ok_or_else(|| format!("unknown item {id}"))?;
        let Some(program) = item.program.filter(|_| item.kind == ItemKind::Program) else {
            return Err("that is not a program".into());
        };
        let (_, deck) = self.deck(c).ok_or_else(|| "no deck jacked".to_string())?;
        if self.count(id) == 0 {
            return Err("not in your pack".into());
        }
        if self.loaded.contains(id) {
            return Err("already loaded".into());
        }
        if at > self.loaded.len() {
            return Err("no such slot".into());
        }
        if program.size > deck.slots {
            return Err(format!(
                "too big for this deck: needs {} of {} slots",
                program.size, deck.slots
            ));
        }
        let size_of = |p: &ItemId| c.item(p).and_then(|i| i.program).map_or(0, |p| p.size);
        let mut loaded = self.loaded.clone();
        let mut displaced = Vec::new();
        if at < loaded.len() {
            displaced.push(loaded.remove(at));
        }
        loaded.insert(at, id.clone());
        let mut at = at;
        // Over the deck: the program after it goes first, then the one before.
        while loaded.iter().map(size_of).sum::<u8>() > deck.slots {
            if at + 1 < loaded.len() {
                displaced.push(loaded.remove(at + 1));
            } else {
                displaced.push(loaded.remove(at - 1));
                at -= 1;
            }
        }
        Ok(SlotPlan {
            loaded,
            at,
            displaced,
        })
    }

    /// Slots a program at `at` under `plan_slot`; what it covers goes back to the pack.
    pub fn slot_at(&mut self, c: &Content, id: &ItemId, at: usize) -> Result<String, String> {
        let plan = self.plan_slot(c, id, at)?;
        self.loaded = plan.loaded;
        let name = |id: &ItemId| {
            c.item(id)
                .map(|i| i.name.as_str())
                .unwrap_or("?")
                .to_string()
        };
        if plan.displaced.is_empty() {
            let slots = self.deck(c).map(|(_, d)| d.slots).unwrap_or(0);
            return Ok(format!(
                "`{}` loaded · {}/{slots} slots",
                name(id),
                self.loaded_size(c)
            ));
        }
        let back: Vec<String> = plan
            .displaced
            .iter()
            .map(|d| format!("`{}`", name(d)))
            .collect();
        Ok(format!(
            "`{}` slotted · {} to inventory",
            name(id),
            back.join(", ")
        ))
    }

    /// The character a template describes, or none for an unknown template.
    pub fn from_template(c: &Content, id: &TemplateId) -> Option<Character> {
        let t = c.template(id)?;
        Some(Self::from_build(c, &Build::from_template(c, t)))
    }

    /// The debug runner: Vex's build as D'SHAWN at level 50 with every talent at
    /// its best, full HP and focus, a rebuilt five-slot deck jacked in with its
    /// sockets empty, the deck hardware and the weapon pieces in the pack and
    /// every program that fits loaded. For the title's NEW GAME (DEBUG).
    pub fn debug_runner(c: &Content) -> Character {
        let mut d = Self::from_template(c, &TemplateId::new("vex"))
            .expect("the vex template is in the content");
        d.handle = "D'SHAWN".to_string();
        d.level = 50;
        d.xp = 0;
        d.talent_points = 0;
        d.talents = c.talents.iter().map(|t| (t.id.clone(), t.ranks)).collect();
        d.hp = d.hp_max(c);
        d.focus = d.focus_max(c);
        d.give(ItemId::new("backdoor"), 1);
        // The pawnshop deck goes back to the pack; the rebuilt one takes its slot.
        let deck = ItemId::new("rebuilt_deck");
        let copy = d.new_copy(c, &deck);
        if let Some(old) = d.wear(Slot::Deck, deck, Some(copy)) {
            d.stow(old);
        }
        for id in ["coldfin_1", "coldfin_2", "sidecar", "hushcan", "redeye"] {
            d.give_item(c, ItemId::new(id), 1);
        }
        d.load_all(c);
        d
    }

    pub fn has_skill(&self, id: &SkillId) -> bool {
        self.skills.contains(id)
    }

    /// Whether the character has an ability from any source: an item, a category,
    /// a talent.
    pub fn has_ability(&self, c: &Content, id: &AbilityId) -> bool {
        self.abilities(c).iter().any(|(a, _)| a == id)
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
        let perks: i32 = self
            .perks(c)
            .filter_map(|p| match p {
                Perk::Stat(s, n) if *s == stat => Some(*n as i32),
                _ => None,
            })
            .sum();
        let fitted: i32 = self
            .worn_mods(c)
            .iter()
            .filter_map(|m| match m {
                Mod::Stat(s, n) if *s == stat => Some(*n as i32),
                _ => None,
            })
            .sum();
        let bonus = perks + fitted;
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
        let ready = fired + self.ability(c, id)?.cooldown;
        (ready > tick).then(|| ready - tick)
    }

    /// Puts `qty` of an item in the pack, merging into a stack of the same item that
    /// holds no copy.
    pub fn give(&mut self, item: ItemId, qty: u32) {
        if qty == 0 {
            return;
        }
        match self
            .inventory
            .iter_mut()
            .find(|s| s.item == item && s.copy.is_none())
        {
            Some(s) => s.qty = s.qty.saturating_add(qty),
            None => self.inventory.push(Stack::new(item, qty)),
        }
    }

    /// Puts `qty` of an item in the pack: an item held per copy as that many new
    /// copies, anything else as `give` does.
    pub fn give_item(&mut self, c: &Content, item: ItemId, qty: u32) {
        if !c.item(&item).is_some_and(Item::per_copy) {
            self.give(item, qty);
            return;
        }
        for _ in 0..qty {
            let copy = self.new_copy(c, &item);
            self.inventory.push(Stack {
                item: item.clone(),
                qty: 1,
                copy: Some(copy),
            });
        }
    }

    /// Puts a stack in the pack as it is: a copy as its own stack, anything else
    /// merged by `give`.
    pub fn stow(&mut self, s: Stack) {
        match s.copy {
            Some(_) => self.inventory.push(s),
            None => self.give(s.item, s.qty),
        }
    }

    /// Puts a stack in the pack: a per-copy item that has no copy gets one on the
    /// way in, so hardware can always be fitted by id.
    pub fn stow_item(&mut self, c: &Content, s: Stack) {
        if s.copy.is_none() {
            self.give_item(c, s.item, s.qty);
        } else {
            self.stow(s);
        }
    }

    /// A new copy of an item, every socket empty.
    pub fn new_copy(&mut self, c: &Content, item: &ItemId) -> ItemCopy {
        let id = CopyId(self.next_copy);
        self.next_copy += 1;
        let sockets = c.item(item).map_or(0, |i| i.sockets.len());
        ItemCopy {
            id,
            fitted: vec![None; sockets],
        }
    }

    /// Wears an item in its slot, with its copy when it has one; returns what the
    /// slot wore, with that one's copy.
    pub fn wear(&mut self, slot: Slot, item: ItemId, copy: Option<ItemCopy>) -> Option<Stack> {
        let old = self.equipment.insert(slot, item);
        let old_copy = match copy {
            Some(k) => self.worn.insert(slot, k),
            None => self.worn.remove(&slot),
        };
        old.map(|item| Stack {
            item,
            qty: 1,
            copy: old_copy,
        })
    }

    /// Takes off what a slot wears, with its copy.
    pub fn take_off(&mut self, slot: Slot) -> Option<Stack> {
        let item = self.equipment.remove(&slot)?;
        Some(Stack {
            item,
            qty: 1,
            copy: self.worn.remove(&slot),
        })
    }

    /// The pack stack holding a copy.
    pub fn copy_in_pack(&self, id: CopyId) -> Option<&Stack> {
        self.inventory.iter().find(|s| s.copy_id() == Some(id))
    }

    /// The copy a slot wears. It counts only while the slot still wears something:
    /// `equipment` is the truth, and `worn` can be left behind.
    pub fn worn_copy(&self, slot: Slot) -> Option<&ItemCopy> {
        self.equipment.get(&slot)?;
        self.worn.get(&slot)
    }

    /// The mods of the hardware fitted in anything worn, slots in order and
    /// sockets in order. Hardware in the pack, or in an item taken off, counts
    /// for nothing.
    pub fn worn_mods(&self, c: &Content) -> Vec<Mod> {
        Slot::ALL
            .iter()
            .filter_map(|slot| self.worn_copy(*slot))
            .flat_map(|copy| copy.fitted.iter().flatten())
            .filter_map(|s| c.item(&s.item)?.hardware.as_ref())
            .flat_map(|h| h.mods.iter().copied())
            .collect()
    }

    /// Percent off each trace the deck makes; stacked hardware cuts no more than half.
    pub fn trace_cut(&self, c: &Content) -> u8 {
        let sum: u32 = self
            .worn_mods(c)
            .iter()
            .map(|m| match m {
                Mod::TraceCut(n) => *n as u32,
                _ => 0,
            })
            .sum();
        sum.min(50) as u8
    }

    /// Added to every program turn's power.
    pub fn deck_power(&self, c: &Content) -> i32 {
        self.worn_mods(c)
            .iter()
            .map(|m| match m {
                Mod::Power(n) => *n as i32,
                _ => 0,
            })
            .sum()
    }

    pub fn is_worn_copy(&self, id: CopyId) -> bool {
        self.worn
            .iter()
            .any(|(slot, k)| k.id == id && self.equipment.contains_key(slot))
    }

    /// The item a host is: what the slot wears, or the copy's item in the pack.
    pub fn host_item(&self, host: Host) -> Option<&ItemId> {
        match host {
            Host::Worn(slot) => self.equipped(slot),
            Host::Pack(id) => self.copy_in_pack(id).map(|s| &s.item),
        }
    }

    /// The host's copy, when it has one.
    pub fn host_copy(&self, host: Host) -> Option<&ItemCopy> {
        match host {
            Host::Worn(slot) => self.worn_copy(slot),
            Host::Pack(id) => self.copy_in_pack(id)?.copy.as_ref(),
        }
    }

    /// The host's copy to change. A per-copy item worn without one gets it now.
    pub fn host_copy_mut(&mut self, c: &Content, host: Host) -> Option<&mut ItemCopy> {
        match host {
            Host::Worn(slot) => {
                let item = self.equipment.get(&slot)?.clone();
                if !c.item(&item).is_some_and(Item::per_copy) {
                    return None;
                }
                if !self.worn.contains_key(&slot) {
                    let copy = self.new_copy(c, &item);
                    self.worn.insert(slot, copy);
                }
                self.worn.get_mut(&slot)
            }
            Host::Pack(id) => self
                .inventory
                .iter_mut()
                .find_map(|s| s.copy.as_mut().filter(|k| k.id == id)),
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

    /// Carried weight: inventory plus everything equipped, fitted pieces included.
    pub fn weight(&self, c: &Content) -> f32 {
        let carried: f32 = self.inventory.iter().map(|s| s.weight(c)).sum();
        let worn: f32 = self
            .equipment
            .iter()
            .map(|(slot, id)| {
                let own = c.item(id).map_or(0.0, |i| i.weight);
                own + self.worn.get(slot).map_or(0.0, |k| k.fitted_weight(c))
            })
            .sum();
        carried + worn
    }

    pub fn value(&self, c: &Content) -> u32 {
        let carried: u32 = self.inventory.iter().map(|s| s.value(c)).sum();
        let worn: u32 = self
            .equipment
            .iter()
            .map(|(slot, id)| {
                let own = c.item(id).map_or(0, |i| i.value);
                own + self.worn.get(slot).map_or(0, |k| k.fitted_value(c))
            })
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
        // An ability granted by a talent and an item lists once, from the first source.
        let mut ch = vex(&c);
        let (handshake, ping, port) = (
            TalentId::new("handshake"),
            AbilityId::new("ping"),
            ItemId::new("neural_port"),
        );
        assert!(!ch.abilities(&c).iter().any(|(a, _)| a == &ping));
        ch.talents.insert(handshake.clone(), 1);
        let listed = ch.abilities(&c);
        assert!(
            listed.contains(&(ping.clone(), Source::Talent(handshake.clone()))),
            "{listed:?}"
        );
        ch.give(port.clone(), 1);
        ch.equipment.insert(Slot::Neural, port.clone());
        let listed = ch.abilities(&c);
        assert_eq!(
            listed.iter().filter(|(a, _)| a == &ping).count(),
            1,
            "{listed:?}"
        );
        assert!(
            listed.contains(&(ping.clone(), Source::Item(port))),
            "the item comes first: {listed:?}"
        );
        let _ = granted;
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
            "later: quick_draw",
            "a flag nothing reads yet says so"
        );
        // The flags the Net reads say what they do.
        assert_eq!(Boon::Flag("ice_pick".into()).words(&c), "+2 power to crack");
        assert_eq!(
            Boon::Flag("deep_dive".into()).words(&c),
            "+2 power to copy · a vault copy makes no heat"
        );
        assert_eq!(Boon::Flag("black_ice".into()).words(&c), "+1 attack damage");
        assert_eq!(
            Boon::Flag("null_route".into()).words(&c),
            "the first dump in a construct routes away"
        );
        assert_eq!(
            Boon::Flag("deep_end".into()).words(&c),
            "black ICE hits halved · the failsafe waits for an eighth · ten minutes cooling"
        );
    }

    #[test]
    fn debug_runner_jacks_the_rebuilt_deck() {
        let c = Content::load().unwrap();
        let d = Character::debug_runner(&c);
        assert_eq!(d.equipped(Slot::Deck), Some(&ItemId::new("rebuilt_deck")));
        assert_eq!(d.count(&ItemId::new("pawnshop_deck")), 1);
        assert_eq!(d.count(&ItemId::new("rebuilt_deck")), 0);
        let ids = ["crack", "copy", "attack", "backdoor"].map(ItemId::new);
        assert_eq!(d.loaded, ids);
        assert_eq!(d.loaded_size(&c), 5);
        assert_eq!(
            d.deck(&c).unwrap().1,
            DeckSpec {
                slots: 5,
                rating: 3,
                look: DeckLook::Pawn
            }
        );
    }

    #[test]
    fn the_kit_loads_into_the_deck() {
        let c = Content::load().unwrap();
        let ch = vex(&c);
        assert_eq!(ch.loaded, vec![ItemId::new("crack"), ItemId::new("copy")]);
        assert_eq!(ch.loaded_size(&c), 2);
        assert_eq!(
            ch.deck(&c).map(|(i, s)| (i.id.clone(), s.slots)),
            Some((ItemId::new("pawnshop_deck"), 2))
        );
        let k = Character::from_template(&c, &TemplateId::new("kessler")).unwrap();
        assert!(k.loaded.is_empty() && k.deck(&c).is_none());
    }

    #[test]
    fn loading_follows_the_rules_in_order() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let (spoof, copy, crack) = (
            ItemId::new("spoof"),
            ItemId::new("copy"),
            ItemId::new("crack"),
        );
        assert_eq!(ch.can_load(&c, &spoof).unwrap_err(), "not in your pack");
        ch.give(spoof.clone(), 1);
        assert_eq!(
            ch.can_load(&c, &spoof).unwrap_err(),
            "no room: 2 of 2 slots"
        );
        assert_eq!(ch.unload(&c, &copy).unwrap(), "`Copy` unloaded");
        assert_eq!(ch.load(&c, &spoof).unwrap(), "`Spoof` loaded · 2/2 slots");
        assert_eq!(ch.load(&c, &spoof).unwrap_err(), "already loaded");
        assert_eq!(
            ch.load(&c, &ItemId::new("hardhat")).unwrap_err(),
            "that is not a program"
        );
        assert_eq!(ch.unload(&c, &copy).unwrap_err(), "not loaded");
        let names: Vec<String> = ch.loaded_programs(&c).map(|i| i.name.clone()).collect();
        assert_eq!(names, vec!["Crack", "Spoof"]);
        let mut k = Character::from_template(&c, &TemplateId::new("kessler")).unwrap();
        k.give(crack.clone(), 1);
        assert_eq!(k.can_load(&c, &crack).unwrap_err(), "no deck jacked");
    }

    fn ids(names: &[&str]) -> Vec<ItemId> {
        names.iter().map(|n| ItemId::new(n)).collect()
    }

    #[test]
    fn slot_at_the_end_loads() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let spoof = ItemId::new("spoof");
        ch.give(spoof.clone(), 1);
        ch.unload(&c, &ItemId::new("copy")).unwrap();
        assert_eq!(
            ch.slot_at(&c, &spoof, 1).unwrap(),
            "`Spoof` loaded · 2/2 slots"
        );
        assert_eq!(ch.loaded, ids(&["crack", "spoof"]));
    }

    #[test]
    fn slot_onto_a_program_swaps_in_place() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let spoof = ItemId::new("spoof");
        ch.give(spoof.clone(), 1);
        assert_eq!(
            ch.slot_at(&c, &spoof, 0).unwrap(),
            "`Spoof` slotted · `Crack` to inventory"
        );
        assert_eq!(ch.loaded, ids(&["spoof", "copy"]));
    }

    #[test]
    fn a_size_two_program_over_two_sends_both_back() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let backdoor = ItemId::new("backdoor");
        ch.give(backdoor.clone(), 1);
        let plan = ch.plan_slot(&c, &backdoor, 0).unwrap();
        assert_eq!(plan.loaded, ids(&["backdoor"]));
        assert_eq!(plan.at, 0);
        assert_eq!(plan.displaced, ids(&["crack", "copy"]));
        assert_eq!(
            ch.slot_at(&c, &backdoor, 0).unwrap(),
            "`Backdoor` slotted · `Crack`, `Copy` to inventory"
        );
        assert_eq!(ch.loaded, ids(&["backdoor"]));
    }

    #[test]
    fn a_size_two_program_appended_displaces_the_one_before() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let backdoor = ItemId::new("backdoor");
        ch.give(backdoor.clone(), 1);
        ch.unload(&c, &ItemId::new("copy")).unwrap();
        let plan = ch.plan_slot(&c, &backdoor, 1).unwrap();
        assert_eq!(plan.loaded, ids(&["backdoor"]));
        assert_eq!(plan.at, 0);
        assert_eq!(plan.displaced, ids(&["crack"]));
    }

    #[test]
    fn plan_order_after_then_before() {
        let mut c = Content::load().unwrap();
        let mut deck = c.item(&ItemId::new("pawnshop_deck")).unwrap().clone();
        deck.id = ItemId::new("test_deck");
        deck.deck.as_mut().unwrap().slots = 3;
        c.items.push(deck);
        let mut ch = vex(&c);
        ch.equipment.insert(Slot::Deck, ItemId::new("test_deck"));
        ch.give(ItemId::new("backdoor"), 1);
        ch.loaded = ids(&["crack", "copy", "attack"]);
        let plan = ch.plan_slot(&c, &ItemId::new("backdoor"), 1).unwrap();
        assert_eq!(plan.displaced, ids(&["copy", "attack"]));
        assert_eq!(plan.loaded, ids(&["crack", "backdoor"]));
        assert_eq!(plan.at, 1);
    }

    #[test]
    fn slot_refusals_change_nothing() {
        let mut c = Content::load().unwrap();
        let mut big = c.item(&ItemId::new("backdoor")).unwrap().clone();
        big.id = ItemId::new("big");
        big.program.as_mut().unwrap().size = 3;
        c.items.push(big);
        let mut ch = vex(&c);
        ch.give(ItemId::new("copper_coil"), 1);
        ch.give(ItemId::new("big"), 1);
        let refuse = |ch: &mut Character, id: &str, at: usize, msg: &str| {
            let (loaded, inventory) = (ch.loaded.clone(), ch.inventory.clone());
            assert_eq!(ch.slot_at(&c, &ItemId::new(id), at).unwrap_err(), msg);
            assert_eq!((&ch.loaded, &ch.inventory), (&loaded, &inventory));
        };
        refuse(&mut ch, "copper_coil", 0, "that is not a program");
        refuse(&mut ch, "spoof", 0, "not in your pack");
        refuse(&mut ch, "crack", 0, "already loaded");
        ch.give(ItemId::new("spoof"), 1);
        refuse(&mut ch, "spoof", 3, "no such slot");
        refuse(
            &mut ch,
            "big",
            0,
            "too big for this deck: needs 3 of 2 slots",
        );
        ch.equipment.remove(&Slot::Deck);
        refuse(&mut ch, "spoof", 0, "no deck jacked");
    }

    #[test]
    fn has_ability_reads_every_source() {
        let c = Content::load().unwrap();
        let ch = vex(&c);
        assert!(
            ch.has_ability(&c, &AbilityId::new("signal_ghost")),
            "the netrunner category"
        );
        assert!(ch.has_ability(&c, &AbilityId::new("netscan")), "the deck");
        assert!(!ch.has_ability(&c, &AbilityId::new("ping")));
    }

    #[test]
    fn the_debug_runner_is_vex_maxed_out_under_another_name() {
        let c = Content::load().unwrap();
        let d = Character::debug_runner(&c);
        assert_eq!(d.handle, "D'SHAWN");
        assert!(
            d.inventory
                .iter()
                .any(|s| s.item == ItemId::new("backdoor"))
        );
        assert_eq!((d.level, d.xp, d.talent_points), (50, 0, 0));
        for t in &c.talents {
            assert_eq!(d.talent_rank(&t.id), t.ranks, "{}", t.id);
        }
        assert!(d.flags(&c).contains("deep_end") && d.flags(&c).contains("null_route"));
        assert!(d.has_ability(&c, &AbilityId::new("arc_trace")));
        assert!(d.has_ability(&c, &AbilityId::new("ping")));
        assert_eq!(d.hp, d.hp_max(&c));
        assert_eq!(d.focus, d.focus_max(&c));
        let vex = vex(&c);
        // Vex's kit, plus the Backdoor program, the pawnshop deck set aside and the
        // three pieces of deck hardware and the two weapon pieces
        assert_eq!(d.inventory.len(), vex.inventory.len() + 7, "Vex's kit");
        assert!(d.loaded.starts_with(&vex.loaded));
    }

    #[test]
    fn per_copy_items_never_stack() {
        let c = Content::load().unwrap();
        let mut ch = Character::from_template(&c, &TemplateId::new("kessler")).unwrap();
        let (deck, crack) = (ItemId::new("rebuilt_deck"), ItemId::new("crack"));
        assert_eq!((ch.count(&deck), ch.count(&crack)), (0, 0));
        ch.give_item(&c, deck.clone(), 2);
        let decks: Vec<&Stack> = ch.inventory.iter().filter(|s| s.item == deck).collect();
        assert_eq!(decks.len(), 2);
        for s in &decks {
            assert_eq!(s.qty, 1);
            assert_eq!(s.copy.as_ref().unwrap().fitted, vec![None, None]);
        }
        assert_ne!(decks[0].copy_id(), decks[1].copy_id());
        ch.give_item(&c, crack.clone(), 2);
        let cracks = || ch.inventory.iter().filter(|s| s.item == crack);
        assert_eq!(cracks().count(), 1);
        assert_eq!(cracks().next().unwrap(), &Stack::new(crack.clone(), 2));
        ch.give(crack.clone(), 1);
        assert_eq!(ch.count(&crack), 3);
        assert_eq!(ch.inventory.iter().filter(|s| s.item == crack).count(), 1);
        assert_eq!(ch.count(&deck), 2);
    }

    #[test]
    fn debug_runner_carries_hardware() {
        let c = Content::load().unwrap();
        let d = Character::debug_runner(&c);
        assert_eq!(d.equipped(Slot::Deck), Some(&ItemId::new("rebuilt_deck")));
        let worn = d
            .worn_copy(Slot::Deck)
            .expect("the rebuilt deck is worn as a copy");
        assert_eq!(worn.fitted, vec![None, None]);
        assert!(d.is_worn_copy(worn.id));
        let mut ids = vec![worn.id];
        let mut at = Vec::new();
        for name in ["coldfin_1", "coldfin_2", "sidecar"] {
            let id = ItemId::new(name);
            assert_eq!(d.count(&id), 1, "{name}");
            let i = d.inventory.iter().position(|s| s.item == id).unwrap();
            let copy = d.inventory[i]
                .copy
                .as_ref()
                .expect("hardware is held per copy");
            assert!(copy.fitted.is_empty(), "hardware has no sockets");
            assert_eq!(d.copy_in_pack(copy.id), Some(&d.inventory[i]));
            ids.push(copy.id);
            at.push(i);
        }
        assert!(at.is_sorted(), "packed in order: {at:?}");
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 4, "each its own copy");
    }

    fn fit_direct(ch: &mut Character, item: &str, socket: usize) {
        fit_worn(ch, Slot::Deck, item, socket);
    }

    /// Puts a pack piece straight into a socket of what the slot wears.
    fn fit_worn(ch: &mut Character, slot: Slot, item: &str, socket: usize) {
        let at = ch
            .inventory
            .iter()
            .position(|s| s.item == ItemId::new(item))
            .unwrap();
        let piece = ch.inventory.remove(at);
        ch.worn.get_mut(&slot).unwrap().fitted[socket] = Some(piece);
    }

    #[test]
    fn hardware_in_any_worn_item_counts() {
        let c = Content::load().unwrap();
        let mut vex = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        assert_eq!(
            vex.equipped(Slot::Primary),
            Some(&ItemId::new("ratchet_smg"))
        );
        let gun = vex
            .worn_copy(Slot::Primary)
            .expect("the Ratchet, worn as a copy");
        assert_eq!(gun.fitted, vec![None, None]);
        let (cool, reflex) = (vex.stat(&c, Stat::Cool), vex.stat(&c, Stat::Reflex));
        vex.give_item(&c, ItemId::new("hushcan"), 1);
        vex.give_item(&c, ItemId::new("redeye"), 1);
        assert_eq!(
            vex.stat(&c, Stat::Cool),
            cool,
            "in the pack it does nothing"
        );
        fit_worn(&mut vex, Slot::Primary, "hushcan", 0);
        assert_eq!(vex.stat(&c, Stat::Cool), cool + 1);
        fit_worn(&mut vex, Slot::Primary, "redeye", 1);
        assert_eq!(vex.stat(&c, Stat::Reflex), reflex + 1);
        assert_eq!((vex.trace_cut(&c), vex.deck_power(&c)), (0, 0));
        // The gun off the body: its pieces count for nothing.
        let mut off = vex.clone();
        off.equipment.remove(&Slot::Primary);
        assert_eq!(off.stat(&c, Stat::Cool), cool);
    }

    #[test]
    fn dshawn_carries_the_weapon_pieces_and_stays_capped() {
        let c = Content::load().unwrap();
        let mut d = Character::debug_runner(&c);
        assert_eq!(d.equipped(Slot::Primary), Some(&ItemId::new("ratchet_smg")));
        let gun = d
            .worn_copy(Slot::Primary)
            .expect("the Ratchet, worn as a copy");
        assert_eq!(gun.fitted, vec![None, None]);
        assert_eq!(
            (
                d.count(&ItemId::new("hushcan")),
                d.count(&ItemId::new("redeye"))
            ),
            (1, 1)
        );
        assert_eq!(d.stat(&c, Stat::Cool), 10);
        fit_worn(&mut d, Slot::Primary, "hushcan", 0);
        assert_eq!(d.stat(&c, Stat::Cool), 10, "the clamp holds");
    }

    #[test]
    fn deck_mods_come_from_the_worn_deck() {
        let mut c = Content::load().unwrap();
        let mut ch = Character::debug_runner(&c);
        assert_eq!((ch.trace_cut(&c), ch.deck_power(&c)), (0, 0));
        fit_direct(&mut ch, "coldfin_2", 0);
        assert_eq!(ch.trace_cut(&c), 20);
        fit_direct(&mut ch, "coldfin_1", 1);
        assert_eq!(ch.trace_cut(&c), 30);
        assert_eq!(ch.deck_power(&c), 0);
        // Two coldfin vII.
        let mut two = Character::debug_runner(&c);
        two.give_item(&c, ItemId::new("coldfin_2"), 1);
        fit_direct(&mut two, "coldfin_2", 0);
        fit_direct(&mut two, "coldfin_2", 1);
        assert_eq!(two.trace_cut(&c), 40);
        let mut car = Character::debug_runner(&c);
        fit_direct(&mut car, "sidecar", 0);
        assert_eq!(car.deck_power(&c), 1);
        // The deck off the body: nothing counts; back on, it all does.
        let mut off = ch.clone();
        let deck = off.equipment.remove(&Slot::Deck).unwrap();
        assert_eq!((off.trace_cut(&c), off.deck_power(&c)), (0, 0));
        off.equipment.insert(Slot::Deck, deck);
        assert_eq!(off.trace_cut(&c), 30);
        // Past half it caps.
        let cold = c
            .items
            .iter_mut()
            .find(|i| i.id == ItemId::new("coldfin_2"))
            .unwrap();
        cold.hardware.as_mut().unwrap().mods = vec![Mod::TraceCut(40)];
        assert_eq!(two.trace_cut(&c), 50);
    }
}
