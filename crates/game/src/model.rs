//! Ids, enums and the content structs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    Torso,
    Legs,
    Primary,
    Secondary,
    Deck,
}

impl Slot {
    pub const ALL: [Slot; 7] = [
        Slot::Head,
        Slot::Eyes,
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
            Slot::Torso => "torso",
            Slot::Legs => "legs",
            Slot::Primary => "primary",
            Slot::Secondary => "secondary",
            Slot::Deck => "deck",
        }
    }
}

/// A place on the body that holds one piece of chrome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Site {
    Neural,
    Eyes,
    Ears,
    Jaw,
    Spine,
    Back,
    Organs,
    Skin,
    LeftArm,
    RightArm,
    Legs,
}

impl Site {
    /// Every site, in ledger order.
    pub const ALL: [Site; 11] = [
        Site::Neural,
        Site::Eyes,
        Site::Ears,
        Site::Jaw,
        Site::Spine,
        Site::Back,
        Site::Organs,
        Site::Skin,
        Site::LeftArm,
        Site::RightArm,
        Site::Legs,
    ];

    /// The short name in caps, as messages set it.
    pub fn label(self) -> &'static str {
        match self {
            Site::Neural => "NEURAL",
            Site::Eyes => "EYES",
            Site::Ears => "EARS",
            Site::Jaw => "JAW",
            Site::Spine => "SPINE",
            Site::Back => "BACK",
            Site::Organs => "ORGANS",
            Site::Skin => "SKIN",
            Site::LeftArm => "L ARM",
            Site::RightArm => "R ARM",
            Site::Legs => "LEGS",
        }
    }

    /// The ledger's row for an empty site.
    pub fn lower(self) -> &'static str {
        match self {
            Site::Neural => "neural",
            Site::Eyes => "eyes",
            Site::Ears => "ears",
            Site::Jaw => "jaw",
            Site::Spine => "spine",
            Site::Back => "back",
            Site::Organs => "organs",
            Site::Skin => "skin",
            Site::LeftArm => "l arm",
            Site::RightArm => "r arm",
            Site::Legs => "legs",
        }
    }

    /// The name in full, for the card.
    pub fn long(self) -> &'static str {
        match self {
            Site::LeftArm => "LEFT ARM",
            Site::RightArm => "RIGHT ARM",
            s => s.label(),
        }
    }

    /// The ledger's group: `head`, `body` or `limbs`.
    pub fn group(self) -> &'static str {
        match self {
            Site::Neural | Site::Eyes | Site::Ears | Site::Jaw => "head",
            Site::Spine | Site::Back | Site::Organs | Site::Skin => "body",
            Site::LeftArm | Site::RightArm | Site::Legs => "limbs",
        }
    }

    /// What a piece must fit to go here; both arms are one kind.
    pub fn fits(self) -> Fits {
        match self {
            Site::Neural => Fits::Neural,
            Site::Eyes => Fits::Eyes,
            Site::Ears => Fits::Ears,
            Site::Jaw => Fits::Jaw,
            Site::Spine => Fits::Spine,
            Site::Back => Fits::Back,
            Site::Organs => Fits::Organs,
            Site::Skin => Fits::Skin,
            Site::LeftArm | Site::RightArm => Fits::Arm,
            Site::Legs => Fits::Legs,
        }
    }
}

/// What a piece of chrome fits: a kind of site, or a kind of socket on a chrome limb.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Fits {
    Neural,
    Eyes,
    Ears,
    Jaw,
    Spine,
    Back,
    Organs,
    Skin,
    Arm,
    Legs,
    Socket(SocketKind),
}

impl Fits {
    pub fn label(self) -> &'static str {
        match self {
            Fits::Neural => "neural",
            Fits::Eyes => "eyes",
            Fits::Ears => "ears",
            Fits::Jaw => "jaw",
            Fits::Spine => "spine",
            Fits::Back => "back",
            Fits::Organs => "organs",
            Fits::Skin => "skin",
            Fits::Arm => "arm",
            Fits::Legs => "legs",
            Fits::Socket(k) => k.fits(),
        }
    }
}

/// A piece of chrome: what it fits and what it takes of your load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Implant {
    pub fits: Fits,
    /// What it takes of your load.
    pub load: u8,
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
    /// Fits into a socket on another item and adds its mods there.
    Hardware,
    /// A decal stuck on an item's drawing; changes nothing but how it looks.
    Sticker,
    /// A small vehicle that docks in a socket and comes out with you: a drone.
    Bot,
}

impl ItemKind {
    pub const ALL: [ItemKind; 11] = [
        ItemKind::Weapon,
        ItemKind::Armor,
        ItemKind::Cyberware,
        ItemKind::Deck,
        ItemKind::Consumable,
        ItemKind::Component,
        ItemKind::Junk,
        ItemKind::Program,
        ItemKind::Hardware,
        ItemKind::Sticker,
        ItemKind::Bot,
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
            ItemKind::Hardware => "hardware",
            ItemKind::Sticker => "sticker",
            ItemKind::Bot => "bot",
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

    /// What a landed run did, as the tether's label reads it: `cracked`.
    pub fn done_label(self) -> &'static str {
        match self {
            Op::Crack => "cracked",
            Op::Spoof => "spoofed",
            Op::Blind => "blinded",
            Op::Copy => "copied",
            Op::Backdoor => "planted",
            Op::Attack => "hit",
            Op::Shield | Op::Mask => "raised",
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

/// Which drawing a deck wears on its blueprint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeckLook {
    #[default]
    Pawn,
}

impl DeckLook {
    pub fn name(self) -> &'static str {
        match self {
            DeckLook::Pawn => "pawn",
        }
    }

    /// Program slots the drawing has room for.
    pub fn max_slots(self) -> u8 {
        match self {
            DeckLook::Pawn => 5,
        }
    }

    /// The sockets the drawing places, by name and kind; its length is the room for them.
    pub fn sockets(self) -> &'static [(&'static str, SocketKind)] {
        match self {
            DeckLook::Pawn => &[("L", SocketKind::Deck), ("R", SocketKind::Deck)],
        }
    }

    /// ROM abilities the drawing has room for.
    pub fn max_grants(self) -> usize {
        match self {
            DeckLook::Pawn => 3,
        }
    }
}

/// Which drawing a non-deck item wears on its blueprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemLook {
    Ratchet,
}

impl ItemLook {
    /// The sockets the drawing places, by name and kind; its length is the room for them.
    pub fn sockets(self) -> &'static [(&'static str, SocketKind)] {
        match self {
            ItemLook::Ratchet => &[("MUZZLE", SocketKind::Muzzle), ("RAIL", SocketKind::Rail)],
        }
    }
}

/// What a socket takes and what a piece of hardware fits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SocketKind {
    Deck,
    Muzzle,
    Rail,
    /// On a chrome arm: the hand.
    Hand,
    /// On a chrome arm: along the forearm.
    Forearm,
    /// On a drone cradle: where a drone docks.
    Bay,
}

impl SocketKind {
    /// The socket's own name, lower case: what the ledger and the card call it.
    pub fn label(self) -> &'static str {
        match self {
            SocketKind::Deck => "deck",
            SocketKind::Muzzle => "muzzle",
            SocketKind::Rail => "rail",
            SocketKind::Hand => "hand",
            SocketKind::Forearm => "forearm",
            SocketKind::Bay => "bay",
        }
    }

    /// What a socket of this kind takes.
    pub fn takes(self) -> &'static str {
        match self {
            SocketKind::Deck => "deck hardware",
            SocketKind::Muzzle => "muzzle hardware",
            SocketKind::Rail => "rail hardware",
            SocketKind::Hand => "hand chrome",
            SocketKind::Forearm => "forearm chrome",
            SocketKind::Bay => "drones",
        }
    }

    /// What a piece that fits this kind of socket is called on its card.
    pub fn fits(self) -> &'static str {
        match self {
            SocketKind::Deck => "deck socket",
            SocketKind::Muzzle => "muzzle socket",
            SocketKind::Rail => "rail socket",
            SocketKind::Hand => "hand socket",
            SocketKind::Forearm => "forearm socket",
            SocketKind::Bay => "bay",
        }
    }

    /// What goes in or out of it is surgery, in a ripperdoc's chair: a limb's
    /// sockets. A drone docks in its bay anywhere.
    pub fn needs_a_doc(self) -> bool {
        matches!(self, SocketKind::Hand | SocketKind::Forearm)
    }
}

/// How a bot gets about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotBody {
    /// In the air, over any floor.
    Air,
    /// On the ground.
    Ground,
}

impl BotBody {
    /// What the game calls a bot with this body.
    pub const fn word(self) -> &'static str {
        match self {
            BotBody::Air => "drone",
            BotBody::Ground => "rover",
        }
    }
}

/// A small vehicle: what it is when it is out with you, docked in a socket of your
/// chrome. Its perks and grants are the item's, and count while it is docked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bot {
    pub body: BotBody,
    /// The kind of socket it docks in.
    pub docks: SocketKind,
    /// Its look on the map: the frames it cycles, in its light.
    pub frames: Vec<char>,
    pub light: (u8, u8, u8),
    /// Moving leaves a wake behind it.
    pub wake: bool,
    /// Its seat in a fight, and how far it moves in a turn.
    pub reflex: u8,
    pub moves: u8,
}

/// What a fitted piece of hardware changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mod {
    /// Percent off each trace the deck makes.
    TraceCut(u8),
    /// Added to every program turn's power.
    Power(i8),
    /// Added to one of the wearer's attributes.
    Attribute(Attribute, i8),
}

/// A number with its sign always shown; a negative takes a real minus, as the
/// other mod text does.
fn signed(n: i8) -> String {
    if n < 0 {
        format!("\u{2212}{}", n.unsigned_abs())
    } else {
        format!("+{n}")
    }
}

impl Mod {
    /// The line the item card reads.
    pub fn line(&self) -> String {
        match self {
            Mod::TraceCut(n) => format!("trace per cast \u{2212}{n}%"),
            Mod::Power(n) => format!("program power {}", signed(*n)),
            Mod::Attribute(s, n) => format!("{} {}", s.label(), signed(*n)),
        }
    }

    /// The short form stuck on the drawing.
    pub fn sticker(&self) -> String {
        match self {
            Mod::TraceCut(n) => format!("TRACE \u{2212}{n}%"),
            Mod::Power(n) => format!("POWER {}", signed(*n)),
            Mod::Attribute(s, n) => format!("{} {}", s.name(), signed(*n)),
        }
    }
}

/// Which drawings a piece of hardware wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HardwareLook {
    ColdfinI,
    ColdfinII,
    Sidecar,
    Hushcan,
    Redeye,
}

impl HardwareLook {
    /// The kind of socket the drawing is made for.
    pub fn kind(self) -> SocketKind {
        match self {
            HardwareLook::ColdfinI | HardwareLook::ColdfinII | HardwareLook::Sidecar => {
                SocketKind::Deck
            }
            HardwareLook::Hushcan => SocketKind::Muzzle,
            HardwareLook::Redeye => SocketKind::Rail,
        }
    }
}

/// A piece of hardware: the socket it fits, what it does, how it looks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hardware {
    pub fits: SocketKind,
    pub mods: Vec<Mod>,
    pub look: HardwareLook,
}

fn bold() -> bool {
    true
}

/// A sticker's graphic: one row of lettering on a ground, or half-block pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StickerArt {
    /// The text carries its own margin, a space at each end.
    Lettered {
        text: String,
        ink: (u8, u8, u8),
        ground: (u8, u8, u8),
        #[serde(default = "bold")]
        bold: bool,
    },
    /// One char per pixel, two pixels to a cell stacked; `.` is empty.
    Picture {
        rows: Vec<String>,
        colours: BTreeMap<char, (u8, u8, u8)>,
    },
}

/// How far a sticker is turned, clockwise in quarters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Turn {
    #[default]
    Level,
    Quarter,
    Half,
    ThreeQuarter,
}

impl Turn {
    pub const ALL: [Turn; 4] = [Turn::Level, Turn::Quarter, Turn::Half, Turn::ThreeQuarter];

    /// A quarter further clockwise.
    pub fn cw(self) -> Turn {
        match self {
            Turn::Level => Turn::Quarter,
            Turn::Quarter => Turn::Half,
            Turn::Half => Turn::ThreeQuarter,
            Turn::ThreeQuarter => Turn::Level,
        }
    }

    pub fn degrees(self) -> u16 {
        match self {
            Turn::Level => 0,
            Turn::Quarter => 90,
            Turn::Half => 180,
            Turn::ThreeQuarter => 270,
        }
    }
}

/// A sticker stuck on an item's drawing: its top-left cell as turned, in drawing cells.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stuck {
    pub sticker: ItemId,
    pub x: u16,
    pub y: u16,
    #[serde(default)]
    pub turn: Turn,
}

/// A deck's slots for programs, its rating and the look it wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckSpec {
    pub slots: u8,
    pub rating: u8,
    #[serde(default)]
    pub look: DeckLook,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Attribute {
    Body,
    Reflex,
    Tech,
    Cool,
    Net,
}

impl Attribute {
    pub const ALL: [Attribute; 5] = [
        Attribute::Body,
        Attribute::Reflex,
        Attribute::Tech,
        Attribute::Cool,
        Attribute::Net,
    ];

    /// The label in caps, as the sheet sets it.
    pub fn name(self) -> &'static str {
        match self {
            Attribute::Body => "BODY",
            Attribute::Reflex => "REFLEX",
            Attribute::Tech => "TECH",
            Attribute::Cool => "COOL",
            Attribute::Net => "NET",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Attribute::Body => "body",
            Attribute::Reflex => "reflex",
            Attribute::Tech => "tech",
            Attribute::Cool => "cool",
            Attribute::Net => "net",
        }
    }

    /// Three letters in caps, for a line with no room for the names.
    pub fn short(self) -> &'static str {
        match self {
            Attribute::Body => "BOD",
            Attribute::Reflex => "REF",
            Attribute::Tech => "TEC",
            Attribute::Cool => "COL",
            Attribute::Net => "NET",
        }
    }
}

/// The kind of harm a blast does, and the kind a resistance turns aside.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum Harm {
    #[default]
    Kinetic,
    Thermal,
    Shock,
    Emp,
    Toxin,
    Feedback,
}

impl Harm {
    pub const ALL: [Harm; 6] = [
        Harm::Kinetic,
        Harm::Thermal,
        Harm::Shock,
        Harm::Emp,
        Harm::Toxin,
        Harm::Feedback,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Harm::Kinetic => "kinetic",
            Harm::Thermal => "thermal",
            Harm::Shock => "shock",
            Harm::Emp => "emp",
            Harm::Toxin => "toxin",
            Harm::Feedback => "feedback",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Harm::Kinetic => "KINETIC",
            Harm::Thermal => "THERMAL",
            Harm::Shock => "SHOCK",
            Harm::Emp => "EMP",
            Harm::Toxin => "TOXIN",
            Harm::Feedback => "FEEDBACK",
        }
    }

    /// Three letters in caps, for a line with no room for the names.
    pub fn short(self) -> &'static str {
        match self {
            Harm::Kinetic => "KIN",
            Harm::Thermal => "THM",
            Harm::Shock => "SHK",
            Harm::Emp => "EMP",
            Harm::Toxin => "TOX",
            Harm::Feedback => "FBK",
        }
    }
}

/// A modifier an item applies while it is equipped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Perk {
    Attribute(Attribute, i8),
    /// Field-of-view radius, in tiles.
    Sight(i8),
    /// Carry capacity, in kilograms.
    Carry(f32),
    HpMax(i32),
    /// Max focus.
    Focus(i32),
    /// Armour: each point turns 10% of the rounds that hit, at most 60%.
    Armor(i8),
    /// Extra move, in tiles a turn.
    Move(i8),
    /// Percent of a kind of harm turned aside; negative takes more.
    Resist(Harm, i8),
    /// Extra throw range, in tiles.
    Throw(i8),
}

/// What a consumable does when used; one unit is consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Use {
    Heal(u32),
    /// Thrown at a tile within `range`; the blast goes off there.
    Throw {
        range: i32,
        arc: Arc,
        blast: Blast,
    },
}

/// What a blast does where it lands, from a grenade or an ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blast {
    /// Tiles, Chebyshev; at least one.
    pub radius: i32,
    /// At the centre; rings take less. Zero for a stun-only blast.
    pub damage: u8,
    /// Turns of stun, every ring the same.
    pub stun: u8,
    /// Turns it lies live; zero goes off on landing.
    #[serde(default)]
    pub fuse: u8,
    /// Only things with chrome are hurt.
    #[serde(default)]
    pub chrome: bool,
    /// The kind of harm it does.
    #[serde(default)]
    pub harm: Harm,
}

/// How a thrown thing crosses the line: how it is drawn in flight. Either kind
/// lands only on a tile you see down a clear line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arc {
    /// Lobbed: it rises over the line and comes down on the tile.
    Lob,
    /// Slid along the ground, on the line all the way.
    Shadow,
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
    /// What it fits and takes of your load (cyberware only).
    #[serde(default)]
    pub implant: Option<Implant>,
    /// Modifiers while equipped or installed (slot items and chrome only).
    #[serde(default)]
    pub perks: Vec<Perk>,
    /// Abilities conferred while equipped or installed (slot items and chrome only).
    #[serde(default)]
    pub grants: Vec<AbilityId>,
    /// What using it does (consumables only).
    #[serde(default, rename = "use")]
    pub on_use: Option<Use>,
    /// What opening it gives, one per open (the item is used up).
    #[serde(default)]
    pub opens: Vec<(ItemId, u32)>,
    /// The program's numbers (programs only).
    #[serde(default)]
    pub program: Option<Program>,
    /// The deck's slots and rating (decks only).
    #[serde(default)]
    pub deck: Option<DeckSpec>,
    /// The sockets a new copy is made with.
    #[serde(default)]
    pub sockets: Vec<SocketKind>,
    /// The drawing a non-deck item wears on its blueprint.
    #[serde(default)]
    pub look: Option<ItemLook>,
    /// What the piece fits and does (hardware only).
    #[serde(default)]
    pub hardware: Option<Hardware>,
    /// Its graphic (stickers only).
    #[serde(default)]
    pub sticker: Option<StickerArt>,
    /// The stickers a new copy is made with.
    #[serde(default)]
    pub stickers: Vec<Stuck>,
    /// What it shoots (weapons only).
    #[serde(default)]
    pub weapon: Option<Weapon>,
    /// What it is out of its socket (bots only).
    #[serde(default)]
    pub bot: Option<Bot>,
    /// What a throw looks like, in order (throwables only).
    #[serde(default)]
    pub fx: Vec<Fx>,
}

impl Item {
    /// The throw it makes, if it is thrown: range, arc and blast.
    pub fn throw(&self) -> Option<(i32, Arc, Blast)> {
        match self.on_use {
            Some(Use::Throw { range, arc, blast }) => Some((range, arc, blast)),
            _ => None,
        }
    }

    /// Held per copy, since it can carry state or be fitted: it has sockets, is
    /// hardware or chrome that fits a socket, or takes stickers.
    pub fn per_copy(&self) -> bool {
        !self.sockets.is_empty() || self.fits_socket().is_some() || self.takes_stickers()
    }

    /// The kind of socket it goes in: hardware's, socket chrome's, a bot's dock.
    pub fn fits_socket(&self) -> Option<SocketKind> {
        match (&self.hardware, self.implant, &self.bot) {
            (Some(h), _, _) => Some(h.fits),
            (
                None,
                Some(Implant {
                    fits: Fits::Socket(k),
                    ..
                }),
                _,
            ) => Some(k),
            (None, _, Some(b)) => Some(b.docks),
            _ => None,
        }
    }

    /// It can be worn and has a drawing, so stickers can go on it.
    pub fn takes_stickers(&self) -> bool {
        self.slot.is_some() && (self.deck.is_some() || self.look.is_some())
    }

    /// The places its drawing sockets: the deck look's, else the item look's.
    pub fn drawn_sockets(&self) -> &'static [(&'static str, SocketKind)] {
        match (self.deck, self.look) {
            (Some(d), _) => d.look.sockets(),
            (None, Some(l)) => l.sockets(),
            (None, None) => &[],
        }
    }
}

/// What an active ability does when fired.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Active {
    /// Paints the map within `radius` tiles into memory.
    Scan { radius: i32 },
    /// Lights terminals and stairs within `radius` tiles; its glow is an `Fx`.
    Ping { radius: i32 },
    /// Arcs to every net node within `radius` tiles and marks them; all `Fx`.
    Trace { radius: i32 },
    /// Hits one target in sight within `range`: `damage` on a hit, `stun` turns of
    /// stun; the roll uses `skill`; it costs `ap` action points.
    Strike {
        range: i32,
        damage: u8,
        stun: u8,
        skill: SkillId,
        ap: u8,
        /// The attribute the roll uses.
        #[serde(default = "default_net")]
        attribute: Attribute,
        /// Tiles the target is thrown back on a hit.
        #[serde(default)]
        push: u8,
        /// More targets it jumps to on a hit.
        #[serde(default)]
        chain: u8,
    },
    /// No target: the pools refill this turn; `once` a fight.
    Boost { once: bool },
    /// Marks one foe in sight within `range` for the fight: `bonus` on the chance;
    /// it costs `ap` action points.
    Mark { range: i32, bonus: u8, ap: u8 },
}

fn default_net() -> Attribute {
    Attribute::Net
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
    /// A round in flight: a bright head and three trailing cells, `speed` tiles a tick.
    Tracer { colour: (u8, u8, u8), speed: u8 },
    /// A round drawn whole from muzzle to impact: white, then the colour, then ember.
    Streak { colour: (u8, u8, u8) },
    /// The muzzle: the firer's tile lit and the two cells toward the target; `big`
    /// adds the cells above and below the muzzle.
    Flash { big: bool },
    /// A hit: the target lit red, chips flying off it.
    Chips,
    /// A miss: a spark where the round stopped.
    Spark,
    /// A hit: the cells around the target torn into the net's glitch set for a beat.
    Tear,
    /// The damage lifting off the target; `tag` draws it white on a red field.
    Number { tag: bool },
    /// A spent casing out of the muzzle: it hops a cell or two to the side and lies
    /// there, dimming.
    Casing,
    /// A thrown thing crossing the line two tiles a tick, a bounce, then it settles.
    Throw,
    /// A blast: white core, an amber ring to the edge, smoke that thins, a scorch.
    Bloom,
    /// A blast: the footprint inverts white, then orange, debris out, a black scorch.
    Whiteout,
    /// A blast: cyan rings of `≋` out from the centre; the net's blue.
    Shock,
    /// A blast: sickly green `░▒` that fills the footprint, thickens, then thins away.
    Haze,
    /// Where a shoved thing was: an arrow trail along the push, fading.
    Trail,
    /// A ring of `·` pulsing out from the firer to three tiles.
    Pulse { colour: (u8, u8, u8) },
}

impl Fx {
    /// A round's way from muzzle to impact.
    pub fn is_flight(&self) -> bool {
        matches!(self, Fx::Tracer { .. } | Fx::Streak { .. })
    }

    /// Played at the impact: on a hit all but the spark, on a miss the spark alone.
    pub fn is_impact(&self) -> bool {
        matches!(self, Fx::Chips | Fx::Spark | Fx::Tear | Fx::Number { .. })
    }

    /// A blast's shape, played over the whole footprint at the landing tile.
    pub fn is_blast(&self) -> bool {
        matches!(self, Fx::Bloom | Fx::Whiteout | Fx::Shock | Fx::Haze)
    }

    /// Any shape a shot is made of; only weapons and strike abilities play them.
    pub fn is_shot(&self) -> bool {
        self.is_flight()
            || self.is_impact()
            || matches!(
                self,
                Fx::Flash { .. } | Fx::Casing | Fx::Trail | Fx::Pulse { .. }
            )
    }
}

/// A weapon's numbers: what it rolls with, what a round does, how many it fires,
/// how far it reaches, and what one round looks like.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weapon {
    pub skill: SkillId,
    pub damage: u8,
    pub rounds: u8,
    pub range: i32,
    #[serde(default)]
    pub fx: Vec<Fx>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    pub attribute: Attribute,
    pub desc: String,
    /// The attributes it asks for, each 1 to 10; shown on its card, not yet enforced.
    #[serde(default)]
    pub needs: Vec<(Attribute, u8)>,
    /// The skills it wants first; shown on its card, not yet enforced.
    #[serde(default)]
    pub after: Vec<SkillId>,
}

/// Text over a drawing's folded cells: cell x, cell y, the text, its ink and ground.
pub type ArtLabel = (u16, u16, String, (u8, u8, u8), (u8, u8, u8));

/// A skill's art is this many pixels wide and tall, two pixels to a cell stacked.
pub const SKILL_ART_W: usize = 34;
pub const SKILL_ART_H: usize = 20;

/// A skill's drawing on its card, `SKILL_ART_W` by `SKILL_ART_H` pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SkillArt {
    /// Hardware in its own colours: one char per pixel, `.` empty, as a sticker picture.
    Hardware {
        rows: Vec<String>,
        colours: BTreeMap<char, (u8, u8, u8)>,
        #[serde(default)]
        labels: Vec<ArtLabel>,
    },
    /// A neon sign: `#` a tube in the skill's attribute colour, `+` a tube in the accent, `.`
    /// the plate.
    Neon {
        rows: Vec<String>,
        accent: (u8, u8, u8),
    },
}

/// A piece of chrome's art is this many pixels wide and tall, two pixels to a cell stacked.
pub const IMPLANT_ART_W: usize = 30;
pub const IMPLANT_ART_H: usize = 12;

/// A piece of chrome's drawing on its card: one char per pixel, `.` empty, a colour per
/// char, as a sticker picture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImplantArt {
    pub rows: Vec<String>,
    pub colours: BTreeMap<char, (u8, u8, u8)>,
}

/// A talent's icon is this many pixels wide and tall, two pixels to a cell stacked.
pub const TALENT_ART_W: usize = 6;
pub const TALENT_ART_H: usize = 6;

/// A talent's icon: one char per pixel, `.` empty, a colour per char.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TalentArt {
    pub rows: Vec<String>,
    pub colours: BTreeMap<char, (u8, u8, u8)>,
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

/// A talent in a tree: ranks, each with its boons, behind an optional prerequisite on a
/// lower tier of the same tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Talent {
    pub id: TalentId,
    pub name: String,
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

/// Slots in every tier of a tree.
pub const TREE_COLS: usize = 4;
/// Tiers a tree has at most, so the board fits the smallest deck.
pub const TREE_TIERS: usize = 5;

/// A tree of talents under one attribute: tiers from the first, each a row of
/// `TREE_COLS` slots, `None` where a slot is empty.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    pub attribute: Attribute,
    pub name: String,
    pub line: String,
    pub tiers: Vec<Vec<Option<TalentId>>>,
}

impl Tree {
    /// The talent in a slot, if the slot holds one.
    pub fn slot(&self, tier: usize, col: usize) -> Option<&TalentId> {
        self.tiers.get(tier)?.get(col)?.as_ref()
    }

    /// Every talent with its tier and column, tier by tier, left to right.
    pub fn talents(&self) -> impl Iterator<Item = (usize, usize, &TalentId)> {
        self.tiers.iter().enumerate().flat_map(|(tier, row)| {
            row.iter()
                .enumerate()
                .filter_map(move |(col, id)| id.as_ref().map(|id| (tier, col, id)))
        })
    }
}

/// The slots, as `(tier, col)`, a trace from a prerequisite at `from` to a talent at `to`
/// passes through: sideways along the prerequisite's tier to the talent's column, then
/// down that column to the tier above the talent.
pub fn trace_slots(from: (usize, usize), to: (usize, usize)) -> Vec<(usize, usize)> {
    let (ft, fc) = from;
    let (tt, tc) = to;
    let mut out = Vec::new();
    let mut col = fc;
    while col != tc {
        col = if tc > col { col + 1 } else { col - 1 };
        out.push((ft, col));
    }
    out.extend((ft + 1..tt).map(|tier| (tier, tc)));
    out
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
    /// The steps the workbench's process sheet numbers in its box: up to four lines of
    /// at most 16 characters. A sheet drawn exploded shows none.
    #[serde(default)]
    pub steps: Vec<String>,
}

/// Where you were made. Cheap attributes cost one less per point, dear ones one more.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Origin {
    pub id: OriginId,
    pub name: String,
    pub line: String,
    pub desc: String,
    pub cheap: Vec<Attribute>,
    pub dear: Vec<Attribute>,
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
    pub cheap: Vec<Attribute>,
    pub dear: Vec<Attribute>,
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

/// SP comes in steps of this: every price and the creation budget is a multiple of it.
pub const SP_STEP: u32 = 50;

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
    pub attributes: Attributes,
    pub categories: Vec<CategoryId>,
    pub skills: Vec<SkillId>,
    /// Extra kit on top of the profession's.
    #[serde(default)]
    pub kit: Vec<(ItemId, u32)>,
}

/// The numbers every new character is built under.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creation {
    pub attribute_base: u8,
    pub attribute_cap: u8,
    pub attribute_points: u32,
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
pub struct Attributes {
    pub body: u8,
    pub reflex: u8,
    pub tech: u8,
    pub cool: u8,
    pub net: u8,
}

impl Attributes {
    pub fn get(&self, attribute: Attribute) -> u8 {
        match attribute {
            Attribute::Body => self.body,
            Attribute::Reflex => self.reflex,
            Attribute::Tech => self.tech,
            Attribute::Cool => self.cool,
            Attribute::Net => self.net,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_op_has_a_past_tense() {
        assert_eq!(Op::Crack.done_label(), "cracked");
        assert_eq!(Op::Spoof.done_label(), "spoofed");
        assert_eq!(Op::Blind.done_label(), "blinded");
        assert_eq!(Op::Copy.done_label(), "copied");
        assert_eq!(Op::Backdoor.done_label(), "planted");
        assert_eq!(Op::Attack.done_label(), "hit");
        assert_eq!(Op::Shield.done_label(), "raised");
        assert_eq!(Op::Mask.done_label(), "raised");
        assert!(Op::ALL.iter().all(|op| !op.done_label().is_empty()));
    }

    #[test]
    fn a_bay_reads_from_its_kind() {
        assert_eq!(SocketKind::Bay.label(), "bay");
        assert_eq!(SocketKind::Bay.takes(), "drones");
        assert_eq!(SocketKind::Bay.fits(), "bay");
        assert!(!SocketKind::Bay.needs_a_doc());
        assert!(SocketKind::Hand.needs_a_doc() && SocketKind::Forearm.needs_a_doc());
        assert!(!SocketKind::Deck.needs_a_doc());
    }

    #[test]
    fn sites_in_ledger_order() {
        assert_eq!(Site::ALL.len(), 11);
        assert_eq!(Site::LeftArm.label(), "L ARM");
        assert_eq!(Site::LeftArm.lower(), "l arm");
        assert_eq!(Site::RightArm.long(), "RIGHT ARM");
        let groups: Vec<&str> = Site::ALL.iter().map(|s| s.group()).collect();
        let count = |g: &str| groups.iter().filter(|h| **h == g).count();
        assert_eq!((count("head"), count("body"), count("limbs")), (4, 4, 3));
        let rank = |g: &&str| ["head", "body", "limbs"].iter().position(|h| h == g);
        assert!(groups.is_sorted_by_key(rank), "{groups:?}");
        assert_eq!(Site::LeftArm.fits(), Fits::Arm);
        assert_eq!(Site::RightArm.fits(), Fits::Arm);
        assert_eq!(Fits::Arm.label(), "arm");
        assert_eq!(Fits::Socket(SocketKind::Forearm).label(), "forearm socket");
        assert_eq!(SocketKind::Hand.takes(), "hand chrome");
        assert_eq!(SocketKind::Forearm.label(), "forearm");
        assert_eq!(Slot::ALL.len(), 7);
    }
}
