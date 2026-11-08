//! The player's character: attributes, skills, talents, equipment, inventory and quest log.

use crate::action::Host;
use crate::build::Build;
use crate::content::Content;
use crate::model::*;
use std::collections::{BTreeMap, BTreeSet};

/// The SP every level up brings.
pub const SP_PER_LEVEL: u32 = 100;

/// Every level that is a multiple of this brings an attribute point: 3, 6, 9 and on.
pub const LEVELS_PER_ATTRIBUTE_POINT: u8 = 3;

/// What every installed piece of chrome takes off EMP.
pub const EMP_PER_PIECE: i32 = 2;

/// A copy's id, unique for the character that made it.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct CopyId(pub u32);

/// One copy of an item that carries state of its own.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemCopy {
    pub id: CopyId,
    /// One entry per socket; a fitted piece is a stack of one with its own copy.
    pub fitted: Vec<Option<Stack>>,
    /// The stickers on its drawing, oldest first; each newer one goes on top.
    pub stickers: Vec<Stuck>,
}

impl ItemCopy {
    /// What its fitted pieces weigh.
    fn fitted_weight(&self, c: &Content) -> f32 {
        self.fitted.iter().flatten().map(|s| s.weight(c)).sum()
    }

    fn fitted_value(&self, c: &Content) -> u32 {
        self.fitted.iter().flatten().map(|s| s.value(c)).sum()
    }

    /// Empties each socket whose piece `c` lacks and peels each sticker it lacks, all
    /// the way down; a line in `lost` for each. What a piece taken out held that `c`
    /// has goes in `out`.
    pub(crate) fn forget_unknown(
        &mut self,
        c: &Content,
        lost: &mut Vec<String>,
        out: &mut Vec<Stack>,
    ) {
        for socket in &mut self.fitted {
            *socket = socket.take().and_then(|s| s.sift(c, lost, out));
        }
        self.stickers.retain(|s| {
            let known = c.item(&s.sticker).is_some();
            if !known {
                lost.push(sticker_lost(&s.sticker));
            }
            known
        });
    }

    /// The copy of an item `c` lacks, taken apart: each fitted piece `c` has goes in
    /// `out` as it is (what it holds sifted the same way), and each sticker `c` has as
    /// one of it; a line in `lost` for each of the rest.
    pub(crate) fn salvage(self, c: &Content, lost: &mut Vec<String>, out: &mut Vec<Stack>) {
        for s in self.fitted.into_iter().flatten() {
            if let Some(kept) = s.sift(c, lost, out) {
                out.push(kept);
            }
        }
        for stuck in self.stickers {
            if c.item(&stuck.sticker).is_some() {
                out.push(Stack::new(stuck.sticker, 1));
            } else {
                lost.push(sticker_lost(&stuck.sticker));
            }
        }
    }
}

/// What a save's reader says of an item this build's content lacks.
fn item_lost(id: &ItemId) -> String {
    format!("an item ({id})")
}

fn sticker_lost(id: &ItemId) -> String {
    format!("a sticker ({id})")
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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

    /// The stack as `c` keeps it. None when `c` lacks its item: a line in `lost`, and
    /// what its copy held that `c` has goes in `out`. Else the stack, what its copy
    /// holds that `c` lacks dropped.
    pub(crate) fn sift(
        mut self,
        c: &Content,
        lost: &mut Vec<String>,
        out: &mut Vec<Stack>,
    ) -> Option<Stack> {
        if c.item(&self.item).is_none() {
            lost.push(item_lost(&self.item));
            if let Some(k) = self.copy {
                k.salvage(c, lost, out);
            }
            return None;
        }
        if let Some(k) = self.copy.as_mut() {
            k.forget_unknown(c, lost, out);
        }
        Some(self)
    }
}

/// Where an ability comes from.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Source {
    Innate,
    Category(CategoryId),
    Item(ItemId),
    Talent(TalentId),
}

/// A number the character sheet shows: an attribute, a secondary number, or a resistance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Num {
    Attribute(Attribute),
    Hp,
    Focus,
    Armor,
    Sight,
    Carry,
    Move,
    Resist(Harm),
}

impl Num {
    /// In the sheet's reading order: the five attributes, the six secondary numbers, the
    /// six resistances.
    pub const ALL: [Num; 17] = [
        Num::Attribute(Attribute::Body),
        Num::Attribute(Attribute::Reflex),
        Num::Attribute(Attribute::Tech),
        Num::Attribute(Attribute::Cool),
        Num::Attribute(Attribute::Net),
        Num::Hp,
        Num::Focus,
        Num::Armor,
        Num::Sight,
        Num::Carry,
        Num::Move,
        Num::Resist(Harm::Kinetic),
        Num::Resist(Harm::Thermal),
        Num::Resist(Harm::Shock),
        Num::Resist(Harm::Emp),
        Num::Resist(Harm::Toxin),
        Num::Resist(Harm::Feedback),
    ];

    /// The tile's label: three letters, two for HP.
    pub fn label(self) -> &'static str {
        match self {
            Num::Attribute(s) => s.short(),
            Num::Hp => "HP",
            Num::Focus => "FOC",
            Num::Armor => "ARM",
            Num::Sight => "SGT",
            Num::Carry => "CAR",
            Num::Move => "MOV",
            Num::Resist(h) => h.short(),
        }
    }

    /// The full name in caps, as the readout heads with it.
    pub fn name(self) -> &'static str {
        match self {
            Num::Attribute(s) => s.name(),
            Num::Hp => "HP",
            Num::Focus => "FOCUS",
            Num::Armor => "ARMOUR",
            Num::Sight => "SIGHT",
            Num::Carry => "CARRY",
            Num::Move => "MOVE",
            Num::Resist(h) => h.name(),
        }
    }

    /// What a perk adds to this number; nothing when it moves another. Carry counts
    /// in whole kilograms.
    pub fn of_perk(self, p: &Perk) -> i32 {
        match (self, p) {
            (Num::Attribute(s), Perk::Attribute(t, n)) if s == *t => *n as i32,
            (Num::Hp, Perk::HpMax(n)) | (Num::Focus, Perk::Focus(n)) => *n,
            (Num::Armor, Perk::Armor(n))
            | (Num::Sight, Perk::Sight(n))
            | (Num::Move, Perk::Move(n)) => *n as i32,
            (Num::Carry, Perk::Carry(kg)) => kg.round() as i32,
            (Num::Resist(h), Perk::Resist(k, n)) if h == *k => *n as i32,
            _ => 0,
        }
    }
}

/// Where part of a number comes from.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Contrib {
    /// Something worn.
    Gear(ItemId),
    /// A piece of chrome installed, at a site or in a socket.
    Chrome(ItemId),
    /// Hardware fitted in something worn or installed.
    Hardware(ItemId),
    /// A bot docked in installed chrome, with its body: what the sheet calls it.
    Bot(ItemId, BotBody),
    /// Every talent rank held, summed.
    Talents,
    /// The chrome installed, counted: what it costs EMP.
    Pieces(usize),
}

/// Where a program lands among the loaded ones, and what it sends back.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    pub attributes: Attributes,
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
    /// The chrome installed at each site.
    pub chrome: BTreeMap<Site, ItemId>,
    /// The copy of each installed piece held per copy (a limb with sockets).
    pub chrome_copies: BTreeMap<Site, ItemCopy>,
    pub inventory: Vec<Stack>,
    /// The tick each active ability last fired, for cooldowns.
    pub fired: BTreeMap<AbilityId, u64>,
    /// Unspent attribute points: what creation left, and one every
    /// `LEVELS_PER_ATTRIBUTE_POINT` levels; spent from the deck under creation's rules.
    pub attribute_points: u32,
    pub sp: u32,
    /// Unspent talent points: one at creation, one per level.
    pub talent_points: u32,
    /// Programs loaded into the jacked deck, in loading order; each is also in the pack.
    pub loaded: Vec<ItemId>,
    /// The id the next copy takes.
    pub next_copy: u32,
}

/// What a piece of chrome takes of the load, with what is fitted in its copy; what is
/// not chrome (hardware, gear) takes nothing. The one count of a piece's load: the
/// install rule, `load_at` and the sheet's picker all use it.
pub fn piece_load(c: &Content, item: &ItemId, copy: Option<&ItemCopy>) -> u32 {
    let own = |id: &ItemId| {
        c.item(id)
            .and_then(|i| i.implant)
            .map_or(0, |i| u32::from(i.load))
    };
    copy.iter()
        .flat_map(|k| k.fitted.iter().flatten())
        .map(|s| own(&s.item))
        .fold(own(item), |a, b| a + b)
}

/// What one of a stack takes of the load, with what is fitted in it.
pub fn stack_load(c: &Content, s: &Stack) -> u32 {
    piece_load(c, &s.item, s.copy.as_ref())
}

/// What chrome costs past the budget.
pub(crate) fn over_budget(load: u32, max: u32) -> String {
    format!("too much chrome: load {load} of {max}")
}

fn item_name(c: &Content, id: &ItemId) -> String {
    c.item(id)
        .map_or_else(|| id.to_string(), |i| i.name.clone())
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
            attributes: b.attributes.clone(),
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
            chrome: BTreeMap::new(),
            chrome_copies: BTreeMap::new(),
            inventory: Vec::new(),
            fired: BTreeMap::new(),
            attribute_points: b.points_left(c).max(0) as u32,
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
        // The first item that fits a free slot is worn, and chrome goes in at each free
        // site of its kind while the load allows, as a new copy when it is held per
        // copy; everything else is packed. Hardware waits for the rest, then goes in
        // the first kit item with a free socket of its kind.
        let mut hosts = Vec::new();
        let mut hardware = Vec::new();
        for (id, qty) in kit {
            let Some(item) = c.item(&id) else { continue };
            if item.hardware.is_some() {
                hardware.push((id, qty));
                continue;
            }
            let mut qty = qty;
            if let Some(slot) = item.slot
                && qty > 0
                && !ch.equipment.contains_key(&slot)
            {
                let copy = item.per_copy().then(|| ch.new_copy(c, &id));
                ch.wear(slot, id.clone(), copy);
                hosts.push(Host::Worn(slot));
                qty -= 1;
            }
            while qty > 0
                && let Some(site) = ch.free_site(c, item)
            {
                let copy = item.per_copy().then(|| ch.new_copy(c, &id));
                ch.install(site, id.clone(), copy);
                hosts.push(Host::Site(site));
                qty -= 1;
            }
            // Each copy packed is a new one, numbered on from here.
            let first = ch.next_copy;
            ch.give_item(c, id, qty);
            hosts.extend((first..ch.next_copy).map(|n| Host::Pack(CopyId(n))));
        }
        for (id, qty) in hardware {
            let mut qty = qty;
            while qty > 0
                && let Some((host, socket)) = ch.free_socket(c, &hosts, &id)
            {
                let piece = Stack {
                    item: id.clone(),
                    qty: 1,
                    copy: Some(ch.new_copy(c, &id)),
                };
                if let Some(copy) = ch.host_copy_mut(c, host) {
                    copy.fitted[socket] = Some(piece);
                }
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
    /// sockets empty, the deck hardware, the weapon pieces and one of every piece
    /// of chrome it hasn't installed in the pack, and every program that fits
    /// loaded. The chrome leaves it overburdened. For the title's NEW GAME (DEBUG).
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
        // Enough of the Ratchet to build it at the bench, short of the barrel.
        d.skills.insert(SkillId::new("scrapcraft"));
        for (id, n) in [
            ("ratchet_receiver", 1),
            ("ratchet_stock", 1),
            ("ratchet_trigger", 1),
            ("ratchet_mag", 2),
        ] {
            d.give_item(c, ItemId::new(id), n);
        }
        // The pawnshop deck goes back to the pack; the rebuilt one takes its slot.
        let deck = ItemId::new("rebuilt_deck");
        let copy = d.new_copy(c, &deck);
        if let Some(old) = d.wear(Slot::Deck, deck, Some(copy)) {
            d.stow(old);
        }
        for id in ["coldfin_1", "coldfin_2", "sidecar", "hushcan", "redeye"] {
            d.give_item(c, ItemId::new(id), 1);
        }
        // Two of every sticker, so sticking and peeling can be tried twice.
        for i in c.items.iter().filter(|i| i.kind == ItemKind::Sticker) {
            d.give(i.id.clone(), 2);
        }
        // The Drone cradle goes in with the first bot that docks in each of its sockets,
        // so a debug start has a drone out; every other bot goes in the pack.
        let cradle = ItemId::new("drone_cradle");
        let mut copy = d.new_copy(c, &cradle);
        let mut docked = Vec::new();
        let kinds = c.item(&cradle).map_or(Vec::new(), |i| i.sockets.clone());
        for (n, kind) in kinds.into_iter().enumerate() {
            let bot = c
                .items
                .iter()
                .find(|i| i.bot.as_ref().is_some_and(|b| b.docks == kind));
            if let Some(bot) = bot {
                let item = bot.id.clone();
                let piece = d.new_copy(c, &item);
                copy.fitted[n] = Some(Stack {
                    item: item.clone(),
                    qty: 1,
                    copy: Some(piece),
                });
                docked.push(item);
            }
        }
        d.install(Site::Back, cradle, Some(copy));
        for i in c.items.iter().filter(|i| i.kind == ItemKind::Bot) {
            if !docked.contains(&i.id) {
                d.give_item(c, i.id.clone(), 1);
            }
        }
        // One of every piece of chrome not already installed, to try on at ATTRIBUTES.
        for i in c.items.iter().filter(|i| i.kind == ItemKind::Cyberware) {
            if !d.chrome.values().any(|id| *id == i.id) {
                d.give_item(c, i.id.clone(), 1);
            }
        }
        d.load_all(c);
        d
    }

    /// The debug runner dressed for the street fight: a neural port installed
    /// (Static), the handguns skill, three more stim patches. For NEW GAME (COMBAT).
    pub fn combat_runner(c: &Content) -> Character {
        let mut d = Self::debug_runner(c);
        d.install_new(c, Site::Neural, ItemId::new("neural_port"));
        d.skills.insert(SkillId::new("handguns"));
        d.give(ItemId::new("stim_patch"), 3);
        // The other SMG, in the pack, to swap in and see its look.
        d.give_item(c, ItemId::new("whisper_smg"), 1);
        // The throwables, and the adrenal pump and target lens installed; the optics
        // the lens replaces go to the pack.
        d.give(ItemId::new("frag"), 3);
        d.give(ItemId::new("flashbang"), 2);
        d.give(ItemId::new("emp_puck"), 2);
        d.give(ItemId::new("satchel"), 1);
        d.give(ItemId::new("choke_can"), 2);
        for (id, site) in [("adrenal_pump", Site::Organs), ("target_lens", Site::Eyes)] {
            d.install_new(c, site, ItemId::new(id));
        }
        d
    }

    /// Installs `item` at `site`: the one in the pack when there is one, so no spare
    /// is left there, else a new one, with a copy of its own when it is held per
    /// copy. What the site held goes to the pack.
    fn install_new(&mut self, c: &Content, site: Site, item: ItemId) {
        let stack = crate::action::take_from_pack(self, &item, 1).pop();
        let copy = match stack {
            Some(s) => s.copy,
            None => c
                .item(&item)
                .is_some_and(Item::per_copy)
                .then(|| self.new_copy(c, &item)),
        };
        if let Some(old) = self.install(site, item, copy) {
            self.stow_item(c, old);
        }
    }

    /// The first free site a piece of chrome fits, if its load fits the budget.
    fn free_site(&self, c: &Content, item: &Item) -> Option<Site> {
        let implant = item.implant?;
        if self.chrome_load(c) + implant.load as u32 > self.load_max() {
            return None;
        }
        Site::ALL
            .into_iter()
            .find(|s| s.fits() == implant.fits && !self.chrome.contains_key(s))
    }

    /// The first of `hosts` with an empty socket a piece fits, and that socket.
    fn free_socket(&self, c: &Content, hosts: &[Host], piece: &ItemId) -> Option<(Host, usize)> {
        let kind = c.item(piece)?.fits_socket()?;
        hosts.iter().find_map(|&host| {
            let sockets = &c.item(self.host_item(host)?)?.sockets;
            let copy = self.host_copy(host)?;
            let mut free = sockets.iter().zip(&copy.fitted);
            let socket = free.position(|(k, s)| *k == kind && s.is_none())?;
            Some((host, socket))
        })
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

    /// Perks of everything equipped, in slot order, then of installed chrome (see
    /// `chrome_ids`), then those of every talent rank.
    pub fn perks<'a>(&'a self, c: &'a Content) -> impl Iterator<Item = &'a Perk> + 'a {
        Slot::ALL
            .iter()
            .filter_map(move |s| self.equipment.get(s))
            .chain(self.chrome_ids())
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
    /// gained. Each level brings a talent point and `SP_PER_LEVEL` SP, and each one
    /// that is a multiple of `LEVELS_PER_ATTRIBUTE_POINT` an attribute point too,
    /// counted for every level crossed at once. Level 255 never gains another; the
    /// points still add.
    pub fn gain_xp(&mut self, points: u32) -> u8 {
        self.xp = self.xp.saturating_add(points);
        let from = self.level;
        let mut gained = 0u8;
        while self.level < u8::MAX && self.xp >= Self::xp_to_next(self.level) {
            self.xp -= Self::xp_to_next(self.level);
            self.level += 1;
            gained += 1;
        }
        self.talent_points += gained as u32;
        self.sp = self.sp.saturating_add(SP_PER_LEVEL * gained as u32);
        self.attribute_points += Self::attribute_points_for(from, self.level);
        gained
    }

    /// The attribute points levelling from `from` to `to` brings: one for each level
    /// past `from`, up to `to`, that is a multiple of `LEVELS_PER_ATTRIBUTE_POINT`.
    pub fn attribute_points_for(from: u8, to: u8) -> u32 {
        let n = LEVELS_PER_ATTRIBUTE_POINT;
        u32::from((to / n).saturating_sub(from / n))
    }

    pub fn attribute(&self, c: &Content, attribute: Attribute) -> u8 {
        let perks: i32 = self
            .perks(c)
            .filter_map(|p| match p {
                Perk::Attribute(s, n) if *s == attribute => Some(*n as i32),
                _ => None,
            })
            .sum();
        let fitted: i32 = self
            .worn_mods(c)
            .iter()
            .filter_map(|m| match m {
                Mod::Attribute(s, n) if *s == attribute => Some(*n as i32),
                _ => None,
            })
            .sum();
        let bonus = perks + fitted;
        (self.attributes.get(attribute) as i32 + bonus).clamp(0, 10) as u8
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

    /// Armour from equipment.
    pub fn armor(&self, c: &Content) -> i32 {
        self.perks(c)
            .filter_map(|p| match p {
                Perk::Armor(n) => Some(*n as i32),
                _ => None,
            })
            .sum()
    }

    /// Extra move from equipment.
    pub fn move_bonus(&self, c: &Content) -> i32 {
        self.perks(c)
            .filter_map(|p| match p {
                Perk::Move(n) => Some(*n as i32),
                _ => None,
            })
            .sum()
    }

    /// Extra throw range from equipment.
    pub fn throw_bonus(&self, c: &Content) -> i32 {
        self.perks(c)
            .filter_map(|p| match p {
                Perk::Throw(n) => Some(*n as i32),
                _ => None,
            })
            .sum()
    }

    /// Percent of a kind of harm turned aside; EMP takes 2 for every installed piece.
    pub fn resist(&self, c: &Content, h: Harm) -> i32 {
        let perks: i32 = self
            .perks(c)
            .filter_map(|p| match p {
                Perk::Resist(k, n) if *k == h => Some(*n as i32),
                _ => None,
            })
            .sum();
        let chrome = if h == Harm::Emp {
            EMP_PER_PIECE * self.pieces(c) as i32
        } else {
            0
        };
        perks - chrome
    }

    /// Every non-zero part of a number that is not its own: what is worn in slot
    /// order, the chrome in `installed` order, the hardware fitted in what is worn
    /// then in what is installed, the talents summed into one, and for EMP the pieces
    /// installed at 2 each.
    pub fn sources(&self, c: &Content, n: Num) -> Vec<(Contrib, i32)> {
        let adds = |id: &ItemId| -> i32 {
            c.item(id)
                .map_or(0, |i| i.perks.iter().map(|p| n.of_perk(p)).sum())
        };
        let mut out = Vec::new();
        for id in Slot::ALL.iter().filter_map(|s| self.equipment.get(s)) {
            out.push((Contrib::Gear(id.clone()), adds(id)));
        }
        for (_, _, id) in self.installed() {
            let v = adds(&id);
            if let Some(body) = c.item(&id).and_then(|i| i.bot.as_ref()).map(|b| b.body) {
                out.push((Contrib::Bot(id, body), v));
            } else {
                out.push((Contrib::Chrome(id), v));
            }
        }
        if let Num::Attribute(attribute) = n {
            let worn = Slot::ALL.iter().filter_map(|slot| self.worn_copy(*slot));
            let installed = Site::ALL.iter().filter_map(|site| self.chrome_copy(*site));
            for s in worn
                .chain(installed)
                .flat_map(|k| k.fitted.iter().flatten())
            {
                let v: i32 = c
                    .item(&s.item)
                    .and_then(|i| i.hardware.as_ref())
                    .map_or(0, |h| {
                        h.mods
                            .iter()
                            .map(|m| match m {
                                Mod::Attribute(t, v) if *t == attribute => *v as i32,
                                _ => 0,
                            })
                            .sum()
                    });
                out.push((Contrib::Hardware(s.item.clone()), v));
            }
        }
        let talents: i32 = self
            .talent_effects(c)
            .map(|b| match b {
                Boon::Perk(p) => n.of_perk(p),
                _ => 0,
            })
            .sum();
        out.push((Contrib::Talents, talents));
        if n == Num::Resist(Harm::Emp) {
            let pieces = self.pieces(c);
            out.push((Contrib::Pieces(pieces), -EMP_PER_PIECE * pieces as i32));
        }
        out.retain(|(_, v)| *v != 0);
        out
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
        8.0 + 2.0 * self.attributes.body as f32 + bonus
    }

    pub fn overburdened(&self, c: &Content) -> bool {
        self.weight(c) > self.carry_cap(c)
    }

    /// Innate abilities first, then those granted by owned categories in content order,
    /// then those granted by equipment in slot order, then by installed chrome (see
    /// `chrome_ids`), then by talents; deduplicated, the first source winning.
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
        let worn = Slot::ALL.iter().filter_map(|s| self.equipment.get(s));
        for item in worn.chain(self.chrome_ids()).filter_map(|id| c.item(id)) {
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
        // Reach: a scan's, ping's or trace's radius, a strike's or mark's range.
        if let AbilityKind::Active(
            Active::Scan { radius: reach }
            | Active::Ping { radius: reach }
            | Active::Trace { radius: reach }
            | Active::Strike { range: reach, .. }
            | Active::Mark { range: reach, .. },
        ) = &mut a.kind
        {
            *reach = (*reach + dr).max(1);
        }
        a.cooldown = (a.cooldown as i64 + dc).max(0) as u64;
        Some(a)
    }

    /// The rank held in a talent; zero when unlearned.
    pub fn talent_rank(&self, id: &TalentId) -> u8 {
        self.talents.get(id).copied().unwrap_or(0)
    }

    /// Ranks held across one tree.
    pub fn tree_points(&self, c: &Content, attribute: Attribute) -> u32 {
        c.tree(attribute)
            .map(|t| {
                t.talents()
                    .map(|(_, _, id)| self.talent_rank(id) as u32)
                    .sum()
            })
            .unwrap_or(0)
    }

    /// Whether a tier (from zero) of a tree has opened: `tier_gate` points per tier
    /// below it.
    pub fn tier_open(&self, c: &Content, attribute: Attribute, tier: usize) -> bool {
        self.tree_points(c, attribute) >= c.creation.tier_gate * tier as u32
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
        let have = self.tree_points(c, tree.attribute);
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

    /// Drops every item, piece of chrome, talent, skill, ability, category and sticker
    /// `c` lacks, as a run saved by a build that had them comes back in one that
    /// doesn't. A worn item, an installed piece or a fitted one goes with its copy, and
    /// what that copy held that `c` has (its fitted pieces, its stickers) comes out
    /// into the pack. A line for each thing gone, naming its id.
    pub fn forget_unknown(&mut self, c: &Content) -> Vec<String> {
        let mut lost = Vec::new();
        // What comes out of the pieces taken away, for the pack.
        let mut out = Vec::new();
        self.inventory = std::mem::take(&mut self.inventory)
            .into_iter()
            .filter_map(|s| s.sift(c, &mut lost, &mut out))
            .collect();
        let worn: Vec<Slot> = self
            .equipment
            .iter()
            .filter(|(_, id)| c.item(id).is_none())
            .map(|(slot, _)| *slot)
            .collect();
        for slot in worn {
            if let Some(id) = self.equipment.remove(&slot) {
                lost.push(item_lost(&id));
            }
            if let Some(k) = self.worn.remove(&slot) {
                k.salvage(c, &mut lost, &mut out);
            }
        }
        for k in self.worn.values_mut() {
            k.forget_unknown(c, &mut lost, &mut out);
        }
        let installed: Vec<Site> = self
            .chrome
            .iter()
            .filter(|(_, id)| c.item(id).is_none())
            .map(|(site, _)| *site)
            .collect();
        for site in installed {
            if let Some(id) = self.chrome.remove(&site) {
                lost.push(format!("chrome ({id})"));
            }
            if let Some(k) = self.chrome_copies.remove(&site) {
                k.salvage(c, &mut lost, &mut out);
            }
        }
        for k in self.chrome_copies.values_mut() {
            k.forget_unknown(c, &mut lost, &mut out);
        }
        for s in out {
            self.stow(s);
        }
        self.abilities.retain(|a| {
            let known = c.ability(a).is_some();
            if !known {
                lost.push(format!("an ability ({a})"));
            }
            known
        });
        // A cooldown on an ability the content lacks times nothing.
        self.fired.retain(|a, _| c.ability(a).is_some());
        self.categories.retain(|k| {
            let known = c.category(k).is_some();
            if !known {
                lost.push(format!("a category ({k})"));
            }
            known
        });
        self.talents.retain(|t, _| {
            let known = c.talent(t).is_some();
            if !known {
                lost.push(format!("a talent ({t})"));
            }
            known
        });
        self.skills.retain(|s| {
            let known = c.skill(s).is_some();
            if !known {
                lost.push(format!("a skill ({s})"));
            }
            known
        });
        self.loaded.retain(|id| {
            let known = c.item(id).is_some();
            if !known {
                lost.push(item_lost(id));
            }
            known
        });
        lost
    }

    /// A new copy of an item, every socket empty, with the stickers it is made with.
    pub fn new_copy(&mut self, c: &Content, item: &ItemId) -> ItemCopy {
        let id = CopyId(self.next_copy);
        self.next_copy += 1;
        let item = c.item(item);
        ItemCopy {
            id,
            fitted: vec![None; item.map_or(0, |i| i.sockets.len())],
            stickers: item.map(|i| i.stickers.clone()).unwrap_or_default(),
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

    /// Installs a piece at a site, with its copy when it has one; returns what the
    /// site held, with that one's copy.
    pub fn install(&mut self, site: Site, item: ItemId, copy: Option<ItemCopy>) -> Option<Stack> {
        let old = self.chrome.insert(site, item);
        let old_copy = match copy {
            Some(k) => self.chrome_copies.insert(site, k),
            None => self.chrome_copies.remove(&site),
        };
        old.map(|item| Stack {
            item,
            qty: 1,
            copy: old_copy,
        })
    }

    pub fn chrome_at(&self, site: Site) -> Option<&ItemId> {
        self.chrome.get(&site)
    }

    /// The copy a site's chrome is held as. It counts only while the site still
    /// holds something: `chrome` is the truth, and `chrome_copies` can be left behind.
    pub fn chrome_copy(&self, site: Site) -> Option<&ItemCopy> {
        self.chrome.get(&site)?;
        self.chrome_copies.get(&site)
    }

    /// Installed chrome in site order, then what is fitted in it, sites and sockets
    /// in order.
    fn chrome_ids(&self) -> impl Iterator<Item = &ItemId> {
        let fitted = Site::ALL
            .iter()
            .filter_map(|s| self.chrome_copy(*s))
            .flat_map(|k| k.fitted.iter().flatten())
            .map(|s| &s.item);
        self.chrome.values().chain(fitted)
    }

    /// Every installed piece: each site's own (`None`), then what its sockets hold
    /// (`Some(socket)`), sites and sockets in order.
    pub fn installed(&self) -> Vec<(Site, Option<usize>, ItemId)> {
        let mut out = Vec::new();
        for (&site, id) in &self.chrome {
            out.push((site, None, id.clone()));
            let Some(copy) = self.chrome_copy(site) else {
                continue;
            };
            for (n, s) in copy.fitted.iter().enumerate() {
                if let Some(s) = s {
                    out.push((site, Some(n), s.item.clone()));
                }
            }
        }
        out
    }

    /// Installed pieces, sockets included: chrome, what EMP counts. A docked bot is
    /// not in the body.
    pub fn pieces(&self, c: &Content) -> usize {
        self.installed()
            .iter()
            .filter(|(_, _, id)| c.item(id).is_none_or(|i| i.bot.is_none()))
            .count()
    }

    /// Every bot docked in installed chrome, sites and sockets in order.
    pub fn docked(&self, c: &Content) -> Vec<(Site, usize, ItemId)> {
        self.installed()
            .into_iter()
            .filter_map(|(site, socket, id)| {
                let socket = socket?;
                c.item(&id)?.bot.as_ref()?;
                Some((site, socket, id))
            })
            .collect()
    }

    /// What the installed pieces take of the load.
    pub fn chrome_load(&self, c: &Content) -> u32 {
        self.installed()
            .iter()
            .filter_map(|(_, _, id)| c.item(id)?.implant)
            .map(|i| i.load as u32)
            .sum()
    }

    /// Whether a piece fits `at`: a site (`None`), or a socket of the piece installed
    /// there (`Some`). Says nothing of what the pack holds or the budget.
    pub fn fits_at(&self, c: &Content, item: &ItemId, at: (Site, Option<usize>)) -> bool {
        let Some(item) = c.item(item) else {
            return false;
        };
        match at {
            (site, None) => item.implant.is_some_and(|i| i.fits == site.fits()),
            (site, Some(socket)) => {
                let kind = self
                    .chrome_at(site)
                    .and_then(|id| c.item(id))
                    .and_then(|i| i.sockets.get(socket));
                // Only chrome and bots go in the body's sockets, never hardware.
                let body = item.implant.is_some() || item.bot.is_some();
                body && kind.is_some_and(|&k| item.fits_socket() == Some(k))
            }
        }
    }

    /// Every refusal `Action::Install` makes, before it acts.
    pub fn can_install(&self, c: &Content, item: &ItemId, site: Site) -> Result<(), String> {
        let stack = self
            .inventory
            .iter()
            .find(|s| &s.item == item && s.qty > 0)
            .ok_or_else(|| format!("`{}` is not in your pack", item_name(c, item)))?;
        self.can_install_stack(c, stack, site)
    }

    /// Every refusal installing this pack stack at `site` makes: a limb comes with
    /// what is fitted in it, and that takes load too. What the site holds is swapped
    /// out to the pack, so its load, with what is fitted in it, leaves first.
    pub fn can_install_stack(&self, c: &Content, stack: &Stack, site: Site) -> Result<(), String> {
        let item = c
            .item(&stack.item)
            .ok_or_else(|| format!("unknown item {}", stack.item))?;
        let implant = item
            .implant
            .ok_or_else(|| format!("`{}` is not chrome", item.name))?;
        if implant.fits != site.fits() {
            return Err(format!("`{}` doesn't fit {}", item.name, site.label()));
        }
        let out = self.load_at(c, (site, None));
        let after = self.chrome_load(c).saturating_sub(out) + stack_load(c, stack);
        if after > self.load_max() {
            return Err(over_budget(after, self.load_max()));
        }
        Ok(())
    }

    /// What the piece at a site (`None`), or in a socket of it (`Some`), takes of the
    /// load, with what is fitted in it; an empty place takes nothing.
    pub fn load_at(&self, c: &Content, (site, socket): (Site, Option<usize>)) -> u32 {
        match socket {
            None => self
                .chrome_at(site)
                .map_or(0, |id| piece_load(c, id, self.chrome_copy(site))),
            Some(n) => self
                .chrome_copy(site)
                .and_then(|k| k.fitted.get(n)?.as_ref())
                .map_or(0, |s| stack_load(c, s)),
        }
    }

    /// The chrome a body takes: from base BODY and COOL, so chrome that raises them
    /// never raises its own budget.
    pub fn load_max(&self) -> u32 {
        2 * (self.attributes.body as u32 + self.attributes.cool as u32) + 6
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

    /// The mods of the hardware fitted in anything worn or installed: slots in
    /// order, then sites in order, sockets in order. Hardware in the pack, or in an
    /// item taken off, counts for nothing.
    pub fn worn_mods(&self, c: &Content) -> Vec<Mod> {
        let worn = Slot::ALL.iter().filter_map(|slot| self.worn_copy(*slot));
        let installed = Site::ALL.iter().filter_map(|site| self.chrome_copy(*site));
        worn.chain(installed)
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

    /// The item a host is: what the slot wears, the chrome the site holds, or the
    /// copy's item in the pack.
    pub fn host_item(&self, host: Host) -> Option<&ItemId> {
        match host {
            Host::Worn(slot) => self.equipped(slot),
            Host::Site(site) => self.chrome_at(site),
            Host::Pack(id) => self.copy_in_pack(id).map(|s| &s.item),
        }
    }

    /// The host's copy, when it has one.
    pub fn host_copy(&self, host: Host) -> Option<&ItemCopy> {
        match host {
            Host::Worn(slot) => self.worn_copy(slot),
            Host::Site(site) => self.chrome_copy(site),
            Host::Pack(id) => self.copy_in_pack(id)?.copy.as_ref(),
        }
    }

    /// The host's copy to change. A per-copy item worn or installed without one
    /// gets it now.
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
            Host::Site(site) => {
                let item = self.chrome.get(&site)?.clone();
                if !c.item(&item).is_some_and(Item::per_copy) {
                    return None;
                }
                if !self.chrome_copies.contains_key(&site) {
                    let copy = self.new_copy(c, &item);
                    self.chrome_copies.insert(site, copy);
                }
                self.chrome_copies.get_mut(&site)
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
    /// Installed chrome is left out: it is part of you, and LOAD is its cost.
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

    /// What it all is worth: the pack, what is worn and the chrome in you.
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
        let installed: u32 = self
            .chrome
            .iter()
            .map(|(site, id)| {
                let own = c.item(id).map_or(0, |i| i.value);
                own + self
                    .chrome_copies
                    .get(site)
                    .map_or(0, |k| k.fitted_value(c))
            })
            .sum();
        carried + worn + installed
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
                ch.attributes.body,
                ch.attributes.reflex,
                ch.attributes.tech,
                ch.attributes.cool,
                ch.attributes.net
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
            ch.chrome_at(Site::Eyes),
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
        assert_eq!((ch.attribute_points, ch.sp), (0, 0));
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
        assert_eq!((ch.attribute_points, ch.sp), (0, 0));
        let mut spare = Build::blank(&c);
        spare.handle = "Nix".into();
        spare.origin = Some(OriginId::new("vat_grown"));
        spare.profession = Some(ProfessionId::new("techie"));
        let ch = Character::from_build(&c, &spare);
        assert_eq!(
            (ch.attribute_points, ch.sp),
            (20, 600),
            "unspent pools ride along"
        );
        assert_eq!(ch.credits, 300);
        assert_eq!(ch.handle, "Nix");
    }

    /// What each socket of a copy holds, by item id.
    fn held(copy: &ItemCopy) -> Vec<Option<&str>> {
        let held = copy.fitted.iter().map(|s| s.as_ref());
        held.map(|s| s.map(|s| s.item.0.as_str())).collect()
    }

    #[test]
    fn kit_fits_hardware_into_a_socket() {
        let c = Content::load().unwrap();
        // The Ratchet comes first in Kessler's kit: the first Hushcan goes in its
        // muzzle and the Redeye on its rail; the second Hushcan finds the muzzle full.
        let mut b = Build::from_template(&c, c.template(&TemplateId::new("kessler")).unwrap());
        let (can, sight) = (ItemId::new("hushcan"), ItemId::new("redeye"));
        b.kit.extend([(can.clone(), 2), (sight.clone(), 1)]);
        let ch = Character::from_build(&c, &b);
        let gun = ch.worn_copy(Slot::Primary).unwrap();
        assert_eq!(held(gun), [Some("hushcan"), Some("redeye")]);
        assert!(
            gun.fitted
                .iter()
                .flatten()
                .all(|s| s.qty == 1 && s.copy.is_some()),
            "a fitted piece is a copy of its own"
        );
        assert_eq!((ch.count(&can), ch.count(&sight)), (1, 0));
        let packed = ch.inventory.iter().find(|s| s.item == can).unwrap();
        assert!(packed.copy.is_some(), "a packed piece is a copy");
        // A kit with nothing that takes it packs every piece.
        let mut bare = Build::blank(&c);
        bare.kit = vec![(can.clone(), 2), (ItemId::new("salvage_jacket"), 1)];
        let ch = Character::from_build(&c, &bare);
        assert_eq!(ch.count(&can), 2);
        assert_eq!(
            ch.equipped(Slot::Torso),
            Some(&ItemId::new("salvage_jacket"))
        );
    }

    #[test]
    fn every_build_starts_with_a_fused_keyjack() {
        let c = Content::load().unwrap();
        let (keyjack, fuse) = (ItemId::new("keyjack"), ItemId::new("fuse_stock"));
        let mut builds: Vec<Build> = c
            .professions
            .iter()
            .map(|p| {
                let mut b = Build::blank(&c);
                b.profession = Some(p.id.clone());
                b
            })
            .collect();
        builds.extend(c.templates.iter().map(|t| Build::from_template(&c, t)));
        assert!(builds.len() >= 6);
        for b in &builds {
            let ch = Character::from_build(&c, b);
            let who = b.profession.as_ref().unwrap();
            let jacks: Vec<&Stack> = ch.inventory.iter().filter(|s| s.item == keyjack).collect();
            assert_eq!(jacks.len(), 1, "{who}");
            let copy = jacks[0].copy.as_ref().expect("a copy");
            assert_eq!(held(copy), [Some("fuse_stock")], "{who}");
            assert_eq!(ch.count(&fuse), 2, "{who}: two spares");
            assert!(
                ch.inventory
                    .iter()
                    .filter(|s| s.item == fuse)
                    .all(|s| s.copy.is_some()),
                "{who}"
            );
        }
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
        assert_eq!(ch.attribute(&c, Attribute::Net), 7, "deck +1 net");
        assert_eq!(ch.sight_bonus(&c), 4);
        ch.chrome.remove(&Site::Eyes);
        assert_eq!(ch.sight_bonus(&c), 0);
        ch.equipment.remove(&Slot::Deck);
        assert_eq!(ch.attribute(&c, Attribute::Net), 6);
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
        let edge = c.tree(Attribute::Reflex).unwrap();
        let first = edge.slot(0, 0).unwrap().clone();
        let second = edge.slot(1, 0).unwrap().clone();
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
        assert_eq!(ch.tree_points(&c, Attribute::Reflex), ranks as u32);
        let other = edge.slot(0, 1).unwrap().clone();
        while ch.tree_points(&c, Attribute::Reflex) < 3 {
            assert!(!ch.tier_open(&c, Attribute::Reflex, 1));
            ch.learn(&c, &other).unwrap();
        }
        assert!(ch.tier_open(&c, Attribute::Reflex, 1));
        // A prerequisite must be maxed before its dependant opens.
        let needy = c.talents.iter().find(|t| t.requires.is_some()).unwrap();
        let (tree, tier, _) = c.tree_of(&needy.id).unwrap();
        let attribute = tree.attribute;
        let req = c.talent(needy.requires.as_ref().unwrap()).unwrap();
        let mut ch2 = vex(&c);
        ch2.talent_points = 30;
        for lower in 0..tier {
            for id in c.tree(attribute).unwrap().tiers[lower].iter().flatten() {
                if id == &req.id {
                    continue;
                }
                while ch2.can_learn(&c, id).is_ok() {
                    ch2.learn(&c, id).unwrap();
                }
            }
        }
        while !ch2.tier_open(&c, attribute, tier) {
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
    fn a_sharpened_strike_reaches_further() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let range =
            |ch: &Character, id: &str| match ch.ability(&c, &AbilityId::new(id)).unwrap().kind {
                AbilityKind::Active(Active::Strike { range, .. } | Active::Mark { range, .. }) => {
                    range
                }
                _ => panic!("{id} is no strike"),
            };
        assert_eq!(range(&ch, "jolt"), 3);
        ch.talents.insert(TalentId::new("arc_lash"), 1);
        assert_eq!(range(&ch, "jolt"), 4);
        ch.talents.insert(TalentId::new("static_bloom"), 2);
        assert_eq!(range(&ch, "static"), 10);
        let stat = ch.ability(&c, &AbilityId::new("static")).unwrap();
        assert_eq!(stat.cooldown, 180);
        // A mark reaches further the same way.
        let mut marked = c.clone();
        let t = marked
            .talents
            .iter_mut()
            .find(|t| t.id.0 == "cold_read")
            .unwrap();
        t.effects[1] = vec![Boon::Sharpen {
            ability: AbilityId::new("deadeye"),
            radius: 3,
            cooldown: 0,
        }];
        let mut ch = vex(&marked);
        ch.talents.insert(TalentId::new("cold_read"), 2);
        let deadeye = ch.ability(&marked, &AbilityId::new("deadeye")).unwrap();
        assert!(matches!(
            deadeye.kind,
            AbilityKind::Active(Active::Mark { range: 15, .. })
        ));
    }

    #[test]
    fn a_sideways_prerequisite_and_an_old_rank_follow_the_rules() {
        let c = Content::load().unwrap();
        let id = TalentId::new;
        let mut ch = vex(&c);
        ch.talent_points = 20;
        for (t, r) in [("quick_draw", 3), ("double_tap", 2), ("light_step", 2)] {
            ch.talents.insert(id(t), r);
        }
        // Counter cut hangs off Backstab, one column over.
        assert_eq!(
            ch.can_learn(&c, &id("counter_cut")).unwrap_err(),
            "needs Backstab 2/2"
        );
        ch.learn(&c, &id("backstab")).unwrap();
        ch.learn(&c, &id("backstab")).unwrap();
        assert!(ch.can_learn(&c, &id("counter_cut")).is_ok());
        // A rank from before the trees moved still counts, and still opens what it opens.
        let mut old = vex(&c);
        old.talent_points = 5;
        for (t, r) in [
            ("quick_draw", 3),
            ("double_tap", 2),
            ("light_step", 2),
            ("bullet_time", 2),
        ] {
            old.talents.insert(id(t), r);
        }
        assert_eq!(old.tree_points(&c, Attribute::Reflex), 9);
        assert_eq!(old.talent_rank(&id("long_shot")), 0);
        assert_eq!(
            old.can_learn(&c, &id("half_second")).unwrap_err(),
            "3 more points in EDGE"
        );
        old.talents.insert(id("hair_trigger"), 2);
        old.talents.insert(id("steady_hand"), 1);
        assert!(old.can_learn(&c, &id("half_second")).is_ok());
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
        let attribute_t = with(|e| matches!(e, Boon::Perk(Perk::Attribute(..))));
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
        let first_attribute = c
            .talent(&attribute_t)
            .unwrap()
            .effects
            .iter()
            .flatten()
            .find_map(|e| match e {
                Boon::Perk(Perk::Attribute(s, n)) => Some((*s, *n)),
                _ => None,
            })
            .unwrap();
        let before = ch.attribute(&c, first_attribute.0);
        force(&mut ch, &attribute_t);
        assert!(
            ch.attribute(&c, first_attribute.0) as i32 >= before as i32 + first_attribute.1 as i32
        );
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
            AbilityKind::Active(Active::Strike { range, .. } | Active::Mark { range, .. }) => range,
            AbilityKind::Active(Active::Boost { .. }) | AbilityKind::Passive => 0,
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
        ch.chrome.insert(Site::Neural, port.clone());
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
    fn a_level_brings_a_talent_point_and_sp() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        ch.talent_points = 0;
        ch.sp = 0;
        assert_eq!(ch.gain_xp(Character::xp_to_next(1)), 1);
        assert_eq!((ch.talent_points, ch.sp), (1, 100));
        assert_eq!(
            ch.gain_xp(Character::xp_to_next(2) + Character::xp_to_next(3)),
            2
        );
        assert_eq!((ch.talent_points, ch.sp), (3, 300));
    }

    #[test]
    fn every_third_level_brings_an_attribute_point() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        ch.attribute_points = 0;
        assert_eq!(LEVELS_PER_ATTRIBUTE_POINT, 3);
        // From level 1: 2 brings nothing, 3 a point.
        assert_eq!(ch.gain_xp(Character::xp_to_next(1)), 1);
        assert_eq!((ch.level, ch.attribute_points), (2, 0));
        assert_eq!(ch.gain_xp(Character::xp_to_next(2)), 1);
        assert_eq!((ch.level, ch.attribute_points), (3, 1));
        // From 2 to 7 at once: 3 and 6 bring one each.
        let mut ch = vex(&c);
        ch.attribute_points = 0;
        ch.level = 2;
        let xp: u32 = (2..7).map(Character::xp_to_next).sum();
        assert_eq!(ch.gain_xp(xp), 5);
        assert_eq!((ch.level, ch.attribute_points), (7, 2));
        assert_eq!(Character::attribute_points_for(2, 7), 2);
        assert_eq!(Character::attribute_points_for(3, 5), 0);
        assert_eq!(Character::attribute_points_for(5, 6), 1);
        // 254 to 255 brings one; 255 never levels, and brings nothing more.
        ch.level = 254;
        ch.xp = 0;
        assert_eq!(ch.gain_xp(Character::xp_to_next(254)), 1);
        assert_eq!((ch.level, ch.attribute_points), (u8::MAX, 3));
        assert_eq!(ch.gain_xp(u32::MAX), 0);
        assert_eq!((ch.level, ch.attribute_points), (u8::MAX, 3));
    }

    #[test]
    fn new_perks_sum() {
        let text = include_str!("../../../flavor/content.ron").replace(
            r#"perks: [Sight(4)], grants: ["lowlight"]"#,
            r#"perks: [Sight(4), Armor(2), Move(1), Resist(Kinetic, 15), Throw(3)], grants: ["lowlight"]"#,
        );
        let c = Content::parse(&text).unwrap();
        let ch = vex(&c);
        assert_eq!(ch.armor(&c), 2);
        assert_eq!(ch.move_bonus(&c), 1);
        assert_eq!(ch.resist(&c, Harm::Kinetic), 15);
        assert_eq!(ch.resist(&c, Harm::Thermal), 0);
        assert_eq!(ch.throw_bonus(&c), 3);
    }

    #[test]
    fn boons_read_in_words() {
        let c = Content::load().unwrap();
        assert_eq!(
            Boon::Perk(Perk::Attribute(Attribute::Reflex, 1)).words(&c),
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
    fn a_character_comes_back() {
        let c = Content::load().unwrap();
        let mut ch = Character::debug_runner(&c);
        ch.give(ItemId::new("copper_coil"), 3);
        let worn = ch.worn_copy(Slot::Deck).expect("a worn deck");
        assert!(!worn.stickers.is_empty());
        let bytes = rmp_serde::to_vec_named(&ch).unwrap();
        let back: Character = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back, ch);
    }

    #[test]
    fn new_copies_come_with_their_factory_stickers() {
        let c = Content::load().unwrap();
        let mut ch = Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        let copy = ch.new_copy(&c, &ItemId::new("pawnshop_deck"));
        let item = c.item(&ItemId::new("pawnshop_deck")).unwrap();
        assert_eq!(copy.stickers, item.stickers);
        assert_eq!(copy.stickers.len(), 2);
        assert!(
            ch.new_copy(&c, &ItemId::new("ratchet_smg"))
                .stickers
                .is_empty()
        );
    }

    #[test]
    fn debug_runner_carries_two_of_every_sticker() {
        let c = Content::load().unwrap();
        let d = Character::debug_runner(&c);
        for i in c.items.iter().filter(|i| i.kind == ItemKind::Sticker) {
            assert_eq!(d.count(&i.id), 2, "{}", i.id);
        }
        assert_eq!(d.worn_copy(Slot::Deck).unwrap().stickers.len(), 2);
        let pawn = d
            .inventory
            .iter()
            .find(|s| s.item == ItemId::new("pawnshop_deck"))
            .unwrap();
        assert_eq!(pawn.copy.as_ref().unwrap().stickers.len(), 2);
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
    fn combat_runner_wears_the_port_knows_handguns_and_carries_stims() {
        let c = Content::load().unwrap();
        let d = Character::combat_runner(&c);
        assert_eq!(d.handle, "D'SHAWN");
        assert_eq!(d.chrome_at(Site::Neural), Some(&ItemId::new("neural_port")));
        assert!(d.has_ability(&c, &AbilityId::new("static")));
        assert!(d.has_skill(&SkillId::new("handguns")));
        assert_eq!(
            d.count(&ItemId::new("stim_patch")),
            4,
            "Vex's one and three more"
        );
        assert_eq!(d.equipped(Slot::Primary), Some(&ItemId::new("ratchet_smg")));
        assert_eq!(
            d.count(&ItemId::new("whisper_smg")),
            1,
            "the streak SMG, in the pack"
        );
        assert_eq!(d.level, 50);
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
        // Vex's kit, plus the Backdoor program, the pawnshop deck set aside, the
        // three pieces of deck hardware, the two weapon pieces, four stacks of
        // Ratchet parts, a stack of every sticker, every piece of chrome but the
        // optics Vex wears and the cradle, and every bot but the one docked.
        let kind = |k: ItemKind| c.items.iter().filter(|i| i.kind == k).count();
        let chrome = kind(ItemKind::Cyberware) - d.pieces(&c);
        let bots = kind(ItemKind::Bot) - d.docked(&c).len();
        assert_eq!(
            d.inventory.len(),
            vex.inventory.len() + 11 + kind(ItemKind::Sticker) + chrome + bots,
            "Vex's kit"
        );
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
    fn debug_runner_has_the_ratchet_parts_but_no_barrel() {
        let c = Content::load().unwrap();
        let d = Character::debug_runner(&c);
        assert!(d.has_skill(&SkillId::new("scrapcraft")));
        for (id, n) in [
            ("ratchet_receiver", 1),
            ("ratchet_stock", 1),
            ("ratchet_trigger", 1),
            ("ratchet_mag", 2),
            ("ratchet_barrel", 0),
        ] {
            assert_eq!(d.count(&ItemId::new(id)), n, "{id}");
        }
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
        let (cool, reflex) = (
            vex.attribute(&c, Attribute::Cool),
            vex.attribute(&c, Attribute::Reflex),
        );
        vex.give_item(&c, ItemId::new("hushcan"), 1);
        vex.give_item(&c, ItemId::new("redeye"), 1);
        assert_eq!(
            vex.attribute(&c, Attribute::Cool),
            cool,
            "in the pack it does nothing"
        );
        fit_worn(&mut vex, Slot::Primary, "hushcan", 0);
        assert_eq!(vex.attribute(&c, Attribute::Cool), cool + 1);
        fit_worn(&mut vex, Slot::Primary, "redeye", 1);
        assert_eq!(vex.attribute(&c, Attribute::Reflex), reflex + 1);
        assert_eq!((vex.trace_cut(&c), vex.deck_power(&c)), (0, 0));
        // The gun off the body: its pieces count for nothing.
        let mut off = vex.clone();
        off.equipment.remove(&Slot::Primary);
        assert_eq!(off.attribute(&c, Attribute::Cool), cool);
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
        assert_eq!(d.attribute(&c, Attribute::Cool), 10);
        fit_worn(&mut d, Slot::Primary, "hushcan", 0);
        assert_eq!(d.attribute(&c, Attribute::Cool), 10, "the clamp holds");
    }

    #[test]
    fn deck_hardware_counts_in_the_worn_deck() {
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

    #[test]
    fn combat_runner_carries_the_kit() {
        let c = Content::load().unwrap();
        let d = Character::combat_runner(&c);
        assert_eq!(d.count(&ItemId::new("frag")), 3);
        assert_eq!(d.count(&ItemId::new("flashbang")), 2);
        assert_eq!(d.count(&ItemId::new("emp_puck")), 2);
        assert_eq!(d.count(&ItemId::new("satchel")), 1);
        for a in ["static", "jolt", "shove", "redline", "deadeye"] {
            assert!(d.has_ability(&c, &AbilityId::new(a)), "{a}");
        }
    }

    #[test]
    fn runners_install_their_chrome() {
        let c = Content::load().unwrap();
        let d = Character::combat_runner(&c);
        for (site, id) in [
            (Site::Neural, "neural_port"),
            (Site::Organs, "adrenal_pump"),
            (Site::Eyes, "target_lens"),
        ] {
            assert_eq!(d.chrome_at(site), Some(&ItemId::new(id)), "{id}");
            // Installed, not also carried: the pack holds no spare.
            assert_eq!(d.count(&ItemId::new(id)), 0, "a spare {id} in the pack");
        }
        assert_eq!(
            d.count(&ItemId::new("nightglass_optics")),
            1,
            "the optics the lens replaced"
        );
        assert_eq!(d.count(&ItemId::new("choke_can")), 2);
        let d = Character::debug_runner(&c);
        assert_eq!(
            d.chrome_at(Site::Eyes),
            Some(&ItemId::new("nightglass_optics"))
        );
        // The cradle is in with the Scout drone docked, so a debug start has its drone
        // out; every other piece of chrome is in the pack, one of each, to try on.
        assert_eq!(d.chrome_at(Site::Back), Some(&ItemId::new("drone_cradle")));
        assert_eq!(
            d.docked(&c),
            vec![(Site::Back, 0, ItemId::new("scout_drone"))]
        );
        for i in c.items.iter().filter(|i| i.kind == ItemKind::Cyberware) {
            let installed = ["nightglass_optics", "drone_cradle"].contains(&i.id.0.as_str());
            assert_eq!(d.count(&i.id), if installed { 0 } else { 1 }, "{}", i.id);
        }
        // And every other drone in the pack, to swap in.
        for i in c.items.iter().filter(|i| i.kind == ItemKind::Bot) {
            let docked = i.id.0 == "scout_drone";
            assert_eq!(d.count(&i.id), if docked { 0 } else { 1 }, "{}", i.id);
        }
    }

    #[test]
    fn kits_install_chrome() {
        let c = Content::load().unwrap();
        let v = vex(&c);
        let optics = ItemId::new("nightglass_optics");
        assert_eq!(v.chrome_at(Site::Eyes), Some(&optics));
        assert_eq!((v.count(&optics), v.equipped(Slot::Eyes)), (0, None));
        // Two arms take L ARM, then R ARM, each its own copy; a lens finds the eyes
        // taken and a hand finds no site, so both are packed.
        let mut b = Build::from_template(&c, c.template(&TemplateId::new("vex")).unwrap());
        let (grip, lens, hand) = (
            ItemId::new("grip_arm"),
            ItemId::new("target_lens"),
            ItemId::new("arc_hand"),
        );
        b.kit
            .extend([(grip.clone(), 2), (lens.clone(), 1), (hand.clone(), 1)]);
        let ch = Character::from_build(&c, &b);
        assert_eq!(ch.chrome_at(Site::LeftArm), Some(&grip));
        assert_eq!(ch.chrome_at(Site::RightArm), Some(&grip));
        let (l, r) = (
            ch.chrome_copy(Site::LeftArm).unwrap(),
            ch.chrome_copy(Site::RightArm).unwrap(),
        );
        assert_ne!(l.id, r.id);
        assert_eq!(l.fitted, vec![None, None]);
        assert_eq!(ch.count(&grip), 0);
        assert_eq!((ch.count(&lens), ch.count(&hand)), (1, 1));
        let packed = ch.inventory.iter().find(|s| s.item == hand).unwrap();
        assert!(packed.copy.is_some(), "a socket piece is packed as a copy");
        // Over the budget: the optics take 2 and the first arm 3 of 6; the second is packed.
        b.attributes.body = 0;
        b.attributes.cool = 0;
        let ch = Character::from_build(&c, &b);
        assert_eq!((ch.load_max(), ch.chrome_load(&c)), (6, 5));
        assert_eq!(ch.chrome_at(Site::LeftArm), Some(&grip));
        assert_eq!(ch.chrome_at(Site::RightArm), None);
        assert_eq!(ch.count(&grip), 1);
    }

    #[test]
    fn installed_chrome_counts() {
        let c = Content::load().unwrap();
        let mut ch = Character::from_template(&c, &TemplateId::new("kessler")).unwrap();
        assert_eq!((ch.pieces(&c), ch.chrome_load(&c)), (0, 0));
        let (weight, value, body) = (
            ch.weight(&c),
            ch.value(&c),
            ch.attribute(&c, Attribute::Body),
        );
        let (port, arm, hand) = (
            ItemId::new("neural_port"),
            ItemId::new("piston_arm"),
            ItemId::new("arc_hand"),
        );
        let mut copy = ch.new_copy(&c, &arm);
        copy.fitted[0] = Some(Stack {
            item: hand.clone(),
            qty: 1,
            copy: Some(ch.new_copy(&c, &hand)),
        });
        ch.chrome.insert(Site::LeftArm, arm.clone());
        ch.chrome_copies.insert(Site::LeftArm, copy);
        ch.chrome.insert(Site::Neural, port.clone());
        assert_eq!(
            ch.installed(),
            vec![
                (Site::Neural, None, port.clone()),
                (Site::LeftArm, None, arm.clone()),
                (Site::LeftArm, Some(0), hand.clone()),
            ]
        );
        assert_eq!((ch.pieces(&c), ch.chrome_load(&c)), (3, 5));
        assert_eq!(ch.attribute(&c, Attribute::Body), body + 1);
        let abilities = ch.abilities(&c);
        for (a, from) in [
            ("ping", &port),
            ("static", &port),
            ("shove", &arm),
            ("jolt", &hand),
        ] {
            let listed = (AbilityId::new(a), Source::Item(from.clone()));
            assert!(abilities.contains(&listed), "{a}: {abilities:?}");
        }
        assert_eq!(ch.resist(&c, Harm::Shock), 10);
        assert_eq!(ch.resist(&c, Harm::Emp), -6, "two for every piece");
        // Chrome in you weighs nothing (LOAD is its cost), but it is still worth
        // what it is.
        assert!((ch.weight(&c) - weight).abs() < 1e-4, "{}", ch.weight(&c));
        assert_eq!(ch.value(&c), value + 1500 + 2800 + 2600);
        // A copy left behind at an empty site counts for nothing.
        ch.chrome.remove(&Site::LeftArm);
        assert!(ch.chrome_copy(Site::LeftArm).is_none());
        assert_eq!((ch.pieces(&c), ch.resist(&c, Harm::Shock)), (1, 0));
    }

    #[test]
    fn stack_load_is_the_piece_and_what_is_fitted_in_it() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let (arm, hand) = (ItemId::new("piston_arm"), ItemId::new("arc_hand"));
        let mut k = ch.new_copy(&c, &arm);
        let hc = ch.new_copy(&c, &hand);
        k.fitted[0] = Some(Stack {
            item: hand.clone(),
            qty: 1,
            copy: Some(hc),
        });
        let limb = Stack {
            item: arm.clone(),
            qty: 1,
            copy: Some(k.clone()),
        };
        assert_eq!(stack_load(&c, &limb), 4, "the arm's 3 and the hand's 1");
        assert_eq!(stack_load(&c, &Stack::new(arm.clone(), 1)), 3);
        assert_eq!(stack_load(&c, &Stack::new(ItemId::new("hushcan"), 1)), 0);
        assert_eq!(stack_load(&c, &Stack::new(ItemId::new("hardhat"), 1)), 0);
        // Installed, the same count: `load_at` and the total agree with it.
        ch.install(Site::LeftArm, arm, Some(k));
        assert_eq!(ch.load_at(&c, (Site::LeftArm, None)), 4);
        assert_eq!(ch.load_at(&c, (Site::LeftArm, Some(0))), 1);
        assert_eq!(ch.load_at(&c, (Site::LeftArm, Some(1))), 0);
        assert_eq!(ch.load_at(&c, (Site::RightArm, None)), 0);
        let total: u32 = Site::ALL.iter().map(|&s| ch.load_at(&c, (s, None))).sum();
        assert_eq!(total, ch.chrome_load(&c));
    }

    #[test]
    fn load_max_reads_base_attributes() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        assert_eq!((ch.attributes.body, ch.attributes.cool), (4, 5));
        assert_eq!(ch.load_max(), 24);
        ch.chrome.insert(Site::LeftArm, ItemId::new("piston_arm"));
        assert_eq!(ch.attribute(&c, Attribute::Body), 5, "the arm raises BODY");
        assert_eq!(ch.load_max(), 24, "but not its own budget");
    }

    /// Vex with the Drone cradle at the back and `drone` docked in its bay, or the bay
    /// empty.
    fn cradled(c: &Content, drone: Option<&str>) -> Character {
        let mut ch = vex(c);
        let mut cradle = ch.new_copy(c, &ItemId::new("drone_cradle"));
        if let Some(d) = drone {
            let copy = ch.new_copy(c, &ItemId::new(d));
            cradle.fitted[0] = Some(Stack {
                item: ItemId::new(d),
                qty: 1,
                copy: Some(copy),
            });
        }
        ch.install(Site::Back, ItemId::new("drone_cradle"), Some(cradle));
        ch
    }

    #[test]
    fn a_docked_drone_grants_and_perks_like_chrome() {
        let c = Content::load().unwrap();
        let scout = cradled(&c, Some("scout_drone"));
        assert!(
            scout.abilities(&c).iter().any(
                |(a, from)| a.0 == "scout" && *from == Source::Item(ItemId::new("scout_drone"))
            )
        );
        assert!(
            !cradled(&c, None)
                .abilities(&c)
                .iter()
                .any(|(a, _)| a.0 == "scout"),
            "the cradle grants nothing of its own"
        );
        let empty = cradled(&c, None);
        let mule = cradled(&c, Some("mule_drone"));
        assert_eq!(mule.carry_cap(&c), empty.carry_cap(&c) + 8.0);
        let lamp = cradled(&c, Some("lamp_drone"));
        assert_eq!(lamp.sight_bonus(&c), empty.sight_bonus(&c) + 3);
    }

    #[test]
    fn a_docked_drone_costs_no_load_and_no_emp() {
        let c = Content::load().unwrap();
        let (with, without) = (cradled(&c, Some("scout_drone")), cradled(&c, None));
        assert_eq!(with.pieces(&c), without.pieces(&c));
        assert_eq!(with.chrome_load(&c), without.chrome_load(&c));
        assert_eq!(with.resist(&c, Harm::Emp), without.resist(&c, Harm::Emp));
        assert_eq!(
            with.weight(&c),
            without.weight(&c),
            "docked, it weighs nothing"
        );
    }

    #[test]
    fn a_docked_drone_is_its_own_source() {
        let c = Content::load().unwrap();
        let ch = cradled(&c, Some("mule_drone"));
        assert!(
            ch.sources(&c, Num::Carry)
                .contains(&(Contrib::Bot(ItemId::new("mule_drone"), BotBody::Air), 8))
        );
        assert_eq!(
            ch.sources(&c, Num::Resist(Harm::Emp)),
            cradled(&c, None).sources(&c, Num::Resist(Harm::Emp)),
            "the drone is no piece"
        );
    }

    #[test]
    fn docked_lists_bots_in_their_bays() {
        let c = Content::load().unwrap();
        assert_eq!(
            cradled(&c, Some("scout_drone")).docked(&c),
            vec![(Site::Back, 0, ItemId::new("scout_drone"))]
        );
        assert!(cradled(&c, None).docked(&c).is_empty());
    }

    #[test]
    fn sources_name_each_contribution() {
        let c = Content::load().unwrap();
        let mut ch = Character::from_template(&c, &TemplateId::new("kessler")).unwrap();
        let (arm, shades) = (ItemId::new("piston_arm"), ItemId::new("mirrorshades"));
        ch.install(Site::LeftArm, arm.clone(), None);
        ch.wear(Slot::Eyes, shades.clone(), None);
        assert_eq!(
            ch.sources(&c, Num::Attribute(Attribute::Body)),
            vec![(Contrib::Chrome(arm.clone()), 1)]
        );
        assert_eq!(
            ch.sources(&c, Num::Attribute(Attribute::Cool)),
            vec![(Contrib::Gear(shades.clone()), 1)]
        );
        assert_eq!(
            ch.sources(&c, Num::Resist(Harm::Emp)),
            vec![(Contrib::Pieces(1), -2)]
        );
        assert_eq!(ch.sources(&c, Num::Armor), vec![], "nothing moves it");
        // Talents sum into one; carry rounds to whole kilograms.
        ch.talents.insert(TalentId::new("thick_skin"), 2);
        assert_eq!(
            ch.sources(&c, Num::Hp),
            vec![
                (Contrib::Gear(ItemId::new("ballistic_weave")), 8),
                (Contrib::Talents, 6)
            ]
        );
        assert_eq!(
            ch.sources(&c, Num::Carry),
            vec![(Contrib::Gear(ItemId::new("cargo_pants")), 4)]
        );
        // Hardware fitted in what you wear or carry in you is named on its own.
        let hush = ItemId::new("hushcan");
        let copy = ch.new_copy(&c, &hush);
        ch.host_copy_mut(&c, Host::Site(Site::LeftArm))
            .unwrap()
            .fitted[1] = Some(Stack {
            item: hush.clone(),
            qty: 1,
            copy: Some(copy),
        });
        assert_eq!(
            ch.sources(&c, Num::Attribute(Attribute::Cool)),
            vec![(Contrib::Gear(shades), 1), (Contrib::Hardware(hush), 1)]
        );
        assert_eq!(
            ch.sources(&c, Num::Resist(Harm::Emp)),
            vec![(Contrib::Pieces(2), -4)]
        );
    }

    #[test]
    fn nums_read_in_the_sheets_order() {
        let labels: Vec<&str> = Num::ALL.iter().map(|n| n.label()).collect();
        assert_eq!(
            labels,
            [
                "BOD", "REF", "TEC", "COL", "NET", "HP", "FOC", "ARM", "SGT", "CAR", "MOV", "KIN",
                "THM", "SHK", "EMP", "TOX", "FBK"
            ]
        );
        let names: Vec<&str> = Num::ALL.iter().map(|n| n.name()).collect();
        assert_eq!(
            names,
            [
                "BODY", "REFLEX", "TECH", "COOL", "NET", "HP", "FOCUS", "ARMOUR", "SIGHT", "CARRY",
                "MOVE", "KINETIC", "THERMAL", "SHOCK", "EMP", "TOXIN", "FEEDBACK"
            ]
        );
    }

    #[test]
    fn hardware_in_installed_chrome_counts() {
        let c = Content::load().unwrap();
        let mut ch = vex(&c);
        let arm = ItemId::new("piston_arm");
        let mut copy = ch.new_copy(&c, &arm);
        let cool = ch.attribute(&c, Attribute::Cool);
        copy.fitted[1] = Some(Stack {
            item: ItemId::new("hushcan"),
            qty: 1,
            copy: Some(ch.new_copy(&c, &ItemId::new("hushcan"))),
        });
        ch.chrome.insert(Site::RightArm, arm);
        ch.chrome_copies.insert(Site::RightArm, copy);
        assert_eq!(ch.worn_mods(&c), vec![Mod::Attribute(Attribute::Cool, 1)]);
        assert_eq!(ch.attribute(&c, Attribute::Cool), cool + 1);
    }

    #[test]
    fn a_runner_forgets_nothing_the_content_has() {
        let c = Content::load().unwrap();
        for mut d in [Character::debug_runner(&c), Character::combat_runner(&c)] {
            let was = d.clone();
            assert_eq!(d.forget_unknown(&c), Vec::<String>::new());
            assert_eq!(d, was);
        }
    }

    #[test]
    fn a_runner_forgets_what_the_content_lacks() {
        let c = Content::load().unwrap();
        let mut d = Character::combat_runner(&c);
        let deck = d.equipped(Slot::Deck).cloned().unwrap();
        // A sticker on the pawnshop deck in the pack, which stays.
        let pawn = ItemId::new("pawnshop_deck");
        let held = |d: &Character| {
            d.inventory
                .iter()
                .find(|s| s.item == pawn)
                .and_then(|s| s.copy.clone())
                .unwrap()
        };
        let worn_sticker = held(&d).stickers[0].sticker.clone();
        let cradle = d.chrome_copies.get(&Site::Back).unwrap();
        let bot = cradle.fitted.iter().flatten().next().unwrap().item.clone();
        let neural = d.chrome.get(&Site::Neural).cloned().unwrap();
        let program = d.loaded[0].clone();
        let talent = d.talents.keys().next().cloned().unwrap();
        let skill = SkillId::new("scrapcraft");
        assert!(d.skills.contains(&skill));
        let ability = c.abilities.first().map(|a| a.id.clone()).unwrap();
        d.abilities.push(ability.clone());
        d.fired.insert(ability.clone(), 5);
        let category = d.categories.first().cloned().expect("a category");
        // A stack the content lacks altogether, in the pack.
        let knife = ItemId::new("old_knife");
        d.give(knife.clone(), 2);
        let mut less = c.clone();
        less.items
            .retain(|i| ![&deck, &worn_sticker, &bot, &neural, &program].contains(&&i.id));
        less.talents.retain(|t| t.id != talent);
        less.skills.retain(|s| s.id != skill);
        less.abilities.retain(|a| a.id != ability);
        less.categories.retain(|k| k.id != category);

        let lost = d.forget_unknown(&less);
        assert!(
            lost.contains(&format!("an ability ({ability})")),
            "{lost:?}"
        );
        assert!(
            lost.contains(&format!("a category ({category})")),
            "{lost:?}"
        );
        assert!(!d.abilities.contains(&ability));
        assert!(!d.fired.contains_key(&ability), "nor its cooldown");
        assert!(!d.categories.contains(&category));
        for line in [
            format!("an item ({deck})"),
            format!("a sticker ({worn_sticker})"),
            format!("an item ({bot})"),
            format!("chrome ({neural})"),
            format!("an item ({program})"),
            format!("a talent ({talent})"),
            format!("a skill ({skill})"),
            "an item (old_knife)".to_string(),
        ] {
            assert!(lost.contains(&line), "{line} in {lost:?}");
        }
        assert_eq!(d.equipped(Slot::Deck), None);
        assert!(d.worn_copy(Slot::Deck).is_none(), "its copy goes with it");
        assert!(!d.chrome.contains_key(&Site::Neural));
        assert!(!d.chrome_copies.contains_key(&Site::Neural));
        let cradle = d.chrome_copies.get(&Site::Back).expect("the cradle stays");
        assert!(
            cradle.fitted.iter().flatten().all(|s| s.item != bot),
            "the bot's socket is empty"
        );
        assert!(!d.loaded.contains(&program));
        assert!(!d.talents.contains_key(&talent));
        assert!(!d.skills.contains(&skill));
        assert_eq!(d.count(&knife), 0);
        assert_eq!(d.count(&worn_sticker), 0, "nor in the pack");
        assert!(
            held(&d).stickers.iter().all(|s| s.sticker != worn_sticker),
            "nor stuck on the deck the pack holds"
        );
        // Run again, nothing more goes.
        assert!(d.forget_unknown(&less).is_empty());
    }

    #[test]
    fn a_runner_forgets_a_host_but_not_what_it_held() {
        let c = Content::load().unwrap();
        let mut d = Character::combat_runner(&c);
        let (deck, cradle) = (ItemId::new("rebuilt_deck"), ItemId::new("drone_cradle"));
        assert_eq!(d.equipped(Slot::Deck), Some(&deck));
        assert_eq!(d.chrome.get(&Site::Back), Some(&cradle));
        let stuck: Vec<ItemId> = d
            .worn_copy(Slot::Deck)
            .unwrap()
            .stickers
            .iter()
            .map(|s| s.sticker.clone())
            .collect();
        let docked: Vec<ItemId> = d.chrome_copies[&Site::Back]
            .fitted
            .iter()
            .flatten()
            .map(|s| s.item.clone())
            .collect();
        assert!(!stuck.is_empty() && !docked.is_empty());
        let counts = |d: &Character, ids: &[ItemId]| -> Vec<u32> {
            ids.iter().map(|i| d.count(i)).collect()
        };
        let (stickers, bots) = (counts(&d, &stuck), counts(&d, &docked));
        let mut less = c.clone();
        less.items.retain(|i| i.id != deck && i.id != cradle);

        let lost = d.forget_unknown(&less);
        assert_eq!(
            lost,
            [format!("an item ({deck})"), format!("chrome ({cradle})")],
            "only the hosts are lost"
        );
        // Each id up by as many as the host held of it.
        let up = |was: &[u32], ids: &[ItemId]| -> Vec<u32> {
            ids.iter()
                .zip(was)
                .map(|(i, n)| n + ids.iter().filter(|j| *j == i).count() as u32)
                .collect()
        };
        assert_eq!(
            counts(&d, &stuck),
            up(&stickers, &stuck),
            "the stickers come back to the pack"
        );
        assert_eq!(
            counts(&d, &docked),
            up(&bots, &docked),
            "the docked bots go in the pack"
        );
        let pack_bots = d
            .inventory
            .iter()
            .filter(|s| docked.contains(&s.item))
            .all(|s| s.copy.is_some());
        assert!(pack_bots, "each bot keeps its copy");
    }
}
