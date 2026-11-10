//! Venue types as data: `flavor/sites.ron`, its size table and its types.
//!
//! Struct names in the RON are its labels (`Type(`, `Band(`), so each struct here is
//! renamed to its label.

use crate::geom::Dir;
use crate::interior::{LinkKind, Role, RoomKind};
use crate::tile::Furniture;
use crate::world::{BuildingKind, StratumKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::OnceLock;

const BUILTIN: &str = include_str!("../../../../../flavor/sites.ron");

/// A mission's size: how many floors it keeps, the locks to its objective, its guards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MissionSize {
    Small,
    Medium,
    Large,
}

/// A way past a lock, as the data names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Means {
    Card,
    Code,
    Keyjack,
    Net,
    Force,
    Talk,
    Crawl,
}

/// A set of `Means`, one bit each, as a layout holds them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ways(pub u8);

impl Ways {
    pub const CARD: Ways = Ways(1);
    pub const CODE: Ways = Ways(1 << 1);
    pub const KEYJACK: Ways = Ways(1 << 2);
    pub const NET: Ways = Ways(1 << 3);
    pub const FORCE: Ways = Ways(1 << 4);
    pub const TALK: Ways = Ways(1 << 5);
    pub const CRAWL: Ways = Ways(1 << 6);

    pub fn of(means: &[Means]) -> Ways {
        means.iter().fold(Ways::default(), |w, m| {
            w | match m {
                Means::Card => Ways::CARD,
                Means::Code => Ways::CODE,
                Means::Keyjack => Ways::KEYJACK,
                Means::Net => Ways::NET,
                Means::Force => Ways::FORCE,
                Means::Talk => Ways::TALK,
                Means::Crawl => Ways::CRAWL,
            }
        })
    }

    /// Whether this set holds any of `w`.
    pub fn has(self, w: Ways) -> bool {
        self.0 & w.0 != 0
    }
}

impl std::ops::BitOr for Ways {
    type Output = Ways;

    fn bitor(self, other: Ways) -> Ways {
        Ways(self.0 | other.0)
    }
}

/// What holds a lock shut: a card reader, a keypad, or a crawl's grille.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LockKind {
    Card,
    Keypad,
    Grille,
}

/// A set piece of fixed shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PieceKind {
    /// A car park's ramp up to the street.
    Ramp,
    /// A platform and its track along the level's far side.
    Track,
    /// A line of turnstiles across a level's first border.
    GateLine,
}

/// How a level's corridor runs; `Any` leaves it to the level's seed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Spine {
    #[default]
    Any,
    Straight,
    Bent,
    Ring,
    Hall,
}

/// Which way a link carries you.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LinkDir {
    #[default]
    Both,
    Up,
    Down,
}

/// Where you can get off a link: at every stop, or only at its first and last.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Alight {
    #[default]
    All,
    Ends,
}

/// The levels a link stops on: all, those of some roles, or by number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Serves {
    All,
    Roles(Vec<String>),
    Levels(Vec<i32>),
}

/// `flavor/sites.ron`: the size table and the types.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Sites", deny_unknown_fields)]
pub struct VenuePack {
    /// The Tech a crawl's grille wants.
    pub crawl_tech: u8,
    pub sizes: Vec<SizeRule>,
    pub types: Vec<VenueType>,
}

/// What a mission of one size asks of a site.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Size", deny_unknown_fields)]
pub struct SizeRule {
    pub size: MissionSize,
    /// Floors kept, fewest and most.
    pub floors: (u8, u8),
    /// Locks between a way in and the objective.
    pub locks: (u8, u8),
    /// Ways in, at least.
    pub ways_in: u8,
    pub guards: (u16, u16),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Type", deny_unknown_fields)]
pub struct VenueType {
    pub id: String,
    pub name: String,
    /// The building kinds laid out as this type.
    pub builds: Vec<BuildingKind>,
    /// The type's own frame, for a site no building gives one.
    pub frame: OwnFrame,
    /// Public first; a zone's rank is its index.
    pub zones: Vec<String>,
    pub borders: Vec<BorderDef>,
    pub levels: LevelRoles,
    pub rooms: Vec<RoomDef>,
    #[serde(default)]
    pub pieces: Vec<PieceDef>,
    pub links: Vec<LinkDef>,
    #[serde(default)]
    pub crawls_in: Vec<CrawlInDef>,
    pub objectives: Vec<ObjectiveDef>,
    pub security: Vec<SecurityDef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Frame", deny_unknown_fields)]
pub struct OwnFrame {
    pub kind: BuildingKind,
    pub footprint: (i32, i32),
    /// The top level.
    pub top: i32,
    /// Level -1 reaches the Undercity.
    pub undercity: bool,
    /// The top level has a door onto the roof.
    pub roof: bool,
    /// The level of the Skyways door.
    #[serde(default)]
    pub bridge: Option<i32>,
    pub doors: Vec<OwnDoor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Door", deny_unknown_fields)]
pub struct OwnDoor {
    pub stratum: StratumKind,
    pub side: Dir,
    /// Footprint tiles along that side.
    pub at: i32,
}

/// The lock between two zones (outer, inner) and the ways past it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Border", deny_unknown_fields)]
pub struct BorderDef {
    pub zones: (String, String),
    pub lock: LockKind,
    /// Locked in the plain building, not only in a mission.
    #[serde(default)]
    pub always: bool,
    /// Opens without a key from the inner side.
    #[serde(default)]
    pub one_way: bool,
    pub ways: Vec<Means>,
}

/// The type's levels by role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Levels", deny_unknown_fields)]
pub struct LevelRoles {
    /// From -1 down.
    #[serde(default)]
    pub below: Vec<LevelDef>,
    pub ground: LevelDef,
    /// Repeated to fill the floors between the ground and the top roles.
    #[serde(default)]
    pub middle: Option<LevelDef>,
    /// The highest floors, lowest first.
    #[serde(default)]
    pub top: Vec<LevelDef>,
}

fn full() -> (u8, u8) {
    (100, 100)
}

fn one() -> u8 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Level", deny_unknown_fields)]
pub struct LevelDef {
    /// The role's name: "guest floor".
    pub name: String,
    pub bands: Vec<BandDef>,
    #[serde(default)]
    pub pieces: Vec<String>,
    /// Percent of the frame's floor, width and height; over 100 only below -1.
    #[serde(default = "full")]
    pub scale: (u8, u8),
    #[serde(default)]
    pub spine: Spine,
}

/// A strip of the level across its depth, front to back, in one zone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Band", deny_unknown_fields)]
pub struct BandDef {
    pub zone: String,
    /// Percent of the level's depth.
    pub share: u8,
    pub rooms: Vec<String>,
    /// The room repeated to fill what the listed rooms leave.
    #[serde(default)]
    pub fill: Option<String>,
    /// Doors through its border with the band in front.
    #[serde(default = "one")]
    pub doors: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Room", deny_unknown_fields)]
pub struct RoomDef {
    pub name: String,
    pub kind: RoomKind,
    /// The smallest and the largest, width x height, either way round: width across the
    /// level and height front to back, or turned when only that fits.
    pub size: ((i32, i32), (i32, i32)),
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub furniture: Vec<Furniture>,
    #[serde(default)]
    pub stations: Vec<Role>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Piece", deny_unknown_fields)]
pub struct PieceDef {
    pub name: String,
    pub kind: PieceKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Link", deny_unknown_fields)]
pub struct LinkDef {
    pub name: String,
    pub kind: LinkKind,
    pub zone: String,
    pub serves: Serves,
    /// (level role, zone): a lift's card there.
    #[serde(default)]
    pub cards: Vec<(String, String)>,
    #[serde(default)]
    pub dir: LinkDir,
    #[serde(default)]
    pub alight: Alight,
}

/// A crawl into the site from a stratum, coming out in a zone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Crawl", deny_unknown_fields)]
pub struct CrawlInDef {
    pub stratum: StratumKind,
    pub zone: String,
}

/// Where an objective can go: a room, on a level of a role when it names one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Objective", deny_unknown_fields)]
pub struct ObjectiveDef {
    pub id: String,
    pub name: String,
    pub room: String,
    #[serde(default)]
    pub level: Option<String>,
}

/// The mission layer for one size.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "Security", deny_unknown_fields)]
pub struct SecurityDef {
    pub size: MissionSize,
    /// Zone weights for where guards stand.
    pub guards: Vec<(String, u8)>,
    pub cameras: (u8, u8),
    pub turrets: (u8, u8),
    #[serde(default)]
    pub turret_zones: Vec<String>,
    pub containers: (u8, u8),
}

impl VenuePack {
    /// Parses a pack and checks it.
    pub fn parse(text: &str) -> Result<VenuePack, String> {
        let pack: VenuePack = ron::from_str(text).map_err(|e| format!("sites: {e}"))?;
        pack.check()?;
        Ok(pack)
    }

    /// Whether the pack is whole: a size rule for every size, every type whole, no id
    /// on two types and no building kind laid out by two types.
    pub fn check(&self) -> Result<(), String> {
        for size in [MissionSize::Small, MissionSize::Medium, MissionSize::Large] {
            if !self.sizes.iter().any(|r| r.size == size) {
                return Err(format!("sites: no size rule for {size:?}"));
            }
        }
        for (i, t) in self.types.iter().enumerate() {
            t.check(&self.sizes)?;
            let earlier = &self.types[..i];
            if earlier.iter().any(|u| u.id == t.id) {
                return Err(format!("sites: two types have the id `{}`", t.id));
            }
            for k in &t.builds {
                if let Some(u) = earlier.iter().find(|u| u.builds.contains(k)) {
                    return Err(format!("sites: `{}` and `{}` both build {k:?}", u.id, t.id));
                }
            }
        }
        Ok(())
    }

    /// `flavor/sites.ron`, parsed once.
    pub fn builtin() -> &'static VenuePack {
        static PACK: OnceLock<VenuePack> = OnceLock::new();
        PACK.get_or_init(|| {
            VenuePack::parse(BUILTIN).unwrap_or_else(|e| panic!("flavor/sites.ron: {e}"))
        })
    }

    pub fn by_id(&self, id: &str) -> Option<&VenueType> {
        self.types.iter().find(|t| t.id == id)
    }

    /// The type that lays out buildings of `kind`.
    pub fn for_kind(&self, kind: BuildingKind) -> Option<&VenueType> {
        self.types.iter().find(|t| t.builds.contains(&kind))
    }

    /// The rule for `size`; `parse` makes sure every size has one.
    pub fn size(&self, size: MissionSize) -> &SizeRule {
        self.sizes
            .iter()
            .find(|r| r.size == size)
            .expect("a parsed pack has a rule for every size")
    }
}

impl VenueType {
    /// Parses one `Type(...)`; `check` says whether it is whole.
    pub fn parse(text: &str) -> Result<VenueType, String> {
        ron::from_str(text).map_err(|e| format!("site type: {e}"))
    }

    pub fn zone(&self, name: &str) -> Option<u8> {
        self.zones.iter().position(|z| z == name).map(|i| i as u8)
    }

    pub fn room(&self, name: &str) -> Option<&RoomDef> {
        self.rooms.iter().find(|r| r.name == name)
    }

    /// The border between the zones ranked `a` and `b`, given in either order: a level
    /// can put the deeper zone in front.
    pub fn border(&self, a: u8, b: u8) -> Option<&BorderDef> {
        let (outer, inner) = (a.min(b), a.max(b));
        self.borders
            .iter()
            .find(|d| self.zone(&d.zones.0) == Some(outer) && self.zone(&d.zones.1) == Some(inner))
    }

    /// The level role named `name`.
    pub fn role(&self, name: &str) -> Option<&LevelDef> {
        self.roles().find(|d| d.name == name)
    }

    /// Every role, lowest first.
    fn roles(&self) -> impl Iterator<Item = &LevelDef> {
        let l = &self.levels;
        l.below
            .iter()
            .rev()
            .chain(std::iter::once(&l.ground))
            .chain(l.middle.iter())
            .chain(l.top.iter())
    }

    /// Whether the type is whole: everything it names exists, every border can be got
    /// past more than one way by ways that fit its lock, its frame's doors stand on a
    /// side between its corners, no two level roles share a name, neighbouring bands of
    /// two zones have a border between them, every link joins two levels or more, every
    /// objective has a room, the levels' shares and scales hold, and every size has its
    /// security.
    pub fn check(&self, sizes: &[SizeRule]) -> Result<(), String> {
        let id = &self.id;
        let zone = |z: &str, what: &str| -> Result<u8, String> {
            self.zone(z)
                .ok_or_else(|| format!("{id}: {what} names no zone `{z}`"))
        };
        let room = |r: &str, what: &str| -> Result<&RoomDef, String> {
            self.room(r)
                .ok_or_else(|| format!("{id}: {what} names no room `{r}`"))
        };
        let role = |r: &str, what: &str| -> Result<(), String> {
            self.role(r)
                .map(|_| ())
                .ok_or_else(|| format!("{id}: {what} names no level `{r}`"))
        };

        if self.zones.len() < 2 {
            return Err(format!("{id}: a type needs two zones or more"));
        }

        for b in &self.borders {
            let (a, z) = (&b.zones.0, &b.zones.1);
            let what = format!("the border {a} to {z}");
            if zone(a, &what)? >= zone(z, &what)? {
                return Err(format!("{id}: {what} runs from a deeper zone out"));
            }
            match b.ways.len() {
                0 => return Err(format!("{id}: {what} has no way past")),
                1 => return Err(format!("{id}: {what} has one way past")),
                _ => {}
            }
            let has = |m| b.ways.contains(&m);
            let fits = match b.lock {
                LockKind::Card => has(Means::Card) && !has(Means::Code) && !has(Means::Keyjack),
                LockKind::Keypad => has(Means::Code),
                LockKind::Grille => {
                    return Err(format!("{id}: {what} is a grille, which no border is"));
                }
            };
            if !fits {
                return Err(format!(
                    "{id}: {what}'s ways {:?} don't fit its {:?} lock",
                    b.ways, b.lock
                ));
            }
        }

        for r in &self.rooms {
            let ((w0, h0), (w1, h1)) = r.size;
            if w0 > w1 || h0 > h1 {
                return Err(format!(
                    "{id}: room `{}`'s size {:?} runs from larger to smaller",
                    r.name, r.size
                ));
            }
        }

        let (fw, fh) = self.frame.footprint;
        for d in &self.frame.doors {
            let len = match d.side {
                Dir::N | Dir::S => fw,
                Dir::E | Dir::W => fh,
                _ => 0,
            };
            if !(1..len - 1).contains(&d.at) {
                return Err(format!(
                    "{id}: the frame's {:?} door at {} on {:?} is not on a side between its corners",
                    d.stratum, d.at, d.side
                ));
            }
        }

        let mut names = BTreeSet::new();
        if let Some(d) = self.roles().find(|d| !names.insert(d.name.as_str())) {
            return Err(format!("{id}: two levels are named `{}`", d.name));
        }

        // Each role, and whether it lies below -1.
        let l = &self.levels;
        let deep = l.below.iter().skip(1).map(|d| (d, true));
        let roles = (l.below.first().into_iter())
            .chain(std::iter::once(&l.ground))
            .chain(l.middle.iter())
            .chain(l.top.iter())
            .map(|d| (d, false))
            .chain(deep);
        for (d, deep) in roles {
            let what = format!("level `{}`", d.name);
            for b in &d.bands {
                zone(&b.zone, &what)?;
                for r in b.rooms.iter().chain(b.fill.iter()) {
                    room(r, &what)?;
                }
            }
            for pair in d.bands.windows(2) {
                let (a, z) = (&pair[0].zone, &pair[1].zone);
                let (ra, rz) = (zone(a, &what)?, zone(z, &what)?);
                if ra != rz && self.border(ra, rz).is_none() {
                    return Err(format!(
                        "{id}: {what} has no border between its bands `{a}` and `{z}`"
                    ));
                }
            }
            for p in &d.pieces {
                if !self.pieces.iter().any(|x| &x.name == p) {
                    return Err(format!("{id}: {what} names no piece `{p}`"));
                }
            }
            let share: u32 = d.bands.iter().map(|b| u32::from(b.share)).sum();
            if share != 100 {
                return Err(format!("{id}: {what}'s bands share {share}, not 100"));
            }
            let (w, h) = d.scale;
            if deep {
                if !(100..=200).contains(&w) || !(100..=200).contains(&h) {
                    return Err(format!(
                        "{id}: {what}'s scale ({w}, {h}) is outside 100 to 200"
                    ));
                }
            } else if d.scale != full() {
                return Err(format!(
                    "{id}: {what}'s scale is ({w}, {h}); only a level below -1 takes another"
                ));
            }
        }

        let own = super::levels::roles(self, self.frame.top);
        for link in &self.links {
            let what = format!("the {}", link.name);
            zone(&link.zone, &what)?;
            if let Serves::Roles(names) = &link.serves {
                for n in names {
                    role(n, &what)?;
                }
            }
            for (r, z) in &link.cards {
                role(r, &what)?;
                zone(z, &what)?;
            }
            let served = match &link.serves {
                Serves::All => own.len(),
                Serves::Roles(names) => own.iter().filter(|(_, d)| names.contains(&d.name)).count(),
                Serves::Levels(ls) => own.iter().filter(|(n, _)| ls.contains(n)).count(),
            };
            if served < 2 {
                return Err(format!(
                    "{id}: {what} serves {served} of its own frame's levels, not two or more"
                ));
            }
        }

        for c in &self.crawls_in {
            zone(&c.zone, &format!("the crawl in from {:?}", c.stratum))?;
        }

        for o in &self.objectives {
            let what = format!("objective `{}`", o.id);
            room(&o.room, &what)?;
            if let Some(r) = &o.level {
                role(r, &what)?;
            }
            let holds = |d: &LevelDef| {
                d.bands
                    .iter()
                    .any(|b| b.rooms.contains(&o.room) || b.fill.as_ref() == Some(&o.room))
            };
            let on = self
                .roles()
                .filter(|d| o.level.as_ref().is_none_or(|r| &d.name == r))
                .any(holds);
            if !on {
                return Err(format!(
                    "{id}: {what}'s room `{}` is on no level{}",
                    o.room,
                    o.level
                        .as_ref()
                        .map_or(String::new(), |r| format!(" `{r}`"))
                ));
            }
        }

        for rule in sizes {
            if !self.security.iter().any(|s| s.size == rule.size) {
                return Err(format!("{id}: no security for {:?}", rule.size));
            }
        }
        for s in &self.security {
            let what = format!("the {:?} security", s.size);
            for (z, _) in &s.guards {
                zone(z, &what)?;
            }
            for z in &s.turret_zones {
                zone(z, &what)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interior::venue::testutil::test_type;

    fn sizes() -> &'static [SizeRule] {
        &VenuePack::builtin().sizes
    }

    #[test]
    fn the_builtin_pack_loads_with_the_size_table() {
        let p = VenuePack::builtin();
        assert!(p.types.len() >= 3);
        assert_eq!(p.crawl_tech, 6);
        let r = |s| {
            let r = p.size(s);
            (r.floors, r.locks, r.ways_in, r.guards)
        };
        assert_eq!(r(MissionSize::Small), ((2, 3), (2, 2), 2, (6, 10)));
        assert_eq!(r(MissionSize::Medium), ((4, 6), (3, 3), 3, (12, 20)));
        assert_eq!(r(MissionSize::Large), ((6, 9), (4, 5), 4, (25, 40)));
        for t in &p.types {
            t.check(&p.sizes)
                .unwrap_or_else(|e| panic!("{}: {e}", t.id));
        }
    }

    #[test]
    fn a_building_kind_has_at_most_one_type() {
        let p = VenuePack::builtin();
        for t in &p.types {
            for k in &t.builds {
                assert_eq!(
                    p.types.iter().filter(|u| u.builds.contains(k)).count(),
                    1,
                    "{k:?}"
                );
            }
        }
        assert!(
            p.for_kind(BuildingKind::CorpTower).is_none(),
            "towers stay on today's builder"
        );
    }

    #[test]
    fn the_test_type_loads() {
        test_type().check(sizes()).unwrap();
    }

    #[test]
    fn check_finds_what_is_wrong() {
        let bad = |f: &dyn Fn(&mut VenueType)| {
            let mut t = test_type();
            f(&mut t);
            t.check(sizes()).unwrap_err()
        };
        assert!(bad(&|t| t.borders[0].zones.1 = "nowhere".into()).contains("nowhere"));
        assert!(bad(&|t| t.borders[0].ways.truncate(1)).contains("one way past"));
        assert!(bad(&|t| t.borders[0].lock = LockKind::Grille).contains("grille"));
        assert!(bad(&|t| t.objectives[0].room = "nowhere".into()).contains("nowhere"));
        assert!(bad(&|t| t.levels.ground.scale = (150, 100)).contains("scale"));
        assert!(bad(&|t| t.security.truncate(1)).contains("security"));
        assert!(bad(&|t| t.frame.doors[0].at = 0).contains("between its corners"));
        assert!(bad(&|t| t.frame.doors[0].side = Dir::NE).contains("between its corners"));
        let ground = test_type().levels.ground.name;
        assert!(bad(&|t| t.levels.top[0].name = ground.clone()).contains("two levels are named"));
        assert!(
            bad(&|t| {
                t.borders.remove(1);
            })
            .contains("no border between")
        );

        let bad_pack = |f: &dyn Fn(&mut VenuePack)| {
            let mut p = VenuePack::builtin().clone();
            f(&mut p);
            p.check().unwrap_err()
        };
        assert!(
            bad_pack(&|p| p.types.extend([test_type(), test_type()]))
                .contains("two types have the id")
        );
        assert!(
            bad_pack(&|p| {
                let kind = p.types.iter().find_map(|t| t.builds.first().copied());
                let mut t = test_type();
                t.builds.extend(kind);
                p.types.push(t);
            })
            .contains("both build")
        );
        let mut p = VenuePack::builtin().clone();
        p.types.push(test_type());
        p.check().unwrap();
    }

    #[test]
    fn a_border_is_found_from_either_side() {
        let t = test_type();
        for b in &t.borders {
            let (outer, inner) = (t.zone(&b.zones.0).unwrap(), t.zone(&b.zones.1).unwrap());
            assert_eq!(t.border(outer, inner), Some(b));
            assert_eq!(t.border(inner, outer), Some(b), "the pair reversed");
            assert_eq!(t.border(inner, inner), None);
        }
    }

    #[test]
    fn ways_are_a_set_of_means() {
        let w = Ways::of(&[Means::Card, Means::Net]);
        assert_eq!(w, Ways::CARD | Ways::NET);
        assert!(w.has(Ways::NET) && w.has(Ways::NET | Ways::FORCE));
        assert!(!w.has(Ways::CODE | Ways::KEYJACK | Ways::TALK | Ways::CRAWL));
        assert_eq!(Ways::of(&[]), Ways::default());
    }
}
