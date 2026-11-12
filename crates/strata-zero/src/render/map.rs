//! One map to draw and walk: a stratum of the city, a floor inside a building, or the
//! inside of a construct.

use crate::constructs::Constructs;
use crate::interiors::Interiors;
use crate::outside::stream::{Beyond, GroundMap, Owner};
use crate::render::{ConnDir, Rgb};
use citygen::venue::LockKind;
use citygen::{
    BuildingKind, City, ConnectorKind, Construct, Floor, FloorPlan, LinkKind, Place, Pos, Size,
    StratumKind, Terrain, Theme, Tile,
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
    /// Whether sight and the walker may go to `p` at all: the map's own cells, and on a
    /// map with what lies past its edge, every cell built there.
    fn reaches(&self, p: Pos) -> bool {
        self.size().contains(p)
    }
    /// What lies beyond the map's edge at `p`, drawn dim: a construct floats on the
    /// Net's lattice, a floor sits in the building's mass, a stratum in nothing.
    fn backdrop(&self, _p: Pos) -> Option<Tile> {
        None
    }
    /// Light the tile throws on its own, ambient excluded; 0 off the strata.
    fn emitted(&self, _p: Pos) -> u8 {
        0
    }
    /// Whether the tile is a fixture glowing right now.
    fn emits(&self, _p: Pos) -> bool {
        false
    }
    /// How much sun: 0 at night, 1 by day; 0 wherever there is no sky.
    fn daylight(&self) -> f32 {
        0.0
    }
    /// Whether street lamps burn; always, where there is no clock.
    fn lamps_on(&self) -> bool {
        true
    }
    /// Whether the lamps on a tile burn: by the clock, and not where a lamp grid is cut.
    fn lamps_on_at(&self, _p: Pos) -> bool {
        self.lamps_on()
    }
    /// Whether light past the dark radius is seen here: the physical strata only.
    fn lit_sight(&self) -> bool {
        false
    }
    /// The sky's colour and weight on a sunlit stratum by day; none elsewhere.
    fn sky(&self) -> Option<((u8, u8, u8), f32)> {
        None
    }
    /// The light's colour on the tile, where light has one (an interior floor).
    fn glow(&self, _p: Pos) -> Option<Rgb> {
        None
    }
    /// Whether the door at `p` has a card reader on it: a floor's card lock.
    fn reader(&self, _p: Pos) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
pub struct StratumMap<'a> {
    pub city: &'a City,
    pub stratum: StratumKind,
    /// The minute of the day the stratum is seen at; ambient and lamps follow it.
    pub minute: u16,
    /// What lies past the city's edge, and the cells of it other areas own: none on a
    /// map that ends at the city's edge (`MapRef::beyond` sets it). With it, `tile` may
    /// answer with another area's tile, its building and district ids that area's:
    /// such a map is for walking, sight and drawing, never for reading ids against
    /// `city`.
    pub beyond: Option<&'a Beyond<'a>>,
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
    /// The level of a site's Net you're on; none in any other construct.
    pub level: Option<i32>,
}

impl TileMap for ConstructMap<'_> {
    fn size(&self) -> Size {
        self.construct.tiles.size()
    }

    fn backdrop(&self, p: Pos) -> Option<Tile> {
        let mut t = Tile::new(Terrain::NetGrid);
        t.district = self
            .city
            .building_opt(self.construct.building)
            .map(|b| b.district);
        // The lattice dots of the Net stage, with the hash in the other bits.
        let lattice = u8::from(p.x.rem_euclid(4) == 0 && p.y.rem_euclid(2) == 0);
        t.variant = (citygen::rng::tile_hash(self.city.seed, StratumKind::Net, p) & !1) | lattice;
        Some(t)
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
    /// A stratum at the night the game starts, for tests and screens before a world exists.
    pub fn stratum_of(city: &'a City, stratum: StratumKind) -> Self {
        Self::stratum_at(city, stratum, crate::ui::hud::START_MINUTE_OF_DAY)
    }

    /// A stratum at `minute` of the day.
    pub fn stratum_at(city: &'a City, stratum: StratumKind, minute: u16) -> Self {
        MapRef::Stratum(StratumMap {
            city,
            stratum,
            minute,
            beyond: None,
        })
    }

    /// The street's or the Net's map carried on past the city's edge by `b`; floors,
    /// constructs and the other strata as they are.
    pub fn beyond(self, b: &'a Beyond<'a>) -> MapRef<'a> {
        match self {
            MapRef::Stratum(m) if matches!(m.stratum, StratumKind::Street | StratumKind::Net) => {
                MapRef::Stratum(StratumMap {
                    beyond: Some(b),
                    ..m
                })
            }
            other => other,
        }
    }

    /// The map at a place at `minute` of the day, if its floor or construct has been
    /// generated.
    pub fn for_place(
        city: &'a City,
        interiors: &'a Interiors,
        constructs: &'a Constructs,
        place: Place,
        minute: u16,
    ) -> Option<Self> {
        match place {
            Place::Stratum(k) => Some(MapRef::stratum_at(city, k, minute)),
            Place::Floor { building, level } => {
                let (plan, floor) = interiors.floor(building, level)?;
                Some(MapRef::Floor(FloorMap { city, plan, floor }))
            }
            Place::Construct(b) => {
                let construct = constructs.get(b)?;
                Some(MapRef::Construct(ConstructMap {
                    city,
                    construct,
                    level: None,
                }))
            }
            Place::SiteNet { building, level } => {
                let construct = constructs.site(building)?;
                Some(MapRef::Construct(ConstructMap {
                    city,
                    construct,
                    level: Some(level),
                }))
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
            MapRef::Construct(m) => match m.level {
                Some(level) => Place::SiteNet {
                    building: m.construct.building,
                    level,
                },
                None => Place::Construct(m.construct.building),
            },
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
            MapRef::Construct(m) => match m.level {
                Some(level) => {
                    let role = m.construct.net_level(level).map(|l| l.name.as_str());
                    let at = format!("construct · {}", FloorPlan::label(level));
                    match role {
                        Some(role) => format!("{at} {role}"),
                        None => at,
                    }
                }
                None => "construct".to_string(),
            },
        }
    }
}

impl<'a> StratumMap<'a> {
    /// What the area owning `p` says of it when that isn't this map's city: another
    /// held city's map at its own cell, or the stream's at the world cell. None for
    /// the city's own cells, and on a map with nothing past it.
    fn elsewhere<T>(&self, p: Pos, ask: impl FnOnce(&dyn TileMap, Pos) -> T) -> Option<T> {
        Some(match self.beyond?.elsewhere(p)? {
            Owner::City(city, q) => ask(
                &StratumMap {
                    city,
                    beyond: None,
                    ..*self
                },
                q,
            ),
            Owner::Ground(stream, at) => ask(&self.ground(stream), at),
        })
    }

    /// The stream's layer for this stratum, at this hour.
    fn ground(&self, stream: &'a crate::outside::stream::Stream) -> GroundMap<'a> {
        GroundMap {
            stream,
            layer: self.stratum,
            minute: self.minute,
        }
    }
}

impl TileMap for StratumMap<'_> {
    fn size(&self) -> Size {
        self.city.size
    }

    fn tile(&self, p: Pos) -> Option<&Tile> {
        match self.beyond.and_then(|b| b.elsewhere(p)) {
            None => self.city.tile(self.stratum, p),
            Some(Owner::City(city, q)) => city.tile(self.stratum, q),
            Some(Owner::Ground(stream, at)) => self.ground(stream).cell(at),
        }
    }

    /// As any map, and past the city's edge the Net's void too.
    fn walkable(&self, p: Pos) -> bool {
        let void_walks = self.beyond.is_some() && self.stratum == StratumKind::Net;
        self.tile(p)
            .is_some_and(|t| t.is_walkable() || (void_walks && t.terrain == Terrain::Void))
    }

    /// The city's cells; with what lies past its edge, every cell built.
    fn reaches(&self, p: Pos) -> bool {
        match self.beyond {
            None => self.size().contains(p),
            Some(_) => self.tile(p).is_some(),
        }
    }

    fn light(&self, p: Pos) -> u8 {
        if let Some(v) = self.elsewhere(p, |m, q| m.light(q)) {
            return v;
        }
        self.city
            .stratum(self.stratum)
            .map_or(0, |s| s.light_at(p, self.minute))
    }

    fn emitted(&self, p: Pos) -> u8 {
        if let Some(v) = self.elsewhere(p, |m, q| m.emitted(q)) {
            return v;
        }
        self.city
            .stratum(self.stratum)
            .map_or(0, |s| s.emitted(p, self.minute))
    }

    fn emits(&self, p: Pos) -> bool {
        if let Some(v) = self.elsewhere(p, |m, q| m.emits(q)) {
            return v;
        }
        self.city.tile(self.stratum, p).is_some_and(|t| {
            citygen::lighting::emits(t.terrain, t.feature, self.stratum, self.lamps_on_at(p))
        })
    }

    fn daylight(&self) -> f32 {
        if citygen::lighting::sunlit(self.stratum) {
            citygen::lighting::daylight(self.minute)
        } else {
            0.0
        }
    }

    fn lamps_on(&self) -> bool {
        citygen::lighting::lamps_on_at(self.stratum, self.minute)
    }

    fn lamps_on_at(&self, p: Pos) -> bool {
        if let Some(v) = self.elsewhere(p, |m, q| m.lamps_on_at(q)) {
            return v;
        }
        self.city
            .stratum(self.stratum)
            .map_or_else(|| self.lamps_on(), |s| s.lamps_lit(p, self.minute))
    }

    fn lit_sight(&self) -> bool {
        self.stratum != StratumKind::Net
    }

    fn sky(&self) -> Option<((u8, u8, u8), f32)> {
        citygen::lighting::sky(self.stratum, self.minute)
    }

    fn theme(&self, p: Pos) -> Option<Theme> {
        if let Some(v) = self.elsewhere(p, |m, q| m.theme(q)) {
            return v;
        }
        self.city
            .tile(self.stratum, p)
            .and_then(|t| t.district)
            .and_then(|d| self.city.district_opt(d))
            .map(|d| d.theme)
    }

    fn kind(&self, p: Pos) -> Option<BuildingKind> {
        if let Some(v) = self.elsewhere(p, |m, q| m.kind(q)) {
            return v;
        }
        self.city
            .tile(self.stratum, p)
            .and_then(|t| t.building)
            .and_then(|b| self.city.building_opt(b))
            .map(|b| b.kind)
    }

    fn connector(&self, p: Pos) -> Option<ConnDir> {
        if let Some(v) = self.elsewhere(p, |m, q| m.connector(q)) {
            return v;
        }
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

    fn backdrop(&self, p: Pos) -> Option<Tile> {
        let mut t = Tile::new(Terrain::Mass);
        t.building = Some(self.plan.building);
        t.district = self
            .city
            .building_opt(self.plan.building)
            .map(|b| b.district);
        t.variant = citygen::rng::tile_hash(self.city.seed, StratumKind::Street, p);
        Some(t)
    }

    fn tile(&self, p: Pos) -> Option<&Tile> {
        self.floor.tiles.get(p)
    }

    fn light(&self, p: Pos) -> u8 {
        self.floor.light.get(p).copied().unwrap_or(0)
    }

    fn glow(&self, p: Pos) -> Option<Rgb> {
        self.floor.glow.get(p).map(|&(r, g, b)| Rgb(r, g, b))
    }

    fn reader(&self, p: Pos) -> bool {
        (self.floor.lock_at(p)).is_some_and(|l| l.kind == LockKind::Card)
    }

    fn theme(&self, _p: Pos) -> Option<Theme> {
        Some(self.plan.theme)
    }

    fn kind(&self, _p: Pos) -> Option<BuildingKind> {
        Some(self.plan.kind)
    }

    fn connector(&self, p: Pos) -> Option<ConnDir> {
        // Asked of every tile drawn, every frame: only a landing's terrain can be one.
        if !self.floor.tiles.get(p)?.is_connector() {
            return None;
        }
        let level = self.floor.level;
        let link = self.plan.link_at(level, p)?;
        let kind = match self.plan.links[link].kind {
            LinkKind::Lift => ConnectorKind::Lift,
            LinkKind::Stair | LinkKind::Escalator => ConnectorKind::Stair,
            LinkKind::Ladder | LinkKind::Crawl => ConnectorKind::Ladder,
        };
        Some(ConnDir {
            kind,
            up: self.plan.next_stop(link, level, true).is_some(),
            down: self.plan.next_stop(link, level, false).is_some(),
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

    fn glow(&self, p: Pos) -> Option<Rgb> {
        match self {
            MapRef::Stratum(m) => m.glow(p),
            MapRef::Floor(m) => m.glow(p),
            MapRef::Construct(m) => m.glow(p),
        }
    }

    fn reader(&self, p: Pos) -> bool {
        match self {
            MapRef::Stratum(m) => m.reader(p),
            MapRef::Floor(m) => m.reader(p),
            MapRef::Construct(m) => m.reader(p),
        }
    }

    fn emitted(&self, p: Pos) -> u8 {
        match self {
            MapRef::Stratum(m) => m.emitted(p),
            MapRef::Floor(m) => m.emitted(p),
            MapRef::Construct(m) => m.emitted(p),
        }
    }

    fn emits(&self, p: Pos) -> bool {
        match self {
            MapRef::Stratum(m) => m.emits(p),
            MapRef::Floor(m) => m.emits(p),
            MapRef::Construct(m) => m.emits(p),
        }
    }

    fn daylight(&self) -> f32 {
        match self {
            MapRef::Stratum(m) => m.daylight(),
            MapRef::Floor(m) => m.daylight(),
            MapRef::Construct(m) => m.daylight(),
        }
    }

    fn lamps_on(&self) -> bool {
        match self {
            MapRef::Stratum(m) => m.lamps_on(),
            MapRef::Floor(m) => m.lamps_on(),
            MapRef::Construct(m) => m.lamps_on(),
        }
    }

    fn lamps_on_at(&self, p: Pos) -> bool {
        match self {
            MapRef::Stratum(m) => m.lamps_on_at(p),
            MapRef::Floor(m) => m.lamps_on_at(p),
            MapRef::Construct(m) => m.lamps_on_at(p),
        }
    }

    fn lit_sight(&self) -> bool {
        match self {
            MapRef::Stratum(m) => m.lit_sight(),
            MapRef::Floor(m) => m.lit_sight(),
            MapRef::Construct(m) => m.lit_sight(),
        }
    }

    fn sky(&self) -> Option<((u8, u8, u8), f32)> {
        match self {
            MapRef::Stratum(m) => m.sky(),
            MapRef::Floor(m) => m.sky(),
            MapRef::Construct(m) => m.sky(),
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

    fn walkable(&self, p: Pos) -> bool {
        match self {
            MapRef::Stratum(m) => m.walkable(p),
            MapRef::Floor(m) => m.walkable(p),
            MapRef::Construct(m) => m.walkable(p),
        }
    }

    fn reaches(&self, p: Pos) -> bool {
        match self {
            MapRef::Stratum(m) => m.reaches(p),
            MapRef::Floor(m) => m.reaches(p),
            MapRef::Construct(m) => m.reaches(p),
        }
    }

    fn backdrop(&self, p: Pos) -> Option<Tile> {
        match self {
            MapRef::Stratum(m) => m.backdrop(p),
            MapRef::Floor(m) => m.backdrop(p),
            MapRef::Construct(m) => m.backdrop(p),
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
        assert!(
            map.glow(floor.stations[0].pos).is_some(),
            "a floor's light has a colour"
        );
        assert_eq!(
            MapRef::stratum_at(&c, StratumKind::Street, 0).glow(Pos::new(5, 5)),
            None
        );
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
                    && d.down == (plan.lowest() < 0))
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
                keep: None,
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
                },
                crate::ui::hud::START_MINUTE_OF_DAY,
            )
            .is_none()
        );
    }

    /// The connector is asked of every tile drawn, every frame: it answers on each tile of
    /// every landing (a lift's bank both) and nowhere else, in a venue and a tower alike.
    #[test]
    fn a_connector_stands_on_every_landing_and_only_there() {
        let c = city();
        let mut interiors = Interiors::default();
        let towers = (c.buildings.iter())
            .filter(|b| matches!(b.kind, BuildingKind::CorpTower | BuildingKind::Office))
            .map(|b| b.id);
        let mut landings = 0;
        for b in towers {
            let levels: Vec<i32> = interiors.plan(&c, b).unwrap().levels().collect();
            for level in levels {
                interiors.ensure(&c, b, level).unwrap();
                let (plan, floor) = interiors.floor(b, level).unwrap();
                let map = FloorMap {
                    city: &c,
                    plan,
                    floor,
                };
                for (p, _) in floor.tiles.iter() {
                    let on = (plan.links.iter()).position(|l| l.tiles(level).contains(&p));
                    assert_eq!(plan.link_at(level, p), on, "{b:?} {level} {p:?}");
                    assert_eq!(
                        map.connector(p).is_some(),
                        on.is_some(),
                        "{b:?} {level} {p:?}"
                    );
                    landings += usize::from(on.is_some());
                }
            }
        }
        assert!(landings > 0);
    }
}
