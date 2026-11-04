//! The city: strata, districts, buildings, connectors.

use crate::geom::{Grid, Pos, Rect, Size};
use crate::ids::{BuildingId, DistrictId};
use crate::net::Net;
use crate::params::{GenParams, SizePreset};
use crate::tile::{Terrain, Tile};
use serde::{Deserialize, Serialize};

/// A vertical level of the city. `Net` is reserved for the cyberspace projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StratumKind {
    Undercity,
    Street,
    Skyways,
    Heights,
    Net,
}

impl StratumKind {
    pub const PHYSICAL: [StratumKind; 4] = [
        StratumKind::Undercity,
        StratumKind::Street,
        StratumKind::Skyways,
        StratumKind::Heights,
    ];

    /// Every stratum a finished city has, bottom first, Net last.
    pub const ALL: [StratumKind; 5] = [
        StratumKind::Undercity,
        StratumKind::Street,
        StratumKind::Skyways,
        StratumKind::Heights,
        StratumKind::Net,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StratumKind::Undercity => "Undercity",
            StratumKind::Street => "Street",
            StratumKind::Skyways => "Skyways",
            StratumKind::Heights => "Heights",
            StratumKind::Net => "Net",
        }
    }

    /// Position in the physical stack, bottom first.
    pub fn index(self) -> usize {
        match self {
            StratumKind::Undercity => 0,
            StratumKind::Street => 1,
            StratumKind::Skyways => 2,
            StratumKind::Heights => 3,
            StratumKind::Net => 4,
        }
    }

    pub fn from_index(i: usize) -> Option<StratumKind> {
        match i {
            0 => Some(StratumKind::Undercity),
            1 => Some(StratumKind::Street),
            2 => Some(StratumKind::Skyways),
            3 => Some(StratumKind::Heights),
            4 => Some(StratumKind::Net),
            _ => None,
        }
    }

    pub fn above(self) -> Option<StratumKind> {
        match self {
            StratumKind::Undercity => Some(StratumKind::Street),
            StratumKind::Street => Some(StratumKind::Skyways),
            StratumKind::Skyways => Some(StratumKind::Heights),
            _ => None,
        }
    }

    pub fn below(self) -> Option<StratumKind> {
        match self {
            StratumKind::Street => Some(StratumKind::Undercity),
            StratumKind::Skyways => Some(StratumKind::Street),
            StratumKind::Heights => Some(StratumKind::Skyways),
            _ => None,
        }
    }
}

/// One full tile map plus its baked emitters; ambient comes from the clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stratum {
    pub kind: StratumKind,
    pub tiles: Grid<Tile>,
    /// Light from fixtures that run all day: neon, holo ads, helipads, windows, the
    /// Net's grid. 0 dark .. 255 bright, no ambient.
    pub steady: Grid<u8>,
    /// Light from street lamps, which switch off by day. No ambient.
    pub lamps: Grid<u8>,
    /// Districts whose street lamps are cut for now (a lamp grid's lights out). Only for
    /// the session: never saved.
    #[serde(skip)]
    pub dark: std::collections::BTreeSet<DistrictId>,
}

impl Stratum {
    pub fn new(kind: StratumKind, size: Size) -> Self {
        Stratum {
            kind,
            tiles: Grid::filled(size, Tile::VOID),
            steady: Grid::filled(size, 0),
            lamps: Grid::filled(size, 0),
            dark: std::collections::BTreeSet::new(),
        }
    }

    /// Whether the lamps light the tile at `minute`: they burn by the clock, and not in
    /// a district whose grid is cut.
    pub fn lamps_lit(&self, p: Pos, minute: u16) -> bool {
        crate::lighting::lamps_on_at(self.kind, minute)
            && (self.dark.is_empty()
                || !self
                    .tiles
                    .get(p)
                    .and_then(|t| t.district)
                    .is_some_and(|d| self.dark.contains(&d)))
    }

    /// Light the tile throws on its own at `minute`: steady fixtures plus lamps while on.
    pub fn emitted(&self, p: Pos, minute: u16) -> u8 {
        let steady = self.steady.get(p).copied().unwrap_or(0);
        let lamps = if self.lamps_lit(p, minute) {
            self.lamps.get(p).copied().unwrap_or(0)
        } else {
            0
        };
        steady.saturating_add(lamps)
    }

    /// The light on a tile at `minute`: ambient for the stratum plus what it emits.
    pub fn light_at(&self, p: Pos, minute: u16) -> u8 {
        crate::lighting::ambient(self.kind, minute).saturating_add(self.emitted(p, minute))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Theme {
    CorpCore,
    NightMarket,
    Megablocks,
    CombatZone,
    Docks,
    Industrial,
    NeonStrip,
    OldTown,
}

impl Theme {
    pub const ALL: [Theme; 8] = [
        Theme::CorpCore,
        Theme::NightMarket,
        Theme::Megablocks,
        Theme::CombatZone,
        Theme::Docks,
        Theme::Industrial,
        Theme::NeonStrip,
        Theme::OldTown,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Theme::CorpCore => "Corp Core",
            Theme::NightMarket => "Night Market",
            Theme::Megablocks => "Megablocks",
            Theme::CombatZone => "Combat Zone",
            Theme::Docks => "Docks",
            Theme::Industrial => "Industrial",
            Theme::NeonStrip => "Neon Strip",
            Theme::OldTown => "Old Town",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct District {
    pub id: DistrictId,
    pub name: String,
    pub theme: Theme,
    pub center: Pos,
    pub wealth: f32,
    pub decay: f32,
    pub bounds: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingKind {
    CorpTower,
    Office,
    Megablock,
    Apartment,
    Hotel,
    Shop,
    NoodleBar,
    Bar,
    Club,
    Arcade,
    Pawn,
    Clinic,
    Temple,
    Warehouse,
    Factory,
    Depot,
    Garage,
    Parking,
    Market,
}

impl BuildingKind {
    /// Closed buildings are solid on the city maps and open as interiors; stalls,
    /// small shops and lots stay walk-in.
    pub fn is_closed(self) -> bool {
        !matches!(
            self,
            BuildingKind::NoodleBar
                | BuildingKind::Market
                | BuildingKind::Shop
                | BuildingKind::Pawn
                | BuildingKind::Arcade
                | BuildingKind::Parking
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            BuildingKind::CorpTower => "corp tower",
            BuildingKind::Office => "office",
            BuildingKind::Megablock => "megablock",
            BuildingKind::Apartment => "apartment",
            BuildingKind::Hotel => "hotel",
            BuildingKind::Shop => "shop",
            BuildingKind::NoodleBar => "noodle bar",
            BuildingKind::Bar => "bar",
            BuildingKind::Club => "club",
            BuildingKind::Arcade => "arcade",
            BuildingKind::Pawn => "pawn shop",
            BuildingKind::Clinic => "clinic",
            BuildingKind::Temple => "temple",
            BuildingKind::Warehouse => "warehouse",
            BuildingKind::Factory => "factory",
            BuildingKind::Depot => "depot",
            BuildingKind::Garage => "garage",
            BuildingKind::Parking => "parking",
            BuildingKind::Market => "market",
        }
    }

    pub fn is_corp(self) -> bool {
        matches!(
            self,
            BuildingKind::CorpTower | BuildingKind::Office | BuildingKind::Megablock
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeightClass {
    Low,
    Mid,
    Tall,
}

/// Inclusive range of physical strata a building occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StrataSpan {
    pub lo: StratumKind,
    pub hi: StratumKind,
}

impl StrataSpan {
    pub fn from_class(class: HeightClass, basement: bool) -> Self {
        let hi = match class {
            HeightClass::Low => StratumKind::Street,
            HeightClass::Mid => StratumKind::Skyways,
            HeightClass::Tall => StratumKind::Heights,
        };
        let lo = if basement {
            StratumKind::Undercity
        } else {
            StratumKind::Street
        };
        StrataSpan { lo, hi }
    }

    pub fn contains(self, k: StratumKind) -> bool {
        k.index() >= self.lo.index() && k.index() <= self.hi.index()
    }

    pub fn iter(self) -> impl Iterator<Item = StratumKind> {
        (self.lo.index()..=self.hi.index()).filter_map(StratumKind::from_index)
    }

    pub fn len(self) -> usize {
        self.hi.index() - self.lo.index() + 1
    }

    pub fn is_empty(self) -> bool {
        false
    }
}

/// Where a map is: a stratum of the city, or a floor inside a building.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Place {
    Stratum(StratumKind),
    Floor {
        building: BuildingId,
        level: i32,
    },
    /// Inside a building's construct, behind its gate on the Net.
    Construct(BuildingId),
}

impl Place {
    pub fn stratum(self) -> Option<StratumKind> {
        match self {
            Place::Stratum(k) => Some(k),
            Place::Floor { .. } | Place::Construct(_) => None,
        }
    }

    pub fn is_street(self) -> bool {
        self == Place::Stratum(StratumKind::Street)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Building {
    pub id: BuildingId,
    pub kind: BuildingKind,
    pub district: DistrictId,
    /// Footprint at Street level; the same rect on every stratum of `span`.
    pub footprint: Rect,
    pub span: StrataSpan,
    pub name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectorKind {
    Stair,
    Lift,
    Ladder,
    Manhole,
    Jack,
}

impl ConnectorKind {
    pub fn label(self) -> &'static str {
        match self {
            ConnectorKind::Stair => "stairs",
            ConnectorKind::Lift => "lift",
            ConnectorKind::Ladder => "ladder",
            ConnectorKind::Manhole => "manhole",
            ConnectorKind::Jack => "jack",
        }
    }
}

/// A vertical link between strata at one position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    pub kind: ConnectorKind,
    pub pos: Pos,
    /// Served strata, ordered low to high.
    pub serves: Vec<StratumKind>,
    /// Reachable from the street without entering a building.
    pub public: bool,
}

impl Connector {
    /// The strata directly below and above `from` in `serves`, or `None` if not served.
    pub fn serves_from(
        &self,
        from: StratumKind,
    ) -> Option<(Option<StratumKind>, Option<StratumKind>)> {
        let i = self.serves.iter().position(|k| *k == from)?;
        let below = if i > 0 {
            Some(self.serves[i - 1])
        } else {
            None
        };
        let above = self.serves.get(i + 1).copied();
        Some((below, above))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct City {
    pub seed: u64,
    pub params: GenParams,
    pub name: String,
    pub size: Size,
    pub strata: Vec<Stratum>,
    pub districts: Vec<District>,
    pub buildings: Vec<Building>,
    pub connectors: Vec<Connector>,
    /// Centre paths of the arterial roads, in generation order.
    pub arterials: Vec<Vec<Pos>>,
    pub net: Net,
}

impl City {
    /// Four physical strata of `Void`, nothing else.
    pub fn empty(params: GenParams) -> City {
        let size = params.size.size();
        City {
            seed: params.seed,
            name: String::new(),
            size,
            strata: StratumKind::PHYSICAL
                .iter()
                .map(|k| Stratum::new(*k, size))
                .collect(),
            districts: Vec::new(),
            buildings: Vec::new(),
            connectors: Vec::new(),
            arterials: Vec::new(),
            net: Net::default(),
            params,
        }
    }

    /// A flat lot for a fight: a road, a sidewalk around it, a wall around that, and
    /// nothing else. No districts, buildings, connectors or net.
    pub fn lot(seed: u64) -> City {
        let mut city = City::empty(GenParams {
            seed,
            size: SizePreset::Custom { w: 41, h: 19 },
            ..Default::default()
        });
        city.name = "the lot".to_string();
        let street = city
            .stratum_mut(StratumKind::Street)
            .expect("a physical stratum");
        street
            .tiles
            .fill_rect(Rect::new(0, 0, 41, 19), Tile::new(Terrain::Wall));
        street
            .tiles
            .fill_rect(Rect::new(1, 1, 39, 17), Tile::new(Terrain::Sidewalk));
        street
            .tiles
            .fill_rect(Rect::new(2, 2, 37, 15), Tile::new(Terrain::Road));
        city
    }

    /// A lot, not a city: nothing was generated around its road.
    pub fn is_lot(&self) -> bool {
        self.districts.is_empty()
    }

    pub fn stratum(&self, k: StratumKind) -> Option<&Stratum> {
        self.strata.iter().find(|s| s.kind == k)
    }

    pub fn stratum_mut(&mut self, k: StratumKind) -> Option<&mut Stratum> {
        self.strata.iter_mut().find(|s| s.kind == k)
    }

    pub fn tile(&self, k: StratumKind, p: Pos) -> Option<&Tile> {
        self.stratum(k).and_then(|s| s.tiles.get(p))
    }

    pub fn tile_mut(&mut self, k: StratumKind, p: Pos) -> Option<&mut Tile> {
        self.stratum_mut(k).and_then(|s| s.tiles.get_mut(p))
    }

    pub fn walkable(&self, k: StratumKind, p: Pos) -> bool {
        self.tile(k, p).is_some_and(|t| t.is_walkable())
    }

    pub fn connectors_at(&self, p: Pos) -> impl Iterator<Item = &Connector> {
        self.connectors.iter().filter(move |c| c.pos == p)
    }

    /// Panics if the id is unknown; use `district_opt` on a city that may still be generating.
    pub fn district(&self, id: DistrictId) -> &District {
        &self.districts[id.index()]
    }

    pub fn district_opt(&self, id: DistrictId) -> Option<&District> {
        self.districts.get(id.index())
    }

    /// Panics if the id is unknown; use `building_opt` on a city that may still be generating.
    pub fn building(&self, id: BuildingId) -> &Building {
        &self.buildings[id.index()]
    }

    /// A closed building's doors on every stratum it spans: the tiles its interior
    /// joins. Empty for an open building.
    pub fn portals(&self, b: BuildingId) -> Vec<(StratumKind, Pos)> {
        let Some(b) = self.building_opt(b) else {
            return Vec::new();
        };
        if !b.kind.is_closed() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for k in b.span.iter() {
            let tiles: Vec<Pos> = if k == StratumKind::Heights {
                b.footprint.iter().collect()
            } else {
                b.footprint.border().collect()
            };
            for p in tiles {
                if !self
                    .tile(k, p)
                    .is_some_and(|t| t.terrain == crate::tile::Terrain::Door)
                {
                    continue;
                }
                // A door the connectivity repair cut through a facade may face the void;
                // only a door onto standable ground counts.
                if k != StratumKind::Heights {
                    let fp = b.footprint;
                    let out = if p.x == fp.x {
                        p.offset(-1, 0)
                    } else if p.x == fp.right() - 1 {
                        p.offset(1, 0)
                    } else if p.y == fp.y {
                        p.offset(0, -1)
                    } else {
                        p.offset(0, 1)
                    };
                    let standable = self
                        .tile(k, out)
                        .is_some_and(|t| crate::tile::Tile::new(t.terrain).is_walkable());
                    if !standable {
                        continue;
                    }
                }
                out.push((k, p));
            }
        }
        out
    }

    pub fn building_opt(&self, id: BuildingId) -> Option<&Building> {
        self.buildings.get(id.index())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{GenParams, SizePreset};
    use crate::tile::Terrain;

    #[test]
    fn the_lot_is_a_walled_road_with_nothing_else() {
        let lot = City::lot(1);
        assert!(lot.validate().is_ok());
        let street = StratumKind::Street;
        let terrain = |x, y| lot.tile(street, Pos::new(x, y)).unwrap().terrain;
        assert_eq!(terrain(2, 2), Terrain::Road);
        assert_eq!(terrain(20, 9), Terrain::Road);
        assert_eq!(terrain(37, 15), Terrain::Road);
        assert_eq!(terrain(1, 1), Terrain::Sidewalk);
        assert_eq!(terrain(39, 17), Terrain::Sidewalk);
        assert_eq!(terrain(0, 0), Terrain::Wall);
        assert_eq!(terrain(40, 18), Terrain::Wall);
        assert_eq!(terrain(0, 8), Terrain::Wall);
        assert!(lot.walkable(street, Pos::new(6, 8)));
        assert!(lot.walkable(street, Pos::new(18, 8)));
        assert!(!lot.walkable(street, Pos::new(0, 8)));
        assert!(lot.districts.is_empty() && lot.buildings.is_empty() && lot.connectors.is_empty());
        assert!(lot.is_lot());
        assert_eq!(lot.name, "the lot");
        assert_eq!(lot.size, Size { w: 41, h: 19 });
        let city = crate::Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        assert!(!city.is_lot());
    }

    #[test]
    fn span_iter_and_class() {
        let s = StrataSpan::from_class(HeightClass::Tall, true);
        assert_eq!(s.iter().collect::<Vec<_>>(), StratumKind::PHYSICAL.to_vec());
        let s = StrataSpan::from_class(HeightClass::Mid, false);
        assert_eq!(
            s.iter().collect::<Vec<_>>(),
            vec![StratumKind::Street, StratumKind::Skyways]
        );
        assert!(s.contains(StratumKind::Skyways));
        assert!(!s.contains(StratumKind::Heights));
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn city_empty_has_four_void_strata() {
        let c = City::empty(GenParams {
            size: SizePreset::Custom { w: 10, h: 5 },
            ..GenParams::default()
        });
        assert_eq!(c.strata.len(), 4);
        assert_eq!(
            c.stratum(StratumKind::Heights).unwrap().tiles.size(),
            Size { w: 10, h: 5 }
        );
        assert_eq!(
            c.tile(StratumKind::Street, Pos::new(3, 3)).unwrap().terrain,
            Terrain::Void
        );
        assert!(c.tile(StratumKind::Street, Pos::new(10, 3)).is_none());
        assert!(!c.walkable(StratumKind::Street, Pos::new(3, 3)));
    }

    #[test]
    fn connector_neighbors() {
        let c = Connector {
            kind: ConnectorKind::Lift,
            pos: Pos::new(0, 0),
            serves: vec![
                StratumKind::Street,
                StratumKind::Skyways,
                StratumKind::Heights,
            ],
            public: false,
        };
        assert_eq!(
            c.serves_from(StratumKind::Skyways),
            Some((Some(StratumKind::Street), Some(StratumKind::Heights)))
        );
        assert_eq!(
            c.serves_from(StratumKind::Heights),
            Some((Some(StratumKind::Skyways), None))
        );
        assert_eq!(c.serves_from(StratumKind::Undercity), None);
    }
}
