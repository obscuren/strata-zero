//! The city's listings: the places the map marks and lists, nearest walk first.
//! Spec `docs/superpowers/specs/2026-10-31-tracking-and-map-design.md`, 7.

use super::route::{field, open};
use super::ui::{job_source, story_source};
use super::{ListingId, Resolved, Source, Target, label, locate, resolve};
use crate::app::AppState;
use crate::jobs::State;
use crate::render::map::{MapRef, TileMap};
use crate::ui::chrome::MAGENTA;
use crate::ui::hud::AMBER;
use citygen::{BuildingId, BuildingKind, Grid, Place, Pos, StratumKind};
use ratatui::style::Color;
use sim::{PersonId, Work};

/// What a listing is a listing of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Jobs,
    Home,
    Docs,
    Fixers,
    Bars,
    Noodle,
    Shops,
    Pawn,
    Arcades,
    Hotels,
    /// Corp towers and offices.
    Corps,
    /// Factories, warehouses and depots.
    Industry,
    /// Garages and parking.
    Garages,
    Temples,
    /// Apartments and megablocks, but your own.
    Blocks,
}

impl Kind {
    pub const ALL: [Kind; 15] = [
        Kind::Jobs,
        Kind::Home,
        Kind::Docs,
        Kind::Fixers,
        Kind::Bars,
        Kind::Noodle,
        Kind::Shops,
        Kind::Pawn,
        Kind::Arcades,
        Kind::Hotels,
        Kind::Corps,
        Kind::Industry,
        Kind::Garages,
        Kind::Temples,
        Kind::Blocks,
    ];

    pub fn glyph(self) -> char {
        match self {
            Kind::Jobs => '◆',
            Kind::Home => '⌂',
            Kind::Docs => '+',
            Kind::Fixers => '$',
            Kind::Bars => 'Y',
            Kind::Noodle => '≈',
            Kind::Shops => '▪',
            Kind::Pawn => '¢',
            Kind::Arcades => '▣',
            Kind::Hotels => 'H',
            Kind::Corps => '▲',
            Kind::Industry => '▤',
            Kind::Garages => 'P',
            Kind::Temples => '†',
            Kind::Blocks => '#',
        }
    }

    pub fn color(self) -> Color {
        match self {
            Kind::Jobs => MAGENTA,
            Kind::Home => Color::Rgb(124, 255, 154),
            Kind::Docs => Color::Rgb(255, 79, 94),
            Kind::Fixers => AMBER,
            Kind::Bars => Color::Rgb(255, 138, 216),
            Kind::Noodle => Color::Rgb(255, 176, 112),
            Kind::Shops => Color::Rgb(143, 208, 255),
            Kind::Pawn => Color::Rgb(224, 192, 112),
            Kind::Arcades => Color::Rgb(176, 128, 255),
            Kind::Hotels => Color::Rgb(192, 200, 216),
            Kind::Corps => Color::Rgb(110, 180, 255),
            Kind::Industry => Color::Rgb(170, 150, 120),
            Kind::Garages => Color::Rgb(150, 150, 165),
            Kind::Temples => Color::Rgb(255, 230, 150),
            Kind::Blocks => Color::Rgb(140, 175, 150),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Jobs => "jobs & story",
            Kind::Home => "home",
            Kind::Docs => "ripperdocs",
            Kind::Fixers => "fixers",
            Kind::Bars => "bars & clubs",
            Kind::Noodle => "noodle bars",
            Kind::Shops => "shops & markets",
            Kind::Pawn => "pawnshops",
            Kind::Arcades => "arcades",
            Kind::Hotels => "hotels",
            Kind::Corps => "corp towers & offices",
            Kind::Industry => "factories & warehouses",
            Kind::Garages => "garages & parking",
            Kind::Temples => "temples",
            Kind::Blocks => "apartment blocks",
        }
    }

    /// Listed whichever kind is chosen.
    pub fn pinned(self) -> bool {
        matches!(self, Kind::Jobs | Kind::Home)
    }

    /// The kind a building of this kind lists under: every kind of building is one.
    fn of(kind: BuildingKind) -> Kind {
        match kind {
            BuildingKind::Clinic => Kind::Docs,
            BuildingKind::Bar | BuildingKind::Club => Kind::Bars,
            BuildingKind::NoodleBar => Kind::Noodle,
            BuildingKind::Shop | BuildingKind::Market => Kind::Shops,
            BuildingKind::Pawn => Kind::Pawn,
            BuildingKind::Arcade => Kind::Arcades,
            BuildingKind::Hotel => Kind::Hotels,
            BuildingKind::CorpTower | BuildingKind::Office => Kind::Corps,
            BuildingKind::Factory | BuildingKind::Warehouse | BuildingKind::Depot => Kind::Industry,
            BuildingKind::Garage | BuildingKind::Parking => Kind::Garages,
            BuildingKind::Temple => Kind::Temples,
            BuildingKind::Apartment | BuildingKind::Megablock => Kind::Blocks,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    pub id: ListingId,
    pub kind: Kind,
    pub name: String,
    /// The kind, and for a clinic whether it is open; for a job or story its objective.
    pub what: String,
    /// Where its marker stands on the stratum.
    pub tile: Pos,
    pub building: Option<BuildingId>,
    pub person: Option<PersonId>,
    /// Tiles of walk from you; none when no walk reaches it.
    pub walk: Option<u32>,
}

/// Nearest walk first, the unreachable last, ties by name.
pub fn sort_listings(v: &mut [Listing]) {
    v.sort_by(|a, b| (a.walk.is_none(), a.walk, &a.name).cmp(&(b.walk.is_none(), b.walk, &b.name)));
}

/// What `t` tracks on a listing: a `◆`'s job or story own source, else the listing.
pub fn source_of(st: &AppState, l: &Listing) -> Source {
    match l.id {
        ListingId::Job(j) => job_source(st, j),
        ListingId::Story(m) => story_source(st, m),
        _ => None,
    }
    .unwrap_or(Source::Listing(l.id))
}

/// The glyph and colour the maps mark a track's target with: what its listing would
/// show, a job's or the story's `◆`, a picked spot `⌖`.
pub fn marker_of(st: &AppState, source: &Source) -> (char, Color) {
    let kind = match *source {
        Source::Job { .. } | Source::Story(_) => Kind::Jobs,
        Source::Listing(ListingId::Job(_) | ListingId::Story(_)) => Kind::Jobs,
        Source::Listing(ListingId::Person(_)) => Kind::Fixers,
        Source::Listing(ListingId::Building(b)) => {
            if st.home.as_ref().is_some_and(|r| r.home.building == b) {
                Kind::Home
            } else {
                match st.city.as_ref().and_then(|c| c.building_opt(b)) {
                    Some(x) => Kind::of(x.kind),
                    None => return ('⌖', MAGENTA),
                }
            }
        }
        Source::Spot { .. } => return ('⌖', MAGENTA),
    };
    (kind.glyph(), kind.color())
}

/// Where you stand on a stratum's map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Standing {
    /// Where your `@` shows: your tile, or the door out of the building you are in.
    pub door: Pos,
    /// Where a walk from you starts: your tile, or the tile outside that door.
    pub landing: Pos,
}

/// Where you stand on `stratum`: your tile when you are on it; on a floor, the way out
/// onto it (this floor's exit, else the exit of the floor that opens onto it once that
/// floor is made, else outside the building's door there); jacked in, your body's.
pub fn standing(st: &AppState, stratum: StratumKind) -> Option<Standing> {
    let p = st.player.as_ref()?;
    let on = |place: Place, pos: Pos| match place {
        Place::Stratum(k) => (k == stratum).then_some(Standing {
            door: pos,
            landing: pos,
        }),
        Place::Floor { building, level } => way_out(st, building, level, pos, stratum),
        Place::Construct(_) => None,
    };
    on(p.place, p.pos).or_else(|| p.body.and_then(|(place, pos)| on(place, pos)))
}

/// From tile `pos` of floor `level` of `b`, the way out onto `stratum`.
fn way_out(
    st: &AppState,
    b: BuildingId,
    level: i32,
    pos: Pos,
    stratum: StratumKind,
) -> Option<Standing> {
    let exit = |l: i32| {
        let (_, floor) = st.interiors.floor(b, l)?;
        floor
            .exits
            .iter()
            .filter(|e| e.stratum == stratum)
            .min_by_key(|e| (e.at.manhattan(pos), e.at.y, e.at.x))
            .map(|e| Standing {
                door: e.door,
                landing: e.landing,
            })
    };
    let below = || {
        let l = st.interiors.plan_of(b)?.door_level(stratum)?;
        exit(l)
    };
    exit(level)
        .or_else(below)
        .or_else(|| outside_door(st, b, stratum))
}

/// A building's first door on `stratum` and the walkable tile outside it (on the
/// Heights, the roof's door itself, as the roof's exit lands).
fn outside_door(st: &AppState, b: BuildingId, stratum: StratumKind) -> Option<Standing> {
    let city = st.city.as_ref()?;
    let fp = city.building_opt(b)?.footprint;
    city.portals(b)
        .into_iter()
        .filter(|(k, _)| *k == stratum)
        .find_map(|(_, door)| {
            let landing = if stratum == StratumKind::Heights {
                Some(door)
            } else {
                door.neighbors4()
                    .find(|n| !fp.contains(*n) && city.walkable(stratum, *n))
            };
            landing.map(|landing| Standing { door, landing })
        })
}

/// The tile a walk from you starts on, on `stratum`.
pub fn walk_from(st: &AppState, stratum: StratumKind) -> Option<Pos> {
    standing(st, stratum).map(|s| s.landing)
}

/// The listings of a stratum, nearest first.
pub fn listings(st: &AppState, stratum: StratumKind) -> Vec<Listing> {
    let mut out = Vec::new();
    let Some(city) = st.city.as_ref() else {
        return out;
    };
    jobs_and_story(st, stratum, &mut out);
    if let Some(rig) = st.home.as_ref() {
        let b = rig.home.building;
        if let Some(tile) = building_tile(st, b, stratum) {
            let floor = citygen::FloorPlan::label(rig.home.level);
            out.push(Listing {
                id: ListingId::Building(b),
                kind: Kind::Home,
                name: format!("home · {}", crate::net::building_name(st, b)),
                what: format!("your flat · {floor}"),
                tile,
                building: Some(b),
                person: None,
                walk: None,
            });
        }
    }
    let home = st.home.as_ref().map(|r| r.home.building);
    for b in &city.buildings {
        // Your own block is home's row.
        if Some(b.id) == home {
            continue;
        }
        let kind = Kind::of(b.kind);
        let Some(tile) = building_tile(st, b.id, stratum) else {
            continue;
        };
        let (name, what) = if kind == Kind::Docs {
            let state = if crate::clinic::is_open(st, b.id) {
                "open".to_string()
            } else {
                format!(
                    "closed · opens {}",
                    crate::clinic::hhmm(crate::clinic::hours(st).0)
                )
            };
            (
                crate::clinic::name(st, b.id),
                format!("ripperdoc · {state}"),
            )
        } else {
            (
                crate::net::building_name(st, b.id),
                citygen::kind_label(city, b.id).to_string(),
            )
        };
        out.push(Listing {
            id: ListingId::Building(b.id),
            kind,
            name,
            what,
            tile,
            building: Some(b.id),
            person: None,
            walk: None,
        });
    }
    if let Some(world) = st.world.as_ref() {
        for p in world.people().iter().filter(|p| p.profession == "fixer") {
            if world.is_dead(p.id) {
                continue;
            }
            let (tile, building) = match p.work {
                Work::Spot(pos) if stratum == StratumKind::Street => (Some(pos), None),
                Work::Building(b) => (building_tile(st, b, stratum), Some(b)),
                _ => (None, None),
            };
            let Some(tile) = tile else {
                continue;
            };
            out.push(Listing {
                id: ListingId::Person(p.id),
                kind: Kind::Fixers,
                name: p.name.clone(),
                what: "fixer".to_string(),
                tile,
                building,
                person: Some(p.id),
                walk: None,
            });
        }
    }
    if let Some(d) = walk_field(st, stratum) {
        distances(st, &d, &mut out);
    }
    sort_listings(&mut out);
    out
}

/// Taken jobs' and open missions' current targets that lie on this stratum.
fn jobs_and_story(st: &AppState, stratum: StratumKind, out: &mut Vec<Listing>) {
    let mut sources = Vec::new();
    for j in st.jobs.list.iter().filter(|j| j.state == State::Taken) {
        if let Some(s @ Source::Job { objective, .. }) = job_source(st, j.id) {
            let what = j.objectives[objective].text.clone();
            sources.push((ListingId::Job(j.id), s, what));
        }
    }
    for (m, run) in st.missions.runs.iter().enumerate() {
        if let Some(s) = story_source(st, m) {
            let what = run.objectives.last().cloned().unwrap_or_default();
            sources.push((ListingId::Story(m), s, what));
        }
    }
    for (id, source, what) in sources {
        let Resolved::Target(target) = resolve(st, &source) else {
            continue;
        };
        let located = match locate(st, target) {
            Resolved::Target(t) => t,
            _ => target,
        };
        let Some(tile) = target_tile(st, stratum, located) else {
            continue;
        };
        out.push(Listing {
            id,
            kind: Kind::Jobs,
            name: label(st, &source),
            what,
            tile,
            building: match located {
                Target::Building(b) => Some(b),
                _ => None,
            },
            person: match target {
                Target::Person(p) => Some(p),
                _ => None,
            },
            walk: None,
        });
    }
}

/// Where a located target stands on a stratum.
pub(crate) fn target_tile(st: &AppState, stratum: StratumKind, t: Target) -> Option<Pos> {
    match t {
        Target::Spot {
            place: Place::Stratum(k),
            pos,
            ..
        } => (k == stratum).then_some(pos),
        Target::Spot {
            place: Place::Floor { building, .. },
            ..
        }
        | Target::Building(building) => building_tile(st, building, stratum),
        Target::Spot { .. } | Target::Person(_) => None,
    }
}

/// A building's door on the stratum, else (a walk-in) its footprint's middle.
pub(crate) fn building_tile(st: &AppState, b: BuildingId, stratum: StratumKind) -> Option<Pos> {
    let city = st.city.as_ref()?;
    let x = city.building_opt(b)?;
    if let Some((_, door)) = city.portals(b).into_iter().find(|(k, _)| *k == stratum) {
        return Some(door);
    }
    (!x.kind.is_closed() && x.span.contains(stratum)).then(|| x.footprint.center())
}

/// Steps of walk from you to every tile of a stratum (`u32::MAX` where none reaches):
/// one BFS from where you start.
pub fn walk_field(st: &AppState, stratum: StratumKind) -> Option<Grid<u32>> {
    let (city, from) = (st.city.as_ref()?, walk_from(st, stratum)?);
    let map = MapRef::stratum_of(city, stratum);
    Some(field(map.size(), &[from], open(map, true, &[], &[])))
}

/// Fills in each listing's walk from the walk field `d`.
fn distances(st: &AppState, d: &Grid<u32>, out: &mut [Listing]) {
    let Some(city) = st.city.as_ref() else {
        return;
    };
    let at = |p: Pos| d.get(p).copied().filter(|n| *n != u32::MAX);
    for l in out.iter_mut() {
        // On it, else beside it (a door is a wall to the walk).
        let beside = l.tile.neighbors4().filter_map(at).min().map(|n| n + 1);
        let mut best = at(l.tile).into_iter().chain(beside).min();
        // A walk-in is entered from any side.
        let walk_in = l
            .building
            .and_then(|b| city.building_opt(b))
            .filter(|b| !b.kind.is_closed());
        if let Some(b) = walk_in {
            let ring = b.footprint.grow(1).iter().filter_map(at).min();
            best = best.into_iter().chain(ring.map(|n| n + 1)).min();
        }
        l.walk = best;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::tests::outside;
    use super::*;
    use crate::jobs::testutil::{a_fixer, state_home};
    use crate::jobs::{KindTag, generate};

    const STREET: StratumKind = StratumKind::Street;

    fn of_kind(v: &[Listing], k: Kind) -> Vec<&Listing> {
        v.iter().filter(|l| l.kind == k).collect()
    }

    /// A taken job of this kind.
    fn taken(st: &mut AppState, tag: KindTag) -> crate::jobs::JobId {
        let fixer = a_fixer(st);
        st.jobs.fixer_mut(fixer).standing = 10;
        let job = (0..40)
            .find_map(|n| generate::generate(st, fixer, tag, n))
            .unwrap();
        let id = st.jobs.push(job);
        assert!(st.jobs.take(id, 0));
        id
    }

    fn clinics(st: &AppState) -> Vec<BuildingId> {
        let city = st.city.as_ref().unwrap();
        city.buildings
            .iter()
            .filter(|b| crate::clinic::is_clinic(st, b.id))
            .map(|b| b.id)
            .collect()
    }

    /// The first stratum a building has a door on.
    fn stratum_of(st: &AppState, b: BuildingId) -> StratumKind {
        st.city.as_ref().unwrap().portals(b)[0].0
    }

    #[test]
    fn kinds_have_their_glyphs() {
        let glyphs: String = Kind::ALL.iter().map(|k| k.glyph()).collect();
        assert_eq!(glyphs, "◆⌂+$Y≈▪¢▣H▲▤P†#");
        assert_eq!(Kind::Jobs.color(), MAGENTA);
        assert_eq!(Kind::Fixers.color(), AMBER);
        assert_eq!(Kind::Home.color(), Color::Rgb(124, 255, 154));
        assert_eq!(Kind::Hotels.label(), "hotels");
        for (k, colour, label) in [
            (Kind::Corps, (110, 180, 255), "corp towers & offices"),
            (Kind::Industry, (170, 150, 120), "factories & warehouses"),
            (Kind::Garages, (150, 150, 165), "garages & parking"),
            (Kind::Temples, (255, 230, 150), "temples"),
            (Kind::Blocks, (140, 175, 150), "apartment blocks"),
        ] {
            assert_eq!(k.color(), Color::Rgb(colour.0, colour.1, colour.2));
            assert_eq!(k.label(), label);
        }
        let pinned: Vec<Kind> = Kind::ALL.into_iter().filter(|k| k.pinned()).collect();
        assert_eq!(pinned, [Kind::Jobs, Kind::Home]);
    }

    #[test]
    fn every_building_kind_lists_under_a_kind() {
        let st = state_home();
        let home = st.home.unwrap().home.building;
        let city = st.city.as_ref().unwrap();
        for k in StratumKind::ALL {
            let all = listings(&st, k);
            for b in &city.buildings {
                if b.id == home || building_tile(&st, b.id, k).is_none() {
                    continue;
                }
                let want = Kind::of(b.kind);
                assert!(
                    all.iter()
                        .any(|l| l.id == ListingId::Building(b.id) && l.kind == want),
                    "{:?} {} on {k:?}",
                    b.kind,
                    b.id.0
                );
            }
        }
        use BuildingKind as B;
        for (b, k) in [
            (B::CorpTower, Kind::Corps),
            (B::Office, Kind::Corps),
            (B::Factory, Kind::Industry),
            (B::Warehouse, Kind::Industry),
            (B::Depot, Kind::Industry),
            (B::Garage, Kind::Garages),
            (B::Parking, Kind::Garages),
            (B::Temple, Kind::Temples),
            (B::Apartment, Kind::Blocks),
            (B::Megablock, Kind::Blocks),
        ] {
            assert_eq!(Kind::of(b), k, "{b:?}");
        }
        // Each new kind the city builds has some on the street (seed 8 builds no
        // temple); home stays one row, its own.
        let street = listings(&st, STREET);
        for k in [
            Kind::Corps,
            Kind::Industry,
            Kind::Garages,
            Kind::Temples,
            Kind::Blocks,
        ] {
            let built = city.buildings.iter().any(|b| Kind::of(b.kind) == k);
            assert_eq!(!of_kind(&street, k).is_empty(), built, "{k:?}");
        }
        let rows: Vec<&Listing> = street
            .iter()
            .filter(|l| l.id == ListingId::Building(home))
            .collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, Kind::Home);
    }

    #[test]
    fn the_kiln_stacks_have_listings() {
        let st = state_home();
        let city = st.city.as_ref().unwrap();
        let kiln = city
            .districts
            .iter()
            .find(|d| d.name == "Kiln Stacks")
            .expect("seed 8 has the Kiln Stacks")
            .id;
        let here: Vec<Listing> = StratumKind::ALL
            .into_iter()
            .flat_map(|k| listings(&st, k))
            .filter(|l| {
                l.building
                    .and_then(|b| city.building_opt(b))
                    .is_some_and(|b| b.district == kiln)
            })
            .collect();
        assert!(here.len() >= 3, "{}", here.len());
        assert!(here.iter().any(|l| l.kind == Kind::Industry));
    }

    #[test]
    fn clinics_list_as_ripperdocs_under_their_front() {
        let st = state_home();
        let docs = clinics(&st);
        assert_eq!(docs.len(), 2);
        for b in docs {
            let all = listings(&st, stratum_of(&st, b));
            let l = all
                .iter()
                .find(|l| l.id == ListingId::Building(b))
                .expect("the clinic is listed");
            assert_eq!(l.kind, Kind::Docs);
            assert_eq!(l.name, crate::clinic::name(&st, b));
            assert!(l.what.starts_with("ripperdoc"), "{}", l.what);
            assert_eq!(l.building, Some(b));
        }
    }

    #[test]
    fn fixers_list_at_their_spots() {
        let mut st = state_home();
        let fixers: Vec<_> = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .filter(|p| p.profession == "fixer")
            .map(|p| (p.id, p.work))
            .collect();
        assert_eq!(fixers.len(), 3);
        let all = listings(&st, STREET);
        for (id, work) in &fixers {
            let l = all
                .iter()
                .find(|l| l.id == ListingId::Person(*id))
                .expect("the fixer is listed");
            assert_eq!(l.kind, Kind::Fixers);
            assert_eq!(l.person, Some(*id));
            if let Work::Spot(pos) = work {
                assert_eq!(l.tile, *pos);
            }
        }
        let gone = fixers[0].0;
        st.world.as_mut().unwrap().kill(gone);
        let all = listings(&st, STREET);
        assert_eq!(of_kind(&all, Kind::Fixers).len(), 2);
        assert!(all.iter().all(|l| l.id != ListingId::Person(gone)));
    }

    #[test]
    fn the_job_and_the_story_are_listed_as_diamonds() {
        let mut st = outside();
        let job = taken(&mut st, KindTag::Deliver);
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        crate::missions::end(&mut st, 0, 0);
        let body = st.missions.bodies[0].clone();
        assert_eq!(body.place, Place::Stratum(STREET));
        let all = listings(&st, STREET);
        let story = all
            .iter()
            .find(|l| l.id == ListingId::Story(0))
            .expect("the story is listed");
        assert_eq!(story.kind, Kind::Jobs);
        assert_eq!(story.tile, body.pos);
        assert_eq!(
            source_of(&st, story),
            story_source(&st, 0).expect("the mission is open")
        );
        let j = all
            .iter()
            .find(|l| l.id == ListingId::Job(job))
            .expect("the job is listed");
        assert_eq!(j.kind, Kind::Jobs);
        assert_eq!(source_of(&st, j), job_source(&st, job).unwrap());
        assert!(of_kind(&all, Kind::Jobs).len() >= 2);
        // A building is its own listing.
        let b = of_kind(&all, Kind::Fixers)[0];
        assert_eq!(source_of(&st, b), Source::Listing(b.id));
    }

    #[test]
    fn a_job_at_a_listed_building_is_two_rows() {
        let mut st = outside();
        let id = taken(&mut st, KindTag::Plant);
        let clinic = clinics(&st)[0];
        st.jobs.list.iter_mut().find(|j| j.id == id).unwrap().kind =
            crate::jobs::Kind::Plant { building: clinic };
        let all = listings(&st, stratum_of(&st, clinic));
        let doc = all
            .iter()
            .find(|l| l.id == ListingId::Building(clinic))
            .unwrap();
        let job = all.iter().find(|l| l.id == ListingId::Job(id)).unwrap();
        assert_eq!(doc.kind, Kind::Docs);
        assert_eq!(job.kind, Kind::Jobs);
        assert_eq!(doc.tile, job.tile);
        assert_eq!(job.building, Some(clinic));
    }

    #[test]
    fn home_is_listed_at_its_door() {
        let st = state_home();
        let b = st.home.unwrap().home.building;
        let city = st.city.as_ref().unwrap();
        let door = city
            .portals(b)
            .into_iter()
            .find(|(k, _)| *k == STREET)
            .unwrap()
            .1;
        let all = listings(&st, STREET);
        let homes = of_kind(&all, Kind::Home);
        assert_eq!(homes.len(), 1);
        assert_eq!(homes[0].tile, door);
        assert!(homes[0].name.starts_with("home · "));
        assert!(homes[0].what.starts_with("your flat · "));
    }

    #[test]
    fn listings_sort_by_walk_with_the_unreachable_last() {
        let st = outside();
        let all = listings(&st, STREET);
        assert!(all.len() > 5);
        let walks: Vec<Option<u32>> = all.iter().map(|l| l.walk).collect();
        let seen: Vec<u32> = walks.iter().map_while(|w| *w).collect();
        assert!(!seen.is_empty(), "something is reachable");
        assert!(seen.windows(2).all(|w| w[0] <= w[1]));
        assert!(walks[seen.len()..].iter().all(|w| w.is_none()));
        // The hand-made case: ties by name, none last.
        let l = |name: &str, walk| Listing {
            id: ListingId::Story(0),
            kind: Kind::Bars,
            name: name.to_string(),
            what: String::new(),
            tile: Pos::new(0, 0),
            building: None,
            person: None,
            walk,
        };
        let mut v = vec![
            l("c", None),
            l("b", Some(4)),
            l("a", Some(9)),
            l("a2", Some(4)),
        ];
        sort_listings(&mut v);
        let names: Vec<&str> = v.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["a2", "b", "a", "c"]);
    }

    #[test]
    fn walk_from_inside_uses_the_way_out() {
        let mut st = state_home();
        crate::screens::explore::MapScreen::walking(&mut st);
        let b = st.home.unwrap().home.building;
        let city = st.city.as_ref().unwrap();
        st.interiors.ensure(city, b, 0).unwrap();
        let (_, floor) = st.interiors.floor(b, 0).unwrap();
        let exit = *floor.exits.iter().find(|e| e.stratum == STREET).unwrap();
        let p = st.player.as_mut().unwrap();
        p.place = Place::Floor {
            building: b,
            level: 0,
        };
        p.pos = exit.at;
        assert_eq!(walk_from(&st, STREET), Some(exit.landing));
        // A stratum this floor has no exit onto: the building's door there, if any.
        let city = st.city.as_ref().unwrap();
        for k in [StratumKind::Heights, StratumKind::Undercity] {
            let door = city.portals(b).into_iter().find(|(s, _)| *s == k);
            assert_eq!(standing(&st, k).map(|s| s.door), door.map(|d| d.1), "{k:?}");
        }
        let p = st.player.as_mut().unwrap();
        p.place = Place::Stratum(STREET);
        p.pos = Pos::new(3, 4);
        assert_eq!(walk_from(&st, STREET), Some(Pos::new(3, 4)));
    }

    /// Awake in your flat on 18F, where the game starts.
    pub(crate) fn in_the_flat() -> AppState {
        let mut st = state_home();
        crate::screens::explore::MapScreen::walking(&mut st);
        let rig = st.home.unwrap();
        assert_eq!(rig.home.level, 18);
        assert_eq!(st.player.as_ref().unwrap().place, rig.place());
        st
    }

    #[test]
    fn from_the_flat_the_walk_starts_outside_the_door() {
        let mut st = in_the_flat();
        let b = st.home.unwrap().home.building;
        let at = standing(&st, STREET).expect("you show on the street");
        let city = st.city.as_ref().unwrap();
        assert!(city.portals(b).contains(&(STREET, at.door)), "{at:?}");
        assert_eq!(at.door.manhattan(at.landing), 1);
        assert!(city.walkable(STREET, at.landing));
        assert!(!city.building(b).footprint.contains(at.landing));
        assert_eq!(walk_from(&st, STREET), Some(at.landing));
        // Listings have walks, nearest first.
        let all = listings(&st, STREET);
        let walks: Vec<Option<u32>> = all.iter().map(|l| l.walk).collect();
        let seen: Vec<u32> = walks.iter().map_while(|w| *w).collect();
        assert!(seen.len() > 5, "{} reached", seen.len());
        assert!(seen.windows(2).all(|w| w[0] <= w[1]));
        // The ground floor made: its door out.
        let city = st.city.as_ref().unwrap();
        st.interiors.ensure(city, b, 0).unwrap();
        let (_, lobby) = st.interiors.floor(b, 0).unwrap();
        let from = walk_from(&st, STREET).unwrap();
        assert!(
            lobby
                .exits
                .iter()
                .any(|e| e.stratum == STREET && e.landing == from),
            "{from:?}"
        );
        // Jacked in from the flat: your body's.
        let p = st.player.as_mut().unwrap();
        p.body = Some((p.place, p.pos));
        p.place = Place::Construct(b);
        p.pos = Pos::new(1, 1);
        assert_eq!(walk_from(&st, STREET), Some(from));
    }
}
