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
id_type!(AbilityId);
id_type!(OriginId);
id_type!(ProfessionId);
id_type!(CategoryId);
id_type!(TemplateId);

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
    /// Software for a deck's slots; never worn, loaded instead.
    Program,
}

impl ItemKind {
    pub const ALL: [ItemKind; 8] = [
        ItemKind::Weapon,
        ItemKind::Armor,
        ItemKind::Cyberware,
        ItemKind::Deck,
        ItemKind::Consumable,
        ItemKind::Component,
        ItemKind::Junk,
        ItemKind::Program,
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
            ItemKind::Program => "program",
        }
    }
}

/// What a program does to a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Op {
    /// Opens anything that answers, loudly; the only op that takes ICE down.
    Crack,
    /// The quiet unlock: door locks and lift controls.
    Spoof,
    /// A camera stops seeing.
    Blind,
    /// Paydata off a server or a vault.
    Copy,
    /// A server keeps the construct open for you.
    Backdoor,
    /// Takes integrity off a piece of ICE.
    Attack,
    /// Soaks the next hit on you.
    Shield,
    /// Halves how far ICE sees you, for a while.
    Mask,
}

impl Op {
    pub const ALL: [Op; 8] = [
        Op::Crack,
        Op::Spoof,
        Op::Blind,
        Op::Copy,
        Op::Backdoor,
        Op::Attack,
        Op::Shield,
        Op::Mask,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Op::Crack => "crack",
            Op::Spoof => "spoof",
            Op::Blind => "blind",
            Op::Copy => "copy",
            Op::Backdoor => "backdoor",
            Op::Attack => "attack",
            Op::Shield => "shield",
            Op::Mask => "mask",
        }
    }
}

/// A program's numbers: what it does, how well, how much deck it takes, how loud
/// a turn is and how many successful turns land it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub op: Op,
    pub rating: u8,
    pub size: u8,
    pub noise: u8,
    pub work: u8,
}

/// A deck's slots for programs and its rating.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckSpec {
    pub slots: u8,
    pub rating: u8,
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
    pub const ALL: [Stat; 5] = [Stat::Body, Stat::Reflex, Stat::Tech, Stat::Cool, Stat::Net];

    /// The label in caps, as the sheet sets it.
    pub fn name(self) -> &'static str {
        match self {
            Stat::Body => "BODY",
            Stat::Reflex => "REFLEX",
            Stat::Tech => "TECH",
            Stat::Cool => "COOL",
            Stat::Net => "NET",
        }
    }

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

/// A modifier an item applies while it is equipped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Perk {
    Stat(Stat, i8),
    /// Field-of-view radius, in tiles.
    Sight(i8),
    /// Carry capacity, in kilograms.
    Carry(f32),
    HpMax(i32),
    /// Max focus.
    Focus(i32),
}

/// What a consumable does when used; one unit is consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Use {
    Heal(u32),
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
    /// Modifiers while equipped (slot items only).
    #[serde(default)]
    pub perks: Vec<Perk>,
    /// Abilities conferred while equipped (slot items only).
    #[serde(default)]
    pub grants: Vec<AbilityId>,
    /// What using it does (consumables only).
    #[serde(default, rename = "use")]
    pub on_use: Option<Use>,
    /// The program's numbers (programs only).
    #[serde(default)]
    pub program: Option<Program>,
    /// The deck's slots and rating (decks only).
    #[serde(default)]
    pub deck: Option<DeckSpec>,
}

/// What an active ability does when fired.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Active {
    /// Paints the map within `radius` tiles into memory.
    Scan { radius: i32 },
    /// Lights terminals and stairs within `radius` tiles; its glow is an `Fx`.
    Ping { radius: i32 },
    /// Arcs to every net node within `radius` tiles and marks them; all `Fx`.
    Trace { radius: i32 },
}

/// A visual effect an ability plays when it fires; the app draws it over the map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Fx {
    /// A glitch burst over the map for this many ticks, thinning as it ends.
    Kick(u64),
    /// A ring that expands from the firer to the ability's radius over `ticks`.
    Wave { colour: (u8, u8, u8), ticks: u64 },
    /// A pulsing marker on terminals and stairs within the radius for `ticks`.
    Glow { colour: (u8, u8, u8), ticks: u64 },
    /// Snaking lines from the firer to every target, growing over `ticks`.
    Bolt { colour: (u8, u8, u8), ticks: u64 },
    /// A pulsing mark on every target tile, through walls, for `ticks`.
    Mark { colour: (u8, u8, u8), ticks: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityKind {
    Passive,
    Active(Active),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ability {
    pub id: AbilityId,
    pub name: String,
    pub desc: String,
    /// Ticks between firings; 0 means none.
    pub cooldown: u64,
    pub kind: AbilityKind,
    /// What firing looks like, in order; empty plays nothing.
    #[serde(default)]
    pub fx: Vec<Fx>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    pub id: SkillId,
    pub name: String,
    pub stat: Stat,
    pub desc: String,
}

/// What one rank of a talent adds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Boon {
    Perk(Perk),
    /// An ability the character gains.
    Grant(AbilityId),
    /// Reach added to an ability and ticks added to its cooldown (negative cools sooner).
    Sharpen {
        ability: AbilityId,
        #[serde(default)]
        radius: i32,
        #[serde(default)]
        cooldown: i64,
    },
    /// A named promise a later system reads.
    Flag(String),
}

/// A talent in a tree: ranks, each with its boons, behind an optional prerequisite
/// straight below it in the same column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Talent {
    pub id: TalentId,
    pub name: String,
    pub icon: char,
    pub ranks: u8,
    #[serde(default)]
    pub requires: Option<TalentId>,
    /// One list per rank: what that rank adds.
    pub effects: Vec<Vec<Boon>>,
    /// The skill it flavours.
    #[serde(default)]
    pub skill: Option<SkillId>,
    pub desc: String,
}

/// A tree of talents under one stat: tiers from the bottom, each a row of talents.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    pub stat: Stat,
    pub name: String,
    pub line: String,
    pub tiers: Vec<Vec<TalentId>>,
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
    pub inputs: Vec<(ItemId, u32)>,
    pub output: (ItemId, u32),
    pub desc: String,
}

/// Where you were made. Cheap stats cost one less per point, dear ones one more.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Origin {
    pub id: OriginId,
    pub name: String,
    pub line: String,
    pub desc: String,
    pub cheap: Vec<Stat>,
    pub dear: Vec<Stat>,
    /// Three rows of seven glyphs for the dossier.
    pub emblem: Vec<String>,
}

/// What you did before this: affinity, a kit and credits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profession {
    pub id: ProfessionId,
    pub name: String,
    pub line: String,
    pub desc: String,
    pub cheap: Vec<Stat>,
    pub dear: Vec<Stat>,
    pub kit: Vec<(ItemId, u32)>,
    pub credits: u32,
    pub emblem: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategorySkill {
    pub id: SkillId,
    pub cost: u32,
}

/// What owning a category opens besides its skills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unlock {
    Workbench,
}

/// A skill category: costs SP to own, then its skills cost SP each.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub line: String,
    pub cost: u32,
    pub skills: Vec<CategorySkill>,
    #[serde(default)]
    pub grants: Vec<AbilityId>,
    #[serde(default)]
    pub unlocks: Vec<Unlock>,
}

/// A complete build the chooser offers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub id: TemplateId,
    pub handle: String,
    pub line: String,
    pub origin: OriginId,
    pub profession: ProfessionId,
    pub stats: Stats,
    pub categories: Vec<CategoryId>,
    pub skills: Vec<SkillId>,
    /// Extra kit on top of the profession's.
    #[serde(default)]
    pub kit: Vec<(ItemId, u32)>,
}

/// The numbers every new character is built under.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creation {
    pub stat_base: u8,
    pub stat_cap: u8,
    pub stat_points: u32,
    pub point_cost: u32,
    pub sp: u32,
    pub name_max: usize,
    pub hp: u32,
    pub focus: u32,
    /// Talent points at creation.
    #[serde(default = "one")]
    pub talent_points: u32,
    /// Points a tree needs per tier below the one opening.
    #[serde(default = "three")]
    pub tier_gate: u32,
}

fn one() -> u32 {
    1
}
fn three() -> u32 {
    3
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub body: u8,
    pub reflex: u8,
    pub tech: u8,
    pub cool: u8,
    pub net: u8,
}

impl Stats {
    pub fn get(&self, stat: Stat) -> u8 {
        match stat {
            Stat::Body => self.body,
            Stat::Reflex => self.reflex,
            Stat::Tech => self.tech,
            Stat::Cool => self.cool,
            Stat::Net => self.net,
        }
    }
}
