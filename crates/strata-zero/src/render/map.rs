//! One map to draw and walk: a stratum of the city, a floor inside a building, or the
//! inside of a construct.

use crate::constructs::Constructs;
use crate::interiors::Interiors;
use crate::render::ConnDir;
use citygen::{
    BuildingKind, City, ConnectorKind, Construct, Floor, FloorPlan, Place, Pos, Size, StratumKind,
    Terrain, Theme, Tile,
};

/// What the viewport, the field of view, the walker and the inspector need from a map.
pub trait TileMap {
    fn size(&self) -> Size;
    fn tile(&self, p: Pos) -> Option<&Tile>;
    /// 0 dark .. 255 bright.
    fn light(&self, p: Pos) -> u8;
    /// The theme that colours a tile.
    fn theme(&self, p: Pos) -> Option<Theme>;
    /// The kind of building a tile belongs to, for wall styles.
    fn kind(&self, p: Pos) -> Option<BuildingKind>;
    /// A stair or lift on the tile and which way it goes.
    fn connector(&self, p: Pos) -> Option<ConnDir>;
    fn walkable(&self, p: Pos) -> bool {
        self.tile(p).is_some_and(|t| t.is_walkable())
    }
}

#[derive(Clone, Copy)]
pub struct StratumMap<'a> {
    pub city: &'a City,
    pub stratum: StratumKind,
}

#[derive(Clone, Copy)]
pub struct FloorMap<'a> {
    pub city: &'a City,
    pub plan: &'a FloorPlan,
    pub floor: &'a Floor,
}

/// A stratum or a floor, with what the overlays need from the city around it.
#[derive(Clone, Copy)]
pub enum MapRef<'a> {
    Stratum(StratumMap<'a>),
    Floor(FloorMap<'a>),
    Construct(ConstructMap<'a>),
}

#[derive(Clone, Copy)]
pub struct ConstructMap<'a> {
    pub city: &'a City,
    pub construct: &'a Construct,
}

impl TileMap for ConstructMap<'_> {
    fn size(&self) -> Size {
        self.construct.tiles.size()
    }

    fn tile(&self, p: Pos) -> Option<&Tile> {
        self.construct.tiles.get(p)
    }

    fn light(&self, p: Pos) -> u8 {
        self.construct.light.get(p).copied().unwrap_or(0)
    }

    fn theme(&self, _p: Pos) -> Option<Theme> {
        let b = self.city.building_opt(self.construct.building)?;
        Some(self.city.district(b.district).theme)
    }

    fn kind(&self, _p: Pos) -> Option<BuildingKind> {
        self.city
            .building_opt(self.construct.building)
            .map(|b| b.kind)
    }

    fn connector(&self, _p: Pos) -> Option<ConnDir> {
        None
    }
}

impl<'a> MapRef<'a> {
    pub fn stratum_of(city: &'a City, stratum: StratumKind) -> Self {
        MapRef::Stratum(StratumMap { city, stratum })
    }

    /// The map at a place, if its floor or construct has been generated.
    pub fn for_place(
        city: &'a City,
        interiors: &'a Interiors,
        constructs: &'a Constructs,
        place: Place,
    ) -> Option<Self> {
        match place {
            Place::Stratum(k) => Some(MapRef::stratum_of(city, k)),
            Place::Floor { building, level } => {
                let (plan, floor) = interiors.floor(building, level)?;
                Some(MapRef::Floor(FloorMap { city, plan, floor }))
            }
            Place::Construct(b) => {
                let construct = constructs.get(b)?;
                Some(MapRef::Construct(ConstructMap { city, construct }))
            }
        }
    }

    pub fn place(&self) -> Place {
        match self {
            MapRef::Stratum(m) => Place::Stratum(m.stratum),
            MapRef::Floor(m) => Place::Floor {
                building: m.plan.building,
                level: m.floor.level,
            },
            MapRef::Construct(m) => Place::Construct(m.construct.building),
        }
    }

    pub fn city(&self) -> &'a City {
        match self {
            MapRef::Stratum(m) => m.city,
            MapRef::Floor(m) => m.city,
            MapRef::Construct(m) => m.city,
        }
    }

    pub fn stratum(&self) -> Option<StratumKind> {
        match self {
            MapRef::Stratum(m) => Some(m.stratum),
            MapRef::Floor(_) | MapRef::Construct(_) => None,
        }
    }

    pub fn floor(&self) -> Option<(&'a FloorPlan, &'a Floor)> {
        match self {
            MapRef::Stratum(_) | MapRef::Construct(_) => None,
            MapRef::Floor(m) => Some((m.plan, m.floor)),
        }
    }

    /// What the status line calls this map.
    pub fn label(&self) -> String {
        match self {
            MapRef::Stratum(m) => m.stratum.label().to_string(),
            MapRef::Floor(m) => FloorPlan::label(m.floor.level),
            MapRef::Construct(_) => "construct".to_string(),
        }
    }
}

impl TileMap for StratumMap<'_> {
    fn size(&self) -> Size {
        self.city.size
    }

    fn tile(&self, p: Pos) -> Option<&Tile> {
        self.city.tile(self.stratum, p)
    }

    fn light(&self, p: Pos) -> u8 {
        self.city
            .stratum(self.stratum)
            .and_then(|s| s.light.get(p).copied())
            .unwrap_or(0)
    }

    fn theme(&self, p: Pos) -> Option<Theme> {
        self.tile(p)
            .and_then(|t| t.district)
            .and_then(|d| self.city.district_opt(d))
            .map(|d| d.theme)
    }

    fn kind(&self, p: Pos) -> Option<BuildingKind> {
        self.tile(p)
            .and_then(|t| t.building)
            .and_then(|b| self.city.building_opt(b))
            .map(|b| b.kind)
    }

    fn connector(&self, p: Pos) -> Option<ConnDir> {
        self.city
            .connectors_at(p)
            .filter(|c| c.serves.contains(&self.stratum))
            .find_map(|c| {
                c.serves_from(self.stratum).map(|(below, above)| ConnDir {
                    kind: c.kind,
                    up: above.is_some(),
                    down: below.is_some(),
                })
            })
    }
}

impl TileMap for FloorMap<'_> {
    fn size(&self) -> Size {
        self.floor.tiles.size()
    }

    fn tile(&self, p: Pos) -> Option<&Tile> {
        self.floor.tiles.get(p)
    }

    fn light(&self, p: Pos) -> u8 {
        self.floor.light.get(p).copied().unwrap_or(0)
    }

    fn theme(&self, _p: Pos) -> Option<Theme> {
        Some(self.plan.theme)
    }

    fn kind(&self, _p: Pos) -> Option<BuildingKind> {
        Some(self.plan.kind)
    }

    fn connector(&self, p: Pos) -> Option<ConnDir> {
        let kind = match self.floor.tiles.get(p)?.terrain {
            Terrain::Lift => ConnectorKind::Lift,
            Terrain::Stair => ConnectorKind::Stair,
            _ => return None,
        };
        Some(ConnDir {
            kind,
            up: self.floor.level < self.plan.top,
            down: self.floor.level > self.plan.lowest,
        })
    }
}

impl TileMap for MapRef<'_> {
    fn size(&self) -> Size {
        match self {
            MapRef::Stratum(m) => m.size(),
            MapRef::Floor(m) => m.size(),
            MapRef::Construct(m) => m.size(),
        }
    }

    fn tile(&self, p: Pos) -> Option<&Tile> {
        match self {
            MapRef::Stratum(m) => m.tile(p),
            MapRef::Floor(m) => m.tile(p),
            MapRef::Construct(m) => m.tile(p),
        }
    }

    fn light(&self, p: Pos) -> u8 {
        match self {
            MapRef::Stratum(m) => m.light(p),
            MapRef::Floor(m) => m.light(p),
            MapRef::Construct(m) => m.light(p),
        }
    }

    fn theme(&self, p: Pos) -> Option<Theme> {
        match self {
            MapRef::Stratum(m) => m.theme(p),
            MapRef::Floor(m) => m.theme(p),
            MapRef::Construct(m) => m.theme(p),
        }
    }

    fn kind(&self, p: Pos) -> Option<BuildingKind> {
        match self {
            MapRef::Stratum(m) => m.kind(p),
            MapRef::Floor(m) => m.kind(p),
            MapRef::Construct(m) => m.kind(p),
        }
    }

    fn connector(&self, p: Pos) -> Option<ConnDir> {
        match self {
            MapRef::Stratum(m) => m.connector(p),
            MapRef::Floor(m) => m.connector(p),
            MapRef::Construct(m) => m.connector(p),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Visibility;
    use crate::render::style::StylePack;
    use crate::render::viewport::MapView;
    use crate::walker::Player;
    use citygen::{Dir, Feature, GenParams, Generator, SizePreset};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Widget;

    fn city() -> City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn a_floor_renders_like_a_stratum_and_the_walker_walks_it() {
        let c = city();
        let b = c
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::CorpTower)
            .unwrap();
        let mut interiors = Interiors::default();
        interiors.ensure(&c, b.id, 0).unwrap();
        let (plan, floor) = interiors.floor(b.id, 0).unwrap();
        let map = MapRef::Floor(FloorMap {
            city: &c,
            plan,
            floor,
        });
        assert_eq!(map.size(), plan.size);
        assert!(map.light(floor.stations[0].pos) > 0);
        assert_eq!(
            map.theme(Pos::new(1, 1)),
            Some(c.district(b.district).theme)
        );
        let lift = floor
            .tiles
            .iter()
            .find(|(_, t)| t.terrain == Terrain::Lift)
            .unwrap()
            .0;
        assert!(
            map.connector(lift)
                .is_some_and(|d| d.kind == ConnectorKind::Lift
                    && d.up
                    && d.down == (plan.lowest < 0))
        );
        assert_eq!(
            map.place(),
            Place::Floor {
                building: b.id,
                level: 0
            }
        );
        assert_eq!(map.label(), "L");
        let s = StylePack::load().unwrap();
        let mut t = Terminal::new(TestBackend::new(80, 30)).unwrap();
        t.draw(|f| {
            MapView {
                map: &map,
                origin: Pos::new(0, 0),
                tick: 1,
                style: &s,
                vis: None,
                overlays: &[],
                cursor: None,
            }
            .render(f.area(), f.buffer_mut())
        })
        .unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains('▯'), "doors drawn");
        assert!(text.contains('═'), "the reception counter drawn");
        let mut p = Player::spawn(&c, 1);
        p.place = map.place();
        p.pos = floor.doorstep(&floor.exits[0]);
        p.update_fov(&map, 0);
        assert_eq!(p.vis().get(p.pos), Some(&Visibility::Visible));
        let before = p.pos;
        let moved = [Dir::N, Dir::E, Dir::S, Dir::W]
            .iter()
            .any(|d| p.try_move(&map, *d));
        assert!(moved && p.pos != before);
        let desk = floor
            .tiles
            .iter()
            .find(|(_, t)| matches!(t.feature, Some(Feature::Furniture(_))))
            .unwrap()
            .0;
        assert!(!map.walkable(desk), "nobody walks through furniture");
        let street = MapRef::stratum_of(&c, StratumKind::Street);
        let mass = b.footprint.inset(1).iter().next().unwrap();
        assert!(!street.walkable(mass), "a closed building is solid");
        assert_eq!(street.label(), "Street");
        // Street memory survives the visit inside.
        let mut walker = Player::spawn(&c, 1);
        let seen_outside = walker.vis().count(|v| *v != Visibility::Unseen);
        walker.place = map.place();
        walker.pos = floor.doorstep(&floor.exits[0]);
        walker.update_fov(&map, 0);
        walker.place = Place::Stratum(StratumKind::Street);
        assert_eq!(
            walker.vis().count(|v| *v != Visibility::Unseen),
            seen_outside
        );
        assert!(
            MapRef::for_place(
                &c,
                &interiors,
                &Constructs::default(),
                Place::Floor {
                    building: b.id,
                    level: 7
                }
            )
            .is_none()
        );
    }
}
